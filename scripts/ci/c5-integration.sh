#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

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

cargo test -p forgeiso-engine "${offline_flag[@]}"

mkdir -p artifacts/integration
cargo run -p forgeiso-cli "${offline_flag[@]}" -- doctor --json > artifacts/integration/doctor.json
cargo run -p forgeiso-cli "${offline_flag[@]}" -- inspect --source README.md > artifacts/integration/inspect-invalid.txt || true

if command -v grub2-mkrescue >/dev/null 2>&1 || command -v grub-mkrescue >/dev/null 2>&1; then
  smoke_dir="artifacts/integration/smoke"
  clean_path "$smoke_dir/out"
  clean_path "$smoke_dir/extract"
  eval "$(scripts/test/make-smoke-iso.sh "$smoke_dir")"

  cargo run -p forgeiso-cli "${offline_flag[@]}" -- inspect --source "$ISO" --json > "$smoke_dir/inspect.json"
  cargo run -p forgeiso-cli "${offline_flag[@]}" -- build \
    --source "$ISO" \
    --out "$smoke_dir/out" \
    --name ci-integration \
    --overlay "$OVERLAY" \
    --profile minimal \
    --json > "$smoke_dir/build.json"
  cargo run -p forgeiso-cli "${offline_flag[@]}" -- report --build "$smoke_dir/out" --format html > "$smoke_dir/report-path.txt"

  extract_dir="$smoke_dir/extract"
  mkdir -p "$extract_dir"
  xorriso -osirrox on -indev "$smoke_dir/out/ci-integration.iso" -extract / "$extract_dir" >/dev/null 2>&1
  test -f "$extract_dir/forgeiso-build.json"
  test -f "$extract_dir/LOCAL-OVERLAY.txt"
fi
