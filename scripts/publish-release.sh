#!/usr/bin/env bash
# Upload locally built Tide artifacts to a GitHub release.
#
# Usage:
#   ./scripts/publish-release.sh <tag> [options]
#
# Options:
#   --create        Create the release if it does not exist yet
#   --prerelease    Mark a newly created release as a prerelease
#   --dry-run       Print what would be uploaded
#   -h, --help      Show this help
#
# Requirements: GitHub CLI (`gh`) authenticated with permission to write releases,
# and a completed `./build.sh all` or `./build.sh build-iso` run in `output/`.
#
# The tag must already exist in the repository. Assets are uploaded with
# `--clobber`, so re-running replaces assets for the same tag.

set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

say() { printf '\n[Tide publish] %s\n' "$*"; }
die() { printf '\n[Tide publish] ERROR: %s\n' "$*" >&2; exit 1; }

CREATE=0
PRERELEASE=0
DRY_RUN=0
TAG=""

while (($#)); do
    case "$1" in
        --create) CREATE=1 ;;
        --prerelease) PRERELEASE=1 ;;
        --dry-run) DRY_RUN=1 ;;
        -h|--help)
            sed -n '4,17p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        -*)
            die "unknown option: $1 (try --help)"
            ;;
        *)
            [[ -z "$TAG" ]] || die "only one tag may be provided"
            TAG="$1"
            ;;
    esac
    shift
done

[[ -n "$TAG" ]] || die "a release tag is required"

command -v gh >/dev/null 2>&1 || die "GitHub CLI 'gh' is required"
command -v sha256sum >/dev/null 2>&1 || die "sha256sum is required"

[[ -d output ]] || die "output/ does not exist; run ./build.sh all first"

shopt -s nullglob
isos=(output/*.iso)
manifests=(output/manifests/*.json)
checksums=(output/*.sha256)
[[ ${#isos[@]} -gt 0 ]] || die "no ISO found in output/; run ./build.sh all first"
[[ -f output/SHA256SUMS ]] || die "output/SHA256SUMS is missing; run ./build.sh manifest"

assets=("${isos[@]}" "${checksums[@]}" output/SHA256SUMS "${manifests[@]}")

say "Verifying checksums before publishing"
./build.sh verify

if ((DRY_RUN)); then
    say "Would publish tag $TAG with:"
    printf '  %s\n' "${assets[@]}"
    exit 0
fi

if ! gh release view "$TAG" >/dev/null 2>&1; then
    ((CREATE)) || die "release '$TAG' does not exist; pass --create or publish it first"
    say "Creating release $TAG"
    args=(release create "$TAG" --verify-tag --title "Tide OS $TAG" --generate-notes)
    ((PRERELEASE)) && args+=(--prerelease)
    gh "${args[@]}"
fi

say "Uploading ${#assets[@]} asset(s) to $TAG"
gh release upload "$TAG" "${assets[@]}" --clobber

say "Published: $(gh release view "$TAG" --json url --jq .url)"
say "REMINDER: release assets are checksummed but not signed."
