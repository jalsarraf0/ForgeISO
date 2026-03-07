use std::path::PathBuf;

use clap::{Parser, Subcommand};
use forgeiso_engine::{
    BuildConfig, ContainerConfig, Distro, EventPhase, FirewallConfig, ForgeIsoEngine, GrubConfig,
    InjectConfig, IsoSource, NetworkConfig, ProfileKind, ProxyConfig, SshConfig, SwapConfig,
    UserConfig,
};

#[derive(Debug, Parser)]
#[command(name = "forgeiso", version, about = "ForgeISO local bare-metal CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
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
    Verify {
        #[arg(long)]
        source: String,
        #[arg(long)]
        sums_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Inject {
        #[arg(long)]
        source: String,
        #[arg(long)]
        autoinstall: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        volume_label: Option<String>,

        // Identity
        #[arg(long)]
        hostname: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        password_file: Option<PathBuf>,
        #[arg(long)]
        password_stdin: bool,
        #[arg(long)]
        realname: Option<String>,

        // SSH
        #[arg(long, action = clap::ArgAction::Append)]
        ssh_key: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        ssh_key_file: Vec<PathBuf>,
        #[arg(long)]
        ssh_password_auth: bool,

        // Network
        #[arg(long, action = clap::ArgAction::Append)]
        dns: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        ntp_server: Vec<String>,

        // System
        #[arg(long)]
        timezone: Option<String>,
        #[arg(long)]
        locale: Option<String>,
        #[arg(long)]
        keyboard_layout: Option<String>,

        // Storage/Apt
        #[arg(long)]
        storage_layout: Option<String>,
        #[arg(long)]
        apt_mirror: Option<String>,

        // Packages
        #[arg(long, action = clap::ArgAction::Append)]
        package: Vec<String>,

        // Branding
        #[arg(long)]
        wallpaper: Option<PathBuf>,

        // Escape hatches
        #[arg(long, action = clap::ArgAction::Append)]
        late_command: Vec<String>,
        #[arg(long)]
        no_user_interaction: bool,

        // User & access management
        #[arg(long, action = clap::ArgAction::Append)]
        group: Vec<String>,
        #[arg(long)]
        shell: Option<String>,
        #[arg(long)]
        sudo_nopasswd: bool,
        #[arg(long, action = clap::ArgAction::Append)]
        sudo_command: Vec<String>,

        // Firewall
        #[arg(long)]
        firewall: bool,
        #[arg(long)]
        firewall_policy: Option<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        allow_port: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        deny_port: Vec<String>,

        // Network extras
        #[arg(long)]
        static_ip: Option<String>,
        #[arg(long)]
        gateway: Option<String>,
        #[arg(long)]
        http_proxy: Option<String>,
        #[arg(long)]
        https_proxy: Option<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        no_proxy: Vec<String>,

        // Services
        #[arg(long, action = clap::ArgAction::Append)]
        enable_service: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        disable_service: Vec<String>,

        // Kernel
        #[arg(long, action = clap::ArgAction::Append)]
        sysctl: Vec<String>,

        // Swap
        #[arg(long)]
        swap_size: Option<u32>,
        #[arg(long)]
        swap_file: Option<String>,
        #[arg(long)]
        swappiness: Option<u8>,

        // APT repos
        #[arg(long, action = clap::ArgAction::Append)]
        apt_repo: Vec<String>,

        // Containers
        #[arg(long)]
        docker: bool,
        #[arg(long)]
        podman: bool,
        #[arg(long, action = clap::ArgAction::Append)]
        docker_user: Vec<String>,

        // GRUB
        #[arg(long)]
        grub_timeout: Option<u32>,
        #[arg(long, action = clap::ArgAction::Append)]
        grub_cmdline: Vec<String>,
        #[arg(long)]
        grub_default: Option<String>,

        // Encryption
        #[arg(long)]
        encrypt: bool,
        #[arg(long)]
        encrypt_passphrase: Option<String>,
        #[arg(long)]
        encrypt_passphrase_file: Option<PathBuf>,

        // Mounts
        #[arg(long, action = clap::ArgAction::Append)]
        mount: Vec<String>,

        // Run commands
        #[arg(long, action = clap::ArgAction::Append)]
        run_command: Vec<String>,

        // Target distro: ubuntu (default), fedora, arch
        #[arg(long, value_name = "DISTRO")]
        distro: Option<String>,

        #[arg(long)]
        json: bool,
    },
    Diff {
        #[arg(long)]
        base: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let engine = ForgeIsoEngine::new();

    // Subscribe to engine events and spawn event handler
    let mut rx = engine.subscribe();
    let _event_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event.phase {
                EventPhase::Download => {
                    eprint!("\r[Download] {:<40}", event.message);
                    let _ = std::io::Write::flush(&mut std::io::stderr());
                }
                _ => {
                    eprintln!("[{:?}] {}", event.phase, event.message);
                }
            }
        }
    });

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
            let info = engine.inspect_source(&source, None).await?;
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
                if let Some(iso) = result.artifacts.first() {
                    println!("Built ISO: {}", iso.display());
                }
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
        Commands::Verify {
            source,
            sums_url,
            json,
        } => {
            let result = engine.verify(&source, sums_url.as_deref()).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Verifying: {}", result.filename);
                println!("Expected: {}", result.expected);
                println!("Actual:   {}", result.actual);
                println!("Match:    {}", result.matched);
            }
        }
        Commands::Inject {
            source,
            autoinstall,
            out,
            name,
            volume_label,
            hostname,
            username,
            password,
            password_file,
            password_stdin,
            realname,
            ssh_key,
            ssh_key_file,
            ssh_password_auth,
            dns,
            ntp_server,
            timezone,
            locale,
            keyboard_layout,
            storage_layout,
            apt_mirror,
            package,
            wallpaper,
            late_command,
            no_user_interaction,
            group,
            shell,
            sudo_nopasswd,
            sudo_command,
            firewall,
            firewall_policy,
            allow_port,
            deny_port,
            static_ip,
            gateway,
            http_proxy,
            https_proxy,
            no_proxy,
            enable_service,
            disable_service,
            sysctl,
            swap_size,
            swap_file,
            swappiness,
            apt_repo,
            docker,
            podman,
            docker_user,
            grub_timeout,
            grub_cmdline,
            grub_default,
            encrypt,
            encrypt_passphrase,
            encrypt_passphrase_file,
            mount,
            run_command,
            distro,
            json,
        } => {
            // Resolve password (priority: stdin > file > cli arg)
            let resolved_password = if password_stdin {
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf)?;
                Some(buf.trim().to_string())
            } else if let Some(ref pf) = password_file {
                Some(std::fs::read_to_string(pf)?.trim().to_string())
            } else {
                password
            };

            // Read SSH keys from files
            let mut all_ssh_keys = ssh_key;
            for kf in ssh_key_file {
                all_ssh_keys.push(std::fs::read_to_string(&kf)?.trim().to_string());
            }

            // Resolve encryption passphrase
            let resolved_encrypt_passphrase = if let Some(ref f) = encrypt_passphrase_file {
                Some(std::fs::read_to_string(f)?.trim().to_string())
            } else {
                encrypt_passphrase
            };

            // Parse distro
            let resolved_distro = match distro.as_deref() {
                None | Some("ubuntu") => None,
                Some("fedora") => Some(Distro::Fedora),
                Some("arch") => Some(Distro::Arch),
                Some("mint") => Some(Distro::Mint),
                Some(other) => {
                    eprintln!("ERROR: unknown distro '{other}'. Valid: ubuntu, fedora, arch, mint");
                    std::process::exit(1);
                }
            };

            // Parse sysctl "key=value" pairs
            let sysctl_pairs: Vec<(String, String)> = sysctl
                .iter()
                .filter_map(|s| {
                    let mut parts = s.splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect();

            let cfg = InjectConfig {
                source: IsoSource::from_raw(source),
                autoinstall_yaml: autoinstall,
                out_name: name.unwrap_or_else(|| "injected.iso".to_string()),
                output_label: volume_label,
                hostname,
                username,
                password: resolved_password,
                realname,
                ssh: SshConfig {
                    authorized_keys: all_ssh_keys,
                    allow_password_auth: if ssh_password_auth { Some(true) } else { None },
                    install_server: None,
                },
                network: NetworkConfig {
                    dns_servers: dns,
                    ntp_servers: ntp_server,
                },
                timezone,
                locale,
                keyboard_layout,
                storage_layout,
                apt_mirror,
                extra_packages: package,
                wallpaper,
                extra_late_commands: late_command,
                no_user_interaction,
                user: UserConfig {
                    groups: group,
                    shell,
                    sudo_nopasswd,
                    sudo_commands: sudo_command,
                },
                firewall: FirewallConfig {
                    enabled: firewall,
                    default_policy: firewall_policy,
                    allow_ports: allow_port,
                    deny_ports: deny_port,
                },
                proxy: ProxyConfig {
                    http_proxy,
                    https_proxy,
                    no_proxy,
                },
                static_ip,
                gateway,
                enable_services: enable_service,
                disable_services: disable_service,
                sysctl: sysctl_pairs,
                swap: swap_size.map(|mb| SwapConfig {
                    size_mb: mb,
                    filename: swap_file,
                    swappiness,
                }),
                apt_repos: apt_repo,
                containers: ContainerConfig {
                    docker,
                    podman,
                    docker_users: docker_user,
                },
                grub: GrubConfig {
                    timeout: grub_timeout,
                    cmdline_extra: grub_cmdline,
                    default_entry: grub_default,
                },
                encrypt,
                encrypt_passphrase: resolved_encrypt_passphrase,
                mounts: mount,
                run_commands: run_command,
                distro: resolved_distro,
            };
            let result = engine.inject_autoinstall(&cfg, &out).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else if let Some(iso) = result.artifacts.first() {
                println!("Injected ISO: {}", iso.display());
            }
        }
        Commands::Diff { base, target, json } => {
            let result = engine.diff_isos(&base, &target).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("ISO Diff: {} vs {}", base.display(), target.display());
                println!();
                if !result.added.is_empty() {
                    println!("Added ({}):", result.added.len());
                    for file in &result.added {
                        println!("  + {}", file);
                    }
                    println!();
                }
                if !result.removed.is_empty() {
                    println!("Removed ({}):", result.removed.len());
                    for file in &result.removed {
                        println!("  - {}", file);
                    }
                    println!();
                }
                if !result.modified.is_empty() {
                    println!("Modified ({}):", result.modified.len());
                    for entry in &result.modified {
                        println!(
                            "  ~ {} ({} → {})",
                            entry.path, entry.base_size, entry.target_size
                        );
                    }
                    println!();
                }
                println!("Unchanged: {}", result.unchanged);
            }
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
