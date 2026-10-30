#!/usr/bin/env bash
# Entry point for the Tide OS build system. Run `./build.sh --help` for commands.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

command -v python3 >/dev/null 2>&1 || {
    echo "build.sh: python3 is required (run scripts/install-dev-deps.sh)" >&2
    exit 1
}
python3 -c 'import yaml' 2>/dev/null || {
    echo "build.sh: PyYAML is missing (apt install python3-yaml, or pip install -r requirements-dev.txt)" >&2
    exit 1
}

exec python3 -m builder.main "$@"
