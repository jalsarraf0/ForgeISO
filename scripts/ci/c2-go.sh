#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "$ROOT_DIR/.tmpgo" "$ROOT_DIR/.tmpgo-cache"
export TMPDIR="${TMPDIR:-$ROOT_DIR/.tmpgo}"
export GOCACHE="${GOCACHE:-$ROOT_DIR/.tmpgo-cache}"
cd "$ROOT_DIR/agent"

go test ./...
go vet ./...
