#!/usr/bin/env bash
# ForgeISO release common helpers
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

  git -C "${root_dir}" describe --tags --abbrev=0 2>/dev/null | sed 's/^v//' || \
    grep '^version' "${root_dir}/Cargo.toml" | head -1 | cut -d'"' -f2
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
    echo "Run: cargo build --release" >&2
    exit 1
  fi

  chmod +x "${binary_path}" || true
}

# Build a canonical staging tree under STAGING_DIR suitable for all package formats.
# Arguments: BIN_DIR ROOT_DIR STAGING_DIR
forgeiso_build_staging() {
  local bin_dir="$1"
  local root_dir="$2"
  local staging="$3"

  install -Dm755 "${bin_dir}/forgeiso"     "${staging}/usr/bin/forgeiso"
  install -Dm755 "${bin_dir}/forgeiso-tui" "${staging}/usr/bin/forgeiso-tui"

  install -Dm644 "${root_dir}/README.md" \
    "${staging}/usr/share/doc/forgeiso/README.md"

  if [[ -f "${root_dir}/CHANGELOG.md" ]]; then
    install -Dm644 "${root_dir}/CHANGELOG.md" \
      "${staging}/usr/share/doc/forgeiso/CHANGELOG.md"
  fi

  # Man page
  local man_src="${root_dir}/packaging/man/forgeiso.1"
  if [[ -f "${man_src}" ]]; then
    install -Dm644 "${man_src}" "${staging}/usr/share/man/man1/forgeiso.1"
    gzip -9f "${staging}/usr/share/man/man1/forgeiso.1"
  fi

  # Bash completion stub
  install -dm755 "${staging}/usr/share/bash-completion/completions"
  cat > "${staging}/usr/share/bash-completion/completions/forgeiso" <<'COMP'
# ForgeISO bash completion
_forgeiso() {
  local cur prev
  _init_completion || return
  local commands="doctor inspect verify build inject diff scan test report"
  case "${prev}" in
    forgeiso) COMPREPLY=($(compgen -W "${commands}" -- "${cur}")) ;;
    --source|--out|--base|--target|--build|--autoinstall|--wallpaper)
      _filedir ;;
    --profile)
      COMPREPLY=($(compgen -W "minimal desktop" -- "${cur}")) ;;
    --format)
      COMPREPLY=($(compgen -W "html json" -- "${cur}")) ;;
    --storage-layout)
      COMPREPLY=($(compgen -W "lvm direct zfs" -- "${cur}")) ;;
  esac
}
complete -F _forgeiso forgeiso
COMP

  echo "Staged package contents in ${staging}:"
  find "${staging}" -type f | sort | sed "s|^${staging}||"
}
