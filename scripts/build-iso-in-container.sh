#!/usr/bin/env bash
# Build the Tide ISO inside a Debian container with Debian's own live-build.
#
# Ubuntu hosts (including GitHub's ubuntu-24.04 runners) ship live-build 3.0~a57, a 2013 fork
# that cannot build a current bookworm image (it fetches the pre-2023 Contents-*.gz layout).
# Running in debian:<codename> gives the matching live-build without changing the host.
#
# Usage: scripts/build-iso-in-container.sh [build.sh options...]   (default: all)
# Env:   TIDE_CONTAINER_IMAGE (default debian:bookworm), TIDE_CONTAINER_ENGINE (docker|podman)
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${TIDE_CONTAINER_IMAGE:-debian:bookworm}"
ENGINE="${TIDE_CONTAINER_ENGINE:-docker}"

command -v "$ENGINE" >/dev/null 2>&1 || {
    echo "build-iso-in-container: $ENGINE is required" >&2
    exit 1
}
(($#)) || set -- all

SUDO=()
[[ "$ENGINE" == docker && "$(id -u)" -ne 0 ]] && ! docker info >/dev/null 2>&1 && SUDO=(sudo)

# shellcheck disable=SC2016  # the inner script is expanded by the container's bash
# --privileged is required: debootstrap and live-build mount /proc, /sys, and /dev in the chroot.
exec "${SUDO[@]}" "$ENGINE" run --rm --privileged \
    -v "$ROOT:/src" -w /src \
    -e DEBIAN_FRONTEND=noninteractive \
    "$IMAGE" \
    bash -euo pipefail -c '
        ./scripts/install-dev-deps.sh
        git config --global --add safe.directory /src
        export PATH="$HOME/.cargo/bin:$PATH"
        ./build.sh doctor
        ./build.sh "$@"
        [[ " $* " == *" --dry-run "* ]] || ./build.sh verify
    ' tide-build "$@"
