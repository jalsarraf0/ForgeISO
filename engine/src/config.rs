use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, EngineResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Distro {
    Ubuntu,
    Mint,
    Fedora,
    Arch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    Minimal,
    Desktop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Passed,
    Failed,
    Unavailable,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum IsoSource {
    Path(PathBuf),
    Url(String),
}

impl IsoSource {
    pub fn from_raw(input: impl Into<String>) -> Self {
        let raw = input.into();
        if raw.starts_with("http://") || raw.starts_with("https://") {
            Self::Url(raw)
        } else {
            Self::Path(PathBuf::from(raw))
        }
    }

    pub fn display_value(&self) -> String {
        match self {
            Self::Path(path) => path.display().to_string(),
            Self::Url(url) => url.clone(),
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Url(_))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanPolicy {
    #[serde(default = "default_true")]
    pub enable_sbom: bool,
    #[serde(default = "default_true")]
    pub enable_trivy: bool,
    #[serde(default)]
    pub enable_syft_grype: bool,
    #[serde(default)]
    pub enable_open_scap: bool,
    #[serde(default = "default_true")]
    pub enable_secrets_scan: bool,
    #[serde(default)]
    pub strict_secrets: bool,
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self {
            enable_sbom: default_true(),
            enable_trivy: default_true(),
            enable_syft_grype: false,
            enable_open_scap: false,
            enable_secrets_scan: default_true(),
            strict_secrets: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingPolicy {
    #[serde(default = "default_true")]
    pub bios: bool,
    #[serde(default = "default_true")]
    pub uefi: bool,
    #[serde(default = "default_true")]
    pub smoke: bool,
}

impl Default for TestingPolicy {
    fn default() -> Self {
        Self {
            bios: true,
            uefi: true,
            smoke: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub name: String,
    pub source: IsoSource,
    #[serde(default)]
    pub overlay_dir: Option<PathBuf>,
    #[serde(default)]
    pub output_label: Option<String>,
    #[serde(default = "default_profile")]
    pub profile: ProfileKind,
    #[serde(default)]
    pub auto_scan: bool,
    #[serde(default)]
    pub auto_test: bool,
    #[serde(default)]
    pub scanning: ScanPolicy,
    #[serde(default)]
    pub testing: TestingPolicy,
    #[serde(default)]
    pub keep_workdir: bool,
}

impl BuildConfig {
    pub fn from_yaml_str(raw: &str) -> EngineResult<Self> {
        let cfg: Self = serde_yaml::from_str(raw)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn from_path(path: &Path) -> EngineResult<Self> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&raw)
    }

    pub fn validate(&self) -> EngineResult<()> {
        if self.name.trim().is_empty() {
            return Err(EngineError::InvalidConfig(
                "name cannot be empty".to_string(),
            ));
        }

        match &self.source {
            IsoSource::Path(path) => {
                if path.as_os_str().is_empty() {
                    return Err(EngineError::InvalidConfig(
                        "source path cannot be empty".to_string(),
                    ));
                }
            }
            IsoSource::Url(url) => {
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err(EngineError::InvalidConfig(
                        "source URL must start with http:// or https://".to_string(),
                    ));
                }
            }
        }

        if let Some(path) = &self.overlay_dir {
            if !path.exists() {
                return Err(EngineError::InvalidConfig(format!(
                    "overlay_dir does not exist: {}",
                    path.display()
                )));
            }
            if !path.is_dir() {
                return Err(EngineError::InvalidConfig(format!(
                    "overlay_dir must be a directory: {}",
                    path.display()
                )));
            }
        }

        if let Some(label) = &self.output_label {
            if label.trim().is_empty() {
                return Err(EngineError::InvalidConfig(
                    "output_label cannot be blank".to_string(),
                ));
            }
            if label.len() > 32 {
                return Err(EngineError::InvalidConfig(
                    "output_label must be 32 characters or fewer".to_string(),
                ));
            }
        }

        if self.auto_test && !self.testing.smoke {
            return Err(EngineError::InvalidConfig(
                "auto_test requires testing.smoke=true".to_string(),
            ));
        }

        Ok(())
    }
}

fn default_true() -> bool {
    true
}

fn default_profile() -> ProfileKind {
    ProfileKind::Minimal
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshConfig {
    #[serde(default)]
    pub authorized_keys: Vec<String>,
    /// None = engine decides (false if keys present, true otherwise)
    #[serde(default)]
    pub allow_password_auth: Option<bool>,
    /// None = defaults to true (install openssh-server)
    #[serde(default)]
    pub install_server: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkConfig {
    #[serde(default)]
    pub dns_servers: Vec<String>,
    #[serde(default)]
    pub ntp_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectConfig {
    pub source: IsoSource,
    /// Optional: if None, YAML is generated from fields below
    #[serde(default)]
    pub autoinstall_yaml: Option<PathBuf>,
    pub out_name: String,
    #[serde(default)]
    pub output_label: Option<String>,

    // Identity
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    /// Plaintext; hashed to $6$ format before writing
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub realname: Option<String>,

    // SSH
    #[serde(default)]
    pub ssh: SshConfig,

    // Network
    #[serde(default)]
    pub network: NetworkConfig,

    // System
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub keyboard_layout: Option<String>,

    // Storage/Apt
    #[serde(default)]
    pub storage_layout: Option<String>, // "lvm" | "direct" | "zfs"
    #[serde(default)]
    pub apt_mirror: Option<String>,

    // Packages
    #[serde(default)]
    pub extra_packages: Vec<String>,

    // Wallpaper
    #[serde(default)]
    pub wallpaper: Option<PathBuf>,

    // Escape hatches
    #[serde(default)]
    pub extra_late_commands: Vec<String>,
    #[serde(default)]
    pub no_user_interaction: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_url_source() {
        let source = IsoSource::from_raw("https://example.test/test.iso");
        assert!(matches!(source, IsoSource::Url(_)));
    }

    #[test]
    fn rejects_missing_overlay_dir() {
        let cfg = BuildConfig {
            name: "demo".to_string(),
            source: IsoSource::from_raw("/tmp/base.iso"),
            overlay_dir: Some(PathBuf::from("/definitely/missing")),
            output_label: None,
            profile: ProfileKind::Minimal,
            auto_scan: false,
            auto_test: false,
            scanning: ScanPolicy::default(),
            testing: TestingPolicy::default(),
            keep_workdir: false,
        };

        assert!(cfg.validate().is_err());
    }

    #[test]
    fn scan_policy_defaults_enable_local_checks() {
        let policy = ScanPolicy::default();

        assert!(policy.enable_sbom);
        assert!(policy.enable_trivy);
        assert!(policy.enable_secrets_scan);
        assert!(!policy.enable_syft_grype);
        assert!(!policy.enable_open_scap);
    }
}
