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
        .join(format!("{}-ubuntu-{}.iso", cfg.name, cfg.release.version));

    let checksum_step = cfg
        .release
        .base_iso_checksum
        .as_ref()
        .map(|sum| {
            format!(
                "echo '{sum}  /workspace/input/base.iso' > /workspace/input/base.iso.sha256; sha256sum -c /workspace/input/base.iso.sha256"
            )
        })
        .unwrap_or_else(|| "echo 'checksum not provided; using pinned URL only'".to_string());

    let module_script = module_snippets(actions);

    let command = format!(
        "set -euo pipefail; \
        mkdir -p /workspace/input /workspace/work/ubuntu/extract /workspace/work/ubuntu/rootfs /workspace/output; \
        curl -L --fail '{base_iso_url}' -o /workspace/input/base.iso; \
        {checksum_step}; \
        xorriso -osirrox on -indev /workspace/input/base.iso -extract / /workspace/work/ubuntu/extract; \
        unsquashfs -f -d /workspace/work/ubuntu/rootfs /workspace/work/ubuntu/extract/casper/filesystem.squashfs; \
        chroot /workspace/work/ubuntu/rootfs apt-get update; \
        chroot /workspace/work/ubuntu/rootfs apt-get -y dist-upgrade; \
        {module_script}; \
        mksquashfs /workspace/work/ubuntu/rootfs /workspace/work/ubuntu/extract/casper/filesystem.squashfs -comp xz -noappend; \
        xorriso -as mkisofs -r -V 'ForgeISO Ubuntu' -o '{artifact}' -J -l /workspace/work/ubuntu/extract; \
        test -f '{artifact}'",
        artifact = artifact.display()
    );

    Ok(BackendPlan {
        backend_name: "ubuntu-lts-remaster".to_string(),
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
