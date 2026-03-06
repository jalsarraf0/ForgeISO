use std::path::PathBuf;

use forgeiso_engine::{BuildConfig, ForgeIsoEngine, IsoSource, ProfileKind};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use tokio::sync::Mutex;

#[derive(Default)]
struct UiState {
    stream_started: Mutex<bool>,
}

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

#[tauri::command]
async fn doctor(engine: State<'_, ForgeIsoEngine>) -> Result<serde_json::Value, String> {
    serde_json::to_value(engine.doctor().await).map_err(|e| e.to_string())
}

#[tauri::command]
async fn inspect_source(engine: State<'_, ForgeIsoEngine>, source: String) -> Result<serde_json::Value, String> {
    let cache = std::env::temp_dir().join("forgeiso-gui-inspect");
    let value = engine
        .inspect_source(&source, Some(&cache))
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(value).map_err(|e| e.to_string())
}

#[tauri::command]
async fn build_local(engine: State<'_, ForgeIsoEngine>, request: BuildRequest) -> Result<serde_json::Value, String> {
    let cfg = BuildConfig {
        name: request.name,
        source: IsoSource::from_raw(request.source),
        overlay_dir: request
            .overlay_dir
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from),
        output_label: request
            .output_label
            .filter(|value| !value.trim().is_empty()),
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

fn parse_profile(raw: &str) -> Result<ProfileKind, String> {
    match raw {
        "minimal" => Ok(ProfileKind::Minimal),
        "desktop" => Ok(ProfileKind::Desktop),
        _ => Err("unsupported profile".to_string()),
    }
}

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
            start_event_stream
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ForgeISO GUI");
}
