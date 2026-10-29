#!/usr/bin/env bash
# install_languages.sh — install the language toolchains used by the Tide project.
#
# Designed to keep working as the project grows:
#   * detects the host package manager instead of assuming Debian
#   * installs independent groups so one missing optional package cannot abort the run
#   * is idempotent and safe to re-run
#   * supports dry runs and component selection
#   * never pipes unverified data into a privileged shell
#
# Usage:
#   ./install_languages.sh [options]
#
# Options:
#   --minimal        Only C/C++, Rust, Python, and Node (no Qt, no extras)
#   --no-qt          Skip Qt 6 development packages
#   --no-extras      Skip optional tools (shellcheck, shfmt, jq, yq)
#   --no-rust        Skip rustup and the Rust toolchain
#   --no-node        Skip Node.js/npm and TypeScript/Prettier
#   --dry-run        Print the commands without running them
#   --yes            Do not prompt before installing system packages
#   -h, --help       Show this help
#
# Environment:
#   TIDE_INSTALL_QT=0|1        same as --no-qt / default
#   TIDE_INSTALL_EXTRAS=0|1    same as --no-extras
#   NPM_PREFIX=<path>          install global npm tools under this prefix
#
# Exit status: 0 when required toolchains are present, 1 otherwise.

set -Eeuo pipefail

readonly SCRIPT_NAME="${0##*/}"
readonly LOG_PREFIX="[Tide languages]"

INSTALL_QT="${TIDE_INSTALL_QT:-1}"
INSTALL_EXTRAS="${TIDE_INSTALL_EXTRAS:-1}"
INSTALL_RUST=1
INSTALL_NODE=1
DRY_RUN=0
ASSUME_YES=0

say() { printf '\n%s %s\n' "$LOG_PREFIX" "$*"; }
warn() { printf '%s WARNING: %s\n' "$LOG_PREFIX" "$*" >&2; }
die() { printf '%s ERROR: %s\n' "$LOG_PREFIX" "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

on_error() {
    local status=$?
    printf '%s FAILED (exit %s) at line %s: %s\n' \
        "$LOG_PREFIX" "$status" "${BASH_LINENO[0]:-?}" "${BASH_COMMAND:-unknown}" >&2
    exit "$status"
}
trap on_error ERR

usage() {
    sed -n '3,36p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
}

while (($#)); do
    case "$1" in
        --minimal) INSTALL_QT=0; INSTALL_EXTRAS=0 ;;
        --no-qt) INSTALL_QT=0 ;;
        --no-extras) INSTALL_EXTRAS=0 ;;
        --no-rust) INSTALL_RUST=0 ;;
        --no-node) INSTALL_NODE=0 ;;
        --dry-run) DRY_RUN=1 ;;
        --yes|-y) ASSUME_YES=1 ;;
        -h|--help) usage ;;
        *) die "unknown option: $1 (try --help)" ;;
    esac
    shift
done

[[ "$(uname -s)" == "Linux" ]] || die "this script targets Linux; detected $(uname -s)"

# ---------------------------------------------------------------------------
# Privilege handling
# ---------------------------------------------------------------------------

SUDO=()
if [[ "$(id -u)" -ne 0 ]]; then
    if have sudo; then
        SUDO=(sudo)
    else
        die "installing system packages requires root or sudo"
    fi
fi

run_cmd() {
    if ((DRY_RUN)); then
        printf '  [dry-run] %s\n' "$*"
        return 0
    fi
    "$@"
}

run_privileged() {
    run_cmd "${SUDO[@]}" "$@"
}

# ---------------------------------------------------------------------------
# Distribution and package-manager detection
# ---------------------------------------------------------------------------

OS_ID="unknown"
OS_LIKE=""
OS_NAME="unknown Linux"
if [[ -r /etc/os-release ]]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    OS_ID="${ID:-unknown}"
    OS_LIKE="${ID_LIKE:-}"
    OS_NAME="${PRETTY_NAME:-${NAME:-$OS_ID}}"
fi

PM=""
if have apt-get; then PM="apt"
elif have dnf; then PM="dnf"
elif have pacman; then PM="pacman"
elif have zypper; then PM="zypper"
elif have apk; then PM="apk"
fi

[[ -n "$PM" ]] || die "no supported package manager found (looked for apt-get, dnf, pacman, zypper, apk)"

say "Host: $OS_NAME (id=$OS_ID, package manager=$PM)"
((DRY_RUN)) && say "Dry run: no system changes will be made"

declare -a BASE=() PYTHON=() NODE=() QT=() EXTRAS=()
case "$PM" in
    apt)
        BASE=(build-essential pkg-config make cmake ninja-build git curl wget ca-certificates file xz-utils unzip zip rsync gcc g++ clang lld llvm libssl-dev zlib1g-dev)
        PYTHON=(python3 python3-dev python3-venv python3-pip pipx)
        NODE=(nodejs npm)
        QT=(qt6-base-dev qt6-declarative-dev qt6-tools-dev qt6-tools-dev-tools qml6-module-qtquick qml6-module-qtquick-controls qml6-module-qtquick-layouts qml6-module-qtquick-window)
        EXTRAS=(jq shellcheck shfmt yq)
        ;;
    dnf)
        BASE=(gcc gcc-c++ make cmake ninja-build pkgconf-pkg-config git curl wget ca-certificates file xz unzip zip rsync clang lld llvm-devel openssl-devel zlib-devel)
        PYTHON=(python3 python3-devel python3-pip pipx)
        NODE=(nodejs npm)
        QT=(qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qttools-devel)
        EXTRAS=(jq ShellCheck shfmt yq)
        ;;
    pacman)
        BASE=(base-devel cmake ninja pkgconf git curl wget ca-certificates file xz unzip zip rsync clang lld llvm openssl zlib)
        PYTHON=(python python-pip python-pipx)
        NODE=(nodejs npm)
        QT=(qt6-base qt6-declarative qt6-tools)
        EXTRAS=(jq shellcheck shfmt yq)
        ;;
    zypper)
        BASE=(gcc gcc-c++ make cmake ninja pkg-config git curl wget ca-certificates file xz unzip zip rsync clang lld llvm-devel libopenssl-devel zlib-devel)
        PYTHON=(python3 python3-devel python3-pip python3-pipx)
        NODE=(nodejs npm)
        QT=(qt6-base-devel qt6-declarative-devel qt6-tools-devel)
        EXTRAS=(jq ShellCheck shfmt yq)
        ;;
    apk)
        BASE=(build-base cmake ninja pkgconf git curl wget ca-certificates file xz unzip zip rsync clang lld llvm openssl-dev zlib-dev)
        PYTHON=(python3 python3-dev py3-pip)
        NODE=(nodejs npm)
        QT=(qt6-qtbase-dev qt6-qtdeclarative-dev qt6-qttools-dev)
        EXTRAS=(jq shellcheck shfmt yq)
        ;;
esac

pkg_refresh() {
    case "$PM" in
        apt) run_privileged env DEBIAN_FRONTEND=noninteractive apt-get update ;;
        dnf) run_privileged dnf -y makecache ;;
        pacman) run_privileged pacman -Sy --noconfirm ;;
        zypper) run_privileged zypper --non-interactive refresh ;;
        apk) run_privileged apk update ;;
    esac
}

# Install a package group. Returns non-zero only when the whole group fails, so
# callers can decide whether that is fatal.
pkg_install_group() {
    local label="$1"
    shift
    local -a packages=("$@")
    ((${#packages[@]})) || return 0

    say "Installing $label: ${packages[*]}"
    case "$PM" in
        apt)
            run_privileged env DEBIAN_FRONTEND=noninteractive \
                apt-get install -y --no-install-recommends "${packages[@]}"
            ;;
        dnf)
            run_privileged dnf install -y "${packages[@]}"
            ;;
        pacman)
            run_privileged pacman -S --needed --noconfirm "${packages[@]}"
            ;;
        zypper)
            run_privileged zypper --non-interactive install "${packages[@]}"
            ;;
        apk)
            run_privileged apk add --no-cache "${packages[@]}"
            ;;
    esac
}

pkg_install_optional() {
    local label="$1"
    shift
    if ! pkg_install_group "$label" "$@"; then
        warn "$label could not be installed; continuing (install it manually if you need it)"
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------------------
# System packages
# ---------------------------------------------------------------------------

if (( ! ASSUME_YES && ! DRY_RUN )); then
    say "This will install development packages with $PM."
    read -r -p "Continue? [Y/n] " answer
    case "${answer:-Y}" in
        [Yy]|[Yy][Ee][Ss]|"") ;;
        *) die "aborted by user" ;;
    esac
fi

pkg_refresh
pkg_install_group "base build tools" "${BASE[@]}"
pkg_install_optional "Python" "${PYTHON[@]}" || true

if ((INSTALL_NODE)); then
    pkg_install_optional "Node.js" "${NODE[@]}" || INSTALL_NODE=0
fi

if ((INSTALL_QT)); then
    # Qt is valuable but not required for day-to-day work; a failure is non-fatal.
    pkg_install_optional "Qt 6 development packages" "${QT[@]}" || true
fi

if ((INSTALL_EXTRAS)); then
    # Install extras one at a time: shfmt and yq are unavailable in some repos.
    for package in "${EXTRAS[@]}"; do
        pkg_install_optional "optional tool '$package'" "$package" || true
    done
fi

# ---------------------------------------------------------------------------
# Rust
# ---------------------------------------------------------------------------

install_rust() {
    export PATH="$HOME/.cargo/bin:$PATH"
    # shellcheck disable=SC1091
    [[ -f "$HOME/.cargo/env" ]] && . "$HOME/.cargo/env"

    if ! have rustup && [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
        say "Installing rustup (official installer)"
        have curl || die "curl is required to install rustup"
        if ((DRY_RUN)); then
            printf '  [dry-run] curl https://sh.rustup.rs | sh -s -- -y --profile minimal\n'
        else
            # Download first, then execute the local script, so a partial
            # download cannot be fed to a shell.
            local installer
            installer="$(mktemp "${TMPDIR:-/tmp}/rustup-init.XXXXXX.sh")"
            curl --proto '=https' --tlsv1.2 -sSf \
                https://sh.rustup.rs -o "$installer"
            sh "$installer" -y --profile minimal
            rm -f "$installer"
        fi
    fi

    if have rustup; then
        run_cmd rustup toolchain install stable --profile minimal \
            --component rustfmt --component clippy
        run_cmd rustup default stable
    else
        ((DRY_RUN)) || warn "rustup is still unavailable; skipping toolchain setup"
    fi
}

if ((INSTALL_RUST)); then
    install_rust
fi

# ---------------------------------------------------------------------------
# Node tools
# ---------------------------------------------------------------------------

install_node_tools() {
    have npm || { warn "npm not found; skipping TypeScript and Prettier"; return 0; }

    local prefix="${NPM_PREFIX:-}"
    if [[ -z "$prefix" ]]; then
        local default_prefix
        default_prefix="$(npm config get prefix 2>/dev/null || true)"
        if [[ -n "$default_prefix" ]] && [[ ! -w "$default_prefix/lib" ]]; then
            prefix="$HOME/.local"
            say "npm's global prefix is not writable; using $prefix"
            run_cmd npm config set prefix "$prefix"
            export PATH="$prefix/bin:$PATH"
        fi
    elif [[ -n "${NPM_PREFIX:-}" ]]; then
        run_cmd npm config set prefix "$prefix"
        export PATH="$prefix/bin:$PATH"
    fi

    run_cmd npm install --global typescript prettier
}

if ((INSTALL_NODE)); then
    install_node_tools
fi

# ---------------------------------------------------------------------------
# Verification
# ---------------------------------------------------------------------------

say "Toolchain status"

REQUIRED=(gcc g++ make cmake git curl python3)
((INSTALL_RUST)) && REQUIRED+=(rustc cargo)
((INSTALL_NODE)) && REQUIRED+=(node npm)

missing=0
printf '\n  %-12s %-10s %s\n' "TOOL" "STATUS" "VERSION"
for tool in "${REQUIRED[@]}"; do
    if have "$tool"; then
        version="$("$tool" --version 2>/dev/null | head -n 1 || true)"
        [[ -n "$version" ]] || version="(no --version output)"
        printf '  %-12s %-10s %s\n' "$tool" "ok" "$version"
    else
        printf '  %-12s %-10s %s\n' "$tool" "MISSING" "-"
        missing=$((missing + 1))
    fi
done

for tool in clang lld llvm-config shellcheck shfmt jq yq tsc prettier pipx; do
    if have "$tool"; then
        version="$("$tool" --version 2>/dev/null | head -n 1 || true)"
        [[ -n "$version" ]] || version="present"
        printf '  %-12s %-10s %s\n' "$tool" "ok" "$version"
    else
        printf '  %-12s %-10s %s\n' "$tool" "optional" "not found"
    fi
done

if ((INSTALL_QT)); then
    for tool in qmake6 qmllint; do
        if have "$tool"; then
            printf '  %-12s %-10s %s\n' "$tool" "ok" "$("$tool" --version 2>/dev/null | head -n 1 || true)"
        else
            printf '  %-12s %-10s %s\n' "$tool" "optional" "not found"
        fi
    done
fi

say "Next steps"
printf '  source "$HOME/.cargo/env"      # if Rust was just installed\n'
printf '  ./scripts/validate.sh          # run repository checks\n'
printf '  ./build.sh doctor              # check ISO build dependencies\n'

if ((missing > 0)); then
    die "$missing required toolchain(s) are missing; install them and re-run"
fi

say "Development language toolchains are ready."
