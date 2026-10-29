#!/usr/bin/env bash
set -Eeuo pipefail

log() { printf '\n[Tide Sync] %s\n' "$*"; }
fail() { printf '\n[Tide Sync] ERROR: %s\n' "$*" >&2; exit 1; }

command -v git >/dev/null 2>&1 || fail "Git is required."

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" ||
    fail "Run this script inside the Tide repository."
cd "$ROOT"

BRANCH="$(git branch --show-current)"
[[ -n "$BRANCH" ]] || fail "You must be on a branch, not detached HEAD."

REMOTE="origin"
git remote get-url "$REMOTE" >/dev/null 2>&1 ||
    fail "Git remote 'origin' is missing."

STAMP="$(date '+%Y-%m-%d %H:%M:%S %Z')"
SAFE_STAMP="$(date '+%Y%m%d-%H%M%S')"
COMMIT_MSG="Tide sync: ${STAMP}"
BACKUP_BRANCH="backup/${BRANCH}-${SAFE_STAMP}"

log "Creating backup branch: $BACKUP_BRANCH"
git branch "$BACKUP_BRANCH"

log "Fetching GitHub changes"
git fetch "$REMOTE" ||
    fail "Fetch failed. Local files have not been reset."

UPSTREAM="${REMOTE}/${BRANCH}"
git rev-parse --verify "$UPSTREAM" >/dev/null 2>&1 ||
    fail "Remote branch $UPSTREAM does not exist."

log "Merging remote changes without discarding local files"
if ! git merge --no-edit "$UPSTREAM"; then
    fail "Merge conflict or merge failure. Resolve it manually; nothing was force-reset."
fi

log "Staging repository changes"
git add -A

if ! git diff --cached --quiet; then
    git -c user.name="Tide Sync Bot" \
        -c user.email="tide-sync-bot@users.noreply.github.com" \
        commit -m "$COMMIT_MSG"
else
    log "No new changes to commit."
fi

log "Pushing changes to GitHub"
git push "$REMOTE" "$BRANCH" ||
    fail "Push rejected. Remote history was not forcibly overwritten."

log "Sync complete."
log "Commit: $COMMIT_MSG"
log "Backup branch: $BACKUP_BRANCH"