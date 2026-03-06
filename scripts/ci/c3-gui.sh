#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$ROOT_DIR/.cargo-tmp"
export TMPDIR="$ROOT_DIR/.cargo-tmp"
cd "$ROOT_DIR/gui"

offline_flag=()
if [[ "${CI:-false}" != "true" ]]; then
  offline_flag+=(--offline)
fi

if [[ ! -d node_modules ]]; then
  if [[ "${CI:-false}" == "true" ]]; then
    npm ci
  else
    npm ci --offline
  fi
fi
npm run lint
npm run build

cd "$ROOT_DIR/gui/src-tauri"
cargo check "${offline_flag[@]}"
