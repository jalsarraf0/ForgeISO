use std::path::PathBuf;

use forgeiso_engine::{
    BuildConfig, ContainerConfig, Distro, FirewallConfig, ForgeIsoEngine, GrubConfig, InjectConfig,
    IsoSource, NetworkConfig, ProfileKind, ProxyConfig, SshConfig, SwapConfig, UserConfig,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use tokio::sync::Mutex;

#[derive(Default)]
struct UiState {
    stream_started: Mutex<bool>,
}

// ── Build ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildRequest {
    source: String,
    output_dir: String,
    name: String,
    overlay_dir: Option<String>,
    output_label: Option<String>,
    profile: String,
}

// ── Inject ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InjectRequest {
    // Basic
    source: String,
    output_dir: String,
    out_name: String,
    output_label: Option<String>,
    autoinstall_yaml: Option<String>,

    // Identity
    hostname: Option<String>,
    username: Option<String>,
    password: Option<String>,
    realname: Option<String>,

    // SSH
    ssh_keys: Vec<String>,
    ssh_password_auth: bool,
    ssh_install_server: bool,

    // Network
    dns_servers: Vec<String>,
    ntp_servers: Vec<String>,
    static_ip: Option<String>,
    gateway: Option<String>,
    http_proxy: Option<String>,
    https_proxy: Option<String>,
    no_proxy: Vec<String>,

    // System
    timezone: Option<String>,
    locale: Option<String>,
    keyboard_layout: Option<String>,

    // Storage / APT
    storage_layout: Option<String>,
    apt_mirror: Option<String>,

    // User & access management
    groups: Vec<String>,
    shell: Option<String>,
    sudo_nopasswd: bool,
    sudo_commands: Vec<String>,

    // Firewall
    firewall_enabled: bool,
    firewall_policy: Option<String>,
    allow_ports: Vec<String>,
    deny_ports: Vec<String>,

    // Services
    enable_services: Vec<String>,
    disable_services: Vec<String>,

    // Kernel sysctl ("key=value" strings)
    sysctl: Vec<String>,

    // Swap
    swap_size_mb: Option<u32>,
    swap_file: Option<String>,
    swappiness: Option<u8>,

    // Containers
    docker: bool,
    podman: bool,
    docker_users: Vec<String>,

    // GRUB
    grub_timeout: Option<u32>,
    grub_cmdline: Vec<String>,
    grub_default: Option<String>,

    // Encryption
    encrypt: bool,
    encrypt_passphrase: Option<String>,

    // Mounts (raw fstab lines)
    mounts: Vec<String>,

    // Packages & repos
    packages: Vec<String>,
    apt_repos: Vec<String>,

    // Commands
    run_commands: Vec<String>,
    extra_late_commands: Vec<String>,

    // Misc
    no_user_interaction: bool,

    // Target distro: "ubuntu" (default), "fedora", "arch"
    distro: Option<String>,

    // Branding
    wallpaper_path: Option<String>,
}

// ── Native file / folder picker (zenity) ─────────────────────────────────────

/// Open a native OS file-picker dialog filtered to ISO files.
/// Returns the selected path, or null if cancelled.
#[tauri::command]
async fn pick_iso_file() -> Option<String> {
    pick_with_zenity(&[
        "--file-selection",
        "--title=Select ISO Image",
        "--file-filter=ISO Images (*.iso)|*.iso",
    ])
    .await
}

/// Open a native OS folder-picker dialog.
/// Returns the selected path, or null if cancelled.
#[tauri::command]
async fn pick_folder() -> Option<String> {
    pick_with_zenity(&["--file-selection", "--directory", "--title=Select Output Directory"])
        .await
}

/// Open a native OS file-picker for any file (wallpaper images, YAML, etc.).
#[tauri::command]
async fn pick_file() -> Option<String> {
    pick_with_zenity(&["--file-selection", "--title=Select File"]).await
}

async fn pick_with_zenity(args: &[&str]) -> Option<String> {
    // Build the argument list as owned strings so they can be moved into spawn
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let result = tokio::process::Command::new("zenity")
        .args(&args)
        .output()
        .await
        .ok()?;
    if result.status.success() {
        let path = String::from_utf8_lossy(&result.stdout).trim().to_string();
        if !path.is_empty() { Some(path) } else { None }
    } else {
        None
    }
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
async fn doctor(engine: State<'_, ForgeIsoEngine>) -> Result<serde_json::Value, String> {
    serde_json::to_value(engine.doctor().await).map_err(|e| e.to_string())
}

#[tauri::command]
async fn inspect_source(
    engine: State<'_, ForgeIsoEngine>,
    source: String,
) -> Result<serde_json::Value, String> {
    let value = engine
        .inspect_source(&source, None)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(value).map_err(|e| e.to_string())
}

#[tauri::command]
async fn build_local(
    engine: State<'_, ForgeIsoEngine>,
    request: BuildRequest,
) -> Result<serde_json::Value, String> {
    let cfg = BuildConfig {
        name: request.name,
        source: IsoSource::from_raw(request.source),
        overlay_dir: request
            .overlay_dir
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from),
        output_label: request.output_label.filter(|v| !v.trim().is_empty()),
        profile: parse_profile(&request.profile)?,
        auto_scan: false,
        auto_test: false,
        scanning: Default::default(),
        testing: Default::default(),
        keep_workdir: false,
    };

    let out_dir = PathBuf::from(request.output_dir);
    let result = engine.build(&cfg, &out_dir).await.map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn scan_artifact(
    engine: State<'_, ForgeIsoEngine>,
    artifact: String,
    policy: Option<String>,
) -> Result<serde_json::Value, String> {
    let artifact = PathBuf::from(artifact);
    let policy = policy.map(PathBuf::from);
    let out = artifact
        .parent()
        .map(|p| p.join("scan"))
        .unwrap_or_else(|| PathBuf::from("scan"));
    let report = engine
        .scan(&artifact, policy.as_deref(), &out)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(report).map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_iso(
    engine: State<'_, ForgeIsoEngine>,
    iso: String,
    bios: bool,
    uefi: bool,
) -> Result<serde_json::Value, String> {
    let out = PathBuf::from(&iso)
        .parent()
        .map(|p| p.join("test"))
        .unwrap_or_else(|| PathBuf::from("test"));
    let result = engine
        .test_iso(PathBuf::from(iso).as_path(), bios, uefi, &out)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn render_report(
    engine: State<'_, ForgeIsoEngine>,
    build_dir: String,
    format: String,
) -> Result<String, String> {
    let output = engine
        .report(PathBuf::from(build_dir).as_path(), &format)
        .await
        .map_err(|e| e.to_string())?;
    Ok(output.display().to_string())
}

#[tauri::command]
async fn inject_iso(
    engine: State<'_, ForgeIsoEngine>,
    request: InjectRequest,
) -> Result<serde_json::Value, String> {
    let opt_str = |s: Option<String>| s.filter(|v| !v.trim().is_empty());

    let resolved_distro = match request.distro.as_deref() {
        None | Some("") | Some("ubuntu") => None,
        Some("fedora") => Some(Distro::Fedora),
        Some("arch") => Some(Distro::Arch),
        Some("mint") => Some(Distro::Mint),
        Some(_) => None,
    };

    let sysctl: Vec<(String, String)> = request
        .sysctl
        .iter()
        .filter_map(|s| {
            let mut parts = s.splitn(2, '=');
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        })
        .collect();

    let cfg = InjectConfig {
        source: IsoSource::from_raw(request.source),
        out_name: request.out_name,
        output_label: opt_str(request.output_label),
        autoinstall_yaml: opt_str(request.autoinstall_yaml).map(PathBuf::from),
        hostname: opt_str(request.hostname),
        username: opt_str(request.username),
        password: opt_str(request.password),
        realname: opt_str(request.realname),
        ssh: SshConfig {
            authorized_keys: request.ssh_keys,
            allow_password_auth: if request.ssh_password_auth { Some(true) } else { None },
            install_server: if request.ssh_install_server { Some(true) } else { None },
        },
        network: NetworkConfig {
            dns_servers: request.dns_servers,
            ntp_servers: request.ntp_servers,
        },
        static_ip: opt_str(request.static_ip),
        gateway: opt_str(request.gateway),
        proxy: ProxyConfig {
            http_proxy: opt_str(request.http_proxy),
            https_proxy: opt_str(request.https_proxy),
            no_proxy: request.no_proxy,
        },
        timezone: opt_str(request.timezone),
        locale: opt_str(request.locale),
        keyboard_layout: opt_str(request.keyboard_layout),
        storage_layout: opt_str(request.storage_layout),
        apt_mirror: opt_str(request.apt_mirror),
        extra_packages: request.packages,
        wallpaper: opt_str(request.wallpaper_path).map(PathBuf::from),
        extra_late_commands: request.extra_late_commands,
        no_user_interaction: request.no_user_interaction,
        user: UserConfig {
            groups: request.groups,
            shell: opt_str(request.shell),
            sudo_nopasswd: request.sudo_nopasswd,
            sudo_commands: request.sudo_commands,
        },
        firewall: FirewallConfig {
            enabled: request.firewall_enabled,
            default_policy: opt_str(request.firewall_policy),
            allow_ports: request.allow_ports,
            deny_ports: request.deny_ports,
        },
        enable_services: request.enable_services,
        disable_services: request.disable_services,
        sysctl,
        swap: request.swap_size_mb.map(|mb| SwapConfig {
            size_mb: mb,
            filename: opt_str(request.swap_file),
            swappiness: request.swappiness,
        }),
        apt_repos: request.apt_repos,
        containers: ContainerConfig {
            docker: request.docker,
            podman: request.podman,
            docker_users: request.docker_users,
        },
        grub: GrubConfig {
            timeout: request.grub_timeout,
            cmdline_extra: request.grub_cmdline,
            default_entry: opt_str(request.grub_default),
        },
        encrypt: request.encrypt,
        encrypt_passphrase: opt_str(request.encrypt_passphrase),
        mounts: request.mounts,
        run_commands: request.run_commands,
        distro: resolved_distro,
    };

    let out = PathBuf::from(request.output_dir);
    let result = engine
        .inject_autoinstall(&cfg, &out)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn verify_iso(
    engine: State<'_, ForgeIsoEngine>,
    source: String,
    sums_url: Option<String>,
) -> Result<serde_json::Value, String> {
    let result = engine
        .verify(&source, sums_url.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn validate_iso9660(
    engine: State<'_, ForgeIsoEngine>,
    path: String,
) -> Result<serde_json::Value, String> {
    let result = engine
        .validate_iso9660(&path)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn diff_isos(
    engine: State<'_, ForgeIsoEngine>,
    base: String,
    target: String,
) -> Result<serde_json::Value, String> {
    let result = engine
        .diff_isos(PathBuf::from(base).as_path(), PathBuf::from(target).as_path())
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_event_stream(
    app: tauri::AppHandle,
    engine: State<'_, ForgeIsoEngine>,
    state: State<'_, UiState>,
) -> Result<(), String> {
    let mut started = state.stream_started.lock().await;
    if *started {
        return Ok(());
    }

    let mut rx = engine.subscribe();
    *started = true;
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let payload = serde_json::json!({
                "ts": event.ts.to_rfc3339(),
                "phase": format!("{:?}", event.phase),
                "level": format!("{:?}", event.level),
                "message": event.message,
                "substage": event.substage,
                "percent": event.percent,
                "bytesDone": event.bytes_done,
                "bytesTotal": event.bytes_total,
            });
            let _ = app.emit("forgeiso-log", payload);
        }
    });

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn parse_profile(raw: &str) -> Result<ProfileKind, String> {
    match raw {
        "minimal" => Ok(ProfileKind::Minimal),
        "desktop" => Ok(ProfileKind::Desktop),
        _ => Err(format!("unsupported profile: {raw}")),
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .manage(ForgeIsoEngine::new())
        .manage(UiState::default())
        .invoke_handler(tauri::generate_handler![
            doctor,
            inspect_source,
            build_local,
            scan_artifact,
            test_iso,
            render_report,
            inject_iso,
            verify_iso,
            validate_iso9660,
            diff_isos,
            start_event_stream,
            pick_iso_file,
            pick_folder,
            pick_file,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ForgeISO GUI");
}
