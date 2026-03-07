#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
mkdir -p "$ROOT_DIR/.cargo-tmp"
export TMPDIR="$ROOT_DIR/.cargo-tmp"

clean_path() {
  local path="$1"
  if [[ -e "$path" ]]; then
    chmod -R u+w "$path" 2>/dev/null || true
    rm -rf "$path"
  fi
}

offline_flag=()
if [[ "${CI:-false}" != "true" ]]; then
  offline_flag+=(--offline)
fi

mkdir -p artifacts/e2e

fake_iso=artifacts/e2e/fake.iso
head -c 4096 /dev/zero > "$fake_iso"

cargo run -p forgeiso-cli "${offline_flag[@]}" -- inspect --source "$fake_iso" > artifacts/e2e/inspect.txt || true
cargo run -p forgeiso-cli "${offline_flag[@]}" -- test --iso "$fake_iso" --bios --uefi --json > artifacts/e2e/test.json || true

if command -v qemu-system-x86_64 >/dev/null 2>&1; then
  echo '{"nested_virtualization":"available"}' > artifacts/e2e/virt.json
else
  echo '{"nested_virtualization":"unavailable"}' > artifacts/e2e/virt.json
fi

# QEMU boot smoke test: requires qemu, grub-mkrescue, AND /dev/kvm (hardware accel).
# Without KVM the guest runs in software emulation — too slow for CI time limits.
if command -v qemu-system-x86_64 >/dev/null 2>&1 \
    && { command -v grub2-mkrescue >/dev/null 2>&1 || command -v grub-mkrescue >/dev/null 2>&1; } \
    && [ -e /dev/kvm ]; then
  smoke_dir="artifacts/e2e/smoke"
  clean_path "$smoke_dir/out"
  eval "$(scripts/test/make-smoke-iso.sh "$smoke_dir")"

  cargo run -p forgeiso-cli "${offline_flag[@]}" -- build \
    --source "$ISO" \
    --out "$smoke_dir/out" \
    --name ci-e2e \
    --overlay "$OVERLAY" \
    --profile minimal \
    --json > "$smoke_dir/build.json"
  cargo run -p forgeiso-cli "${offline_flag[@]}" -- test \
    --iso "$smoke_dir/out/ci-e2e.iso" \
    --bios \
    --uefi \
    --json > "$smoke_dir/test.json"

  grep -q 'FORGEISO_SMOKE_START' "$smoke_dir/out/test/bios-serial.log"
  grep -q 'FORGEISO_SMOKE_START' "$smoke_dir/out/test/uefi-serial.log"
else
  echo "Skipping QEMU boot smoke test (no KVM or tools unavailable)" >&2
fi
