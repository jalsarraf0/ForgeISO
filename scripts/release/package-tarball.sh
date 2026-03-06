#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
VERSION=$(grep '^version =' Cargo.toml 2>/dev/null | head -n1 | cut -d'"' -f2 || echo "0.1.0")
STAGE_DIR="${ROOT_DIR}/dist/forgeiso-${VERSION}-linux-x86_64"
ARCHIVE="${ROOT_DIR}/dist/forgeiso-${VERSION}-linux-x86_64.tar.gz"
GUI_BIN="${ROOT_DIR}/gui/src-tauri/target/release/forgeiso-gui"

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin"

for bin in \
  "${ROOT_DIR}/target/release/forgeiso" \
  "${ROOT_DIR}/target/release/forgeiso-tui" \
  "${GUI_BIN}"; do
  if [[ ! -x "${bin}" ]]; then
    echo "missing release binary: ${bin}" >&2
    exit 1
  fi
  cp "${bin}" "${STAGE_DIR}/bin/"
done

cp "${ROOT_DIR}/README.md" "${STAGE_DIR}/README.md"
printf '%s\n' "${VERSION}" > "${STAGE_DIR}/VERSION"

tar -C "${ROOT_DIR}/dist" -czf "${ARCHIVE}" "$(basename "${STAGE_DIR}")"
echo "created ${ARCHIVE}"
