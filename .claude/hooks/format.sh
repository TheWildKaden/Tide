#!/usr/bin/env bash
# PostToolUse hook: format Rust files and syntax-check shell scripts after Claude edits them.
# Exits 2 (feeding the error back to Claude) only for shell syntax errors; everything else is silent.
set -uo pipefail

file="$(jq -r '.tool_input.file_path // empty')"
[[ -n "$file" && -f "$file" ]] || exit 0

case "$file" in
    *.rs)
        command -v rustfmt >/dev/null 2>&1 && rustfmt --edition 2021 --quiet "$file" 2>/dev/null
        ;;
    *.sh)
        if ! out="$(bash -n "$file" 2>&1)"; then
            printf 'Shell syntax error in %s:\n%s\n' "$file" "$out" >&2
            exit 2
        fi
        ;;
esac
exit 0
