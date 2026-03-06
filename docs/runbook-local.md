# Runbook: Local Containerized Build

Author: Jamal Al-Sarraf

## Prerequisites

- Docker (preferred) or Podman
- Rust stable toolchain
- Go 1.25+
- Node 22+

## Workflow

1. Validate environment:

```bash
cargo run -p forgeiso-cli -- doctor --json
```

2. Build ISO from config:

```bash
cargo run -p forgeiso-cli -- build --config examples/ubuntu-lts.yaml --out ./artifacts --pinned
```

3. Run scans:

```bash
cargo run -p forgeiso-cli -- scan --artifact ./artifacts/build.iso --json
```

4. Run smoke tests:

```bash
cargo run -p forgeiso-cli -- test --iso ./artifacts/build.iso --bios --uefi --json
```

5. Render report:

```bash
cargo run -p forgeiso-cli -- report --build ./artifacts --format html
```
