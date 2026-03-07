#!/usr/bin/env bash
# Bump ForgeISO version in all required locations.
# Usage: bash scripts/release/bump-version.sh <NEW_VERSION>
# Example: bash scripts/release/bump-version.sh 1.0.0
set -euo pipefail

NEW_VERSION="${1:-}"
if [[ -z "${NEW_VERSION}" ]]; then
  echo "ERROR: version argument required" >&2
  echo "Usage: $0 <version>  (e.g. $0 1.0.0)" >&2
  exit 1
fi

# Validate semver format
if ! [[ "${NEW_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: version must be in semver format X.Y.Z (got '${NEW_VERSION}')" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "Bumping ForgeISO to v${NEW_VERSION}"

# ── 1. Cargo.toml workspace version ─────────────────────────────────────────
CARGO_TOML="${ROOT_DIR}/Cargo.toml"
OLD_VERSION="$(grep -E '^version = ' "${CARGO_TOML}" | head -1 | sed 's/version = "\(.*\)"/\1/')"

if [[ "${OLD_VERSION}" == "${NEW_VERSION}" ]]; then
  echo "  Cargo.toml already at ${NEW_VERSION}, skipping"
else
  sed -i "s/^version = \"${OLD_VERSION}\"/version = \"${NEW_VERSION}\"/" "${CARGO_TOML}"
  echo "  Cargo.toml: ${OLD_VERSION} → ${NEW_VERSION}"
fi

# ── 2. PKGBUILD ──────────────────────────────────────────────────────────────
PKGBUILD="${ROOT_DIR}/packaging/PKGBUILD"
if [[ -f "${PKGBUILD}" ]]; then
  OLD_PKGVER="$(grep '^pkgver=' "${PKGBUILD}" | cut -d= -f2)"
  if [[ "${OLD_PKGVER}" == "${NEW_VERSION}" ]]; then
    echo "  PKGBUILD already at ${NEW_VERSION}, skipping"
  else
    sed -i "s/^pkgver=.*/pkgver=${NEW_VERSION}/" "${PKGBUILD}"
    # Reset sha256sums to SKIP (will be updated after release tarball is published)
    sed -i "s/^sha256sums=.*/sha256sums=('SKIP')  # Replace with actual sha256 after release/" "${PKGBUILD}"
    echo "  PKGBUILD: ${OLD_PKGVER} → ${NEW_VERSION} (sha256sums reset to SKIP)"
  fi
fi

# ── 3. Regenerate Cargo.lock ─────────────────────────────────────────────────
echo "  Regenerating Cargo.lock..."
(cd "${ROOT_DIR}" && cargo generate-lockfile --quiet)
echo "  Cargo.lock updated"

# ── 4. Summary ──────────────────────────────────────────────────────────────
echo ""
echo "Version bump complete: v${OLD_VERSION:-unknown} → v${NEW_VERSION}"
echo ""
echo "Changed files:"
echo "  ${CARGO_TOML}"
[[ -f "${PKGBUILD}" ]] && echo "  ${PKGBUILD}"
echo "  ${ROOT_DIR}/Cargo.lock"
echo ""
echo "Next steps:"
echo "  1. cargo build --workspace --release    # verify all crates compile"
echo "  2. git add Cargo.toml Cargo.lock packaging/PKGBUILD"
echo "  3. git commit -m 'chore: bump version to v${NEW_VERSION}'"
echo "  4. Push branch + open PR"
echo "  5. After merge: git tag -a v${NEW_VERSION} -m 'Release v${NEW_VERSION}' && git push origin v${NEW_VERSION}"
