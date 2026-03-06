use std::path::PathBuf;

use clap::{Parser, Subcommand};
use forgeiso_engine::{BuildConfig, ForgeIsoEngine, IsoSource, ProfileKind};

#[derive(Debug, Parser)]
#[command(name = "forgeiso", version, about = "ForgeISO local bare-metal CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Doctor {
        #[arg(long)]
        json: bool,
    },
    Inspect {
        #[arg(long)]
        source: String,
        #[arg(long)]
        json: bool,
    },
    Build {
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        overlay: Option<PathBuf>,
        #[arg(long)]
        volume_label: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Scan {
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        policy: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Test {
        #[arg(long)]
        iso: PathBuf,
        #[arg(long)]
        bios: bool,
        #[arg(long)]
        uefi: bool,
        #[arg(long)]
        json: bool,
    },
    Report {
        #[arg(long)]
        build: PathBuf,
        #[arg(long)]
        format: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let engine = ForgeIsoEngine::new();

    match cli.command {
        Commands::Doctor { json } => {
            let report = engine.doctor().await;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("ForgeISO doctor @ {}", report.timestamp);
                println!("Host: {} {}", report.host_os, report.host_arch);
                println!("Linux build support: {}", report.linux_supported);
                println!("Tooling:");
                for (name, available) in report.tooling {
                    println!("  - {name}: {available}");
                }
                for warning in report.warnings {
                    println!("warning: {warning}");
                }
            }
        }
        Commands::Inspect { source, json } => {
            let cache_dir = std::env::temp_dir().join("forgeiso-inspect-cache");
            let info = engine.inspect_source(&source, Some(&cache_dir)).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else {
                println!("Source: {}", info.source_value);
                println!("Cached path: {}", info.source_path.display());
                println!(
                    "Detected: distro={} release={} arch={}",
                    info.distro
                        .map(|value| format!("{:?}", value))
                        .unwrap_or_else(|| "unknown".to_string()),
                    info.release.as_deref().unwrap_or("unknown"),
                    info.architecture.as_deref().unwrap_or("unknown")
                );
                println!(
                    "Volume ID: {}",
                    info.volume_id.as_deref().unwrap_or("unknown")
                );
                if !info.warnings.is_empty() {
                    println!("Warnings:");
                    for warning in info.warnings {
                        println!("  - {warning}");
                    }
                }
            }
        }
        Commands::Build {
            source,
            project,
            out,
            name,
            overlay,
            volume_label,
            profile,
            json,
        } => {
            let cfg = if let Some(project) = project {
                BuildConfig::from_path(&project)?
            } else {
                let source = source.ok_or_else(|| {
                    anyhow::anyhow!("--source is required when --project is not used")
                })?;
                BuildConfig {
                    name: name.unwrap_or_else(|| "forgeiso-build".to_string()),
                    source: IsoSource::from_raw(source),
                    overlay_dir: overlay,
                    output_label: volume_label,
                    profile: parse_profile(profile.as_deref().unwrap_or("minimal"))?,
                    auto_scan: false,
                    auto_test: false,
                    scanning: Default::default(),
                    testing: Default::default(),
                    keep_workdir: false,
                }
            };

            let result = engine.build(&cfg, &out).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Built ISO: {}", result.artifacts[0].display());
                println!("Report JSON: {}", result.report_json.display());
                println!("Report HTML: {}", result.report_html.display());
                println!(
                    "Detected source: distro={} release={}",
                    result
                        .iso
                        .distro
                        .map(|value| format!("{:?}", value))
                        .unwrap_or_else(|| "unknown".to_string()),
                    result.iso.release.as_deref().unwrap_or("unknown")
                );
            }
        }
        Commands::Scan {
            artifact,
            policy,
            json,
        } => {
            let out = artifact
                .parent()
                .map(|p| p.join("scan"))
                .unwrap_or_else(|| PathBuf::from("scan"));
            let result = engine.scan(&artifact, policy.as_deref(), &out).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("scan report: {}", result.report_json.display());
                for report in result.report.reports {
                    println!(
                        "  - {}: {:?} ({})",
                        report.tool, report.status, report.message
                    );
                }
            }
        }
        Commands::Test {
            iso,
            bios,
            uefi,
            json,
        } => {
            let run_bios = bios || !uefi;
            let run_uefi = uefi || !bios;
            let out = iso
                .parent()
                .map(|p| p.join("test"))
                .unwrap_or_else(|| PathBuf::from("test"));
            let result = engine.test_iso(&iso, run_bios, run_uefi, &out).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "bios={} uefi={} passed={}",
                    result.bios, result.uefi, result.passed
                );
                for log in result.logs {
                    println!("  - {}", log.display());
                }
            }
        }
        Commands::Report { build, format } => {
            let path = engine.report(&build, &format).await?;
            println!("{}", path.display());
        }
    }

    Ok(())
}

fn parse_profile(raw: &str) -> anyhow::Result<ProfileKind> {
    match raw {
        "minimal" => Ok(ProfileKind::Minimal),
        "desktop" => Ok(ProfileKind::Desktop),
        other => anyhow::bail!("unsupported profile '{other}': expected minimal|desktop"),
    }
}
