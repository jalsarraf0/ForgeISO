#!/usr/bin/env bash
set -euo pipefail

if ! command -v fpm >/dev/null 2>&1; then
  echo "fpm is required to build pacman package" >&2
  exit 1
fi

if ! command -v bsdtar >/dev/null 2>&1; then
  echo "bsdtar is required to build pacman package (install libarchive-tools)" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
BIN_DIR="$(forgeiso_bin_dir "${ROOT_DIR}")"
OUT_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"
PACKAGE_PATH="${OUT_DIR}/forgeiso-${VERSION}-1-x86_64.pkg.tar.zst"

forgeiso_require_binary "${BIN_DIR}" "forgeiso"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-tui"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-agent"

mkdir -p "${OUT_DIR}"
rm -f "${PACKAGE_PATH}"

fpm \
  -s dir \
  -t pacman \
  -n forgeiso \
  -v "${VERSION}" \
  --iteration 1 \
  --architecture x86_64 \
  --license Apache-2.0 \
  --maintainer "Jamal Al-Sarraf <19882582+jalsarraf0@users.noreply.github.com>" \
  --description "Cross-distro ISO customization platform" \
  --url "https://github.com/jalsarraf0/ForgeISO" \
  --depends bash \
  --package "${PACKAGE_PATH}" \
  "${BIN_DIR}/forgeiso=/usr/bin/forgeiso" \
  "${BIN_DIR}/forgeiso-tui=/usr/bin/forgeiso-tui" \
  "${BIN_DIR}/forgeiso-agent=/usr/bin/forgeiso-agent" \
  "${ROOT_DIR}/README.md=/usr/share/doc/forgeiso/README.md" \
  "${ROOT_DIR}/LICENSE=/usr/share/licenses/forgeiso/LICENSE"

echo "Created ${PACKAGE_PATH}"
