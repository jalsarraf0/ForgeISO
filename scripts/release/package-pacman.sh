#!/usr/bin/env bash
set -euo pipefail

if ! command -v tar >/dev/null 2>&1; then
  echo "tar is required to build pacman package" >&2
  exit 1
fi

if ! command -v zstd >/dev/null 2>&1; then
  echo "zstd is required to build pacman package" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
RELEASE="${2:-1}"
BIN_DIR="$(forgeiso_bin_dir "${ROOT_DIR}")"
OUT_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"
WORK_DIR="${ROOT_DIR}/dist/pacman/forgeiso-${VERSION}-${RELEASE}"
ARCHIVE_DIR="${WORK_DIR}/archive"
PACKAGE_PATH="${OUT_DIR}/forgeiso-${VERSION}-${RELEASE}-x86_64.pkg.tar.zst"

forgeiso_require_binary "${BIN_DIR}" "forgeiso"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-tui"
forgeiso_require_binary "${BIN_DIR}" "forgeiso-agent"

mkdir -p "${OUT_DIR}"
rm -rf "${WORK_DIR}"
rm -f "${PACKAGE_PATH}"
mkdir -p "${ARCHIVE_DIR}/usr/bin" "${ARCHIVE_DIR}/usr/share/doc/forgeiso" "${ARCHIVE_DIR}/usr/share/licenses/forgeiso"

install -m 0755 "${BIN_DIR}/forgeiso" "${ARCHIVE_DIR}/usr/bin/forgeiso"
install -m 0755 "${BIN_DIR}/forgeiso-tui" "${ARCHIVE_DIR}/usr/bin/forgeiso-tui"
install -m 0755 "${BIN_DIR}/forgeiso-agent" "${ARCHIVE_DIR}/usr/bin/forgeiso-agent"
install -m 0644 "${ROOT_DIR}/README.md" "${ARCHIVE_DIR}/usr/share/doc/forgeiso/README.md"
install -m 0644 "${ROOT_DIR}/LICENSE" "${ARCHIVE_DIR}/usr/share/licenses/forgeiso/LICENSE"

build_date="$(date +%s)"
installed_size="$(du -sb "${ARCHIVE_DIR}/usr" | awk '{print $1}')"

cat > "${ARCHIVE_DIR}/.PKGINFO" <<PKGINFO
pkgname = forgeiso
pkgbase = forgeiso
pkgver = ${VERSION}-${RELEASE}
pkgdesc = Cross-distro ISO customization platform
url = https://github.com/jalsarraf0/ForgeISO
builddate = ${build_date}
packager = Jamal Al-Sarraf <19882582+jalsarraf0@users.noreply.github.com>
size = ${installed_size}
arch = x86_64
license = Apache-2.0
depend = bash
PKGINFO

cat > "${ARCHIVE_DIR}/.BUILDINFO" <<BUILDINFO
format = 2
pkgname = forgeiso
pkgver = ${VERSION}-${RELEASE}
pkgarch = x86_64
packager = Jamal Al-Sarraf <19882582+jalsarraf0@users.noreply.github.com>
builddate = ${build_date}
builddir = ${WORK_DIR}
startdir = ${ROOT_DIR}
buildtool = forgeiso-release
buildtoolver = ${VERSION}
BUILDINFO

tar \
  --format=posix \
  --sort=name \
  --owner=0 \
  --group=0 \
  --numeric-owner \
  --mtime="@${build_date}" \
  -I "zstd -19 -T0" \
  -cf "${PACKAGE_PATH}" \
  -C "${ARCHIVE_DIR}" \
  .

rm -rf "${WORK_DIR}"

echo "Created ${PACKAGE_PATH}"
