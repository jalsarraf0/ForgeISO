#!/usr/bin/env bash
# ForgeISO release package builder — RPM + DEB + pacman + tarball + checksums
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
BIN_DIR="$(forgeiso_bin_dir "${ROOT_DIR}")"
RELEASE_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"

echo "╔══════════════════════════════════════╗"
echo "║  ForgeISO Release Builder v${VERSION}  "
echo "╚══════════════════════════════════════╝"
echo ""

# ── 1. Validate binaries ────────────────────────────────────────────────────
echo "▶ [1/6] Validating release binaries..."
forgeiso_require_binary "${BIN_DIR}" "forgeiso"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-tui"
echo "  forgeiso:     $(ls -lh "${BIN_DIR}/forgeiso" | awk '{print $5}')"
echo "  forgeiso-tui: $(ls -lh "${BIN_DIR}/forgeiso-tui" | awk '{print $5}')"
if [[ -f "${BIN_DIR}/forge-gui" ]]; then
  echo "  forge-gui:    $(ls -lh "${BIN_DIR}/forge-gui" | awk '{print $5}')"
else
  echo "  forge-gui:    (not built — run: cargo build -p forge-gui --release)"
fi

INSTALLED_VER="$("${BIN_DIR}/forgeiso" --version 2>/dev/null | awk '{print $2}')"
if [[ "${INSTALLED_VER}" != "${VERSION}" ]]; then
  echo "  WARNING: binary version ${INSTALLED_VER} does not match release version ${VERSION}" >&2
fi

# ── 2. Create release dir ───────────────────────────────────────────────────
echo ""
echo "▶ [2/6] Preparing release directory: ${RELEASE_DIR}"
mkdir -p "${RELEASE_DIR}"

# ── 3. Build packages ────────────────────────────────────────────────────────
echo ""
echo "▶ [3/6] Building packages..."

echo "  → RPM..."
"${SCRIPT_DIR}/package-rpm.sh" "${VERSION}" 2>&1 | sed 's/^/    /'

echo "  → DEB..."
"${SCRIPT_DIR}/package-deb.sh" "${VERSION}" 2>&1 | sed 's/^/    /'

echo "  → pacman (.pkg.tar.zst)..."
"${SCRIPT_DIR}/package-pacman.sh" "${VERSION}" 2>&1 | sed 's/^/    /'

# ── 4. Tarball ───────────────────────────────────────────────────────────────
echo ""
echo "▶ [4/6] Building tarball..."
"${SCRIPT_DIR}/package-tarball.sh" "${VERSION}" 2>&1 | sed 's/^/    /'
# Move tarball into release dir if it landed in dist/
TARBALL_SRC="${ROOT_DIR}/dist/forgeiso-${VERSION}-linux-x86_64.tar.gz"
if [[ -f "${TARBALL_SRC}" && "${TARBALL_SRC}" != "${RELEASE_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.gz" ]]; then
  mv "${TARBALL_SRC}" "${RELEASE_DIR}/"
fi

# ── 5. SHA-256 checksums ─────────────────────────────────────────────────────
echo ""
echo "▶ [5/6] Generating checksums..."
CHECKSUM_FILE="${RELEASE_DIR}/checksums.txt"
(
  cd "${RELEASE_DIR}"
  sha256sum \
    "forgeiso-${VERSION}-1.x86_64.rpm" \
    "forgeiso_${VERSION}-1_amd64.deb" \
    "forgeiso-${VERSION}-1-x86_64.pkg.tar.zst" \
    "forgeiso-${VERSION}-linux-x86_64.tar.gz" \
    2>/dev/null > "${CHECKSUM_FILE}" || true
)
echo "  SHA-256 checksums:"
while IFS= read -r line; do
  echo "    ${line}"
done < "${CHECKSUM_FILE}"

# ── 6. Summary ──────────────────────────────────────────────────────────────
echo ""
echo "▶ [6/6] Release artifacts:"
find "${RELEASE_DIR}" -maxdepth 1 -type f | sort | while read -r f; do
  printf "  %-55s  %s\n" "$(basename "${f}")" "$(ls -lh "${f}" | awk '{print $5}')"
done

echo ""
echo "✓ ForgeISO ${VERSION} packages ready in ${RELEASE_DIR}"
