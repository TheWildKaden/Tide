#!/usr/bin/env bash
set -Eeuo pipefail

INSTALL_QT="${INSTALL_QT:-1}"

log() { printf '\n[Tide Setup] %s\n' "$*"; }
fail() { printf '\n[Tide Setup] ERROR: %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Linux" ]] || fail "This script expects Linux."
command -v apt-get >/dev/null 2>&1 || fail "Requires Debian/Ubuntu-based Codespaces."

SUDO=()
if [[ "$(id -u)" -ne 0 ]]; then
    command -v sudo >/dev/null 2>&1 || fail "Package installation requires root or sudo."
    SUDO=(sudo)
fi

log "Updating package lists"
"${SUDO[@]}" apt-get update

PACKAGES=(
    build-essential gcc g++ clang lld llvm
    make cmake ninja-build pkg-config
    git curl wget ca-certificates file
    xz-utils unzip zip
    python3 python3-dev python3-pip python3-venv pipx
    shellcheck shfmt jq yq
    nodejs npm
    libssl-dev zlib1g-dev libapt-pkg-dev
)

if [[ "$INSTALL_QT" == "1" ]]; then
    PACKAGES+=(
        qt6-base-dev
        qt6-declarative-dev
        qt6-tools-dev
        qt6-tools-dev-tools
        qml6-module-qtquick
        qml6-module-qtquick-controls
        qml6-module-qtquick-layouts
        qml6-module-qtquick-window
    )
fi

log "Installing languages, compilers, and build tools"
"${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive \
    apt-get install -y --no-install-recommends "${PACKAGES[@]}"

log "Installing Rust"
if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y
fi

export PATH="$HOME/.cargo/bin:$PATH"
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"

rustup toolchain install stable
rustup default stable
rustup component add rustfmt clippy

log "Installing TypeScript and Prettier"
npm install --global typescript prettier

log "Checking installations"
for tool in rustc cargo gcc g++ clang cmake python3 node npm shellcheck shfmt jq; do
    if command -v "$tool" >/dev/null 2>&1; then
        printf 'OK   %-12s %s\n' "$tool" "$(command -v "$tool")"
    else
        printf 'SKIP %-12s not found\n' "$tool"
    fi
done

rustc --version
cargo --version
python3 --version
node --version
npm --version

if [[ "$INSTALL_QT" == "1" ]]; then
    command -v qmake6 >/dev/null 2>&1 && qmake6 --version || true
    command -v qmllint >/dev/null 2>&1 && qmllint --version || true
fi

log "Tide development environment installation complete."