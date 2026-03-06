# ForgeISO Architecture

Author: Jamal Al-Sarraf

## System overview

ForgeISO is a multi-interface product that routes all build logic through a single Rust engine:
- CLI (`forgeiso`) for automation
- TUI (`forgeiso-tui`) for terminal workflows
- GUI (Tauri + React) for desktop-first operation
- Optional remote agent (`forgeiso-agent`) for Linux-hosted build and VM execution

## Engine responsibilities

- Parse and validate build configuration
- Enforce distro release policy
- Select container runtime (Docker preferred, Podman fallback)
- Generate distro-specific backend execution plans
- Execute containerized build/scan/test stages
- Emit structured events for live log streaming
- Produce JSON/HTML reports with provenance and security summaries

## Distro backend strategy

- Ubuntu LTS and Mint LTS remaster pipeline:
  - ISO extraction
  - rootfs unpack/edit/repack
  - BIOS/UEFI compatible ISO repack path
- Fedora stable pipeline:
  - kickstart generation
  - `livemedia-creator` orchestration
  - lifecycle warning insertion
- Arch rolling pipeline:
  - archiso profile generation
  - package list + rootfs injection

## Security and policy model

- Unsafe operations are gated by explicit dangerous mode
- Host command execution is disabled by default
- Secrets strict mode can fail build
- Severity gates enforce vulnerability thresholds
- Reports include artifact traceability metadata
