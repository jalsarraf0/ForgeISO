#!/usr/bin/env bash
set -euo pipefail
cd /workspace

mkdir -p artifacts/e2e

fake_iso=artifacts/e2e/fake.iso
head -c 4096 /dev/zero > "$fake_iso"

cargo run -p forgeiso-cli -- inspect --iso "$fake_iso" > artifacts/e2e/inspect.json
cargo run -p forgeiso-cli -- test --iso "$fake_iso" --bios --uefi --json > artifacts/e2e/test.json || true

if command -v qemu-system-x86_64 >/dev/null 2>&1; then
  echo '{"nested_virtualization":"available"}' > artifacts/e2e/virt.json
else
  echo '{"nested_virtualization":"unavailable-mocked"}' > artifacts/e2e/virt.json
fi
