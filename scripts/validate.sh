#!/usr/bin/env bash
# Run the checks that this repository can perform without booting an image.
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# shellcheck disable=SC1091
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
export PATH="$HOME/.cargo/bin:$PATH"

log() { printf '\n[Tide validate] %s\n' "$*"; }

command -v cargo >/dev/null 2>&1 || {
    echo "cargo is required; run scripts/install-dev-deps.sh" >&2
    exit 1
}
command -v python3 >/dev/null 2>&1 || {
    echo "python3 is required" >&2
    exit 1
}

log "Python build-system tests"
python3 -m unittest discover -s tests -v

log "Rust formatting"
cargo fmt --all -- --check

log "Rust linting"
cargo clippy --workspace --all-targets -- -D warnings

log "Rust tests"
cargo test --workspace

log "Rust release build"
cargo build --release --workspace --bins

log "Shell syntax"
for script in build.sh scripts/*.sh assets/firstboot/*.sh; do
    bash -n "$script"
done

if command -v shellcheck >/dev/null 2>&1; then
    log "ShellCheck"
    shellcheck build.sh scripts/*.sh assets/firstboot/*.sh
else
    log "ShellCheck not installed; skipping (CI installs it)"
fi

log "Configuration and asset validation"
python3 - <<'PY'
import json
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path
import yaml

repo = Path(".")
with (repo / "config/tide.yaml").open(encoding="utf-8") as handle:
    yaml.safe_load(handle)
for file in sorted((repo / "assets/calamares").rglob("*")):
    if file.suffix in {".conf", ".desc"}:
        with file.open(encoding="utf-8") as handle:
            yaml.safe_load(handle)
for file in (repo / "assets").rglob("*.svg"):
    ET.parse(file)
json.loads((repo / "assets/desktop/plasma/look-and-feel/org.tide.desktop/metadata.json").read_text())
tomllib.loads((repo / "Cargo.toml").read_text())
print("configuration and assets parse successfully")
PY

log "All validation checks passed"
log "This does not prove that an ISO boots, installs, or runs on hardware."
