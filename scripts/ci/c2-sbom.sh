#!/usr/bin/env bash
# C2 SBOM + Dependency Audit — fails on HIGH/CRITICAL advisories or license violations
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

mkdir -p artifacts/sbom

echo "=== C2: SBOM + Dependency Security ==="

# ── 1. cargo-deny: license + advisory policy ─────────────────────────────────
echo ""
echo "▶ [1/3] cargo-deny check (licenses + advisories + bans + sources)..."

offline_flag=()
if [[ "${CI:-false}" != "true" ]]; then
  offline_flag+=(--offline)
fi

# Run cargo-deny — exits non-zero on any 'deny' level violation.
# Our deny.toml is configured to deny HIGH/CRITICAL advisories and
# unapproved licenses. This will FAIL the CI stage.
cargo deny check "${offline_flag[@]}" 2>&1 | tee artifacts/sbom/deny-report.txt
echo "  cargo-deny: PASSED"

# ── 2. cargo-audit: advisory database check ──────────────────────────────────
echo ""
echo "▶ [2/3] cargo-audit (RustSec advisory database)..."

# cargo-audit exits 1 if any vulnerability is found.
# We output JSON for artifact archiving and also human-readable to stdout.
if [[ "${CI:-false}" == "true" ]]; then
  cargo audit --json 2>/dev/null | tee artifacts/sbom/audit.json || {
    echo "ERROR: cargo-audit found vulnerabilities in dependencies" >&2
    exit 1
  }
else
  # Offline mode for local pre-push testing
  cargo audit --no-fetch --json 2>/dev/null | tee artifacts/sbom/audit.json || {
    echo "ERROR: cargo-audit found vulnerabilities" >&2
    exit 1
  }
fi
echo "  cargo-audit: PASSED (no known vulnerabilities)"

# ── 3. SBOM generation ───────────────────────────────────────────────────────
echo ""
echo "▶ [3/3] Generating SBOM artifacts..."

if command -v syft >/dev/null 2>&1; then
  syft dir:. -o cyclonedx-json > artifacts/sbom/sbom.cdx.json
  syft dir:. -o spdx-json      > artifacts/sbom/sbom.spdx.json
  echo "  SBOM (CycloneDX): artifacts/sbom/sbom.cdx.json"
  echo "  SBOM (SPDX):      artifacts/sbom/sbom.spdx.json"

  # Summarize component count
  COMP_COUNT=$(python3 -c "import json,sys; d=json.load(open('artifacts/sbom/sbom.cdx.json')); print(len(d.get('components',[])))" 2>/dev/null || echo "unknown")
  echo "  Components in SBOM: ${COMP_COUNT}"
else
  echo "  WARNING: syft not installed — skipping SBOM generation" >&2
  echo '{"status":"syft-not-installed"}' > artifacts/sbom/sbom.cdx.json
  echo '{"status":"syft-not-installed"}' > artifacts/sbom/sbom.spdx.json
fi

echo ""
echo "=== C2 PASSED: No license violations, no known vulnerabilities ==="
echo "Artifacts written to artifacts/sbom/"
ls -lh artifacts/sbom/ 2>/dev/null || true
