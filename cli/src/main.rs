use std::path::PathBuf;

use clap::{Parser, Subcommand};
use forgeiso_engine::{parse_build_mode, Distro, EngineEvent, EventLevel, ForgeIsoEngine};

#[derive(Debug, Parser)]
#[command(name = "forgeiso", version, about = "ForgeISO enterprise CLI")]
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
    ListReleases {
        #[arg(long)]
        distro: String,
        #[arg(long)]
        json: bool,
    },
    Build {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        latest: bool,
        #[arg(long)]
        pinned: bool,
        #[arg(long)]
        keep_workdir: bool,
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
        uefi: bool,
        #[arg(long)]
        bios: bool,
        #[arg(long)]
        json: bool,
    },
    Report {
        #[arg(long)]
        build: PathBuf,
        #[arg(long)]
        format: String,
    },
    Inspect {
        #[arg(long)]
        iso: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let engine = ForgeIsoEngine::new();

    match cli.command {
        Commands::Doctor { json } => {
            let report = engine.doctor().await;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("ForgeISO doctor @ {}", report.timestamp);
                println!("Runtimes:");
                for (name, available) in report.runtime_candidates {
                    println!("  - {name}: {available}");
                }
                println!("Tooling:");
                for (name, available) in report.tooling {
                    println!("  - {name}: {available}");
                }
            }
        }
        Commands::ListReleases { distro, json } => {
            let distro = parse_distro(&distro)?;
            let releases = engine.list_releases(distro).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&releases)?);
            } else {
                for release in releases {
                    println!(
                        "{} lts={} stable={} warning={}",
                        release.version,
                        release.lts,
                        release.stable,
                        release.eol_warning.unwrap_or_default()
                    );
                }
            }
        }
        Commands::Build {
            config,
            out,
            latest,
            pinned,
            keep_workdir,
        } => {
            let mode = parse_build_mode(latest, pinned)?;
            let mut receiver = engine.subscribe();
            let log_task = tokio::spawn(async move {
                while let Ok(event) = receiver.recv().await {
                    render_event(&event);
                }
            });

            let result = engine
                .build_from_file(&config, &out, mode, keep_workdir)
                .await?;

            log_task.abort();
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Scan {
            artifact,
            policy,
            json,
        } => {
            let mut receiver = engine.subscribe();
            let log_task = tokio::spawn(async move {
                while let Ok(event) = receiver.recv().await {
                    render_event(&event);
                }
            });

            let out = artifact
                .parent()
                .map(|p| p.join("scan"))
                .unwrap_or_else(|| PathBuf::from("scan"));
            let result = engine.scan(&artifact, policy.as_deref(), &out).await?;

            log_task.abort();
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("scan report: {}", result.report_json.display());
            }
        }
        Commands::Test {
            iso,
            uefi,
            bios,
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
                println!("bios={} uefi={} passed={}", result.bios, result.uefi, result.passed);
            }
        }
        Commands::Report { build, format } => {
            let path = engine.report(&build, &format).await?;
            println!("{}", path.display());
        }
        Commands::Inspect { iso } => {
            let info = engine.inspect_iso(&iso).await?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    }

    Ok(())
}

fn parse_distro(raw: &str) -> anyhow::Result<Distro> {
    match raw {
        "ubuntu" => Ok(Distro::Ubuntu),
        "mint" => Ok(Distro::Mint),
        "fedora" => Ok(Distro::Fedora),
        "arch" => Ok(Distro::Arch),
        _ => anyhow::bail!("unsupported distro '{}': expected ubuntu|mint|fedora|arch", raw),
    }
}

fn render_event(event: &EngineEvent) {
    let level = match event.level {
        EventLevel::Debug => "DEBUG",
        EventLevel::Info => "INFO",
        EventLevel::Warn => "WARN",
        EventLevel::Error => "ERROR",
    };

    println!(
        "[{}] [{:?}] [{}] {}",
        event.ts.to_rfc3339(),
        event.phase,
        level,
        event.message
    );
}
