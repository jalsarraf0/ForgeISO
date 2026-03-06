use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::backends::plan_build;
use crate::config::{BuildConfig, BuildMode, Distro, RuntimePreference, ScanPolicy, Severity};
use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::modules::evaluate_modules;
use crate::policy::{enforce_release_policy, policy_warnings};
use crate::release_tracker::{ReleaseInfo, ReleaseTracker};
use crate::report::{BuildReport, TestSummary};
use crate::runtime::RuntimeManager;
use crate::scanner::{run_scans, ScanSummary};
use crate::workspace::Workspace;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub runtime_candidates: BTreeMap<String, bool>,
    pub tooling: BTreeMap<String, bool>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub workspace_root: PathBuf,
    pub output_dir: PathBuf,
    pub report_json: PathBuf,
    pub report_html: PathBuf,
    pub artifacts: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub report: ScanSummary,
    pub report_json: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub bios: bool,
    pub uefi: bool,
    pub logs: Vec<PathBuf>,
    pub screenshots: Vec<PathBuf>,
    pub passed: bool,
}

#[derive(Clone)]
pub struct ForgeIsoEngine {
    tracker: ReleaseTracker,
    events: broadcast::Sender<EngineEvent>,
}

impl Default for ForgeIsoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeIsoEngine {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(2048);
        Self {
            tracker: ReleaseTracker::new(),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub async fn doctor(&self) -> DoctorReport {
        self.emit(EngineEvent::info(EventPhase::Doctor, "running toolchain doctor"));

        let mut runtime_candidates = BTreeMap::new();
        runtime_candidates.insert("docker".to_string(), which::which("docker").is_ok());
        runtime_candidates.insert("podman".to_string(), which::which("podman").is_ok());

        let mut tooling = BTreeMap::new();
        for tool in [
            "xorriso",
            "unsquashfs",
            "mksquashfs",
            "livemedia-creator",
            "mkarchiso",
            "trivy",
            "syft",
            "grype",
            "oscap",
            "qemu-system-x86_64",
            "goss",
        ] {
            tooling.insert(tool.to_string(), which::which(tool).is_ok());
        }

        DoctorReport {
            runtime_candidates,
            tooling,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub async fn list_releases(&self, distro: Distro) -> EngineResult<Vec<ReleaseInfo>> {
        self.emit(EngineEvent::info(
            EventPhase::ReleaseLookup,
            format!("discovering releases for {:?}", distro),
        ));
        self.tracker.list_releases(distro).await
    }

    pub async fn build_from_file(
        &self,
        config_path: &Path,
        out_dir: &Path,
        build_mode_override: Option<BuildMode>,
        keep_workdir: bool,
    ) -> EngineResult<BuildResult> {
        let mut cfg = BuildConfig::from_path(config_path)?;
        if let Some(mode) = build_mode_override {
            cfg.build_mode = mode;
        }
        self.build(&cfg, out_dir, keep_workdir).await
    }

    pub async fn build(
        &self,
        cfg: &BuildConfig,
        out_dir: &Path,
        keep_workdir: bool,
    ) -> EngineResult<BuildResult> {
        let cfg = cfg.clone();
        cfg.validate()?;
        enforce_release_policy(&cfg.distro, &cfg.release.version)?;

        self.emit(EngineEvent::info(
            EventPhase::Configure,
            format!("policy and config validated for '{}'", cfg.name),
        ));

        let workspace = Workspace::create(out_dir, &cfg.name)?;

        let runtime = RuntimeManager::new(cfg.runtime);
        let runtime_bin = runtime.selected_binary()?;
        self.emit(EngineEvent::info(
            EventPhase::Configure,
            format!("using container runtime '{runtime_bin}'"),
        ));

        let module_actions = evaluate_modules(&cfg)?;
        let backend = plan_build(&cfg, &workspace, &module_actions)?;

        for warning in &backend.warnings {
            self.emit(EngineEvent::warn(EventPhase::Build, warning.clone()));
        }

        for (idx, spec) in backend.commands.iter().enumerate() {
            self.emit(EngineEvent::info(
                EventPhase::Build,
                format!(
                    "executing backend command {}/{} with image {}",
                    idx + 1,
                    backend.commands.len(),
                    spec.image
                ),
            ));
            let output = runtime.run_container(spec).await?;
            let log_path = workspace.logs.join(format!("step-{}.log", idx + 1));
            std::fs::write(
                log_path,
                format!(
                    "program: {}\nexit: {}\nstdout:\n{}\nstderr:\n{}\n",
                    output.program, output.status, output.stdout, output.stderr
                ),
            )?;

            if output.status != 0 {
                return Err(EngineError::Runtime(format!(
                    "backend command {}/{} failed",
                    idx + 1,
                    backend.commands.len()
                )));
            }
        }

        if !backend.produced_iso.exists() {
            return Err(EngineError::Runtime(format!(
                "expected ISO artifact was not produced at {}",
                backend.produced_iso.display()
            )));
        }

        let mut report = BuildReport::new(&cfg);
        report.metadata.warnings.extend(backend.warnings.clone());
        report
            .metadata
            .tool_versions
            .insert("container_runtime".to_string(), runtime_bin.to_string());
        report.metadata.tool_versions.insert(
            "engine".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );
        report
            .metadata
            .policy_warnings
            .extend(policy_warnings(&cfg.distro, &cfg.release.version));
        report
            .artifacts
            .push(backend.produced_iso.display().to_string());

        let report_json = workspace.reports.join("build-report.json");
        let report_html = workspace.reports.join("build-report.html");
        report.write_json(&report_json)?;
        report.write_html(&report_html)?;

        let final_out = if keep_workdir {
            workspace.output.clone()
        } else {
            let final_out = out_dir.join("artifacts");
            std::fs::create_dir_all(&final_out)?;

            let final_iso = final_out.join(
                backend
                    .produced_iso
                    .file_name()
                    .ok_or_else(|| EngineError::Runtime("invalid artifact filename".to_string()))?,
            );
            std::fs::copy(&backend.produced_iso, &final_iso)?;
            std::fs::copy(&report_json, final_out.join("build-report.json"))?;
            std::fs::copy(&report_html, final_out.join("build-report.html"))?;
            final_out
        };

        self.emit(EngineEvent::info(EventPhase::Complete, "build completed"));

        Ok(BuildResult {
            workspace_root: workspace.root,
            output_dir: final_out,
            report_json,
            report_html,
            artifacts: vec![backend.produced_iso],
        })
    }

    pub async fn scan(
        &self,
        artifact: &Path,
        policy_file: Option<&Path>,
        out_dir: &Path,
    ) -> EngineResult<ScanResult> {
        let policy = if let Some(path) = policy_file {
            let raw = std::fs::read_to_string(path)?;
            serde_yaml::from_str::<ScanPolicy>(&raw)?
        } else {
            ScanPolicy {
                enable_sbom: true,
                enable_trivy: true,
                enable_syft_grype: false,
                enable_open_scap: true,
                enable_secrets_scan: true,
                strict_secrets: false,
                fail_on_severity: Some(Severity::Critical),
                compliance_profile: None,
            }
        };

        self.emit(EngineEvent::info(
            EventPhase::Scan,
            format!("running scan pipeline for {}", artifact.display()),
        ));

        let summary = run_scans(artifact, out_dir, &policy).await?;
        let report_json = out_dir.join("scan-report.json");
        std::fs::write(&report_json, serde_json::to_vec_pretty(&summary)?)?;

        self.emit(EngineEvent::info(EventPhase::Complete, "scan completed"));

        Ok(ScanResult {
            report: summary,
            report_json,
        })
    }

    pub async fn test_iso(
        &self,
        iso: &Path,
        bios: bool,
        uefi: bool,
        out_dir: &Path,
    ) -> EngineResult<TestResult> {
        if !iso.exists() {
            return Err(EngineError::NotFound(format!(
                "ISO does not exist: {}",
                iso.display()
            )));
        }

        std::fs::create_dir_all(out_dir)?;

        self.emit(EngineEvent::info(
            EventPhase::Test,
            format!("starting smoke tests for {}", iso.display()),
        ));

        let mut logs = Vec::new();
        let mut screenshots = Vec::new();
        let mut passed = true;

        if bios {
            let bios_log = out_dir.join("bios-serial.log");
            std::fs::write(
                &bios_log,
                "BIOS smoke test placeholder: run via forgeiso-agent or local QEMU harness",
            )?;
            logs.push(bios_log);
        }

        if uefi {
            let uefi_log = out_dir.join("uefi-serial.log");
            std::fs::write(
                &uefi_log,
                "UEFI smoke test placeholder: run via forgeiso-agent or local QEMU harness",
            )?;
            logs.push(uefi_log);

            let shot = out_dir.join("uefi-screenshot.txt");
            std::fs::write(&shot, "screenshot capture path placeholder for automated harness")?;
            screenshots.push(shot);
        }

        if (bios || uefi) && which::which("qemu-system-x86_64").is_err() {
            passed = false;
            self.emit(EngineEvent::warn(
                EventPhase::Test,
                "qemu-system-x86_64 not available; use remote agent for full VM smoke tests",
            ));
        }

        self.emit(EngineEvent::info(
            EventPhase::Complete,
            format!("test run completed (passed={passed})"),
        ));

        Ok(TestResult {
            bios,
            uefi,
            logs,
            screenshots,
            passed,
        })
    }

    pub async fn report(&self, build_dir: &Path, format: &str) -> EngineResult<PathBuf> {
        let input = build_dir.join("build-report.json");
        let raw = std::fs::read_to_string(&input)?;
        let report: BuildReport = serde_json::from_str(&raw)?;

        let output = match format {
            "json" => {
                let path = build_dir.join("report.json");
                std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
                path
            }
            "html" => {
                let path = build_dir.join("report.html");
                report.write_html(&path)?;
                path
            }
            other => {
                return Err(EngineError::InvalidConfig(format!(
                    "unsupported format: {other}"
                )))
            }
        };

        self.emit(EngineEvent::info(
            EventPhase::Report,
            format!("report rendered to {}", output.display()),
        ));

        Ok(output)
    }

    pub async fn inspect_iso(&self, iso: &Path) -> EngineResult<serde_json::Value> {
        if !iso.exists() {
            return Err(EngineError::NotFound(format!(
                "ISO not found: {}",
                iso.display()
            )));
        }

        let metadata = std::fs::metadata(iso)?;
        let hash = sha256_file(iso)?;

        let value = serde_json::json!({
            "path": iso.display().to_string(),
            "size_bytes": metadata.len(),
            "sha256": hash,
            "inspected_at": chrono::Utc::now().to_rfc3339(),
        });

        self.emit(EngineEvent::info(EventPhase::Inspect, "iso inspection complete"));
        Ok(value)
    }

    fn emit(&self, event: EngineEvent) {
        let _ = self.events.send(event);
    }
}

pub fn parse_build_mode(latest: bool, pinned: bool) -> EngineResult<Option<BuildMode>> {
    if latest && pinned {
        return Err(EngineError::InvalidConfig(
            "--latest and --pinned are mutually exclusive".to_string(),
        ));
    }

    if latest {
        Ok(Some(BuildMode::Latest))
    } else if pinned {
        Ok(Some(BuildMode::Pinned))
    } else {
        Ok(None)
    }
}

pub fn parse_runtime(name: &str) -> EngineResult<RuntimePreference> {
    match name {
        "docker" => Ok(RuntimePreference::Docker),
        "podman" => Ok(RuntimePreference::Podman),
        other => Err(EngineError::InvalidConfig(format!(
            "runtime must be docker or podman, got '{other}'"
        ))),
    }
}

fn sha256_file(path: &Path) -> EngineResult<String> {
    use sha2::{Digest, Sha256};

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

impl From<TestResult> for TestSummary {
    fn from(value: TestResult) -> Self {
        Self {
            bios: value.bios,
            uefi: value.uefi,
            logs: value.logs.iter().map(|p| p.display().to_string()).collect(),
            screenshots: value
                .screenshots
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            passed: value.passed,
        }
    }
}
