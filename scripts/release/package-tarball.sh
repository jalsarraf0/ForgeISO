#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
BIN_DIR="$(forgeiso_bin_dir "${ROOT_DIR}")"
OUT_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"
STAGE_DIR="${OUT_DIR}/forgeiso-${VERSION}-linux-x86_64"
TARBALL_PATH="${OUT_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.gz"

forgeiso_require_binary "${BIN_DIR}" "forgeiso"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-tui"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-agent"

mkdir -p "${OUT_DIR}"
rm -rf "${STAGE_DIR}"
rm -f "${TARBALL_PATH}"
mkdir -p "${STAGE_DIR}/bin" "${STAGE_DIR}/docs"

cp "${BIN_DIR}/forgeiso" "${STAGE_DIR}/bin/"
cp "${BIN_DIR}/forgeiso-tui" "${STAGE_DIR}/bin/"
cp "${BIN_DIR}/forgeiso-agent" "${STAGE_DIR}/bin/"
cp "${ROOT_DIR}/README.md" "${ROOT_DIR}/LICENSE" "${STAGE_DIR}/"
cp -r "${ROOT_DIR}/docs"/* "${STAGE_DIR}/docs/"

tar -C "${OUT_DIR}" -czf "${TARBALL_PATH}" "forgeiso-${VERSION}-linux-x86_64"
rm -rf "${STAGE_DIR}"

echo "Created ${TARBALL_PATH}"
