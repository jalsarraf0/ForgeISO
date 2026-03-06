use std::fs;

use crate::backends::{spec, BackendPlan};
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
        .join(format!("{}-fedora-{}.iso", cfg.name, cfg.release.version));

    let kickstart_path = workspace.work.join("fedora.ks");
    let kickstart = render_kickstart(cfg, actions);
    fs::write(&kickstart_path, kickstart)?;

    let command = format!(
        "set -euo pipefail; \
        mkdir -p /workspace/output /workspace/work/fedora; \
        livemedia-creator --make-iso --project 'ForgeISO' --releasever '{release}' --ks '/workspace/work/fedora.ks' --resultdir /workspace/work/fedora --no-virt; \
        cp /workspace/work/fedora/*.iso '{artifact}'",
        artifact = artifact.display()
    ,
        release = cfg.release.version
    );

    Ok(BackendPlan {
        backend_name: "fedora-livemedia".to_string(),
        base_iso_url: cfg.release.base_iso_url.clone(),
        base_iso_checksum: cfg.release.base_iso_checksum.clone(),
        commands: vec![spec(
            "ghcr.io/jalsarraf0/forgeiso-fedora-toolchain:latest",
            command,
            workspace,
        )],
        warnings: vec![
            "Fedora has a short lifecycle and is not LTS; track EOL windows in production"
                .to_string(),
        ],
        produced_iso: artifact,
    })
}

fn render_kickstart(cfg: &BuildConfig, actions: &[ModuleAction]) -> String {
    let mut content = String::new();
    content.push_str("lang en_US.UTF-8\nkeyboard us\ntimezone UTC --utc\n");
    content.push_str("rootpw --lock\nselinux --enforcing\nfirewall --enabled\n");
    content.push_str("bootloader --timeout=1\nreboot\n\n%packages\n@core\n");

    if matches!(cfg.profile, crate::config::ProfileKind::Gaming) {
        content.push_str("steam\nlutris\n");
    }

    content.push_str("%end\n\n%post --erroronfail\n");
    for action in actions {
        for line in &action.shell_snippets {
            content.push_str(line);
            content.push('\n');
        }
    }
    content.push_str("%end\n");
    content
}
