#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
RELEASE_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"

require_file() {
  local path="$1"
  if [[ ! -f "${path}" ]]; then
    echo "missing required release artifact: ${path}" >&2
    exit 1
  fi
}

require_glob() {
  local pattern="$1"
  shopt -s nullglob
  local matches=(${pattern})
  shopt -u nullglob
  if (( ${#matches[@]} == 0 )); then
    echo "missing required release artifact pattern: ${pattern}" >&2
    exit 1
  fi
}

if [[ ! -d "${RELEASE_DIR}" ]]; then
  echo "release directory not found: ${RELEASE_DIR}" >&2
  exit 1
fi

require_file "${RELEASE_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.gz"
require_file "${RELEASE_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.zst"
require_file "${RELEASE_DIR}/forgeiso_${VERSION}_amd64.deb"
require_glob "${RELEASE_DIR}/forgeiso-${VERSION}-1*.rpm"
require_file "${RELEASE_DIR}/forgeiso-${VERSION}-1-x86_64.pkg.tar.zst"
require_file "${RELEASE_DIR}/forgeiso-repos-${VERSION}.tar.gz"
require_file "${RELEASE_DIR}/forgeiso-binaries-${VERSION}-linux-x86_64.tar.gz"
require_file "${RELEASE_DIR}/forgeiso-binaries-${VERSION}-windows-x86_64.zip"
require_file "${RELEASE_DIR}/forgeiso-binaries-${VERSION}-macos-x86_64.tar.gz"
require_file "${RELEASE_DIR}/forgeiso-binaries-${VERSION}-macos-arm64.tar.gz"
require_file "${RELEASE_DIR}/checksums.txt"
require_file "${RELEASE_DIR}/release-manifest.json"

if find "${RELEASE_DIR}" -mindepth 1 -maxdepth 1 -type d | grep -q .; then
  echo "release directory contains unexpected subdirectories" >&2
  find "${RELEASE_DIR}" -mindepth 1 -maxdepth 1 -type d >&2
  exit 1
fi

if [[ ! -s "${RELEASE_DIR}/checksums.txt" ]]; then
  echo "checksums.txt is empty" >&2
  exit 1
fi

echo "Release verification passed for version ${VERSION}"
