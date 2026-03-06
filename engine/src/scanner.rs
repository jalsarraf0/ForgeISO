use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::{fs, process::Command};
use walkdir::WalkDir;

use crate::{
    config::{ScanPolicy, Severity},
    error::{EngineError, EngineResult},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeverityCount {
    pub critical: u64,
    pub high: u64,
    pub medium: u64,
    pub low: u64,
}

impl SeverityCount {
    fn add_marker(&mut self, marker: &str) {
        match marker {
            "critical" => self.critical += 1,
            "high" => self.high += 1,
            "medium" => self.medium += 1,
            "low" => self.low += 1,
            _ => {}
        }
    }

    fn meets_or_exceeds(&self, threshold: Severity) -> bool {
        match threshold {
            Severity::Critical => self.critical > 0,
            Severity::High => self.critical > 0 || self.high > 0,
            Severity::Medium => self.critical > 0 || self.high > 0 || self.medium > 0,
            Severity::Low => {
                self.critical > 0 || self.high > 0 || self.medium > 0 || self.low > 0
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolReport {
    pub tool: String,
    pub success: bool,
    pub output: PathBuf,
    pub message: String,
    pub severities: SeverityCount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub sbom_spdx: Option<PathBuf>,
    pub sbom_cyclonedx: Option<PathBuf>,
    pub reports: Vec<ToolReport>,
    pub warnings: Vec<String>,
    pub strict_failed: bool,
}

pub async fn run_scans(target: &Path, out_dir: &Path, policy: &ScanPolicy) -> EngineResult<ScanSummary> {
    fs::create_dir_all(out_dir).await?;

    let mut summary = ScanSummary {
        sbom_spdx: None,
        sbom_cyclonedx: None,
        reports: vec![],
        warnings: vec![],
        strict_failed: false,
    };

    if policy.enable_sbom {
        let spdx = out_dir.join("sbom.spdx.json");
        let cdx = out_dir.join("sbom.cdx.json");
        write_simple_sbom(target, &spdx, "SPDX").await?;
        write_simple_sbom(target, &cdx, "CycloneDX").await?;
        summary.sbom_spdx = Some(spdx);
        summary.sbom_cyclonedx = Some(cdx);
    }

    if policy.enable_trivy {
        let report = run_external_or_stub(
            "trivy",
            vec![
                "fs".to_string(),
                "--quiet".to_string(),
                "--format".to_string(),
                "json".to_string(),
                target.display().to_string(),
            ],
            out_dir.join("trivy.json"),
        )
        .await?;
        summary.reports.push(report);
    }

    if policy.enable_syft_grype {
        let syft = run_external_or_stub(
            "syft",
            vec![target.display().to_string(), "-o".to_string(), "json".to_string()],
            out_dir.join("syft.json"),
        )
        .await?;
        summary.reports.push(syft);

        let grype = run_external_or_stub(
            "grype",
            vec![target.display().to_string(), "-o".to_string(), "json".to_string()],
            out_dir.join("grype.json"),
        )
        .await?;
        summary.reports.push(grype);
    }

    if policy.enable_open_scap {
        let oscap = run_external_or_stub(
            "oscap",
            vec!["--version".to_string()],
            out_dir.join("oscap.txt"),
        )
        .await?;
        summary.reports.push(oscap);
    }

    if policy.enable_secrets_scan {
        let findings = detect_secrets(target)?;
        let output = out_dir.join("secrets.json");
        fs::write(&output, serde_json::to_vec_pretty(&findings)?).await?;

        let mut sev = SeverityCount::default();
        if !findings.is_empty() {
            sev.high = findings.len() as u64;
            if policy.strict_secrets {
                summary.strict_failed = true;
                summary.warnings.push(format!(
                    "Strict secrets mode found {} potential secret(s)",
                    findings.len()
                ));
            } else {
                summary
                    .warnings
                    .push(format!("Potential secrets found: {}", findings.len()));
            }
        }

        summary.reports.push(ToolReport {
            tool: "secrets".to_string(),
            success: !policy.strict_secrets || findings.is_empty(),
            output,
            message: "Local secrets pattern scan".to_string(),
            severities: sev,
        });
    }

    if summary.strict_failed {
        return Err(EngineError::PolicyViolation(
            "Strict secrets policy failed".to_string(),
        ));
    }

    if let Some(threshold) = policy.fail_on_severity {
        let total = summarize_severity(&summary.reports);
        if total.meets_or_exceeds(threshold) {
            return Err(EngineError::PolicyViolation(format!(
                "Severity gate failed at threshold {:?}",
                threshold
            )));
        }
    }

    Ok(summary)
}

fn summarize_severity(reports: &[ToolReport]) -> SeverityCount {
    let mut total = SeverityCount::default();
    for report in reports {
        total.critical += report.severities.critical;
        total.high += report.severities.high;
        total.medium += report.severities.medium;
        total.low += report.severities.low;
    }
    total
}

async fn write_simple_sbom(target: &Path, out: &Path, format: &str) -> EngineResult<()> {
    let files = collect_files(target)?;
    let body = serde_json::json!({
        "format": format,
        "generator": "forgeiso",
        "target": target.display().to_string(),
        "files": files,
    });
    fs::write(out, serde_json::to_vec_pretty(&body)?).await?;
    Ok(())
}

async fn run_external_or_stub(tool: &str, args: Vec<String>, output: PathBuf) -> EngineResult<ToolReport> {
    if which::which(tool).is_err() {
        fs::write(
            &output,
            serde_json::to_vec_pretty(&serde_json::json!({
                "tool": tool,
                "status": "stub",
                "message": "tool missing"
            }))?,
        )
        .await?;
        return Ok(ToolReport {
            tool: tool.to_string(),
            success: true,
            output,
            message: format!("{tool} not found; emitted stub report"),
            severities: SeverityCount::default(),
        });
    }

    let result = Command::new(tool)
        .args(args)
        .output()
        .await
        .map_err(|e| EngineError::Runtime(format!("{tool} failed to start: {e}")))?;

    fs::write(&output, &result.stdout).await?;
    let mut severities = SeverityCount::default();
    infer_severity_markers(&result.stdout, &mut severities);

    Ok(ToolReport {
        tool: tool.to_string(),
        success: result.status.success(),
        output,
        message: String::from_utf8_lossy(&result.stderr).to_string(),
        severities,
    })
}

fn infer_severity_markers(data: &[u8], sev: &mut SeverityCount) {
    let body = String::from_utf8_lossy(data).to_lowercase();
    for marker in ["critical", "high", "medium", "low"] {
        let count = body.matches(marker).count();
        for _ in 0..count {
            sev.add_marker(marker);
        }
    }
}

fn collect_files(target: &Path) -> EngineResult<Vec<String>> {
    if target.is_file() {
        return Ok(vec![target.display().to_string()]);
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(target).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            files.push(entry.path().display().to_string());
        }
    }
    Ok(files)
}

fn detect_secrets(target: &Path) -> EngineResult<Vec<BTreeMap<String, String>>> {
    let markers = ["BEGIN PRIVATE KEY", "AKIA", "ghp_", "xoxb-", "token="];
    let mut findings = Vec::new();

    for entry in WalkDir::new(target).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };

        for marker in markers {
            if content.contains(marker) {
                let mut finding = BTreeMap::new();
                finding.insert("file".to_string(), entry.path().display().to_string());
                finding.insert("marker".to_string(), marker.to_string());
                findings.push(finding);
            }
        }
    }

    Ok(findings)
}
