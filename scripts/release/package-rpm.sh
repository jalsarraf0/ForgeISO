#!/usr/bin/env bash
set -euo pipefail

if ! command -v rpmbuild >/dev/null 2>&1; then
  echo "rpmbuild is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
RELEASE="${2:-1}"
BIN_DIR="$(forgeiso_bin_dir "${ROOT_DIR}")"
OUT_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"
RPM_ROOT="${ROOT_DIR}/dist/rpm"
SRC_ROOT="${ROOT_DIR}/dist/rpm-src/forgeiso-${VERSION}"

forgeiso_require_binary "${BIN_DIR}" "forgeiso"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-tui"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-agent"

rm -rf "${RPM_ROOT}" "${ROOT_DIR}/dist/rpm-src"
mkdir -p "${RPM_ROOT}"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
rm -rf "${SRC_ROOT}"
mkdir -p "${SRC_ROOT}/bin"

cp "${BIN_DIR}/forgeiso" "${SRC_ROOT}/bin/"
cp "${BIN_DIR}/forgeiso-tui" "${SRC_ROOT}/bin/"
cp "${BIN_DIR}/forgeiso-agent" "${SRC_ROOT}/bin/"
cp "${ROOT_DIR}/README.md" "${ROOT_DIR}/LICENSE" "${SRC_ROOT}/"

tar -C "${ROOT_DIR}/dist/rpm-src" -czf "${RPM_ROOT}/SOURCES/forgeiso-${VERSION}.tar.gz" "forgeiso-${VERSION}"
cp "${ROOT_DIR}/packaging/rpm/forgeiso.spec" "${RPM_ROOT}/SPECS/forgeiso.spec"

rpmbuild \
  --define "_topdir ${RPM_ROOT}" \
  --define "version ${VERSION}" \
  --define "release ${RELEASE}" \
  --define "debug_package %{nil}" \
  --define "_enable_debug_packages 0" \
  -ba "${RPM_ROOT}/SPECS/forgeiso.spec"

mkdir -p "${OUT_DIR}"
find "${RPM_ROOT}/RPMS" -name '*.rpm' -exec cp {} "${OUT_DIR}/" \;

echo "RPM packages copied to ${OUT_DIR}"
