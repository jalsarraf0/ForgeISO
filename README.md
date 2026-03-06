# ForgeISO

Author: Jamal Al-Sarraf

ForgeISO is a production-grade, cross-distro ISO customization platform with:
- Premium desktop GUI using Tauri v2 + React
- First-class automation CLI in Rust
- Terminal wizard TUI in Rust
- Optional remote Linux build/test agent in Go over gRPC with TLS/mTLS

## Core capabilities

- Distro policy enforcement:
  - Ubuntu: LTS only
  - Linux Mint: LTS only
  - Fedora: latest stable with non-LTS lifecycle warning
  - Arch: rolling snapshot
- Container-first execution (Docker preferred, Podman supported)
- Rich customization:
  - Multi-user provisioning and SSH key management
  - SSH hardening policy and overrides
  - Desktop theming/wallpapers/bundle presets
  - Safe module model for advanced customization
- Security pipeline:
  - SBOM (SPDX + CycloneDX)
  - Vulnerability scanning (Trivy; optional Syft/Grype)
  - Compliance scanning (OpenSCAP path)
  - Secrets scanning with strict mode gating
- Regression strategy:
  - Unit, integration, and smoke E2E
  - VM test harness (BIOS + UEFI)
  - Optional openQA path

## Monorepo layout

- `engine/` Rust engine library (policy, orchestration, scanning, reports)
- `cli/` Rust CLI binary (`forgeiso`)
- `tui/` Rust TUI binary (`forgeiso-tui`)
- `gui/` Tauri v2 desktop app
- `agent/` Go remote build/test daemon (`forgeiso-agent`)
- `proto/` gRPC contract
- `containers/` CI container definitions (C1..C6)
- `scripts/` local/CI helper scripts
- `examples/` reference config and profile examples
- `docs/` architecture, runbooks, security and policy docs
- `hooks/` optional Python hooks runner (disabled by default)

## Quick start

```bash
make test
make build
cargo run -p forgeiso-cli -- doctor
```

GUI development:

```bash
cd gui
npm ci
npm run dev
```

TUI:

```bash
cargo run -p forgeiso-tui
```

## CLI reference

```bash
forgeiso doctor [--json]
forgeiso list-releases --distro <ubuntu|mint|fedora|arch> [--json]
forgeiso build --config <file> --out <dir> [--latest|--pinned] [--keep-workdir]
forgeiso scan --artifact <path> [--policy <file>] [--json]
forgeiso test --iso <path> [--uefi] [--bios] [--json]
forgeiso report --build <dir> --format html|json
forgeiso inspect --iso <path>
```

## CI/CD

`/.github/workflows/ci.yml` runs six ephemeral container stages in parallel:
- C1 Rust quality gates
- C2 Go quality gates
- C3 GUI lint/build
- C4 security scans
- C5 integration tests
- C6 E2E smoke

Local parity:

```bash
make ci-local
```

## Enterprise operations

- Governance and repo ownership controls: `.github/CODEOWNERS`, `scripts/github-lockdown.sh`
- Security baseline and hardening: `docs/security.md`
- Release and lifecycle policy: `docs/lts-policy.md`
- Build/test/release runbooks:
  - `docs/runbook-local.md`
  - `docs/runbook-remote-agent.md`
  - `docs/runbook-release.md`

## License

Apache-2.0
