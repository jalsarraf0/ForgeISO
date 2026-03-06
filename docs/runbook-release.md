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
- `forgeiso-binaries-<version>-linux-x86_64.tar.gz`
- `forgeiso-binaries-<version>-windows-x86_64.zip`
- `forgeiso-binaries-<version>-macos-x86_64.tar.gz`
- `forgeiso-binaries-<version>-macos-arm64.tar.gz`
- `forgeiso-<version>-linux-x86_64.tar.gz`
- `forgeiso-<version>-linux-x86_64.tar.zst`
- `forgeiso_<version>_amd64.deb` (APT)
- `forgeiso-<version>-1.x86_64.rpm` (DNF/YUM)
- `forgeiso-<version>-1-x86_64.pkg.tar.zst` (Pacman)
- `forgeiso-repos-<version>.tar.gz` (apt/dnf/pacman repository metadata)
- SHA256 checksums
- `release-manifest.json` (artifact inventory with hashes/sizes)

Release workflow implementation:
- CI gates: `.github/workflows/ci.yml`
- Tag release matrix: `.github/workflows/release.yml`
- Gate checks require the tagged commit to be on `main` and to have a successful CI run.
- Linux packaging is matrix-parallel (`tar.gz`, `tar.zst`, `deb`, `rpm`, `pacman`) from one Linux binary build.
- Platform binaries are built in parallel for Linux/Windows/macOS and bundled into release artifacts.
- Final assembly performs clean-release verification before publish.

## Install commands from release artifacts

```bash
# APT (Debian/Ubuntu/Mint)
sudo apt install ./forgeiso_<version>_amd64.deb

# DNF (Fedora/RHEL)
sudo dnf install ./forgeiso-<version>-1*.rpm

# Pacman (Arch)
sudo pacman -U ./forgeiso-<version>-1-x86_64.pkg.tar.zst
```

## Build package repositories locally

After running package build scripts, generate feed metadata:

```bash
scripts/release/build-repos.sh <version>
```

Output directories:
- `dist/repos/apt`
- `dist/repos/rpm`
- `dist/repos/pacman`
- `dist/release/forgeiso-repos-<version>.tar.gz`

To keep local packaging clean, start with:

```bash
scripts/release/clean-release-dir.sh
```

Optional signing:
- Set `FORGEISO_GPG_KEY_ID=<key-id>` before running scripts to emit detached signatures and apt signed indexes.

## Consume generated repositories

APT:

```bash
echo "deb [trusted=yes] https://<host>/forgeiso/apt stable main" | sudo tee /etc/apt/sources.list.d/forgeiso.list
sudo apt update
sudo apt install forgeiso
```

DNF:

```bash
cat <<'EOF' | sudo tee /etc/yum.repos.d/forgeiso.repo
[forgeiso]
name=ForgeISO
baseurl=https://<host>/forgeiso/rpm
enabled=1
gpgcheck=0
EOF
sudo dnf install forgeiso
```

Pacman:

```bash
cat <<'EOF' | sudo tee -a /etc/pacman.conf
[forgeiso]
Server = https://<host>/forgeiso/pacman
SigLevel = Optional TrustAll
EOF
sudo pacman -Sy forgeiso
```

## Post-release

- Archive SBOM + scan reports
- Archive smoke test logs/screenshots
- Publish lifecycle notes for Fedora warning in release notes
