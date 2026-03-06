use crate::backends::{ensure_base_iso_url, module_snippets, spec, BackendPlan};
use crate::config::BuildConfig;
use crate::error::EngineResult;
use crate::modules::ModuleAction;
use crate::workspace::Workspace;

pub fn plan(
    cfg: &BuildConfig,
    workspace: &Workspace,
    actions: &[ModuleAction],
) -> EngineResult<BackendPlan> {
    let base_iso_url = ensure_base_iso_url(cfg)?;
    let artifact = workspace
        .output
        .join(format!("{}-mint-{}.iso", cfg.name, cfg.release.version));
    let module_script = module_snippets(actions);

    let command = format!(
        "set -euo pipefail; \
        mkdir -p /workspace/input /workspace/work/mint/extract /workspace/work/mint/rootfs /workspace/output; \
        curl -L --fail '{base_iso_url}' -o /workspace/input/base.iso; \
        xorriso -osirrox on -indev /workspace/input/base.iso -extract / /workspace/work/mint/extract; \
        unsquashfs -f -d /workspace/work/mint/rootfs /workspace/work/mint/extract/casper/filesystem.squashfs; \
        chroot /workspace/work/mint/rootfs apt-get update; \
        chroot /workspace/work/mint/rootfs apt-get -y dist-upgrade; \
        {module_script}; \
        mksquashfs /workspace/work/mint/rootfs /workspace/work/mint/extract/casper/filesystem.squashfs -comp xz -noappend; \
        xorriso -as mkisofs -r -V 'ForgeISO Mint' -o '{artifact}' -J -l /workspace/work/mint/extract; \
        test -f '{artifact}'",
        artifact = artifact.display()
    );

    Ok(BackendPlan {
        backend_name: "mint-lts-remaster".to_string(),
        base_iso_url: Some(base_iso_url),
        base_iso_checksum: cfg.release.base_iso_checksum.clone(),
        commands: vec![spec(
            "ghcr.io/jalsarraf0/forgeiso-ubuntu-toolchain:latest",
            command,
            workspace,
        )],
        warnings: vec![],
        produced_iso: artifact,
    })
}
