# ForgeISO Release Runbook

This runbook documents the complete process for cutting a versioned ForgeISO release,
from version bump through published GitHub Release with signed packages.

---

## Release Checklist

```
[ ] 1. Bump version in all locations
[ ] 2. Update PKGBUILD sha256sum placeholder
[ ] 3. Commit and push feature branch
[ ] 4. Open PR — wait for all 6 CI stages to pass
[ ] 5. Merge PR via squash
[ ] 6. Create and push annotated git tag
[ ] 7. GitHub Actions release job fires — wait for completion
[ ] 8. Verify all release assets and checksums
[ ] 9. Smoke-install from the published RPM
```

---

## 1. Bump Version

All version strings live in a single file:

```
Cargo.toml          [workspace.package] version = "X.Y.Z"
packaging/PKGBUILD  pkgver=X.Y.Z
```

Use the bump-version script (handles Cargo.toml and PKGBUILD atomically):

```bash
bash scripts/release/bump-version.sh 0.3.2
```

The script will:
1. Update `Cargo.toml` workspace version
2. Update `packaging/PKGBUILD` pkgver
3. Run `cargo build --release` to regenerate `Cargo.lock`
4. Print a summary of changed files

Verify binary version after build:

```bash
./target/release/forgeiso --version
# forgeiso 0.3.2
```

---

## 2. CI Stages

| Stage | Label | What fails | Artifact |
|---|---|---|---|
| C1 | Rust | fmt / clippy / tests | — |
| C2 | SBOM + Audit | license violations, HIGH/CRITICAL CVEs | `sbom.cdx.json`, `sbom.spdx.json`, `audit.json` |
| C3 | GUI | GUI build failures | — |
| C4 | Security | SBOM generation (best-effort) | trivy, grype, gitleaks reports |
| C5 | Integration | Integration test failures | — |
| C6 | E2E Smoke | Boot smoke test failures | — |

C2 is the enforcement gate. The advisory database is fetched live in CI.
Run locally with:

```bash
# Full C2 stage in Docker
docker build -t forgeiso-c2 -f containers/C2.sbom.Dockerfile . \
  && docker run --rm -e CI=true -v "$PWD:/workspace" forgeiso-c2 \
     bash -c "scripts/ci/c2-sbom.sh"

# Just cargo-deny
cargo deny check

# Just cargo-audit
cargo audit
```

---

## 3. Local Packaging

Build all packages locally before tagging:

```bash
# Build release binaries first
cargo build --release -p forgeiso-cli -p forgeiso-tui

# Build RPM + DEB + pacman + tarball + checksums
bash scripts/release/make-packages.sh 0.3.2

# Verify
cd dist/release
sha256sum -c checksums.txt
ls -lh
```

Outputs in `dist/release/`:

| File | Format | Distro |
|---|---|---|
| `forgeiso-0.3.2-1.x86_64.rpm` | RPM | Fedora / RHEL / openSUSE |
| `forgeiso_0.3.2-1_amd64.deb` | DEB | Debian / Ubuntu / Mint |
| `forgeiso-0.3.2-1-x86_64.pkg.tar.zst` | pacman | Arch Linux |
| `forgeiso-0.3.2-linux-x86_64.tar.gz` | tarball | Any x86-64 Linux |
| `checksums.txt` | SHA-256 | — |

---

## 4. Tagging and Publishing

After the PR is merged and local packages verify cleanly:

```bash
# Sync local main
git fetch origin main && git reset --hard origin/main

# Create annotated tag
git tag -a v0.3.2 -m "Release v0.3.2"

# Push tag — this triggers the GitHub Actions release job
git push origin v0.3.2
```

The release job will:
1. Install fpm + syft
2. Build CLI and TUI binaries
3. Run `make-packages.sh` (RPM + DEB + pacman + tarball)
4. Generate `sbom.cdx.json` and `sbom.spdx.json`
5. Verify checksums
6. Publish all artifacts to GitHub Releases

Monitor progress:

```bash
gh run watch --exit-status
```

---

## 5. Post-Release Verification

### Verify checksums

```bash
VERSION=0.3.2
gh release download v${VERSION} -D /tmp/forgeiso-verify-${VERSION}
cd /tmp/forgeiso-verify-${VERSION}
sha256sum -c checksums.txt
```

### Smoke-install RPM

```bash
sudo rpm -e forgeiso 2>/dev/null || true
sudo rpm -ivh forgeiso-${VERSION}-1.x86_64.rpm
forgeiso --version
forgeiso doctor
```

### Smoke-install DEB

```bash
sudo dpkg -r forgeiso 2>/dev/null || true
sudo dpkg -i forgeiso_${VERSION}-1_amd64.deb
forgeiso --version
forgeiso doctor
```

---

## 6. Updating PKGBUILD sha256sums

After the tarball is published, update the Arch Linux PKGBUILD with the real checksum:

```bash
VERSION=0.3.2
TARBALL="forgeiso-${VERSION}-linux-x86_64.tar.gz"

# Get checksum from the published checksums.txt
SHA=$(gh release download v${VERSION} -p checksums.txt --clobber -D /tmp \
      && grep "${TARBALL}" /tmp/checksums.txt | awk '{print $1}')

# Update PKGBUILD
sed -i "s/sha256sums=.*/sha256sums=('${SHA}')/" packaging/PKGBUILD
echo "Updated PKGBUILD sha256sums to: ${SHA}"

# Regenerate .SRCINFO (requires makepkg on Arch)
# makepkg --printsrcinfo > packaging/.SRCINFO
```

---

## 7. Dependency Policy (deny.toml)

`deny.toml` at the repo root controls what C2 enforces:

- **Advisories**: HIGH and CRITICAL CVEs → `deny` (fail build)
- **Unmaintained/unsound crates** → `warn` (report, don't fail)
- **Licenses**: Only Apache-2.0, MIT, BSD-*, ISC, and similar permissive licenses allowed
- **GPL/LGPL/AGPL** → `deny` (fail build)
- **Duplicate crates** → `warn` with explicit ecosystem-split exceptions

To update the advisory database locally:

```bash
cargo deny fetch
```

To check against policy:

```bash
cargo deny check advisories    # just CVEs
cargo deny check licenses      # just license compliance
cargo deny check bans          # just duplicate/wildcard bans
cargo deny check               # all checks
```

---

## 8. Version Locations Reference

| File | Field | Notes |
|---|---|---|
| `Cargo.toml` | `[workspace.package] version` | Single source of truth for Rust |
| `packaging/PKGBUILD` | `pkgver` | AUR/Arch package |
| `Cargo.lock` | auto-generated | Updated by `cargo build` |

GUI versions (updated separately when GUI ships):
- `gui/package.json` → `version`
- `gui/src-tauri/Cargo.toml` → `version`
- `gui/src-tauri/tauri.conf.json` → `version`
