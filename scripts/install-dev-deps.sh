#!/usr/bin/env bash
# Install Tide development and ISO-build dependencies on Debian/Ubuntu hosts.
#
# Required packages abort the script if they cannot be installed. Optional tools
# (ShellCheck, shfmt, yq, ...) are best-effort so one unavailable package cannot
# block an ISO build.
set -Eeuo pipefail

log() { printf '\n[Tide deps] %s\n' "$*"; }
warn() { printf '\n[Tide deps] WARNING: %s\n' "$*" >&2; }
fail() { printf '\n[Tide deps] ERROR: %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Linux" ]] || fail "This script expects Linux."
command -v apt-get >/dev/null 2>&1 || fail "This script supports Debian/Ubuntu hosts with apt-get."

SUDO=()
if [[ "$(id -u)" -ne 0 ]]; then
    command -v sudo >/dev/null 2>&1 || fail "Root or sudo is required to install system packages."
    SUDO=(sudo)
fi

apt_install() {
    "${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive \
        apt-get install -y --no-install-recommends "$@"
}

ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"

REQUIRED=(
    build-essential gcc g++ pkg-config make cmake ninja-build
    git curl wget ca-certificates file
    xz-utils unzip zip rsync
    python3 python3-yaml python3-venv
    debootstrap live-build xorriso mtools
    squashfs-tools dosfstools e2fsprogs
)
case "$ARCH" in
    amd64) REQUIRED+=(grub-pc-bin grub-efi-amd64-bin) ;;
    arm64) REQUIRED+=(grub-efi-arm64-bin) ;;
    *) warn "No GRUB package mapping for $ARCH; install a bootloader package manually." ;;
esac

# Some Ubuntu mirrors package the BIOS boot files differently. Treat these as
# best-effort; live-build will report the missing piece clearly if needed.
BIOS_OPTIONAL=(isolinux syslinux-common syslinux-efi)
TOOLS_OPTIONAL=(shellcheck jq shfmt yq)

log "Updating package lists"
"${SUDO[@]}" apt-get update

log "Installing required build dependencies"
if ! apt_install "${REQUIRED[@]}"; then
    fail "required packages could not be installed; check your mirror and package names"
fi

if ((${#BIOS_OPTIONAL[@]})); then
    log "Installing BIOS boot helpers (best effort)"
    if ! apt_install "${BIOS_OPTIONAL[@]}"; then
        warn "could not install all of: ${BIOS_OPTIONAL[*]}"
        warn "live-build may still work via GRUB, but BIOS boot support could be incomplete"
    fi
fi

for package in "${TOOLS_OPTIONAL[@]}"; do
    if ! apt_install "$package" >/dev/null 2>&1; then
        warn "optional tool '$package' is not available from this mirror; skipping"
    fi
done

install_rust() {
    export PATH="$HOME/.cargo/bin:$PATH"
    # shellcheck disable=SC1091
    [[ -f "$HOME/.cargo/env" ]] && . "$HOME/.cargo/env"

    if ! command -v rustup >/dev/null 2>&1 && [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
        log "Installing Rust with rustup"
        command -v curl >/dev/null 2>&1 || fail "curl is required to install rustup"
        local installer
        installer="$(mktemp "${TMPDIR:-/tmp}/tide-rustup-init.XXXXXX.sh")"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$installer"
        sh "$installer" -y --profile minimal
        rm -f "$installer"
    fi

    log "Ensuring stable toolchain with rustfmt and clippy"
    rustup toolchain install stable --profile minimal --component rustfmt --component clippy
    rustup default stable
}

install_rust

if command -v npm >/dev/null 2>&1; then
    # TypeScript is optional for Tide; do not fail the build if npm is restricted.
    if ! npm install --global typescript prettier >/dev/null 2>&1; then
        warn "could not install global TypeScript/Prettier; continuing"
    fi
fi

log "Dependencies installed"
log "Run: ./build.sh doctor"
log "Then: ./build.sh all"
log "ISO builds need root privileges, network access, and several GiB of free space."
