#!/usr/bin/env bash
set -euo pipefail

git config user.name "Tide Bot"
git config user.email "tide-bot@users.noreply.github.com"

while true; do
    if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
        git add -A
        git commit -m "Auto-commit: $(date -u '+%Y-%m-%d %H:%M:%S UTC')" || true
    fi
    sleep 60
done