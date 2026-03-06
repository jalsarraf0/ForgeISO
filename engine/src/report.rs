use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    config::{BuildConfig, ProfileKind},
    error::EngineResult,
    iso::IsoMetadata,
    scanner::ScanSummary,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildMetadata {
    pub generated_at: DateTime<Utc>,
    pub tool_name: String,
    pub tool_version: String,
    pub profile: ProfileKind,
    pub source: String,
    pub source_sha256: String,
    pub detected_distro: Option<String>,
    pub detected_release: Option<String>,
    pub detected_architecture: Option<String>,
    pub volume_id: Option<String>,
    pub output_label: Option<String>,
    pub warnings: Vec<String>,
    pub tool_versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSummary {
    pub bios: bool,
    pub uefi: bool,
    pub logs: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub metadata: BuildMetadata,
    pub artifacts: Vec<String>,
    pub scan_summary: Option<ScanSummary>,
    pub test_summary: Option<TestSummary>,
}

impl BuildReport {
    pub fn new(cfg: &BuildConfig, iso: &IsoMetadata) -> Self {
        Self {
            metadata: BuildMetadata {
                generated_at: Utc::now(),
                tool_name: "forgeiso".to_string(),
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                profile: cfg.profile,
                source: cfg.source.display_value(),
                source_sha256: iso.sha256.clone(),
                detected_distro: iso.distro.map(|value| format!("{:?}", value)),
                detected_release: iso.release.clone(),
                detected_architecture: iso.architecture.clone(),
                volume_id: iso.volume_id.clone(),
                output_label: cfg.output_label.clone(),
                warnings: iso.warnings.clone(),
                tool_versions: BTreeMap::new(),
            },
            artifacts: Vec::new(),
            scan_summary: None,
            test_summary: None,
        }
    }

    pub fn write_json(&self, out: &Path) -> EngineResult<()> {
        std::fs::write(out, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn write_html(&self, out: &Path) -> EngineResult<()> {
        let artifact_items = self
            .artifacts
            .iter()
            .map(|a| format!("<li>{}</li>", html_escape(a)))
            .collect::<Vec<_>>()
            .join("\n");

        let warning_items = self
            .metadata
            .warnings
            .iter()
            .map(|a| format!("<li>{}</li>", html_escape(a)))
            .collect::<Vec<_>>()
            .join("\n");

        let body = format!(
            "<!doctype html><html><head><meta charset='utf-8'><title>ForgeISO Report</title><style>body{{font-family:Inter,Segoe UI,Arial,sans-serif;background:#0f172a;color:#e2e8f0;padding:24px;line-height:1.55}}section{{background:#111827;border:1px solid #334155;border-radius:12px;padding:16px;margin:12px 0}}h1,h2{{margin:0 0 12px}}ul{{margin:8px 0 0 18px}}code{{background:#020617;padding:2px 6px;border-radius:6px}}</style></head><body><h1>ForgeISO Local Build Report</h1><section><h2>Source</h2><p><b>Input:</b> <code>{source}</code></p><p><b>SHA-256:</b> <code>{sha}</code></p><p><b>Distro:</b> {distro}</p><p><b>Release:</b> {release}</p><p><b>Architecture:</b> {arch}</p><p><b>Volume ID:</b> {volume}</p></section><section><h2>Artifacts</h2><ul>{artifact_items}</ul></section><section><h2>Warnings</h2><ul>{warning_items}</ul></section></body></html>",
            source = html_escape(&self.metadata.source),
            sha = html_escape(&self.metadata.source_sha256),
            distro = html_escape(self.metadata.detected_distro.as_deref().unwrap_or("unknown")),
            release = html_escape(self.metadata.detected_release.as_deref().unwrap_or("unknown")),
            arch = html_escape(self.metadata.detected_architecture.as_deref().unwrap_or("unknown")),
            volume = html_escape(self.metadata.volume_id.as_deref().unwrap_or("unknown")),
            artifact_items = artifact_items,
            warning_items = warning_items,
        );

        std::fs::write(out, body)?;
        Ok(())
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
