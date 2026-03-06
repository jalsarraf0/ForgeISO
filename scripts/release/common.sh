#!/usr/bin/env bash
set -euo pipefail

forgeiso_root_dir() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

forgeiso_release_version() {
  local root_dir="$1"
  local explicit_version="${2:-}"

  if [[ -n "${explicit_version}" ]]; then
    echo "${explicit_version}"
    return
  fi

  git -C "${root_dir}" describe --tags --abbrev=0 2>/dev/null | sed 's/^v//' || echo "0.1.0"
}

forgeiso_bin_dir() {
  local root_dir="$1"
  echo "${FORGEISO_BIN_DIR:-${root_dir}/target/release}"
}

forgeiso_release_dir() {
  local root_dir="$1"
  echo "${FORGEISO_RELEASE_DIR:-${root_dir}/dist/release}"
}

forgeiso_require_binary() {
  local bin_dir="$1"
  local binary_name="$2"
  local binary_path="${bin_dir}/${binary_name}"

  if [[ ! -f "${binary_path}" ]]; then
    echo "required binary not found: ${binary_path}" >&2
    exit 1
  fi

  chmod +x "${binary_path}" || true
}
