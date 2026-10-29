#!/usr/bin/env bash
# Install Tide development and ISO-build dependencies on Debian/Ubuntu hosts.
set -Eeuo pipefail

log() { printf '\n[Tide deps] %s\n' "$*"; }
fail() { printf '\n[Tide deps] ERROR: %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Linux" ]] || fail "This script expects Linux."
command -v apt-get >/dev/null 2>&1 || fail "This script supports Debian/Ubuntu hosts with apt-get."

SUDO=()
if [[ "$(id -u)" -ne 0 ]]; then
    command -v sudo >/dev/null 2>&1 || fail "Root or sudo is required to install system packages."
    SUDO=(sudo)
fi

ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
PACKAGES=(
    build-essential gcc g++ pkg-config make cmake
    git curl wget ca-certificates file
    xz-utils unzip zip rsync
    python3 python3-yaml python3-venv
    shellcheck jq
    debootstrap live-build
    xorriso isolinux syslinux-common syslinux-efi
    squashfs-tools dosfstools e2fsprogs
)
case "$ARCH" in
    amd64) PACKAGES+=(grub-pc-bin grub-efi-amd64-bin) ;;
    arm64) PACKAGES+=(grub-efi-arm64-bin) ;;
    *) log "No GRUB package mapping for $ARCH; install a bootloader package manually." ;;
esac

log "Installing build dependencies"
"${SUDO[@]}" apt-get update
"${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive \
    apt-get install -y --no-install-recommends "${PACKAGES[@]}"

if ! command -v rustup >/dev/null 2>&1 && [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
    log "Installing Rust with rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
export PATH="$HOME/.cargo/bin:$PATH"
rustup toolchain install stable --component rustfmt clippy
rustup default stable

log "Dependencies installed"
log "Run: ./build.sh doctor"
log "Then: ./build.sh all"
log "ISO builds need root privileges, network access, and several GiB of free space."
