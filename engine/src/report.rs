use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{BuildConfig, BuildMode, Distro};
use crate::error::EngineResult;
use crate::policy::PolicyWarning;
use crate::scanner::ScanSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildMetadata {
    pub generated_at: DateTime<Utc>,
    pub tool_name: String,
    pub tool_version: String,
    pub distro: Distro,
    pub release: String,
    pub build_mode: BuildMode,
    pub base_iso_url: Option<String>,
    pub base_iso_checksum: Option<String>,
    pub warnings: Vec<String>,
    pub policy_warnings: Vec<PolicyWarning>,
    pub tool_versions: BTreeMap<String, String>,
    pub container_image_digests: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSummary {
    pub bios: bool,
    pub uefi: bool,
    pub logs: Vec<String>,
    pub screenshots: Vec<String>,
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
    pub fn new(cfg: &BuildConfig) -> Self {
        Self {
            metadata: BuildMetadata {
                generated_at: Utc::now(),
                tool_name: "forgeiso".to_string(),
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                distro: cfg.distro,
                release: cfg.release.version.clone(),
                build_mode: cfg.build_mode,
                base_iso_url: cfg.release.base_iso_url.clone(),
                base_iso_checksum: cfg.release.base_iso_checksum.clone(),
                warnings: Vec::new(),
                policy_warnings: Vec::new(),
                tool_versions: BTreeMap::new(),
                container_image_digests: BTreeMap::new(),
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
        let warning_items = self
            .metadata
            .warnings
            .iter()
            .map(|w| format!("<li>{}</li>", html_escape(w)))
            .collect::<Vec<_>>()
            .join("\n");

        let policy_items = self
            .metadata
            .policy_warnings
            .iter()
            .map(|w| format!("<li><b>{}</b>: {}</li>", html_escape(&w.code), html_escape(&w.message)))
            .collect::<Vec<_>>()
            .join("\n");

        let artifact_items = self
            .artifacts
            .iter()
            .map(|a| format!("<li>{}</li>", html_escape(a)))
            .collect::<Vec<_>>()
            .join("\n");

        let body = format!(
            "<!doctype html><html><head><meta charset='utf-8'><title>ForgeISO Report</title><style>body{{font-family:Inter,Segoe UI,Arial,sans-serif;background:#0b1020;color:#eaf0ff;padding:24px;line-height:1.5}}section{{background:#141b33;border:1px solid #27345c;border-radius:12px;padding:16px;margin:12px 0}}h1,h2{{margin:0 0 12px}}ul{{margin:8px 0 0 18px}}</style></head><body><h1>ForgeISO Build Report</h1><section><h2>Metadata</h2><p><b>Generated:</b> {generated}</p><p><b>Distro:</b> {distro:?} {release}</p><p><b>Mode:</b> {mode:?}</p><p><b>Base ISO URL:</b> {base_url}</p><p><b>Base ISO Checksum:</b> {checksum}</p></section><section><h2>Artifacts</h2><ul>{artifact_items}</ul></section><section><h2>Warnings</h2><ul>{warning_items}</ul></section><section><h2>Policy Warnings</h2><ul>{policy_items}</ul></section></body></html>",
            generated = self.metadata.generated_at,
            distro = self.metadata.distro,
            release = html_escape(&self.metadata.release),
            mode = self.metadata.build_mode,
            base_url = html_escape(
                self.metadata
                    .base_iso_url
                    .as_deref()
                    .unwrap_or("not specified"),
            ),
            checksum = html_escape(
                self.metadata
                    .base_iso_checksum
                    .as_deref()
                    .unwrap_or("not specified"),
            ),
            artifact_items = artifact_items,
            warning_items = warning_items,
            policy_items = policy_items,
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
