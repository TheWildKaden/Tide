from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Callable, List, Optional


class BootstrapStage:
    def __init__(self, config, logger, workdir: Path, sudo: bool = False) -> None:
        self.config = config
        self.logger = logger
        self.workdir = workdir
        self.sudo = sudo
        self.rootfs_dir = workdir / "rootfs"

    def _run(self, command: List[str], cwd: Optional[Path] = None, env: Optional[dict] = None) -> None:
        if os.geteuid() != 0:
            command = ["sudo", *command]
        self.logger.info("Running: " + " ".join(command))
        completed = subprocess.run(command, cwd=str(cwd) if cwd else None, env=env, text=True)
        if completed.returncode != 0:
            raise RuntimeError(f"Command failed: {' '.join(command)}")

    def run(self) -> Path:
        self.logger.stage("Stage 2: Bootstrap")
        self.rootfs_dir.mkdir(parents=True, exist_ok=True)
        self.rootfs_dir.chmod(0o755)

        if self.rootfs_dir.exists() and any(self.rootfs_dir.iterdir()):
            self.logger.warning("Reusing existing rootfs directory")

        debootstrap = shutil.which("debootstrap")
        if not debootstrap:
            raise RuntimeError("debootstrap is required but was not found")

        package_list = ",".join(["systemd", "sudo", "ca-certificates", "openssh-server", "locales", "tzdata"] + self.config.base_packages)

        command = [
            debootstrap,
            "--arch",
            self.config.architecture,
            "--variant=minbase",
            "--include",
            package_list,
            self.config.codename,
            str(self.rootfs_dir),
            self.config.mirror_main,
        ]
        self._run(command)

        self.logger.info("Bootstrap completed")
        return self.rootfs_dir
