use std::fs;

use crate::backends::{module_snippets, spec, BackendPlan};
use crate::config::BuildConfig;
use crate::error::EngineResult;
use crate::modules::ModuleAction;
use crate::workspace::Workspace;

pub fn plan(
    cfg: &BuildConfig,
    workspace: &Workspace,
    actions: &[ModuleAction],
) -> EngineResult<BackendPlan> {
    let artifact = workspace
        .output
        .join(format!("{}-arch-{}.iso", cfg.name, cfg.release.version));

    let profile_dir = workspace.work.join("archiso-profile");
    fs::create_dir_all(profile_dir.join("airootfs/etc/systemd/system"))?;

    let packages = render_packages(cfg);
    fs::write(profile_dir.join("packages.x86_64"), packages)?;

    let module_script = module_snippets(actions);
    fs::write(
        profile_dir.join("airootfs/root/forgeiso-post.sh"),
        format!("#!/usr/bin/env bash\nset -euo pipefail\n{}\n", module_script),
    )?;

    let command = format!(
        "set -euo pipefail; \
        chmod +x /workspace/work/archiso-profile/airootfs/root/forgeiso-post.sh; \
        mkarchiso -v -w /workspace/work/arch-work -o /workspace/output /workspace/work/archiso-profile; \
        cp /workspace/output/*.iso '{artifact}'",
        artifact = artifact.display()
    );

    Ok(BackendPlan {
        backend_name: "arch-archiso".to_string(),
        base_iso_url: cfg.release.base_iso_url.clone(),
        base_iso_checksum: cfg.release.base_iso_checksum.clone(),
        commands: vec![spec(
            "ghcr.io/jalsarraf0/forgeiso-arch-toolchain:latest",
            command,
            workspace,
        )],
        warnings: vec!["Arch is rolling release; snapshot reproducibility is best effort".to_string()],
        produced_iso: artifact,
    })
}

fn render_packages(cfg: &BuildConfig) -> String {
    let mut lines = vec![
        "base".to_string(),
        "linux".to_string(),
        "linux-firmware".to_string(),
        "networkmanager".to_string(),
        "openssh".to_string(),
    ];

    if matches!(cfg.profile, crate::config::ProfileKind::Gaming) {
        lines.push("steam".to_string());
        lines.push("lutris".to_string());
    }

    lines.join("\n")
}
