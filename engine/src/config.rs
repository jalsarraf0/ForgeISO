use std::collections::BTreeMap;
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
#[serde(rename_all = "lowercase")]
pub enum BuildMode {
    Latest,
    Pinned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimePreference {
    Docker,
    Podman,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SudoPolicy {
    None,
    Password,
    Passwordless,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    HardenedServer,
    DeveloperWorkstation,
    Minimal,
    Kiosk,
    Gaming,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModuleType {
    Packages,
    Files,
    Systemd,
    Users,
    Ssh,
    Desktop,
    Browser,
    Drivers,
    Fonts,
    Codecs,
    CustomScript,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSelection {
    pub version: String,
    pub codename: Option<String>,
    pub base_iso_url: Option<String>,
    pub base_iso_checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    pub username: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default = "default_sudo_policy")]
    pub sudo_policy: SudoPolicy,
    pub shell: Option<String>,
    #[serde(default)]
    pub passwordless_login: bool,
    #[serde(default)]
    pub ssh_authorized_keys: Vec<String>,
    #[serde(default)]
    pub force_key_only_ssh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshMatchBlock {
    pub user: Option<String>,
    pub group: Option<String>,
    #[serde(default)]
    pub directives: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshPolicy {
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub permit_root_login: bool,
    #[serde(default)]
    pub password_authentication: bool,
    #[serde(default = "default_true")]
    pub pubkey_authentication: bool,
    #[serde(default)]
    pub allow_users: Vec<String>,
    #[serde(default)]
    pub allow_groups: Vec<String>,
    #[serde(default = "default_max_auth_tries")]
    pub max_auth_tries: u8,
    #[serde(default)]
    pub match_blocks: Vec<SshMatchBlock>,
    #[serde(default = "default_true")]
    pub hardened_preset: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppBundle {
    Gaming,
    Dev,
    Minimal,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DesktopCustomization {
    pub wallpaper: Option<PathBuf>,
    pub theme: Option<String>,
    pub icons: Option<String>,
    pub cursor: Option<String>,
    #[serde(default)]
    pub bundles: Vec<AppBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSpec {
    #[serde(rename = "type")]
    pub module_type: ModuleType,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub dangerous: bool,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanPolicy {
    #[serde(default = "default_true")]
    pub enable_sbom: bool,
    #[serde(default = "default_true")]
    pub enable_trivy: bool,
    #[serde(default)]
    pub enable_syft_grype: bool,
    #[serde(default = "default_true")]
    pub enable_open_scap: bool,
    #[serde(default = "default_true")]
    pub enable_secrets_scan: bool,
    #[serde(default = "default_true")]
    pub strict_secrets: bool,
    pub fail_on_severity: Option<Severity>,
    pub compliance_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingPolicy {
    #[serde(default = "default_true")]
    pub bios: bool,
    #[serde(default = "default_true")]
    pub uefi: bool,
    #[serde(default)]
    pub openqa: bool,
    #[serde(default = "default_true")]
    pub in_guest_goss: bool,
    #[serde(default = "default_true")]
    pub smoke: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAgentConfig {
    #[serde(default)]
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub ca_cert: Option<PathBuf>,
    pub client_cert: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
    pub job_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DangerousMode {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allow_host_exec: bool,
    pub consent_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub name: String,
    pub distro: Distro,
    pub release: ReleaseSelection,
    pub profile: ProfileKind,
    #[serde(default = "default_build_mode")]
    pub build_mode: BuildMode,
    #[serde(default = "default_runtime")]
    pub runtime: RuntimePreference,
    #[serde(default)]
    pub users: Vec<UserAccount>,
    #[serde(default = "default_ssh_policy")]
    pub ssh: SshPolicy,
    #[serde(default)]
    pub desktop: DesktopCustomization,
    #[serde(default)]
    pub modules: Vec<ModuleSpec>,
    #[serde(default = "default_scan_policy")]
    pub scanning: ScanPolicy,
    #[serde(default = "default_testing_policy")]
    pub testing: TestingPolicy,
    #[serde(default = "default_remote_agent")]
    pub remote_agent: RemoteAgentConfig,
    #[serde(default = "default_dangerous_mode")]
    pub dangerous_mode: DangerousMode,
    pub output_dir: Option<PathBuf>,
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
            return Err(EngineError::InvalidConfig("name cannot be empty".to_string()));
        }

        if self.release.version.trim().is_empty() {
            return Err(EngineError::InvalidConfig(
                "release.version cannot be empty".to_string(),
            ));
        }

        if self.users.is_empty() {
            return Err(EngineError::InvalidConfig(
                "at least one user must be specified".to_string(),
            ));
        }

        for user in &self.users {
            if !is_valid_username(&user.username) {
                return Err(EngineError::InvalidConfig(format!(
                    "invalid username '{}'",
                    user.username
                )));
            }

            if user.force_key_only_ssh && user.ssh_authorized_keys.is_empty() {
                return Err(EngineError::InvalidConfig(format!(
                    "user '{}' forces key-only ssh but has no authorized keys",
                    user.username
                )));
            }
        }

        if self.ssh.port == 0 {
            return Err(EngineError::InvalidConfig("ssh.port cannot be 0".to_string()));
        }

        if self.ssh.max_auth_tries == 0 {
            return Err(EngineError::InvalidConfig(
                "ssh.max_auth_tries cannot be 0".to_string(),
            ));
        }

        if self.dangerous_mode.allow_host_exec && !self.dangerous_mode.enabled {
            return Err(EngineError::InvalidConfig(
                "dangerous_mode.allow_host_exec requires dangerous_mode.enabled=true".to_string(),
            ));
        }

        if self.remote_agent.enabled {
            if self.remote_agent.endpoint.as_ref().is_none_or(|v| v.trim().is_empty()) {
                return Err(EngineError::InvalidConfig(
                    "remote_agent.endpoint is required when remote_agent.enabled=true".to_string(),
                ));
            }
            if self.remote_agent.job_token.as_ref().is_none_or(|v| v.trim().is_empty()) {
                return Err(EngineError::InvalidConfig(
                    "remote_agent.job_token is required when remote_agent.enabled=true"
                        .to_string(),
                ));
            }
        }

        for module in &self.modules {
            if module.dangerous && !self.dangerous_mode.enabled {
                return Err(EngineError::InvalidConfig(
                    "dangerous modules require dangerous_mode.enabled=true".to_string(),
                ));
            }
        }

        Ok(())
    }
}

fn is_valid_username(username: &str) -> bool {
    let mut chars = username.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn default_true() -> bool {
    true
}

fn default_sudo_policy() -> SudoPolicy {
    SudoPolicy::Password
}

fn default_ssh_port() -> u16 {
    22
}

fn default_max_auth_tries() -> u8 {
    4
}

fn default_build_mode() -> BuildMode {
    BuildMode::Pinned
}

fn default_runtime() -> RuntimePreference {
    RuntimePreference::Docker
}

fn default_ssh_policy() -> SshPolicy {
    SshPolicy {
        port: default_ssh_port(),
        permit_root_login: false,
        password_authentication: false,
        pubkey_authentication: true,
        allow_users: Vec::new(),
        allow_groups: Vec::new(),
        max_auth_tries: default_max_auth_tries(),
        match_blocks: Vec::new(),
        hardened_preset: true,
    }
}

fn default_scan_policy() -> ScanPolicy {
    ScanPolicy {
        enable_sbom: true,
        enable_trivy: true,
        enable_syft_grype: false,
        enable_open_scap: true,
        enable_secrets_scan: true,
        strict_secrets: true,
        fail_on_severity: Some(Severity::Critical),
        compliance_profile: None,
    }
}

fn default_testing_policy() -> TestingPolicy {
    TestingPolicy {
        bios: true,
        uefi: true,
        openqa: false,
        in_guest_goss: true,
        smoke: true,
    }
}

fn default_remote_agent() -> RemoteAgentConfig {
    RemoteAgentConfig {
        enabled: false,
        endpoint: None,
        ca_cert: None,
        client_cert: None,
        client_key: None,
        job_token: None,
    }
}

fn default_dangerous_mode() -> DangerousMode {
    DangerousMode {
        enabled: false,
        allow_host_exec: false,
        consent_text: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> BuildConfig {
        BuildConfig {
            name: "enterprise-sample".to_string(),
            distro: Distro::Ubuntu,
            release: ReleaseSelection {
                version: "24.04".to_string(),
                codename: Some("noble".to_string()),
                base_iso_url: Some("https://releases.ubuntu.com/24.04/ubuntu.iso".to_string()),
                base_iso_checksum: None,
            },
            profile: ProfileKind::Minimal,
            build_mode: BuildMode::Pinned,
            runtime: RuntimePreference::Docker,
            users: vec![UserAccount {
                username: "jamal".to_string(),
                display_name: Some("Jamal".to_string()),
                groups: vec!["wheel".to_string()],
                sudo_policy: SudoPolicy::Password,
                shell: Some("/bin/bash".to_string()),
                passwordless_login: false,
                ssh_authorized_keys: vec!["ssh-ed25519 AAAA test".to_string()],
                force_key_only_ssh: true,
            }],
            ssh: default_ssh_policy(),
            desktop: DesktopCustomization::default(),
            modules: vec![],
            scanning: default_scan_policy(),
            testing: default_testing_policy(),
            remote_agent: default_remote_agent(),
            dangerous_mode: default_dangerous_mode(),
            output_dir: None,
            keep_workdir: false,
        }
    }

    #[test]
    fn rejects_invalid_username() {
        let mut cfg = sample_config();
        cfg.users[0].username = "BadUser".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn rejects_dangerous_without_gate() {
        let mut cfg = sample_config();
        cfg.modules.push(ModuleSpec {
            module_type: ModuleType::CustomScript,
            enabled: true,
            dangerous: true,
            config: serde_json::json!({"path":"/tmp/script.sh"}),
        });
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn accepts_valid_config() {
        let cfg = sample_config();
        assert!(cfg.validate().is_ok());
    }
}
