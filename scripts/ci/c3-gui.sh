#!/usr/bin/env bash
# C3: forge-gui (egui) build check — fmt + clippy + build
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$ROOT_DIR/.cargo-tmp"
export TMPDIR="$ROOT_DIR/.cargo-tmp"
cd "$ROOT_DIR"

echo "▶ [C3] fmt check..."
cargo fmt --manifest-path forge-gui/Cargo.toml --all --check

echo "▶ [C3] clippy..."
cargo clippy -p forge-gui --all-targets -- -D warnings

echo "▶ [C3] build (dev)..."
cargo build -p forge-gui

echo "▶ [C3] state/worker compile check..."
cargo check -p forge-gui

echo "▶ [C3] OK — forge-gui builds cleanly"
