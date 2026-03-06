#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

ROOT_DIR="$(forgeiso_root_dir)"
VERSION="$(forgeiso_release_version "${ROOT_DIR}" "${1:-}")"
RELEASE_DIR="$(forgeiso_release_dir "${ROOT_DIR}")"
REPOS_DIR="${FORGEISO_REPOS_DIR:-${ROOT_DIR}/dist/repos}"
APT_REPO_DIR="${REPOS_DIR}/apt"
RPM_REPO_DIR="${REPOS_DIR}/rpm"
PACMAN_REPO_DIR="${REPOS_DIR}/pacman"
PACMAN_REPO_NAME="${FORGEISO_PACMAN_REPO_NAME:-forgeiso}"

mkdir -p "${RELEASE_DIR}"
rm -rf "${REPOS_DIR}"
mkdir -p "${APT_REPO_DIR}" "${RPM_REPO_DIR}" "${PACMAN_REPO_DIR}"

# APT repository metadata
shopt -s nullglob
DEB_FILES=("${RELEASE_DIR}"/forgeiso_*_amd64.deb)
if (( ${#DEB_FILES[@]} > 0 )); then
  APT_POOL_DIR="${APT_REPO_DIR}/pool/main/f/forgeiso"
  APT_BIN_DIR="${APT_REPO_DIR}/dists/stable/main/binary-amd64"
  mkdir -p "${APT_POOL_DIR}" "${APT_BIN_DIR}"
  cp "${DEB_FILES[@]}" "${APT_POOL_DIR}/"

  if command -v dpkg-scanpackages >/dev/null 2>&1; then
    (
      cd "${APT_REPO_DIR}"
      dpkg-scanpackages --multiversion pool /dev/null > dists/stable/main/binary-amd64/Packages
      gzip -9c dists/stable/main/binary-amd64/Packages > dists/stable/main/binary-amd64/Packages.gz
    )
  else
    echo "WARNING: dpkg-scanpackages not found; skipping APT Packages index" >&2
  fi

  if command -v apt-ftparchive >/dev/null 2>&1; then
    apt-ftparchive release "${APT_REPO_DIR}/dists/stable" > "${APT_REPO_DIR}/dists/stable/Release"
  else
    echo "WARNING: apt-ftparchive not found; skipping APT Release metadata" >&2
  fi

  if [[ -n "${FORGEISO_GPG_KEY_ID:-}" ]] && command -v gpg >/dev/null 2>&1 && [[ -f "${APT_REPO_DIR}/dists/stable/Release" ]]; then
    gpg --batch --yes --armor --detach-sign \
      -u "${FORGEISO_GPG_KEY_ID}" \
      -o "${APT_REPO_DIR}/dists/stable/Release.gpg" \
      "${APT_REPO_DIR}/dists/stable/Release"
    gpg --batch --yes --clearsign \
      -u "${FORGEISO_GPG_KEY_ID}" \
      -o "${APT_REPO_DIR}/dists/stable/InRelease" \
      "${APT_REPO_DIR}/dists/stable/Release"
  fi
else
  echo "WARNING: no .deb package found under ${RELEASE_DIR}" >&2
fi

# RPM repository metadata
RPM_FILES=("${RELEASE_DIR}"/*.rpm)
if (( ${#RPM_FILES[@]} > 0 )); then
  cp "${RPM_FILES[@]}" "${RPM_REPO_DIR}/"
  if command -v createrepo_c >/dev/null 2>&1; then
    createrepo_c --update "${RPM_REPO_DIR}"
  else
    echo "WARNING: createrepo_c not found; skipping RPM repodata generation" >&2
  fi
else
  echo "WARNING: no .rpm package found under ${RELEASE_DIR}" >&2
fi

# Pacman repository metadata
PACMAN_FILES=("${RELEASE_DIR}"/*.pkg.tar.zst)
if (( ${#PACMAN_FILES[@]} > 0 )); then
  cp "${PACMAN_FILES[@]}" "${PACMAN_REPO_DIR}/"
  if command -v repo-add >/dev/null 2>&1; then
    repo-add "${PACMAN_REPO_DIR}/${PACMAN_REPO_NAME}.db.tar.gz" "${PACMAN_REPO_DIR}"/*.pkg.tar.zst
  else
    echo "WARNING: repo-add not found; skipping pacman repo database generation" >&2
  fi
else
  echo "WARNING: no .pkg.tar.zst package found under ${RELEASE_DIR}" >&2
fi

shopt -u nullglob

if [[ -n "${FORGEISO_GPG_KEY_ID:-}" ]] && command -v gpg >/dev/null 2>&1; then
  for rpm in "${RPM_REPO_DIR}"/*.rpm; do
    [[ -f "${rpm}" ]] || continue
    gpg --batch --yes --armor --detach-sign -u "${FORGEISO_GPG_KEY_ID}" -o "${rpm}.asc" "${rpm}"
  done

  for pkg in "${PACMAN_REPO_DIR}"/*.pkg.tar.zst; do
    [[ -f "${pkg}" ]] || continue
    gpg --batch --yes --armor --detach-sign -u "${FORGEISO_GPG_KEY_ID}" -o "${pkg}.sig" "${pkg}"
  done
fi

tar -C "${REPOS_DIR}" -czf "${RELEASE_DIR}/forgeiso-repos-${VERSION}.tar.gz" apt rpm pacman

echo "Generated package repositories under ${REPOS_DIR}"
echo "Created ${RELEASE_DIR}/forgeiso-repos-${VERSION}.tar.gz"
