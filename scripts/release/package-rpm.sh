#!/usr/bin/env bash
# Build an RPM package for forgeiso using fpm + rpmbuild spec
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

echo "[rpm] staging package contents..."
forgeiso_build_staging "${BIN_DIR}" "${ROOT_DIR}" "${STAGING}"

RPM_OUT="${RELEASE_DIR}/forgeiso-${VERSION}-1.x86_64.rpm"

echo "[rpm] building ${RPM_OUT} ..."
fpm \
  --input-type  dir \
  --output-type rpm \
  --force \
  --name        forgeiso \
  --version     "${VERSION}" \
  --iteration   1 \
  --architecture x86_64 \
  --license     "Apache-2.0" \
  --vendor      "ForgeISO" \
  --maintainer  "Jamal Al-Sarraf <https://github.com/jalsarraf0>" \
  --url         "https://github.com/jalsarraf0/ForgeISO" \
  --description "Linux ISO builder and autoinstall injection tool.
ForgeISO builds custom Linux ISOs locally on bare metal with no cloud agents.
It supports cloud-init autoinstall injection (60+ config flags), SHA-256
verification, ISO diffing, SBOM/vulnerability scanning, and QEMU smoke testing." \
  --category    "System/Base" \
  --depends     "xorriso" \
  --depends     "squashfs-tools" \
  --depends     "mtools" \
  --rpm-summary "ForgeISO - Linux ISO builder and autoinstall injection tool" \
  --package     "${RPM_OUT}" \
  --chdir       "${STAGING}" \
  usr

echo "[rpm] verifying package..."
rpm --queryformat '%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\n  Summary:  %{SUMMARY}\n  License:  %{LICENSE}\n  Size:     %{SIZE}\n' \
  -qp "${RPM_OUT}" 2>/dev/null || true

echo "[rpm] OK: ${RPM_OUT}"
ls -lh "${RPM_OUT}"
