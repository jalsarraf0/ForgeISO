#!/usr/bin/env bash
set -euo pipefail

if ! command -v rpmbuild >/dev/null 2>&1; then
  echo "rpmbuild is required" >&2
  exit 1
fi

VERSION="${1:-$(git describe --tags --abbrev=0 2>/dev/null | sed 's/^v//' || echo 0.1.0)}"
RELEASE="${2:-1}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RPM_ROOT="${ROOT_DIR}/dist/rpm"
PKG_ROOT="${ROOT_DIR}/dist/release/forgeiso-${VERSION}-linux-x86_64"
SRC_ROOT="${ROOT_DIR}/dist/rpm-src/forgeiso-${VERSION}"

mkdir -p "${RPM_ROOT}"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
rm -rf "${SRC_ROOT}"
mkdir -p "${SRC_ROOT}/bin"

cp "${ROOT_DIR}/target/release/forgeiso" "${SRC_ROOT}/bin/"
cp "${ROOT_DIR}/target/release/forgeiso-tui" "${SRC_ROOT}/bin/"
cp "${ROOT_DIR}/target/release/forgeiso-agent" "${SRC_ROOT}/bin/"
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

mkdir -p "${ROOT_DIR}/dist/release"
find "${RPM_ROOT}/RPMS" -name '*.rpm' -exec cp {} "${ROOT_DIR}/dist/release/" \;

echo "RPM packages copied to ${ROOT_DIR}/dist/release"
