#!/usr/bin/env bash
# Build a Debian/Ubuntu .deb package for forgeiso using fpm
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

echo "[deb] staging package contents..."
forgeiso_build_staging "${BIN_DIR}" "${ROOT_DIR}" "${STAGING}"

DEB_OUT="${RELEASE_DIR}/forgeiso_${VERSION}-1_amd64.deb"

echo "[deb] building ${DEB_OUT} ..."
fpm \
  --input-type    dir \
  --output-type   deb \
  --force \
  --name          forgeiso \
  --version       "${VERSION}" \
  --iteration     1 \
  --architecture  amd64 \
  --license       "Apache-2.0" \
  --vendor        "ForgeISO" \
  --maintainer    "Jamal Al-Sarraf <https://github.com/jalsarraf0>" \
  --url           "https://github.com/jalsarraf0/ForgeISO" \
  --description   "Linux ISO builder and autoinstall injection tool
 ForgeISO builds custom Linux ISOs locally on bare metal with no cloud agents.
 It supports cloud-init autoinstall injection (60+ config flags), SHA-256
 verification, ISO diffing, SBOM/vulnerability scanning, and QEMU smoke testing." \
  --depends       "xorriso" \
  --depends       "squashfs-tools" \
  --depends       "mtools" \
  --deb-priority  "optional" \
  --deb-no-default-config-files \
  --package       "${DEB_OUT}" \
  --chdir         "${STAGING}" \
  usr

echo "[deb] verifying package..."
dpkg-deb --info "${DEB_OUT}" 2>/dev/null | grep -E "Package|Version|Architecture|Installed-Size|Depends" || true

echo "[deb] OK: ${DEB_OUT}"
ls -lh "${DEB_OUT}"
