# ForgeISO

ForgeISO is a local-first Linux ISO remastering toolkit.

It now runs on bare metal only:
- no Docker or Podman runtime for product features
- no local or remote agent servers
- no endpoint configuration in the GUI, TUI, or CLI
- Linux is the only supported host for build and VM test workflows

## What works

- Inspect a local ISO or a user-provided download URL
- Detect distro/release/architecture from the ISO itself
- Download the ISO locally when a URL is provided
- Apply a local overlay directory into the ISO/rootfs when the layout is supported
- Repack and export a new ISO locally
- Run local scan, smoke test, and report steps with honest prerequisite checks

## Workspace layout

- `engine/` shared local build engine
- `cli/` automation CLI (`forgeiso`)
- `tui/` terminal UI (`forgeiso-tui`)
- `gui/` Tauri desktop UI for Linux
- `containers/` CI-only ephemeral container definitions
- `examples/` local project examples
- `docs/` local workflow and troubleshooting notes

## Quick start

```bash
cargo run -p forgeiso-cli -- doctor
cargo run -p forgeiso-cli -- inspect --source /path/to/base.iso
cargo run -p forgeiso-cli -- build --source /path/to/base.iso --out ./artifacts --name my-build
```

Use a URL when you want ForgeISO to fetch the ISO onto the local machine first:

```bash
cargo run -p forgeiso-cli -- inspect --source https://example.invalid/distro.iso
cargo run -p forgeiso-cli -- build --source https://example.invalid/distro.iso --out ./artifacts --name downloaded-build
```

## GUI and TUI

```bash
cargo run -p forgeiso-tui
cd gui && npm run build && cargo run --manifest-path src-tauri/Cargo.toml
```

The built GUI is a local desktop app. It does not connect to any endpoint.

## CI/CD

CI containers remain allowed only for pipeline work. They are defined under `containers/` and orchestrated by `docker-compose.ci.yml`. They must remain ephemeral and be torn down at the end of the pipeline.

## License

Apache-2.0
