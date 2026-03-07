#!/usr/bin/env bash
# Build an Arch Linux .pkg.tar.zst package for forgeiso using fpm
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
BIN_DIR="$(forgeiso_bin_dir "${ROOT_DIR}")"
RELEASE_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"

forgeiso_require_binary "${BIN_DIR}" "forgeiso"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-tui"

mkdir -p "${RELEASE_DIR}"

STAGING="$(mktemp -d)"
trap 'rm -rf "${STAGING}"' EXIT

echo "[pacman] staging package contents..."
forgeiso_build_staging "${BIN_DIR}" "${ROOT_DIR}" "${STAGING}"

PKG_OUT="${RELEASE_DIR}/forgeiso-${VERSION}-1-x86_64.pkg.tar.zst"

echo "[pacman] building ${PKG_OUT} ..."
fpm \
  --input-type  dir \
  --output-type pacman \
  --force \
  --name        forgeiso \
  --version     "${VERSION}" \
  --iteration   1 \
  --architecture x86_64 \
  --license     "Apache-2.0" \
  --vendor      "ForgeISO" \
  --maintainer  "Jamal Al-Sarraf <https://github.com/jalsarraf0>" \
  --url         "https://github.com/jalsarraf0/ForgeISO" \
  --description "Linux ISO builder and autoinstall injection tool" \
  --depends     "xorriso" \
  --depends     "squashfs-tools" \
  --depends     "mtools" \
  --package     "${PKG_OUT}" \
  --chdir       "${STAGING}" \
  usr

echo "[pacman] OK: ${PKG_OUT}"
ls -lh "${PKG_OUT}"

# Also generate the AUR PKGBUILD from the template so it's alongside packages
PKGBUILD_SRC="${ROOT_DIR}/packaging/PKGBUILD"
if [[ -f "${PKGBUILD_SRC}" ]]; then
  cp "${PKGBUILD_SRC}" "${RELEASE_DIR}/PKGBUILD"
  echo "[pacman] copied PKGBUILD to ${RELEASE_DIR}/PKGBUILD"
fi
