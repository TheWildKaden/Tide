from __future__ import annotations

import os
import subprocess
from pathlib import Path
from typing import List


class ChrootStage:
    def __init__(self, config, logger, workdir: Path) -> None:
        self.config = config
        self.logger = logger
        self.workdir = workdir
        self.rootfs_dir = workdir / "rootfs"

    def _run(self, command: List[str], chroot: bool = False) -> None:
        if os.geteuid() != 0:
            command = ["sudo", *command]
        if chroot:
            command = ["sudo", "chroot", str(self.rootfs_dir), *command] if os.geteuid() != 0 else ["chroot", str(self.rootfs_dir), *command]
        self.logger.info("Running: " + " ".join(command))
        completed = subprocess.run(command, text=True)
        if completed.returncode != 0:
            raise RuntimeError(f"Command failed: {' '.join(command)}")

    def run(self) -> None:
        self.logger.stage("Stage 3: Configure chroot")
        if not self.rootfs_dir.exists():
            raise RuntimeError("Rootfs directory not found")

        self._run(["/bin/bash", "-c", "echo tide > /etc/hostname"], chroot=True)
        self._run(["/bin/bash", "-c", "mkdir -p /etc/apt/sources.list.d && cat > /etc/apt/sources.list <<'EOF'\ndeb http://deb.debian.org/debian bookworm main contrib non-free non-free-firmware\ndeb http://deb.debian.org/debian bookworm-updates main contrib non-free non-free-firmware\ndeb http://security.debian.org/debian-security bookworm-security main contrib non-free non-free-firmware\nEOF"], chroot=True)
        self._run(["/bin/bash", "-c", "apt-get update"], chroot=True)
        self._run(["/bin/bash", "-c", "ln -sf /usr/share/zoneinfo/UTC /etc/localtime && DEBIAN_FRONTEND=noninteractive dpkg-reconfigure -f noninteractive tzdata"], chroot=True)
        self._run(["/bin/bash", "-c", "export DEBIAN_FRONTEND=noninteractive && apt-get install -y --no-install-recommends locales sudo ca-certificates systemd systemd-sysv openssh-server net-tools"], chroot=True)
        self._run(["/bin/bash", "-c", "useradd --create-home --shell /bin/bash tide || true && usermod -aG sudo tide"], chroot=True)
        self._run(["/bin/bash", "-c", "echo 'tide ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/tide && chmod 440 /etc/sudoers.d/tide"], chroot=True)
        self.logger.info("Chroot configuration completed")
