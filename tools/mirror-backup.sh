#!/usr/bin/env bash
# Push every branch and tag to the remote named "backup".
# The owner adds that remote once (docs/backups.md). Not for everyday use —
# run after a known-good main, or on a schedule the owner chooses.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! git remote get-url backup >/dev/null 2>&1; then
  echo "No git remote named 'backup'."
  echo "Add one first — see docs/backups.md (owner inspection)."
  exit 1
fi

echo "==> fetching origin"
git fetch origin --prune

echo "==> pushing all branches to backup"
git push backup --all

echo "==> pushing all tags to backup"
git push backup --tags

echo "Backup remote is up to date: $(git remote get-url backup)"
