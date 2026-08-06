#!/usr/bin/env bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

if [ ! -f /etc/debian_version ]; then
  echo "[!] This script must run inside a Debian chroot"
  exit 1
fi

cat > /etc/hostname <<'EOF'
tide
EOF

cat > /etc/hosts <<'EOF'
127.0.0.1 localhost
127.0.1.1 tide
EOF

cat > /etc/apt/sources.list <<'EOF'
deb http://deb.debian.org/debian bookworm main contrib non-free
deb http://deb.debian.org/debian-security bookworm-security main contrib non-free
deb http://deb.debian.org/debian bookworm-updates main contrib non-free
EOF

mkdir -p /etc/apt/apt.conf.d
cat > /etc/apt/apt.conf.d/99norecommends <<'EOF'
APT::Install-Recommends "0";
APT::Install-Suggests "0";
EOF

apt-get update
apt-get install -y --no-install-recommends locales net-tools iproute2 openssh-server sudo ca-certificates

cat > /etc/locale.gen <<'EOF'
en_US.UTF-8 UTF-8
EOF
locale-gen
update-locale LANG=en_US.UTF-8

ln -fs /usr/share/zoneinfo/UTC /etc/localtime
DEBIAN_FRONTEND=noninteractive dpkg-reconfigure -f noninteractive tzdata

printf 'root:root\n' | chpasswd
if ! id tide >/dev/null 2>&1; then
  useradd -m -s /bin/bash tide
fi
printf 'tide:tide\n' | chpasswd
usermod -aG sudo tide

mkdir -p /root/.ssh
chmod 700 /root/.ssh

echo "[+] Rootfs defaults applied. user 'tide' with password 'tide' created."
