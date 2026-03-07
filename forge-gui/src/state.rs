use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Stage ordering ─────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Stage {
    Inject,
    Verify,
    Diff,
    Build,
    Completion,
}

impl Stage {
    pub const ALL: &'static [Stage] = &[
        Stage::Inject,
        Stage::Verify,
        Stage::Diff,
        Stage::Build,
        Stage::Completion,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Stage::Inject => "Inject",
            Stage::Verify => "Verify",
            Stage::Diff => "Diff",
            Stage::Build => "Build",
            Stage::Completion => "Complete",
        }
    }

    pub fn sublabel(&self) -> &'static str {
        match self {
            Stage::Inject => "Autoinstall config",
            Stage::Verify => "SHA-256 integrity",
            Stage::Diff => "Compare ISOs",
            Stage::Build => "Fetch & package",
            Stage::Completion => "Artifacts ready",
        }
    }

    pub fn step_num(&self) -> usize {
        match self {
            Stage::Inject => 1,
            Stage::Verify => 2,
            Stage::Diff => 3,
            Stage::Build => 4,
            Stage::Completion => 5,
        }
    }
}

// ── File picker target ─────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PickTarget {
    InjectSource,
    InjectOutputDir,
    InjectWallpaper,
    VerifySource,
    DiffBase,
    DiffTarget,
    BuildSource,
    BuildOutputDir,
    BuildOverlay,
}

// ── Inject form state ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InjectState {
    // Required
    pub source: String,
    pub output_dir: String,
    pub out_name: String,
    pub output_label: String,
    pub distro: String,
    // Identity
    pub hostname: String,
    pub username: String,
    #[serde(skip)] // never persist passwords to disk
    pub password: String,
    pub realname: String,
    // SSH
    pub ssh_keys: String, // newline-separated
    pub ssh_password_auth: bool,
    pub ssh_install_server: bool,
    // Network
    pub dns_servers: String,
    pub ntp_servers: String,
    pub static_ip: String,
    pub gateway: String,
    pub http_proxy: String,
    pub https_proxy: String,
    // System
    pub timezone: String,
    pub locale: String,
    pub keyboard_layout: String,
    pub storage_layout: String,
    pub apt_mirror: String,
    // Packages
    pub packages: String,
    pub apt_repos: String,
    // Commands
    pub run_commands: String,
    pub late_commands: String,
    // Firewall
    pub firewall_enabled: bool,
    pub firewall_policy: String,
    pub allow_ports: String,
    pub deny_ports: String,
    // Containers
    pub docker: bool,
    pub podman: bool,
    // Swap
    pub swap_size_mb: String,
    // Misc
    pub no_user_interaction: bool,
    pub wallpaper_path: String,
}

impl Default for InjectState {
    fn default() -> Self {
        let cache = dirs_cache();
        Self {
            source: String::new(),
            output_dir: cache,
            out_name: "forgeiso-local".into(),
            output_label: String::new(),
            distro: "ubuntu".into(),
            hostname: String::new(),
            username: String::new(),
            password: String::new(),
            realname: String::new(),
            ssh_keys: String::new(),
            ssh_password_auth: false,
            ssh_install_server: true,
            dns_servers: String::new(),
            ntp_servers: String::new(),
            static_ip: String::new(),
            gateway: String::new(),
            http_proxy: String::new(),
            https_proxy: String::new(),
            timezone: String::new(),
            locale: String::new(),
            keyboard_layout: String::new(),
            storage_layout: String::new(),
            apt_mirror: String::new(),
            packages: String::new(),
            apt_repos: String::new(),
            run_commands: String::new(),
            late_commands: String::new(),
            firewall_enabled: false,
            firewall_policy: String::new(),
            allow_ports: String::new(),
            deny_ports: String::new(),
            docker: false,
            podman: false,
            swap_size_mb: String::new(),
            no_user_interaction: false,
            wallpaper_path: String::new(),
        }
    }
}

// ── Verify form state ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VerifyState {
    pub source: String,
    pub sums_url: String,
}

// ── Diff form state ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiffState {
    pub base: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DiffFilter {
    #[default]
    All,
    Added,
    Removed,
    Modified,
}

// ── Build form state ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildState {
    pub source: String,
    pub output_dir: String,
    pub build_name: String,
    pub overlay_dir: String,
    pub output_label: String,
    pub profile: String,
    pub distro: String,
}

impl Default for BuildState {
    fn default() -> Self {
        Self {
            source: String::new(),
            output_dir: "./artifacts".into(),
            build_name: "forgeiso-local".into(),
            overlay_dir: String::new(),
            output_label: String::new(),
            profile: "minimal".into(),
            distro: "ubuntu".into(),
        }
    }
}

// ── Log entry ──────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub phase: String,
    pub message: String,
    pub level: LogLevel,
    pub timestamp: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

// ── Status message ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct StatusMsg {
    pub text: String,
    pub is_error: bool,
}

impl StatusMsg {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }
    pub fn err(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
}

// ── Engine type aliases ────────────────────────────────────────────────────────

pub type BuildResult = forgeiso_engine::BuildResult;
pub type DoctorReport = forgeiso_engine::DoctorReport;
pub type VerifyResult = forgeiso_engine::VerifyResult;
pub type Iso9660Compliance = forgeiso_engine::Iso9660Compliance;
pub type IsoDiff = forgeiso_engine::IsoDiff;
pub type IsoMetadata = forgeiso_engine::IsoMetadata;

// ── Helpers ────────────────────────────────────────────────────────────────────

fn dirs_cache() -> String {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".cache").join("forgeiso"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/forgeiso"))
        .to_string_lossy()
        .into_owned()
}

/// Split a newline-separated textarea into non-empty trimmed strings.
pub fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Treat empty/whitespace-only string as None.
pub fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}
