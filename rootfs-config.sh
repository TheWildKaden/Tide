#!/usr/bin/env bash
# Configure a Tide OS root filesystem.
#
# This script is executed inside a chroot by the Python build system, and can also
# be run manually inside a Debian root filesystem for development. It never sets a
# default password and never grants passwordless sudo.
set -Eeuo pipefail

export DEBIAN_FRONTEND=noninteractive

if [[ ! -f /etc/debian_version ]]; then
    echo "[!] This script must run inside a Debian root filesystem" >&2
    exit 1
fi
if [[ "$(id -u)" -ne 0 ]]; then
    echo "[!] This script must run as root" >&2
    exit 1
fi

CODENAME="${TIDE_CODENAME:-bookworm}"
MIRROR_MAIN="${TIDE_MIRROR_MAIN:-http://deb.debian.org/debian}"
MIRROR_SECURITY="${TIDE_MIRROR_SECURITY:-http://security.debian.org/debian-security}"
ARCHIVE_AREAS="${TIDE_ARCHIVE_AREAS:-main contrib non-free non-free-firmware}"
DESKTOP_PROFILE="${TIDE_DESKTOP_PROFILE:-none}"
PACKAGES_STRING="${TIDE_PACKAGES:-}"

log() { printf '\n[Tide rootfs] %s\n' "$*"; }
fail() { printf '\n[Tide rootfs] ERROR: %s\n' "$*" >&2; exit 1; }

case "$CODENAME" in
    *[!a-z0-9-]*|"") fail "invalid Debian codename: $CODENAME" ;;
esac
[[ "$MIRROR_MAIN" == http://* || "$MIRROR_MAIN" == https://* ]] || fail "invalid main mirror"
[[ "$MIRROR_SECURITY" == http://* || "$MIRROR_SECURITY" == https://* ]] || fail "invalid security mirror"

log "Configuring system identity"
printf 'tide\n' > /etc/hostname
cat > /etc/hosts <<'EOF'
127.0.0.1 localhost
127.0.1.1 tide
::1 localhost ip6-localhost ip6-loopback
ff02::1 ip6-allnodes
ff02::2 ip6-allrouters
EOF
mkdir -p /etc/tide

log "Configuring Debian package sources"
# The trailing ${ARCHIVE_AREAS} is intentionally word-split.
# shellcheck disable=SC2086
cat > /etc/apt/sources.list <<EOF
deb ${MIRROR_MAIN} ${CODENAME} ${ARCHIVE_AREAS}
deb ${MIRROR_MAIN} ${CODENAME}-updates ${ARCHIVE_AREAS}
deb ${MIRROR_SECURITY} ${CODENAME}-security ${ARCHIVE_AREAS}
EOF

mkdir -p /etc/apt/apt.conf.d
cat > /etc/apt/apt.conf.d/99tide <<'EOF'
APT::Install-Recommends "1";
APT::Install-Suggests "0";
Acquire::Retries "5";
EOF

log "Refreshing package metadata"
apt-get update

read -r -a PACKAGES <<<"$PACKAGES_STRING"
INSTALL=()
for package in "${PACKAGES[@]:-}"; do
    [[ -z "$package" ]] && continue
    if [[ ! "$package" =~ ^[a-z0-9][a-z0-9+.:-]*$ ]]; then
        fail "invalid package name from build configuration: $package"
    fi
    INSTALL+=("$package")
done

if ((${#INSTALL[@]})); then
    log "Installing Tide package set (${#INSTALL[@]} packages, profile=$DESKTOP_PROFILE)"
    apt-get install -y --no-install-recommends "${INSTALL[@]}"
else
    log "No additional packages requested"
fi

if command -v locale-gen >/dev/null 2>&1; then
    log "Configuring locale"
    cat > /etc/locale.gen <<'EOF'
en_US.UTF-8 UTF-8
EOF
    locale-gen
    update-locale LANG=en_US.UTF-8
fi

log "Setting timezone to UTC"
ln -sf /usr/share/zoneinfo/UTC /etc/localtime
if [[ -x /usr/sbin/dpkg-reconfigure ]]; then
    dpkg-reconfigure -f noninteractive tzdata
fi

log "Creating the tide user (locked password, no passwordless sudo)"
if ! getent group sudo >/dev/null 2>&1; then
    groupadd --system sudo
fi
if ! id tide >/dev/null 2>&1; then
    useradd --create-home --shell /bin/bash --groups sudo tide
else
    usermod --append --groups sudo tide
fi
passwd --lock tide >/dev/null 2>&1 || true
rm -f /etc/sudoers.d/tide
if [[ -f /etc/sudoers ]]; then
    visudo -c >/dev/null || fail "existing sudoers file is invalid"
fi

log "Configuring services"
if command -v systemctl >/dev/null 2>&1; then
    systemctl enable NetworkManager.service 2>/dev/null || true
    systemctl disable ssh.service 2>/dev/null || true
fi
# Keep the first-boot diagnostic enabled by the branding stage.

log "Cleaning package caches"
apt-get clean
rm -rf /var/lib/apt/lists/*
rm -rf /tmp/tide-rootfs-config.sh
find /tmp -mindepth 1 -maxdepth 1 ! -name '.X11-unix' -exec rm -rf {} + 2>/dev/null || true

rm -f /var/lib/tide/firstboot-done

log "Tide root filesystem configuration complete"
log "User 'tide' has a locked password and belongs to the sudo group."
