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

use crate::autoinstall::{generate_autoinstall_yaml, merge_autoinstall_yaml};
use crate::config::{BuildConfig, Distro, IsoSource};
use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};
use crate::iso::{inspect_iso, IsoMetadata, ResolvedIso, SourceKind};
use crate::kickstart::generate_kickstart_cfg;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub filename: String,
    pub expected: String,
    pub actual: String,
    pub matched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub path: String,
    pub base_size: u64,
    pub target_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<DiffEntry>,
    pub unchanged: usize,
}

/// ISO-9660 compliance check result.
/// `compliant` is true only when the CD001 primary volume descriptor signature
/// is confirmed at the standard sector-16 offset. El Torito boot presence is
/// checked via xorriso when available.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iso9660Compliance {
    /// True if the CD001 ISO-9660 signature was found at sector 16.
    pub compliant: bool,
    /// Primary volume descriptor volume ID label (may be None if empty).
    pub volume_id: Option<String>,
    /// File size in bytes.
    pub size_bytes: u64,
    /// El Torito BIOS boot entry detected (requires xorriso).
    pub boot_bios: bool,
    /// El Torito UEFI boot entry detected (requires xorriso).
    pub boot_uefi: bool,
    /// Any El Torito boot catalog present.
    pub el_torito_present: bool,
    /// Method used: "iso9660_header" or "iso9660_header+xorriso".
    pub check_method: String,
    /// Error message if the check failed (compliant will be false).
    pub error: Option<String>,
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
        let owned_cache_root;
        let cache_root = if let Some(cache_dir) = cache_dir {
            cache_dir
        } else {
            owned_cache_root = default_cache_root()?;
            owned_cache_root.as_path()
        };
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
        // xorriso extracts files with read-only permissions; make writable
        // so we can modify the tree and clean up afterwards.
        chmod_recursive_writable(&extract_dir);

        let mut warnings = iso.warnings.clone();
        let mut rootfs_dir = None;
        if let Some(rootfs_rel) = iso.rootfs_path.as_deref() {
            let rootfs_image = extract_dir.join(rootfs_rel);
            if rootfs_image.exists() && is_squashfs_path(rootfs_rel) {
                require_tools(&["unsquashfs", "mksquashfs"])?;
                let unpack_dir = workspace.work.join("rootfs");
                std::fs::create_dir_all(&unpack_dir)?;
                run_command_lossy(
                    "unsquashfs",
                    &[
                        "-f".to_string(),
                        "-no-xattrs".to_string(),
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
                        "-no-xattrs".to_string(),
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

        let workspace_root = workspace.root.clone();
        if !cfg.keep_workdir {
            if let Err(e) = remove_dir_all_force(&workspace.root) {
                self.emit(EngineEvent::warn(
                    EventPhase::Complete,
                    format!(
                        "failed to clean up workspace {}: {e}",
                        workspace.root.display()
                    ),
                ));
            }
        }

        Ok(BuildResult {
            workspace_root,
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
                || body.contains("boot failed")
                || body.contains("no bootable device")
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
                    EventPhase::Download,
                    format!("downloading source ISO from {url}"),
                ));
                self.download_to_path(url, &target).await?;
                Ok(ResolvedIso {
                    source_path: target.clone(),
                    source_kind: SourceKind::DownloadedUrl,
                    source_value: url.clone(),
                    _download_dir: Some(target),
                })
            }
        }
    }

    async fn download_to_path(&self, url: &str, output: &Path) -> EngineResult<()> {
        let response = reqwest::get(url).await?;
        if !response.status().is_success() {
            return Err(EngineError::Network(format!(
                "download failed with status {}",
                response.status()
            )));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut file = tokio::fs::File::create(output).await?;
        let mut response = response;
        let mut downloaded = 0u64;
        let emit_interval = 512 * 1024; // 512 KB

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if (downloaded.is_multiple_of(emit_interval) || downloaded == total_size)
                && total_size > 0
            {
                let msg = format!("{}/{} bytes", downloaded, total_size);
                self.emit(EngineEvent::info(EventPhase::Download, msg));
            }
        }
        file.flush().await?;
        Ok(())
    }

    pub async fn verify(&self, source: &str, sums_url: Option<&str>) -> EngineResult<VerifyResult> {
        self.emit(EngineEvent::info(
            EventPhase::Verify,
            "verifying ISO checksum",
        ));

        let resolved = self
            .resolve_source(&IsoSource::from_raw(source), &default_cache_root()?)
            .await?;
        let metadata = inspect_iso(
            &resolved.source_path,
            resolved.source_kind,
            resolved.source_value,
        )?;

        let filename = resolved
            .source_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| EngineError::InvalidConfig("Unable to get ISO filename".to_string()))?;

        let effective_sums_url = if let Some(url) = sums_url {
            url.to_string()
        } else if let Some(distro) = metadata.distro {
            if let Some(release) = &metadata.release {
                match distro {
                    crate::config::Distro::Ubuntu => {
                        format!("https://releases.ubuntu.com/{}/SHA256SUMS", release)
                    }
                    _ => {
                        return Err(EngineError::InvalidConfig(
                            "Auto-detection of sums URL not supported for this distro".to_string(),
                        ))
                    }
                }
            } else {
                return Err(EngineError::InvalidConfig(
                    "Release information not available for auto-detection".to_string(),
                ));
            }
        } else {
            return Err(EngineError::InvalidConfig(
                "sums_url must be provided or ISO must be recognized as Ubuntu".to_string(),
            ));
        };

        self.emit(EngineEvent::info(
            EventPhase::Verify,
            format!("fetching checksums from {}", effective_sums_url),
        ));

        let sums_content = reqwest::get(&effective_sums_url).await?.text().await?;

        // Parse SHA256SUMS format: <hash>  <filename>
        let mut expected_hash = None;
        for line in sums_content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let hash = parts[0];
                let file_path = parts[1].trim_start_matches('*');
                if file_path.ends_with(&filename) || file_path == filename {
                    expected_hash = Some(hash.to_string());
                    break;
                }
            }
        }

        let expected = expected_hash
            .ok_or_else(|| EngineError::NotFound(format!("No checksum found for {}", filename)))?;

        let matched = metadata.sha256 == expected;
        self.emit(EngineEvent::info(
            EventPhase::Verify,
            if matched {
                "checksum matches!".to_string()
            } else {
                "checksum mismatch!".to_string()
            },
        ));

        Ok(VerifyResult {
            filename,
            expected,
            actual: metadata.sha256,
            matched,
        })
    }

    pub async fn inject_autoinstall(
        &self,
        cfg: &crate::config::InjectConfig,
        out: &Path,
    ) -> EngineResult<BuildResult> {
        self.emit(EngineEvent::info(
            EventPhase::Inject,
            "starting autoinstall injection",
        ));

        // Create workspace for injection
        let workspace = Workspace::create(&cache_subdir("inject")?, "inject")?;
        let work_dir = workspace.root;

        // Resolve the source ISO
        let resolved = self.resolve_source(&cfg.source, &work_dir).await?;
        let metadata = inspect_iso(
            &resolved.source_path,
            resolved.source_kind,
            resolved.source_value,
        )?;

        // Dispatch to the appropriate injection method based on target distro
        match cfg.distro {
            None | Some(Distro::Ubuntu) | Some(Distro::Mint) => {
                // ── Ubuntu / Mint: cloud-init nocloud overlay ─────────────────
                let nocloud_dir = work_dir.join("overlay").join("nocloud");
                std::fs::create_dir_all(&nocloud_dir)?;

                let user_data = match &cfg.autoinstall_yaml {
                    Some(path) => {
                        let existing = std::fs::read_to_string(path)?;
                        merge_autoinstall_yaml(&existing, cfg)?
                    }
                    None => generate_autoinstall_yaml(cfg)?,
                };
                std::fs::write(nocloud_dir.join("user-data"), &user_data)?;
                std::fs::write(nocloud_dir.join("meta-data"), "")?;

                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "created cloud-init overlay",
                ));
            }
            Some(Distro::Fedora) => {
                // ── Fedora: Kickstart ks.cfg ──────────────────────────────────
                let ks_content = generate_kickstart_cfg(cfg)?;
                std::fs::write(work_dir.join("ks.cfg"), &ks_content)?;

                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "generated Kickstart ks.cfg",
                ));
            }
            Some(Distro::Arch) => {
                // ── Arch Linux: archinstall JSON config ───────────────────────
                let archinstall_cfg = build_archinstall_config(cfg);
                let json_content = serde_json::to_string_pretty(&archinstall_cfg)
                    .map_err(|e| EngineError::Runtime(e.to_string()))?;
                std::fs::write(work_dir.join("archinstall-config.json"), &json_content)?;

                // Create the run-archinstall.sh launcher script
                let launcher = concat!(
                    "#!/usr/bin/env bash\n",
                    "# Generated by ForgeISO -- runs archinstall in silent mode\n",
                    "set -euo pipefail\n",
                    "CONFIG=/run/archiso/bootmnt/arch/boot/archinstall-config.json\n",
                    "if [[ -f \"${CONFIG}\" ]]; then\n",
                    "    archinstall --config \"${CONFIG}\" --silent\n",
                    "else\n",
                    "    echo 'ERROR: archinstall config not found' >&2\n",
                    "    exit 1\n",
                    "fi\n"
                );
                std::fs::write(work_dir.join("run-archinstall.sh"), launcher)?;

                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "generated archinstall config",
                ));
            }
        }

        // --- (distro-specific files are copied to the extracted ISO below) ---

        // Copy wallpaper file if provided
        if let Some(src) = &cfg.wallpaper {
            let fname = src
                .file_name()
                .ok_or_else(|| EngineError::InvalidConfig("invalid wallpaper path".to_string()))?;
            let dest = work_dir.join("wallpaper");
            std::fs::create_dir_all(&dest)?;
            std::fs::copy(src, dest.join(fname))?;
        }

        // Extract ISO
        let extract_dir = work_dir.join("extract");
        std::fs::create_dir_all(&extract_dir)?;
        let output = run_command_lossy(
            "xorriso",
            &[
                "-osirrox".to_string(),
                "on".to_string(),
                "-indev".to_string(),
                resolved.source_path.to_string_lossy().to_string(),
                "-extract".to_string(),
                "/".to_string(),
                extract_dir.to_string_lossy().to_string(),
            ],
            None,
        )?;
        if output.status != 0 {
            return Err(EngineError::Runtime(format!(
                "xorriso extract failed: {}",
                output.stderr
            )));
        }

        self.emit(EngineEvent::info(
            EventPhase::Inject,
            "extracted ISO filesystem",
        ));

        // xorriso extracts files with read-only permissions; make writable
        // so we can modify the tree and inject files without permission errors.
        chmod_recursive_writable(&extract_dir);

        // Copy distro-specific files into the extracted ISO and patch boot entries
        match cfg.distro {
            None | Some(Distro::Ubuntu) | Some(Distro::Mint) => {
                // Cloud-init nocloud overlay
                let nocloud_dir = work_dir.join("overlay").join("nocloud");
                let iso_nocloud = extract_dir.join("cdrom").join("nocloud");
                std::fs::create_dir_all(&iso_nocloud)?;
                for entry in std::fs::read_dir(&nocloud_dir)? {
                    let entry = entry?;
                    std::fs::copy(entry.path(), iso_nocloud.join(entry.file_name()))?;
                }
                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "injected cloud-init files",
                ));

                // Wallpaper
                if let Some(src) = &cfg.wallpaper {
                    let fname = src.file_name().ok_or_else(|| {
                        EngineError::InvalidConfig(format!(
                            "wallpaper path has no filename: {}",
                            src.display()
                        ))
                    })?;
                    let iso_wp = extract_dir.join("cdrom").join("wallpaper");
                    std::fs::create_dir_all(&iso_wp)?;
                    std::fs::copy(work_dir.join("wallpaper").join(fname), iso_wp.join(fname))?;
                }

                // Boot patch — Ubuntu autoinstall kernel params
                let kernel_append = " autoinstall ds=nocloud;s=/cdrom/nocloud/";
                patch_boot_configs(&extract_dir, kernel_append)?;
            }
            Some(Distro::Fedora) => {
                // Copy ks.cfg to ISO root
                std::fs::copy(work_dir.join("ks.cfg"), extract_dir.join("ks.cfg"))?;
                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "injected ks.cfg into ISO root",
                ));

                // Patch Fedora boot entries to add inst.ks=cdrom:/ks.cfg
                let kernel_append = " inst.ks=cdrom:/ks.cfg";
                patch_boot_configs(&extract_dir, kernel_append)?;

                // Also patch EFI/BOOT/grub.cfg if present (UEFI Fedora media)
                let efi_grub = extract_dir.join("EFI").join("BOOT").join("grub.cfg");
                if efi_grub.exists() {
                    let content = std::fs::read_to_string(&efi_grub)?;
                    let patched = content.replace("quiet", &format!("quiet{kernel_append}"));
                    std::fs::write(&efi_grub, patched)?;
                }
            }
            Some(Distro::Arch) => {
                // Copy archinstall config + launcher into arch/boot/ inside the ISO
                let arch_boot = extract_dir.join("arch").join("boot");
                std::fs::create_dir_all(&arch_boot)?;
                std::fs::copy(
                    work_dir.join("archinstall-config.json"),
                    arch_boot.join("archinstall-config.json"),
                )?;
                std::fs::copy(
                    work_dir.join("run-archinstall.sh"),
                    arch_boot.join("run-archinstall.sh"),
                )?;
                self.emit(EngineEvent::info(
                    EventPhase::Inject,
                    "injected archinstall config",
                ));

                // Patch syslinux/archiso_sys.conf or loader/entries/ to run the script
                let syslinux_cfg = extract_dir.join("syslinux").join("archiso_sys.conf");
                if syslinux_cfg.exists() {
                    let content = std::fs::read_to_string(&syslinux_cfg)?;
                    let patched =
                        content.replace("APPEND", "APPEND script=/root/run-archinstall.sh");
                    std::fs::write(&syslinux_cfg, patched)?;
                }

                // Patch systemd-boot loader entries
                let loader_entries = extract_dir.join("loader").join("entries");
                if loader_entries.exists() {
                    for entry in std::fs::read_dir(&loader_entries)? {
                        let entry = entry?;
                        if entry.path().extension().and_then(|e| e.to_str()) == Some("conf") {
                            let content = std::fs::read_to_string(entry.path())?;
                            let patched = content + "options script=/root/run-archinstall.sh\n";
                            std::fs::write(entry.path(), patched)?;
                        }
                    }
                }
            }
        }

        self.emit(EngineEvent::info(
            EventPhase::Inject,
            "patched boot configurations",
        ));

        // Repack ISO
        std::fs::create_dir_all(out)?;
        let output_path = out.join(&cfg.out_name);

        let args = repack_iso_args(
            &resolved.source_path,
            &extract_dir,
            &output_path,
            cfg.output_label.as_deref(),
        )?;

        let output = run_command_lossy("xorriso", &args, None)?;
        if output.status != 0 {
            return Err(EngineError::Runtime(format!(
                "xorriso repack failed: {}",
                output.stderr
            )));
        }

        self.emit(EngineEvent::info(
            EventPhase::Inject,
            format!("created output ISO: {}", output_path.display()),
        ));

        Ok(BuildResult {
            workspace_root: work_dir.to_path_buf(),
            output_dir: out.to_path_buf(),
            report_json: work_dir.join("report.json"),
            report_html: work_dir.join("report.html"),
            artifacts: vec![output_path],
            iso: metadata,
        })
    }

    pub async fn diff_isos(&self, base: &Path, target: &Path) -> EngineResult<IsoDiff> {
        self.emit(EngineEvent::info(
            EventPhase::Diff,
            "comparing ISO filesystems",
        ));

        let base_files = get_iso_file_list(base)?;
        let target_files = get_iso_file_list(target)?;

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();
        let mut unchanged = 0;

        for (path, target_size) in &target_files {
            if let Some(base_size) = base_files.get(path) {
                if base_size == target_size {
                    unchanged += 1;
                } else {
                    modified.push(DiffEntry {
                        path: path.clone(),
                        base_size: *base_size,
                        target_size: *target_size,
                    });
                }
            } else {
                added.push(path.clone());
            }
        }

        for path in base_files.keys() {
            if !target_files.contains_key(path) {
                removed.push(path.clone());
            }
        }

        self.emit(EngineEvent::info(
            EventPhase::Diff,
            format!(
                "diff: {} added, {} removed, {} modified, {} unchanged",
                added.len(),
                removed.len(),
                modified.len(),
                unchanged
            ),
        ));

        Ok(IsoDiff {
            added,
            removed,
            modified,
            unchanged,
        })
    }

    /// Validate ISO-9660 compliance for a local file.
    ///
    /// Returns a structured `Iso9660Compliance` result without emitting errors —
    /// failure information is encoded in the result's `compliant` and `error` fields.
    pub async fn validate_iso9660(&self, path_str: &str) -> EngineResult<Iso9660Compliance> {
        use crate::iso::read_primary_volume_id;

        let path = std::path::Path::new(path_str);

        self.emit(EngineEvent::info(
            EventPhase::Verify,
            format!("checking ISO-9660 compliance: {}", path.display()),
        ));

        if !path.exists() {
            return Ok(Iso9660Compliance {
                compliant: false,
                volume_id: None,
                size_bytes: 0,
                boot_bios: false,
                boot_uefi: false,
                el_torito_present: false,
                check_method: "iso9660_header".into(),
                error: Some(format!("File not found: {}", path.display())),
            });
        }

        let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        // Check CD001 primary volume descriptor at sector 16.
        let volume_id = match read_primary_volume_id(path) {
            Ok(vid) => vid,
            Err(e) => {
                self.emit(EngineEvent::warn(
                    EventPhase::Verify,
                    format!("ISO-9660 compliance failed: {e}"),
                ));
                return Ok(Iso9660Compliance {
                    compliant: false,
                    volume_id: None,
                    size_bytes,
                    boot_bios: false,
                    boot_uefi: false,
                    el_torito_present: false,
                    check_method: "iso9660_header".into(),
                    error: Some(e.to_string()),
                });
            }
        };

        // Enrich with El Torito boot detection via xorriso when available.
        let mut boot_bios = false;
        let mut boot_uefi = false;
        let mut el_torito_present = false;
        let mut check_method = "iso9660_header".to_string();

        if which::which("xorriso").is_ok() {
            check_method = "iso9660_header+xorriso".to_string();
            // xorriso may exit non-zero on some ISOs even when useful output is produced;
            // use the lossy runner so we still get stdout/stderr.
            if let Ok(result) = run_command_lossy(
                "xorriso",
                &[
                    "-indev".to_string(),
                    path.display().to_string(),
                    "-report_el_torito".to_string(),
                    "plain".to_string(),
                ],
                None,
            ) {
                let report = format!(
                    "{}\n{}",
                    result.stdout.to_lowercase(),
                    result.stderr.to_lowercase()
                );
                el_torito_present = report.contains("el torito")
                    || report.contains("boot catalog")
                    || report.contains("boot img");
                boot_bios = report.contains("pltf  bios")
                    || report.contains("boot img :   1  bios")
                    || report.contains("platform id: 0x00")
                    || report.contains("platform id :  0 = 80x86");
                boot_uefi = report.contains("pltf  uefi")
                    || report.contains("boot img :   2  uefi")
                    || report.contains("platform id: 0xef")
                    || report.contains("platform id :  0xef = efi");
            }
        }

        self.emit(EngineEvent::info(
            EventPhase::Verify,
            format!(
                "ISO-9660 compliant — volume_id={:?} boot_bios={} boot_uefi={}",
                volume_id, boot_bios, boot_uefi
            ),
        ));

        Ok(Iso9660Compliance {
            compliant: true,
            volume_id,
            size_bytes,
            boot_bios,
            boot_uefi,
            el_torito_present,
            check_method,
            error: None,
        })
    }
}

pub fn default_cache_root() -> EngineResult<PathBuf> {
    if let Ok(path) = std::env::var("FORGEISO_CACHE_DIR") {
        let path = PathBuf::from(path);
        std::fs::create_dir_all(&path)?;
        return Ok(path);
    }

    // XDG-compliant default: ~/.cache/forgeiso — avoids tmpfs quota issues
    let base = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));
    let path = base.join(".cache").join("forgeiso");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn cache_subdir(name: &str) -> EngineResult<PathBuf> {
    let path = default_cache_root()?.join(name);
    std::fs::create_dir_all(&path)?;
    Ok(path)
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

/// Like `run_command_capture` but tolerates non-zero exit codes (e.g. unsquashfs
/// returning exit 2 for device-node warnings when not running as root).
pub fn run_command_lossy(
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

    Ok(CommandOutput {
        program: program.to_string(),
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
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
    path.ends_with(".squashfs") || path.ends_with(".sfs") || path.ends_with(".erofs")
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

/// Recursively grant user-write permission before removal so files extracted
/// from ISOs (which may carry read-only permissions) can be deleted.
fn remove_dir_all_force(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o700);
        let _ = std::fs::set_permissions(entry.path(), perms);
    }
    std::fs::remove_dir_all(path)
}

fn chmod_recursive_writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o700);
        let _ = std::fs::set_permissions(entry.path(), perms);
    }
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

/// List all files in an ISO with their sizes.
///
/// Tries two methods in order:
///  1. `lsdl` exec action (no arg)  — all xorriso versions >= 1.5.4
///     Output: `perms nlinks uid gid size month day time/year 'path'`
///     Note: `.` and `{}` path tokens are NOT accepted by xorriso 1.5.6 `-find -exec`
///  2. plain `-find / -type f`      — last resort, paths only with size = 0
fn get_iso_file_list(iso_path: &Path) -> EngineResult<std::collections::HashMap<String, u64>> {
    use std::process::Command;

    let iso_str = iso_path.to_string_lossy();

    // ── Method 1: lsdl exec (works on xorriso 1.5.4–1.5.7+) ─────────────────
    // `-exec lsdl` with NO path argument applies lsdl to each found file.
    // xorriso 1.5.6 rejects `.` and `{}` after the exec action name.
    if let Ok(out) = Command::new("xorriso")
        .args([
            "-indev", &iso_str, "-find", "/", "-type", "f", "-exec", "lsdl",
        ])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        let files = parse_lsdl_output(&text);
        if !files.is_empty() {
            return Ok(files);
        }
    }

    // ── Method 2: paths only, no sizes (minimum viable diff) ─────────────────
    let out = Command::new("xorriso")
        .args(["-indev", &iso_str, "-find", "/", "-type", "f"])
        .output()
        .map_err(|e| EngineError::Runtime(format!("xorriso not found: {e}")))?;

    if !out.status.success() {
        return Err(EngineError::Runtime(format!(
            "xorriso failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with('/') || l.starts_with("'/"))
        .map(|l| {
            let path = l.trim_matches('\'').to_string();
            (path, 0u64)
        })
        .collect())
}

fn parse_lsdl_output(text: &str) -> std::collections::HashMap<String, u64> {
    // xorriso -find / -type f -exec lsdl output format (1.5.x):
    // `-rwxr--r--    1 1000     1000       966664 Aug 13  2024 '/EFI/boot/bootx64.efi'`
    // Fields: [0]perms [1]nlinks [2]uid [3]gid [4]size [5]month [6]day [7]year/time [8+]'path'
    let mut files = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // File entries start with permission chars (-, d, l, etc.)
        let first = line.chars().next().unwrap_or(' ');
        if !matches!(first, '-' | 'd' | 'l' | 'c' | 'b' | 'p' | 's') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Need at least: perms nlinks uid gid size month day time/year path
        if parts.len() >= 9 {
            if let Ok(size) = parts[4].parse::<u64>() {
                // Path is the last fields joined, strip surrounding single quotes
                let raw_path = parts[8..].join(" ");
                let path = raw_path.trim_matches('\'').to_string();
                if path.starts_with('/') {
                    files.insert(path, size);
                }
            }
        }
    }
    files
}

/// Build a minimal archinstall JSON config from InjectConfig fields.
fn build_archinstall_config(cfg: &crate::config::InjectConfig) -> serde_json::Value {
    use serde_json::{json, Value};

    let packages: Value = cfg.extra_packages.to_vec().into();
    let services: Value = cfg.enable_services.to_vec().into();

    let mut map = serde_json::Map::new();
    if let Some(h) = &cfg.hostname {
        map.insert("hostname".to_string(), json!(h));
    }
    if let Some(u) = &cfg.username {
        map.insert("username".to_string(), json!(u));
    }
    if let Some(p) = &cfg.password {
        map.insert("!password".to_string(), json!(p));
    }
    if let Some(tz) = &cfg.timezone {
        map.insert("timezone".to_string(), json!(tz));
    } else {
        map.insert("timezone".to_string(), json!("UTC"));
    }
    map.insert("mirror-region".to_string(), json!("Worldwide"));
    if let Some(loc) = &cfg.locale {
        map.insert("sys-language".to_string(), json!(loc));
    } else {
        map.insert("sys-language".to_string(), json!("en_US.UTF-8"));
    }
    if let Some(kb) = &cfg.keyboard_layout {
        map.insert("keyboard-layout".to_string(), json!(kb));
    } else {
        map.insert("keyboard-layout".to_string(), json!("us"));
    }
    map.insert("packages".to_string(), packages);
    map.insert("services".to_string(), services);
    map.insert("script".to_string(), json!("stealth-installation"));

    Value::Object(map)
}

fn patch_boot_configs(extract_dir: &Path, kernel_append: &str) -> EngineResult<()> {
    // Patch grub.cfg
    let grub_path = extract_dir.join("boot").join("grub").join("grub.cfg");
    if grub_path.exists() {
        let content = std::fs::read_to_string(&grub_path)?;
        let patched = content.replace(
            "linux\t/boot/vmlinuz",
            &format!("linux\t/boot/vmlinuz{}", kernel_append),
        );
        std::fs::write(&grub_path, patched)?;
    }

    // Patch isolinux.cfg
    let isolinux_path = extract_dir.join("isolinux").join("isolinux.cfg");
    if isolinux_path.exists() {
        let content = std::fs::read_to_string(&isolinux_path)?;
        let patched = content.replace("/vmlinuz", &format!("/vmlinuz{}", kernel_append));
        std::fs::write(&isolinux_path, patched)?;
    }

    Ok(())
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
