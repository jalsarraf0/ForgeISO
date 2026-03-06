#!/usr/bin/env bash
set -euo pipefail
cd /workspace

mkdir -p artifacts/security

if command -v trivy >/dev/null 2>&1; then
  trivy fs --format json --output artifacts/security/trivy-fs.json . || true
else
  echo '{"status":"trivy-not-installed"}' > artifacts/security/trivy-fs.json
fi

if command -v syft >/dev/null 2>&1; then
  syft dir:. -o cyclonedx-json > artifacts/security/sbom.cdx.json || true
  syft dir:. -o spdx-json > artifacts/security/sbom.spdx.json || true
else
  echo '{"status":"syft-not-installed"}' > artifacts/security/sbom.cdx.json
  echo '{"status":"syft-not-installed"}' > artifacts/security/sbom.spdx.json
fi

if command -v grype >/dev/null 2>&1; then
  grype dir:. -o json > artifacts/security/grype.json || true
else
  echo '{"status":"grype-not-installed"}' > artifacts/security/grype.json
fi

if command -v gitleaks >/dev/null 2>&1; then
  gitleaks detect --source . --report-format json --report-path artifacts/security/gitleaks.json || true
else
  echo '{"status":"gitleaks-not-installed"}' > artifacts/security/gitleaks.json
fi
