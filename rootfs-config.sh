#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

if [ ! -f /etc/debian_version ]; then
    echo "[!] This script must run inside a Debian chroot"
    exit 1
fi


echo "[+] Configuring Tide OS root filesystem"


# -----------------------------
# System identity
# -----------------------------

echo "tide" > /etc/hostname

cat > /etc/hosts <<'EOF'
127.0.0.1 localhost
127.0.1.1 tide
::1 localhost ip6-localhost ip6-loopback
EOF



# -----------------------------
# Debian repositories
# -----------------------------

echo "[+] Configuring Debian repositories"

cat > /etc/apt/sources.list <<'EOF'
deb http://deb.debian.org/debian bookworm main contrib non-free non-free-firmware
deb http://deb.debian.org/debian bookworm-updates main contrib non-free non-free-firmware
deb http://security.debian.org/debian-security bookworm-security main contrib non-free non-free-firmware
EOF



# -----------------------------
# APT configuration
# -----------------------------

mkdir -p /etc/apt/apt.conf.d

cat > /etc/apt/apt.conf.d/99tide <<'EOF'
APT::Install-Recommends "0";
APT::Install-Suggests "0";
Acquire::Retries "5";
EOF



echo "[+] Updating package database"

apt-get update



# -----------------------------
# Base packages
# -----------------------------

echo "[+] Installing base system packages"

apt-get install -y --no-install-recommends \
    apt \
    sudo \
    locales \
    tzdata \
    ca-certificates \
    systemd \
    systemd-sysv \
    dbus \
    udev \
    procps \
    iproute2 \
    net-tools \
    openssh-server \
    nano \
    bash \
    curl \
    wget \
    rsync



# -----------------------------
# Locale
# -----------------------------

echo "[+] Configuring locale"

cat > /etc/locale.gen <<'EOF'
en_US.UTF-8 UTF-8
EOF

locale-gen

update-locale LANG=en_US.UTF-8



# -----------------------------
# Timezone
# -----------------------------

echo "[+] Setting timezone"

ln -sf /usr/share/zoneinfo/UTC /etc/localtime

dpkg-reconfigure -f noninteractive tzdata



# -----------------------------
# User setup
# -----------------------------

echo "[+] Creating Tide user"

if ! id tide >/dev/null 2>&1; then
    useradd \
        --create-home \
        --shell /bin/bash \
        tide
fi


usermod -aG sudo tide



# Allow sudo without password for first boot
cat > /etc/sudoers.d/tide <<'EOF'
tide ALL=(ALL) NOPASSWD:ALL
EOF

chmod 440 /etc/sudoers.d/tide



# -----------------------------
# SSH setup
# -----------------------------

echo "[+] Configuring SSH"

mkdir -p /run/sshd
mkdir -p /home/tide/.ssh

chmod 700 /home/tide/.ssh

chown -R tide:tide /home/tide/.ssh


systemctl enable ssh.service 2>/dev/null || true



# -----------------------------
# Disable unnecessary services
# -----------------------------

systemctl mask \
    systemd-firstboot.service \
    getty@tty1.service \
    2>/dev/null || true



# -----------------------------
# Cleanup
# -----------------------------

echo "[+] Cleaning package cache"

apt-get clean

rm -rf /var/lib/apt/lists/*
rm -rf /tmp/*



echo
echo "[+] Tide OS rootfs configuration complete"
echo "[+] Default user created: tide"
echo "[+] User has sudo access"