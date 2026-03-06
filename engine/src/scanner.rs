use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::{fs, process::Command};
use walkdir::WalkDir;

use crate::{
    config::{ScanPolicy, ToolStatus},
    error::{EngineError, EngineResult},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeverityCount {
    pub critical: u64,
    pub high: u64,
    pub medium: u64,
    pub low: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolReport {
    pub tool: String,
    pub status: ToolStatus,
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

pub async fn run_scans(
    target: &Path,
    out_dir: &Path,
    policy: &ScanPolicy,
) -> EngineResult<ScanSummary> {
    fs::create_dir_all(out_dir).await?;

    let mut summary = ScanSummary {
        sbom_spdx: None,
        sbom_cyclonedx: None,
        reports: Vec::new(),
        warnings: Vec::new(),
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
        summary.reports.push(
            run_external(
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
            .await?,
        );
    }

    if policy.enable_syft_grype {
        summary.reports.push(
            run_external(
                "syft",
                vec![
                    target.display().to_string(),
                    "-o".to_string(),
                    "json".to_string(),
                ],
                out_dir.join("syft.json"),
            )
            .await?,
        );
        summary.reports.push(
            run_external(
                "grype",
                vec![
                    target.display().to_string(),
                    "-o".to_string(),
                    "json".to_string(),
                ],
                out_dir.join("grype.json"),
            )
            .await?,
        );
    }

    if policy.enable_open_scap {
        summary.reports.push(
            run_external(
                "oscap",
                vec!["--version".to_string()],
                out_dir.join("oscap.txt"),
            )
            .await?,
        );
    }

    if policy.enable_secrets_scan {
        let findings = detect_secrets(target)?;
        let output = out_dir.join("secrets.json");
        fs::write(&output, serde_json::to_vec_pretty(&findings)?).await?;
        let status = if findings.is_empty() {
            ToolStatus::Passed
        } else if policy.strict_secrets {
            summary.strict_failed = true;
            ToolStatus::Failed
        } else {
            ToolStatus::Passed
        };

        if !findings.is_empty() {
            summary
                .warnings
                .push(format!("Potential secrets found: {}", findings.len()));
        }

        summary.reports.push(ToolReport {
            tool: "secrets".to_string(),
            status,
            output,
            message: format!(
                "Local content scan found {} possible secret(s)",
                findings.len()
            ),
            severities: SeverityCount {
                high: findings.len() as u64,
                ..SeverityCount::default()
            },
        });
    }

    if summary.strict_failed {
        return Err(EngineError::PolicyViolation(
            "Strict secrets policy failed".to_string(),
        ));
    }

    Ok(summary)
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

async fn run_external(tool: &str, args: Vec<String>, output: PathBuf) -> EngineResult<ToolReport> {
    if which::which(tool).is_err() {
        let body = serde_json::json!({
            "tool": tool,
            "status": "unavailable",
            "message": "Tool is not installed on this machine"
        });
        fs::write(&output, serde_json::to_vec_pretty(&body)?).await?;
        return Ok(ToolReport {
            tool: tool.to_string(),
            status: ToolStatus::Unavailable,
            output,
            message: format!("{tool} is not installed locally"),
            severities: SeverityCount::default(),
        });
    }

    let result = Command::new(tool)
        .args(args)
        .output()
        .await
        .map_err(|e| EngineError::Runtime(format!("{tool} failed to start: {e}")))?;

    fs::write(&output, &result.stdout).await?;

    Ok(ToolReport {
        tool: tool.to_string(),
        status: if result.status.success() {
            ToolStatus::Passed
        } else {
            ToolStatus::Failed
        },
        output,
        message: String::from_utf8_lossy(&result.stderr).trim().to_string(),
        severities: infer_severities(&result.stdout),
    })
}

fn infer_severities(data: &[u8]) -> SeverityCount {
    let body = String::from_utf8_lossy(data).to_lowercase();
    SeverityCount {
        critical: body.matches("critical").count() as u64,
        high: body.matches("high").count() as u64,
        medium: body.matches("medium").count() as u64,
        low: body.matches("low").count() as u64,
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
