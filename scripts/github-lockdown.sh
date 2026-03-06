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
cat > /tmp/forgeiso-branch-protection.json <<'JSON'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "C1 Rust",
      "C2 Go",
      "C3 GUI",
      "C4 Security",
      "C5 Integration",
      "C6 E2E Smoke"
    ]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": true,
    "required_approving_review_count": 1
  },
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "block_creations": false,
  "required_conversation_resolution": true,
  "lock_branch": false,
  "allow_fork_syncing": false
}
JSON

gh api \
  --method PUT \
  -H "Accept: application/vnd.github+json" \
  "/repos/${repo}/branches/main/protection" \
  --input /tmp/forgeiso-branch-protection.json

echo "Repository lock-down applied for ${repo}."
