#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-$(git describe --tags --abbrev=0 2>/dev/null | sed 's/^v//' || echo 0.1.0)}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${ROOT_DIR}/dist/release"
STAGE_DIR="${OUT_DIR}/forgeiso-${VERSION}-linux-x86_64"

mkdir -p "${OUT_DIR}"
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin" "${STAGE_DIR}/docs"

cp "${ROOT_DIR}/target/release/forgeiso" "${STAGE_DIR}/bin/"
cp "${ROOT_DIR}/target/release/forgeiso-tui" "${STAGE_DIR}/bin/"
cp "${ROOT_DIR}/target/release/forgeiso-agent" "${STAGE_DIR}/bin/"
cp "${ROOT_DIR}/README.md" "${ROOT_DIR}/LICENSE" "${STAGE_DIR}/"
cp -r "${ROOT_DIR}/docs"/* "${STAGE_DIR}/docs/"

tar -C "${OUT_DIR}" --zstd -cf "${OUT_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.zst" "forgeiso-${VERSION}-linux-x86_64"

echo "Created ${OUT_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.zst"
