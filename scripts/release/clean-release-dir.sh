#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
RELEASE_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"

rm -rf "${RELEASE_DIR}"
mkdir -p "${RELEASE_DIR}"

echo "Prepared clean release directory: ${RELEASE_DIR}"
