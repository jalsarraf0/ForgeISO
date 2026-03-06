#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::BTreeMap;
use std::path::PathBuf;

use forgeiso_engine::{
    config::{
        BuildConfig, BuildMode, DangerousMode, DesktopCustomization, Distro, ModuleSpec,
        ProfileKind, RemoteAgentConfig, ReleaseSelection, RuntimePreference, ScanPolicy, Severity,
        SshPolicy, TestingPolicy, UserAccount,
    },
    ForgeIsoEngine,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;

#[derive(Default)]
struct UiState {
    stream_started: Mutex<bool>,
    agent_endpoint: Mutex<Option<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InlineBuildRequest {
    name: String,
    distro: String,
    release: String,
    profile: String,
}

#[tauri::command]
async fn doctor(engine: State<'_, ForgeIsoEngine>) -> Result<serde_json::Value, String> {
    serde_json::to_value(engine.doctor().await).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_releases(
    engine: State<'_, ForgeIsoEngine>,
    distro: String,
) -> Result<serde_json::Value, String> {
    let distro = parse_distro(&distro)?;
    let value = engine
        .list_releases(distro)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(value).map_err(|e| e.to_string())
}

#[tauri::command]
async fn build_from_inline(
    engine: State<'_, ForgeIsoEngine>,
    request: InlineBuildRequest,
) -> Result<serde_json::Value, String> {
    let distro = parse_distro(&request.distro)?;
    let profile = parse_profile(&request.profile)?;

    let cfg = BuildConfig {
        name: request.name,
        distro,
        release: ReleaseSelection {
            version: request.release,
            codename: None,
            base_iso_url: Some(default_iso_url(distro)),
            base_iso_checksum: None,
        },
        profile,
        build_mode: BuildMode::Pinned,
        runtime: RuntimePreference::Docker,
        users: vec![UserAccount {
            username: "jamal".to_string(),
            display_name: Some("Jamal Al-Sarraf".to_string()),
            groups: vec!["wheel".to_string()],
            sudo_policy: forgeiso_engine::config::SudoPolicy::Password,
            shell: Some("/bin/bash".to_string()),
            passwordless_login: false,
            ssh_authorized_keys: vec![],
            force_key_only_ssh: false,
        }],
        ssh: SshPolicy {
            port: 22,
            permit_root_login: false,
            password_authentication: false,
            pubkey_authentication: true,
            allow_users: vec![],
            allow_groups: vec![],
            max_auth_tries: 4,
            match_blocks: vec![],
            hardened_preset: true,
        },
        desktop: DesktopCustomization::default(),
        modules: vec![ModuleSpec {
            module_type: forgeiso_engine::config::ModuleType::Ssh,
            enabled: true,
            dangerous: false,
            config: serde_json::json!({
                "preset": "hardened"
            }),
        }],
        scanning: ScanPolicy {
            enable_sbom: true,
            enable_trivy: true,
            enable_syft_grype: false,
            enable_open_scap: true,
            enable_secrets_scan: true,
            strict_secrets: true,
            fail_on_severity: Some(Severity::Critical),
            compliance_profile: None,
        },
        testing: TestingPolicy {
            bios: true,
            uefi: true,
            openqa: false,
            in_guest_goss: true,
            smoke: true,
        },
        remote_agent: RemoteAgentConfig {
            enabled: false,
            endpoint: None,
            ca_cert: None,
            client_cert: None,
            client_key: None,
            job_token: None,
        },
        dangerous_mode: DangerousMode {
            enabled: false,
            allow_host_exec: false,
            consent_text: None,
        },
        output_dir: None,
        keep_workdir: false,
    };

    let out_dir = std::env::temp_dir().join("forgeiso-gui");
    let result = engine
        .build(&cfg, &out_dir, false)
        .await
        .map_err(|e| e.to_string())?;

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
    let out = std::env::temp_dir().join("forgeiso-gui-scan");
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
    let out = std::env::temp_dir().join("forgeiso-gui-test");
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
async fn inspect_iso(engine: State<'_, ForgeIsoEngine>, iso: String) -> Result<serde_json::Value, String> {
    engine
        .inspect_iso(PathBuf::from(iso).as_path())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn connect_agent(state: State<'_, UiState>, endpoint: String) -> Result<(), String> {
    if endpoint.trim().is_empty() {
        return Err("endpoint is required".to_string());
    }

    let mut current = state.agent_endpoint.lock().await;
    *current = Some(endpoint);
    Ok(())
}

#[tauri::command]
async fn disconnect_agent(state: State<'_, UiState>) -> Result<(), String> {
    let mut current = state.agent_endpoint.lock().await;
    *current = None;
    Ok(())
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
            });
            let _ = app.emit("forgeiso-log", payload);
        }
    });

    Ok(())
}

fn parse_distro(raw: &str) -> Result<Distro, String> {
    match raw {
        "ubuntu" => Ok(Distro::Ubuntu),
        "mint" => Ok(Distro::Mint),
        "fedora" => Ok(Distro::Fedora),
        "arch" => Ok(Distro::Arch),
        _ => Err("unsupported distro".to_string()),
    }
}

fn parse_profile(raw: &str) -> Result<ProfileKind, String> {
    match raw {
        "hardened_server" => Ok(ProfileKind::HardenedServer),
        "developer_workstation" => Ok(ProfileKind::DeveloperWorkstation),
        "minimal" => Ok(ProfileKind::Minimal),
        "kiosk" => Ok(ProfileKind::Kiosk),
        "gaming" => Ok(ProfileKind::Gaming),
        _ => Err("unsupported profile".to_string()),
    }
}

fn default_iso_url(distro: Distro) -> String {
    let map = BTreeMap::from([
        (Distro::Ubuntu, "https://releases.ubuntu.com/24.04/ubuntu-24.04-live-server-amd64.iso"),
        (Distro::Mint, "https://mirrors.edge.kernel.org/linuxmint/stable/22/linuxmint-22-cinnamon-64bit.iso"),
        (Distro::Fedora, "https://download.fedoraproject.org/pub/fedora/linux/releases/40/Workstation/x86_64/iso/Fedora-Workstation-Live-x86_64-40-1.14.iso"),
        (Distro::Arch, "https://geo.mirror.pkgbuild.com/iso/latest/archlinux-x86_64.iso"),
    ]);

    map.get(&distro).unwrap_or(&"https://example.invalid/base.iso").to_string()
}

fn main() {
    tauri::Builder::default()
        .manage(ForgeIsoEngine::new())
        .manage(UiState::default())
        .invoke_handler(tauri::generate_handler![
            doctor,
            list_releases,
            build_from_inline,
            scan_artifact,
            test_iso,
            render_report,
            inspect_iso,
            connect_agent,
            disconnect_agent,
            start_event_stream
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ForgeISO GUI");
}
