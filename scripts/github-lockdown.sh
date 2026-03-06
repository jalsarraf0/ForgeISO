#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <owner/repo>"
  exit 1
fi

repo="$1"

gh repo edit "$repo" --enable-issues --enable-wiki=false --enable-projects=false

# Branch protection for main
# Requires GitHub API scopes with repo admin access.
gh api \
  --method PUT \
  -H "Accept: application/vnd.github+json" \
  "/repos/${repo}/branches/main/protection" \
  -f required_status_checks.strict=true \
  -f required_status_checks.contexts[]='C1 Rust' \
  -f required_status_checks.contexts[]='C2 Go' \
  -f required_status_checks.contexts[]='C3 GUI' \
  -f required_status_checks.contexts[]='C4 Security' \
  -f required_status_checks.contexts[]='C5 Integration' \
  -f required_status_checks.contexts[]='C6 E2E Smoke' \
  -f enforce_admins=true \
  -f required_pull_request_reviews.dismiss_stale_reviews=true \
  -f required_pull_request_reviews.require_code_owner_reviews=true \
  -f restrictions=null \
  -f allow_force_pushes=false \
  -f allow_deletions=false

echo "Repository lock-down applied for ${repo}."
