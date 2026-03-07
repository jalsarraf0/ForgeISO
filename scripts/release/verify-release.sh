#!/usr/bin/env bash
# Verify that all expected release artifacts exist and are non-empty
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
RELEASE_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"

PASS=0
FAIL=0

check_file() {
  local path="$1"
  local desc="$2"
  if [[ -f "${path}" && -s "${path}" ]]; then
    printf "  ✓ %-50s  %s\n" "${desc}" "$(ls -lh "${path}" | awk '{print $5}')"
    (( PASS++ )) || true
  else
    printf "  ✗ %-50s  MISSING OR EMPTY\n" "${desc}" >&2
    (( FAIL++ )) || true
  fi
}

echo "ForgeISO ${VERSION} — release verification"
echo "Release dir: ${RELEASE_DIR}"
echo ""

check_file "${RELEASE_DIR}/forgeiso-${VERSION}-linux-x86_64.tar.gz"  "Tarball (linux-x86_64)"
check_file "${RELEASE_DIR}/forgeiso-${VERSION}-1.x86_64.rpm"         "RPM (Fedora/RHEL/openSUSE)"
check_file "${RELEASE_DIR}/forgeiso_${VERSION}-1_amd64.deb"          "DEB (Debian/Ubuntu)"
check_file "${RELEASE_DIR}/forgeiso-${VERSION}-1-x86_64.pkg.tar.zst" "Pacman (Arch Linux)"
check_file "${RELEASE_DIR}/checksums.txt"                              "SHA-256 checksums"

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"

if (( FAIL > 0 )); then
  echo "Release verification FAILED — run make-packages.sh to rebuild." >&2
  exit 1
fi

echo "✓ Release verification passed for ForgeISO ${VERSION}"
