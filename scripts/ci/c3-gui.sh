#!/usr/bin/env bash
set -euo pipefail
cd /workspace/gui

npm ci
npm run lint
npm run build

cd /workspace/gui/src-tauri
cargo check
