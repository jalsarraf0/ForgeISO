use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    config::Distro,
    error::{EngineError, EngineResult},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    LocalPath,
    DownloadedUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BootSupport {
    pub bios: bool,
    pub uefi: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoMetadata {
    pub source_path: PathBuf,
    pub source_kind: SourceKind,
    pub source_value: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub volume_id: Option<String>,
    pub distro: Option<Distro>,
    pub release: Option<String>,
    pub edition: Option<String>,
    pub architecture: Option<String>,
    pub rootfs_path: Option<String>,
    pub boot: BootSupport,
    pub inspected_at: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedIso {
    pub source_path: PathBuf,
    pub source_kind: SourceKind,
    pub source_value: String,
    pub _download_dir: Option<PathBuf>,
}

pub fn inspect_iso(
    path: &Path,
    source_kind: SourceKind,
    source_value: String,
) -> EngineResult<IsoMetadata> {
    if !path.exists() {
        return Err(EngineError::NotFound(format!(
            "ISO not found: {}",
            path.display()
        )));
    }

    let metadata = std::fs::metadata(path)?;
    let sha256 = crate::orchestrator::sha256_file(path)?;
    let volume_id = read_primary_volume_id(path)?;

    let mut info = IsoMetadata {
        source_path: path.to_path_buf(),
        source_kind,
        source_value,
        size_bytes: metadata.len(),
        sha256,
        volume_id,
        distro: None,
        release: None,
        edition: None,
        architecture: None,
        rootfs_path: None,
        boot: BootSupport::default(),
        inspected_at: chrono::Utc::now().to_rfc3339(),
        warnings: Vec::new(),
    };

    if let Some(label) = info.volume_id.clone() {
        infer_from_label(&label, &mut info);
    }

    if which::which("xorriso").is_ok() {
        enrich_with_xorriso(path, &mut info)?;
    } else {
        info.warnings.push(
            "xorriso is not installed; ISO metadata is limited until local tooling is available"
                .to_string(),
        );
    }

    Ok(info)
}

fn enrich_with_xorriso(path: &Path, info: &mut IsoMetadata) -> EngineResult<()> {
    let el_torito = crate::orchestrator::run_command_capture(
        "xorriso",
        &[
            "-indev".to_string(),
            path.display().to_string(),
            "-report_el_torito".to_string(),
            "plain".to_string(),
        ],
        None,
    )?;
    let boot_report = format!(
        "{}\n{}",
        el_torito.stdout.to_lowercase(),
        el_torito.stderr.to_lowercase()
    );
    info.boot.bios = boot_report.contains("pltf  bios")
        || boot_report.contains("boot img :   1  bios")
        || boot_report.contains("bios");
    info.boot.uefi = boot_report.contains("pltf  uefi")
        || boot_report.contains("boot img :   2  uefi")
        || boot_report.contains("uefi");

    if let Some(body) = extract_optional_file(path, "/.disk/info")? {
        infer_from_disk_info(&body, info);
    }
    if let Some(body) = extract_optional_file(path, "/.treeinfo")? {
        infer_from_treeinfo(&body, info);
    }
    if let Some(body) = extract_optional_file(path, "/arch/version")? {
        infer_from_arch_version(&body, info);
    }

    if info.rootfs_path.is_none() {
        for candidate in [
            "/casper/filesystem.squashfs",
            "/live/filesystem.squashfs",
            "/arch/x86_64/airootfs.sfs",
            "/LiveOS/squashfs.img",
        ] {
            if iso_path_exists(path, candidate)? {
                info.rootfs_path = Some(candidate.trim_start_matches('/').to_string());
                break;
            }
        }
    }

    Ok(())
}

fn extract_optional_file(path: &Path, iso_path: &str) -> EngineResult<Option<String>> {
    let tmp = std::env::temp_dir().join(format!("forgeiso-extract-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp)?;
    let out = tmp.join("extract.txt");
    let result = crate::orchestrator::run_command_capture(
        "xorriso",
        &[
            "-osirrox".to_string(),
            "on".to_string(),
            "-indev".to_string(),
            path.display().to_string(),
            "-extract".to_string(),
            iso_path.to_string(),
            out.display().to_string(),
        ],
        None,
    );

    match result {
        Ok(_) if out.exists() => {
            let body = std::fs::read_to_string(&out)?;
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(Some(body))
        }
        Ok(_) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(None)
        }
        Err(_) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(None)
        }
    }
}

fn iso_path_exists(path: &Path, iso_path: &str) -> EngineResult<bool> {
    let result = crate::orchestrator::run_command_capture(
        "xorriso",
        &[
            "-indev".to_string(),
            path.display().to_string(),
            "-find".to_string(),
            iso_path.to_string(),
            "-exec".to_string(),
            "report_found".to_string(),
        ],
        None,
    );

    match result {
        Ok(output) => Ok(output.stdout.contains(iso_path) || output.stderr.contains(iso_path)),
        Err(_) => Ok(false),
    }
}

fn infer_from_label(label: &str, info: &mut IsoMetadata) {
    let lowered = label.to_lowercase();
    if lowered.contains("ubuntu") {
        info.distro = Some(Distro::Ubuntu);
    } else if lowered.contains("mint") {
        info.distro = Some(Distro::Mint);
    } else if lowered.contains("fedora") {
        info.distro = Some(Distro::Fedora);
    } else if lowered.contains("arch") {
        info.distro = Some(Distro::Arch);
    }

    if info.architecture.is_none() {
        info.architecture = infer_architecture(label);
    }
    if info.release.is_none() {
        info.release = capture_version(label);
    }
}

fn infer_from_disk_info(body: &str, info: &mut IsoMetadata) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return;
    }

    infer_from_label(trimmed, info);
    if info.edition.is_none() {
        info.edition = Some(trimmed.to_string());
    }
}

fn infer_from_treeinfo(body: &str, info: &mut IsoMetadata) {
    for line in body.lines() {
        if let Some(value) = line.strip_prefix("family =") {
            let family = value.trim().to_lowercase();
            if family.contains("fedora") {
                info.distro = Some(Distro::Fedora);
            }
        }
        if let Some(value) = line.strip_prefix("version =") {
            info.release = Some(value.trim().to_string());
        }
        if let Some(value) = line.strip_prefix("arch =") {
            info.architecture = Some(value.trim().to_string());
        }
        if let Some(value) = line.strip_prefix("variant =") {
            info.edition = Some(value.trim().to_string());
        }
    }
    if info.rootfs_path.is_none() {
        info.rootfs_path = Some("LiveOS/squashfs.img".to_string());
    }
}

fn infer_from_arch_version(body: &str, info: &mut IsoMetadata) {
    info.distro = Some(Distro::Arch);
    let version = body
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    if let Some(version) = version {
        info.release = Some(version.to_string());
    }
    if info.rootfs_path.is_none() {
        info.rootfs_path = Some("arch/x86_64/airootfs.sfs".to_string());
    }
}

fn capture_version(input: &str) -> Option<String> {
    let regex = Regex::new(r"(\d{4}\.\d{2}\.\d{2}|\d{2}\.\d{2}|\d{1,2}(?:\.\d+)?)").ok()?;
    regex
        .captures(input)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

fn infer_architecture(input: &str) -> Option<String> {
    let lowered = input.to_lowercase();
    if lowered.contains("amd64") || lowered.contains("x86_64") {
        Some("x86_64".to_string())
    } else if lowered.contains("arm64") || lowered.contains("aarch64") {
        Some("aarch64".to_string())
    } else {
        None
    }
}

fn read_primary_volume_id(path: &Path) -> EngineResult<Option<String>> {
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(16 * 2048))?;

    let mut sector = [0_u8; 2048];
    if let Err(error) = file.read_exact(&mut sector) {
        return Err(EngineError::InvalidConfig(format!(
            "{} is too small to be an ISO image: {error}",
            path.display()
        )));
    }

    if &sector[1..6] != b"CD001" {
        return Err(EngineError::InvalidConfig(format!(
            "{} is not an ISO-9660 image",
            path.display()
        )));
    }

    let raw = &sector[40..72];
    let text = String::from_utf8_lossy(raw)
        .trim()
        .trim_matches(char::from(0))
        .trim()
        .to_string();
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_from_label() {
        assert_eq!(
            capture_version("Ubuntu 24.04.1 LTS"),
            Some("24.04".to_string())
        );
        assert_eq!(
            capture_version("Arch Linux 2026.03.05"),
            Some("2026.03.05".to_string())
        );
    }

    #[test]
    fn infers_arch_from_label() {
        assert_eq!(
            infer_architecture("Ubuntu amd64"),
            Some("x86_64".to_string())
        );
        assert_eq!(
            infer_architecture("Fedora aarch64"),
            Some("aarch64".to_string())
        );
    }
}
