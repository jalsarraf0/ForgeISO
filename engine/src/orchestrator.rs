use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::broadcast;
use walkdir::WalkDir;

use crate::config::{BuildConfig, IsoSource};
use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::iso::{inspect_iso, IsoMetadata, ResolvedIso, SourceKind};
use crate::report::{BuildReport, TestSummary};
use crate::scanner::{run_scans, ScanSummary};
use crate::workspace::Workspace;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub host_os: String,
    pub host_arch: String,
    pub linux_supported: bool,
    pub tooling: BTreeMap<String, bool>,
    pub warnings: Vec<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub workspace_root: PathBuf,
    pub output_dir: PathBuf,
    pub report_json: PathBuf,
    pub report_html: PathBuf,
    pub artifacts: Vec<PathBuf>,
    pub iso: IsoMetadata,
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
    pub passed: bool,
}

#[derive(Clone)]
pub struct ForgeIsoEngine {
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
        Self { events }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub async fn doctor(&self) -> DoctorReport {
        self.emit(EngineEvent::info(
            EventPhase::Doctor,
            "checking local bare-metal prerequisites",
        ));

        let tooling = [
            "xorriso",
            "unsquashfs",
            "mksquashfs",
            "qemu-system-x86_64",
            "trivy",
            "syft",
            "grype",
            "oscap",
        ]
        .into_iter()
        .map(|tool| (tool.to_string(), which::which(tool).is_ok()))
        .collect::<BTreeMap<_, _>>();

        let linux_supported = std::env::consts::OS == "linux";
        let mut warnings = Vec::new();
        if !linux_supported {
            warnings
                .push("ISO build and VM test flows are only supported on Linux hosts".to_string());
        }
        if !tooling.get("xorriso").copied().unwrap_or(false) {
            warnings.push("xorriso is required for deep ISO inspection and repacking".to_string());
        }

        DoctorReport {
            host_os: std::env::consts::OS.to_string(),
            host_arch: std::env::consts::ARCH.to_string(),
            linux_supported,
            tooling,
            warnings,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub async fn inspect_source(
        &self,
        source: &str,
        cache_dir: Option<&Path>,
    ) -> EngineResult<IsoMetadata> {
        self.emit(EngineEvent::info(
            EventPhase::Inspect,
            format!("resolving ISO source {source}"),
        ));
        let cache_root = cache_dir.unwrap_or_else(|| Path::new("."));
        let resolved = self
            .resolve_source(&IsoSource::from_raw(source.to_string()), cache_root)
            .await?;
        let metadata = inspect_iso(
            &resolved.source_path,
            resolved.source_kind,
            resolved.source_value,
        )?;
        self.emit(EngineEvent::info(
            EventPhase::Inspect,
            format!(
                "inspection complete: distro={} release={} arch={}",
                metadata
                    .distro
                    .map(|value| format!("{:?}", value))
                    .unwrap_or_else(|| "unknown".to_string()),
                metadata.release.as_deref().unwrap_or("unknown"),
                metadata.architecture.as_deref().unwrap_or("unknown")
            ),
        ));
        Ok(metadata)
    }

    pub async fn build_from_file(
        &self,
        config_path: &Path,
        out_dir: &Path,
    ) -> EngineResult<BuildResult> {
        let cfg = BuildConfig::from_path(config_path)?;
        self.build(&cfg, out_dir).await
    }

    pub async fn build(&self, cfg: &BuildConfig, out_dir: &Path) -> EngineResult<BuildResult> {
        cfg.validate()?;
        ensure_linux_host()?;

        self.emit(EngineEvent::info(
            EventPhase::Configure,
            format!("starting local ISO build for '{}'", cfg.name),
        ));

        let workspace = Workspace::create(out_dir, &cfg.name)?;
        let resolved = self.resolve_source(&cfg.source, &workspace.input).await?;
        let iso = inspect_iso(
            &resolved.source_path,
            resolved.source_kind,
            resolved.source_value.clone(),
        )?;

        self.emit(EngineEvent::info(
            EventPhase::Build,
            format!("using source ISO {}", iso.source_path.display()),
        ));

        require_tools(&["xorriso"])?;
        let extract_dir = workspace.work.join("iso-tree");
        std::fs::create_dir_all(&extract_dir)?;
        run_command_capture(
            "xorriso",
            &[
                "-osirrox".to_string(),
                "on".to_string(),
                "-indev".to_string(),
                iso.source_path.display().to_string(),
                "-extract".to_string(),
                "/".to_string(),
                extract_dir.display().to_string(),
            ],
            None,
        )?;

        let mut warnings = iso.warnings.clone();
        let mut rootfs_dir = None;
        if let Some(rootfs_rel) = iso.rootfs_path.as_deref() {
            let rootfs_image = extract_dir.join(rootfs_rel);
            if rootfs_image.exists() && is_squashfs_path(rootfs_rel) {
                require_tools(&["unsquashfs", "mksquashfs"])?;
                let unpack_dir = workspace.work.join("rootfs");
                std::fs::create_dir_all(&unpack_dir)?;
                run_command_capture(
                    "unsquashfs",
                    &[
                        "-f".to_string(),
                        "-d".to_string(),
                        unpack_dir.display().to_string(),
                        rootfs_image.display().to_string(),
                    ],
                    None,
                )?;
                if let Some(overlay) = cfg.overlay_dir.as_deref() {
                    copy_dir_contents(overlay, &unpack_dir)?;
                }
                write_rootfs_manifest(&unpack_dir, cfg, &iso)?;
                std::fs::remove_file(&rootfs_image)?;
                run_command_capture(
                    "mksquashfs",
                    &[
                        unpack_dir.display().to_string(),
                        rootfs_image.display().to_string(),
                        "-comp".to_string(),
                        "xz".to_string(),
                        "-noappend".to_string(),
                    ],
                    None,
                )?;
                rootfs_dir = Some(unpack_dir);
            } else if rootfs_image.exists() {
                warnings.push(format!(
                    "Root filesystem image '{}' is not yet rewriteable offline; only top-level ISO files will be updated",
                    rootfs_rel
                ));
            }
        } else {
            warnings.push("No known root filesystem image was detected inside the ISO".to_string());
        }

        if rootfs_dir.is_none() {
            if let Some(overlay) = cfg.overlay_dir.as_deref() {
                copy_dir_contents(overlay, &extract_dir)?;
            }
        }
        write_iso_manifest(&extract_dir, cfg, &iso)?;

        let output_iso = out_dir.join(format!("{}.iso", sanitize_filename(&cfg.name)));
        let repack_args = repack_iso_args(
            &iso.source_path,
            &extract_dir,
            &output_iso,
            cfg.output_label.as_deref(),
        )?;
        run_command_capture("xorriso", &repack_args, None)?;

        let mut report = BuildReport::new(cfg, &iso);
        report.metadata.warnings.extend(warnings);
        report
            .metadata
            .tool_versions
            .insert("engine".to_string(), env!("CARGO_PKG_VERSION").to_string());
        report
            .metadata
            .tool_versions
            .insert("host_os".to_string(), std::env::consts::OS.to_string());
        report.artifacts.push(output_iso.display().to_string());

        let report_json = out_dir.join("build-report.json");
        let report_html = out_dir.join("build-report.html");
        report.write_json(&report_json)?;
        report.write_html(&report_html)?;

        self.emit(EngineEvent::info(
            EventPhase::Complete,
            format!("build completed: {}", output_iso.display()),
        ));

        Ok(BuildResult {
            workspace_root: workspace.root,
            output_dir: out_dir.to_path_buf(),
            report_json,
            report_html,
            artifacts: vec![output_iso],
            iso,
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
            serde_yaml::from_str(&raw)?
        } else {
            crate::config::ScanPolicy::default()
        };

        self.emit(EngineEvent::info(
            EventPhase::Scan,
            format!("running local scans for {}", artifact.display()),
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
        ensure_linux_host()?;
        require_tools(&["qemu-system-x86_64"])?;
        if !iso.exists() {
            return Err(EngineError::NotFound(format!(
                "ISO does not exist: {}",
                iso.display()
            )));
        }

        std::fs::create_dir_all(out_dir)?;
        let mut logs = Vec::new();
        let mut passed = true;

        if bios {
            let log = out_dir.join("bios-serial.log");
            run_qemu_smoke(iso, None, &log).await?;
            logs.push(log);
        }

        if uefi {
            let firmware = ovmf_path()?;
            let log = out_dir.join("uefi-serial.log");
            run_qemu_smoke(iso, Some(&firmware), &log).await?;
            logs.push(log);
        }

        for log in &logs {
            if std::fs::metadata(log).map(|meta| meta.len()).unwrap_or(0) == 0 {
                passed = false;
                continue;
            }

            let body = std::fs::read_to_string(log)
                .unwrap_or_default()
                .to_lowercase();
            if body.contains("no bootable option or device")
                || body.contains("failed to load boot")
                || body.contains("kernel panic")
                || body.contains("error:")
            {
                passed = false;
            }
        }

        self.emit(EngineEvent::info(
            EventPhase::Complete,
            format!("test run completed (passed={passed})"),
        ));

        Ok(TestResult {
            bios,
            uefi,
            logs,
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
        let metadata = inspect_iso(iso, SourceKind::LocalPath, iso.display().to_string())?;
        serde_json::to_value(metadata).map_err(EngineError::from)
    }

    fn emit(&self, event: EngineEvent) {
        let _ = self.events.send(event);
    }

    async fn resolve_source(
        &self,
        source: &IsoSource,
        cache_root: &Path,
    ) -> EngineResult<ResolvedIso> {
        match source {
            IsoSource::Path(path) => {
                if !path.exists() {
                    return Err(EngineError::NotFound(format!(
                        "source ISO does not exist: {}",
                        path.display()
                    )));
                }
                Ok(ResolvedIso {
                    source_path: path.to_path_buf(),
                    source_kind: SourceKind::LocalPath,
                    source_value: path.display().to_string(),
                    _download_dir: None,
                })
            }
            IsoSource::Url(url) => {
                std::fs::create_dir_all(cache_root)?;
                let target = cache_root.join(download_filename(url));
                self.emit(EngineEvent::info(
                    EventPhase::Build,
                    format!("downloading source ISO from {url}"),
                ));
                download_to_path(url, &target).await?;
                Ok(ResolvedIso {
                    source_path: target.clone(),
                    source_kind: SourceKind::DownloadedUrl,
                    source_value: url.clone(),
                    _download_dir: Some(target),
                })
            }
        }
    }
}

pub fn run_command_capture(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .map_err(|e| EngineError::Runtime(format!("failed to run {program}: {e}")))?;

    if !output.status.success() {
        return Err(EngineError::Runtime(format!(
            "{program} failed with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(CommandOutput {
        program: program.to_string(),
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub program: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn sha256_file(path: &Path) -> EngineResult<String> {
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

fn ensure_linux_host() -> EngineResult<()> {
    if std::env::consts::OS != "linux" {
        return Err(EngineError::MissingTool(
            "ForgeISO local build/test is supported only on Linux hosts".to_string(),
        ));
    }
    Ok(())
}

fn require_tools(tools: &[&str]) -> EngineResult<()> {
    let missing = tools
        .iter()
        .filter(|tool| which::which(tool).is_err())
        .copied()
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(EngineError::MissingTool(format!(
            "missing local tools: {}",
            missing.join(", ")
        )))
    }
}

fn is_squashfs_path(path: &str) -> bool {
    path.ends_with(".squashfs") || path.ends_with(".sfs")
}

fn write_iso_manifest(
    extract_dir: &Path,
    cfg: &BuildConfig,
    iso: &IsoMetadata,
) -> EngineResult<()> {
    let manifest = serde_json::json!({
        "name": cfg.name,
        "profile": cfg.profile,
        "source": cfg.source.display_value(),
        "inspected": iso,
        "generated_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        extract_dir.join("forgeiso-build.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn write_rootfs_manifest(
    rootfs_dir: &Path,
    cfg: &BuildConfig,
    iso: &IsoMetadata,
) -> EngineResult<()> {
    let etc = rootfs_dir.join("etc");
    std::fs::create_dir_all(&etc)?;
    let manifest = serde_json::json!({
        "name": cfg.name,
        "profile": cfg.profile,
        "source": cfg.source.display_value(),
        "sha256": iso.sha256,
        "generated_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        etc.join("forgeiso-build.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn copy_dir_contents(from: &Path, to: &Path) -> EngineResult<()> {
    for entry in WalkDir::new(from).into_iter().filter_map(Result::ok) {
        let relative = entry.path().strip_prefix(from).map_err(|e| {
            EngineError::Runtime(format!("failed to compute relative overlay path: {e}"))
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = to.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

async fn download_to_path(url: &str, output: &Path) -> EngineResult<()> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(EngineError::Network(format!(
            "download failed with status {}",
            response.status()
        )));
    }

    let mut file = tokio::fs::File::create(output).await?;
    let mut response = response;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}

fn download_filename(url: &str) -> String {
    let fallback = format!("download-{}.iso", chrono::Utc::now().timestamp());
    url.rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .map(sanitize_filename)
        .filter(|segment| !segment.is_empty())
        .unwrap_or(fallback)
}

fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn repack_iso_args(
    source_iso: &Path,
    extract_dir: &Path,
    output_iso: &Path,
    output_label: Option<&str>,
) -> EngineResult<Vec<String>> {
    let report = run_command_capture(
        "xorriso",
        &[
            "-indev".to_string(),
            source_iso.display().to_string(),
            "-report_el_torito".to_string(),
            "as_mkisofs".to_string(),
        ],
        None,
    )?;

    let mut boot_args = parse_mkisofs_report(&report.stdout)?;
    if output_label.is_some() {
        boot_args = strip_volume_args(&boot_args);
    }

    let mut args = vec![
        "-as".to_string(),
        "mkisofs".to_string(),
        "-o".to_string(),
        output_iso.display().to_string(),
    ];
    args.extend(boot_args);
    if let Some(label) = output_label {
        args.push("-V".to_string());
        args.push(label.to_string());
    }
    args.push(extract_dir.display().to_string());
    Ok(args)
}

fn parse_mkisofs_report(report: &str) -> EngineResult<Vec<String>> {
    let mut args = Vec::new();
    for line in report
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let parsed = shell_words::split(line).map_err(|error| {
            EngineError::Runtime(format!(
                "failed to parse xorriso mkisofs report line '{line}': {error}"
            ))
        })?;
        args.extend(parsed);
    }
    Ok(args)
}

fn strip_volume_args(args: &[String]) -> Vec<String> {
    let mut filtered = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "-V" || arg == "-volid" {
            index += 2;
            continue;
        }
        filtered.push(arg.clone());
        index += 1;
    }
    filtered
}

async fn run_qemu_smoke(iso: &Path, firmware: Option<&Path>, log_path: &Path) -> EngineResult<()> {
    let mut args = vec![
        "-m".to_string(),
        "2048".to_string(),
        "-boot".to_string(),
        "d".to_string(),
        "-cdrom".to_string(),
        iso.display().to_string(),
        "-display".to_string(),
        "none".to_string(),
        "-serial".to_string(),
        format!("file:{}", log_path.display()),
        "-monitor".to_string(),
        "none".to_string(),
        "-no-reboot".to_string(),
    ];
    if let Some(path) = firmware {
        args.push("-bios".to_string());
        args.push(path.display().to_string());
    }

    let mut child = Command::new("qemu-system-x86_64")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::Runtime(format!("failed to start qemu-system-x86_64: {e}")))?;

    match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(status) => {
            let status =
                status.map_err(|e| EngineError::Runtime(format!("qemu wait failed: {e}")))?;
            if !status.success() {
                return Err(EngineError::Runtime(format!(
                    "qemu exited before smoke timeout with status {:?}",
                    status.code()
                )));
            }
        }
        Err(_) => {
            child.kill().await.ok();
        }
    }

    Ok(())
}

fn ovmf_path() -> EngineResult<PathBuf> {
    for candidate in [
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/edk2/x64/OVMF_CODE.fd",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(EngineError::MissingTool(
        "OVMF firmware is required for UEFI smoke tests".to_string(),
    ))
}

impl From<TestResult> for TestSummary {
    fn from(value: TestResult) -> Self {
        Self {
            bios: value.bios,
            uefi: value.uefi,
            logs: value.logs.iter().map(|p| p.display().to_string()).collect(),
            passed: value.passed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_xorriso_mkisofs_report() {
        let report = "\
-V 'ISOIMAGE'\n\
--grub2-mbr --interval:local_fs:0s-15s:zero_mbrpt,zero_gpt,zero_apm:'/tmp/source.iso'\n\
-efi-boot-part --efi-boot-image\n\
-c '/boot.catalog'\n";

        let args = parse_mkisofs_report(report).expect("report should parse");

        assert_eq!(args[0], "-V");
        assert_eq!(args[1], "ISOIMAGE");
        assert_eq!(args[2], "--grub2-mbr");
        assert_eq!(
            args[3],
            "--interval:local_fs:0s-15s:zero_mbrpt,zero_gpt,zero_apm:/tmp/source.iso"
        );
        assert_eq!(args[4], "-efi-boot-part");
        assert_eq!(args[5], "--efi-boot-image");
        assert_eq!(args[6], "-c");
        assert_eq!(args[7], "/boot.catalog");
    }

    #[test]
    fn strips_existing_volume_flag_before_override() {
        let args = vec![
            "-V".to_string(),
            "OLDLABEL".to_string(),
            "--grub2-mbr".to_string(),
            "payload".to_string(),
        ];

        let stripped = strip_volume_args(&args);

        assert_eq!(
            stripped,
            vec!["--grub2-mbr".to_string(), "payload".to_string()]
        );
    }
}
