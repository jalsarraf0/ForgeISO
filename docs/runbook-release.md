# Runbook: Release

Author: Jamal Al-Sarraf

## CI quality gate

All six CI containers must pass on `main`:
- C1 Rust
- C2 Go
- C3 GUI
- C4 Security
- C5 Integration
- C6 E2E Smoke

## Tag release

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds and publishes:
- `forgeiso` CLI binary
- `forgeiso-tui` binary
- `forgeiso-agent` binary
- GUI build artifact (platform dependent)
- SHA256 checksums

## Post-release

- Archive SBOM + scan reports
- Archive smoke test logs/screenshots
- Publish lifecycle notes for Fedora warning in release notes
