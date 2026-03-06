#!/usr/bin/env bash
set -euo pipefail

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "dpkg-deb is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
BIN_DIR="$(forgeiso_bin_dir "${ROOT_DIR}")"
OUT_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"
PKG_DIR="${ROOT_DIR}/dist/deb/forgeiso_${VERSION}_amd64"
DEB_PATH="${OUT_DIR}/forgeiso_${VERSION}_amd64.deb"

forgeiso_require_binary "${BIN_DIR}" "forgeiso"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-tui"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-agent"

rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}/DEBIAN" "${PKG_DIR}/usr/bin" "${PKG_DIR}/usr/share/doc/forgeiso"
mkdir -p "${OUT_DIR}"
rm -f "${DEB_PATH}"

cp "${BIN_DIR}/forgeiso" "${PKG_DIR}/usr/bin/"
cp "${BIN_DIR}/forgeiso-tui" "${PKG_DIR}/usr/bin/"
cp "${BIN_DIR}/forgeiso-agent" "${PKG_DIR}/usr/bin/"
cp "${ROOT_DIR}/README.md" "${ROOT_DIR}/LICENSE" "${PKG_DIR}/usr/share/doc/forgeiso/"

cat > "${PKG_DIR}/DEBIAN/control" <<CONTROL
Package: forgeiso
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Jamal Al-Sarraf <19882582+jalsarraf0@users.noreply.github.com>
Depends: bash, docker.io | podman
Description: Cross-distro ISO customization platform
 ForgeISO provides enterprise ISO customization with CLI, TUI, GUI, and optional remote agent support.
CONTROL

dpkg-deb --build "${PKG_DIR}" "${DEB_PATH}"
echo "Created ${DEB_PATH}"
