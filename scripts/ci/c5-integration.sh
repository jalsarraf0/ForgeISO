#!/usr/bin/env bash
set -euo pipefail
cd /workspace

cargo test -p forgeiso-engine

mkdir -p artifacts/integration
cargo run -p forgeiso-cli -- doctor --json > artifacts/integration/doctor.json
