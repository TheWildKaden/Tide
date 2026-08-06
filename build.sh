#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOTFS="$ROOT/rootfs"
CACHE="$ROOT/cache"
LIVE_DIR="$ROOT/live-build"

CODENAME="${CODENAME:-bookworm}"
ARCH="${ARCH:-amd64}"
MIRROR="${MIRROR:-http://deb.debian.org/debian}"
PACKAGES="${PACKAGES:-locales,net-tools,iproute2,openssh-server,apt,ca-certificates,systemd-sysv,sudo,passwd}" 

usage() {
  cat <<EOF
Usage: $0 <command>

Commands:
  bootstrap   Create a Debian root filesystem with debootstrap
  configure   Install OS defaults inside the rootfs chroot
  build-iso   Build a live ISO using live-build (if installed)
  clean       Remove generated build artifacts

Environment variables:
  CODENAME   Debian codename (default: bookworm)
  ARCH       architecture (default: amd64)
  MIRROR     Debian mirror (default: http://deb.debian.org/debian)
  PACKAGES   comma-separated Debian packages to install in bootstrap
EOF
  exit 1
}

run_root() {
  if [ "$(id -u)" -ne 0 ]; then
    sudo "$@"
  else
    "$@"
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[!] Required command '$1' is missing."
    echo "    Install it with: sudo apt update && sudo apt install $1"
    exit 1
  fi
}

get_mount_options() {
  local path="$1"
  if command -v findmnt >/dev/null 2>&1; then
    findmnt -no OPTIONS --target "$path" 2>/dev/null || true
  else
    mount | awk -v p="$path" '$0 ~ " on " p " " {sub(/.*\(/, "", $0); sub(/\).*/, "", $0); print $0}'
  fi
}

mount_has_option() {
  local path="$1"
  local option="$2"
  local opts
  opts="$(get_mount_options "$path")"
  case " $opts " in
    *" $option "*) return 0 ;; 
    *) return 1 ;;
  esac
}

chroot_exec() {
  run_root mount --bind /proc "$ROOTFS/proc"
  run_root mount --bind /sys "$ROOTFS/sys"
  run_root mount --bind /dev "$ROOTFS/dev"
  run_root mount --bind /dev/pts "$ROOTFS/dev/pts"
  run_root chroot "$ROOTFS" /bin/bash -lc "$*"
  run_root umount "$ROOTFS/dev/pts" || true
  run_root umount "$ROOTFS/dev" || true
  run_root umount "$ROOTFS/sys" || true
  run_root umount "$ROOTFS/proc" || true
}

bootstrap() {
  echo "[+] Bootstrapping Debian $CODENAME ($ARCH) in $ROOTFS"
  require_command debootstrap
  run_root mkdir -p "$ROOTFS" "$CACHE"

  if run_root test -f "$ROOTFS/etc/debian_version"; then
    echo "[!] Existing rootfs detected in $ROOTFS"
    exit 1
  fi

  local tmp_rootfs="$(mktemp -d /tmp/tide-rootfs-XXXXXX)"
  trap 'rm -rf "$tmp_rootfs"' EXIT

  echo "[+] Using temporary rootfs path: $tmp_rootfs"
  run_root debootstrap --variant=minbase --arch="$ARCH" --include="$PACKAGES" "$CODENAME" "$tmp_rootfs" "$MIRROR"

  echo "[+] Syncing bootstrap result into $ROOTFS"
  run_root mkdir -p "$ROOTFS"
  run_root rsync -aHAX --delete "$tmp_rootfs/" "$ROOTFS/"

  echo "[+] Bootstrap complete"
}

configure() {
  if ! run_root test -d "$ROOTFS/etc"; then
    echo "[!] No rootfs found. Run '$0 bootstrap' first."
    exit 1
  fi

  echo "[+] Configuring rootfs"
  run_root cp "$ROOT/rootfs-config.sh" "$ROOTFS/usr/local/bin/rootfs-config.sh"
  run_root chmod +x "$ROOTFS/usr/local/bin/rootfs-config.sh"
  chroot_exec "/usr/local/bin/rootfs-config.sh"
  echo "[+] Rootfs configuration complete"
}

build_iso() {
  if [ ! -d "$LIVE_DIR" ]; then
    echo "[!] live-build profile not found in $LIVE_DIR"
    echo "    Create $LIVE_DIR/auto/config and install live-build"
    exit 1
  fi
  if ! command -v lb >/dev/null; then
    echo "[!] live-build is not installed. Install the live-build package and retry."
    exit 1
  fi

  pushd "$LIVE_DIR" >/dev/null
  lb config
  lb build
  popd >/dev/null
  echo "[+] ISO build completed in $LIVE_DIR"
}

clean() {
  echo "[+] Removing generated build artifacts"
  run_root rm -rf "$ROOTFS" "$CACHE"
  echo "[+] Clean complete"
}

if [ $# -ne 1 ]; then
  usage
fi

case "$1" in
  bootstrap) bootstrap ;; 
  configure) configure ;; 
  build-iso) build_iso ;; 
  clean) clean ;; 
  *) usage ;; 
 esac
