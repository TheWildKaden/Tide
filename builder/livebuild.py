from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path
from typing import List


class LiveBuildStage:
    def __init__(self, config, logger, workdir: Path, repo_dir: Path) -> None:
        self.config = config
        self.logger = logger
        self.workdir = workdir
        self.repo_dir = repo_dir
        self.rootfs_dir = workdir / "rootfs"
        self.live_dir = self.workdir / "live-build"
        self.output_dir = repo_dir / "output"
        self.output_dir.mkdir(parents=True, exist_ok=True)

    def _run(self, command: List[str], cwd: Path | None = None) -> None:
        if os.geteuid() != 0:
            command = ["sudo", *command]
        self.logger.info("Running: " + " ".join(command))
        completed = subprocess.run(command, cwd=str(cwd) if cwd else None, text=True)
        if completed.returncode != 0:
            raise RuntimeError(f"Command failed: {' '.join(command)}")

    def run(self) -> Path:
        self.logger.stage("Stage 4: Build live ISO")
        self.output_dir.mkdir(parents=True, exist_ok=True)
        source_live_dir = self.repo_dir / "live-build"
        if not source_live_dir.exists():
            raise RuntimeError("live-build directory not found")

        shutil.copytree(source_live_dir, self.live_dir, dirs_exist_ok=True)

        lb = shutil.which("lb")
        if not lb:
            raise RuntimeError("live-build is required but was not found")

        self._run([
            lb,
            "config",
            "--mode",
            "debian",
            "--distribution",
            self.config.codename,
            "--architectures",
            self.config.architecture,
            "--mirror-bootstrap",
            self.config.mirror_main,
            "--mirror-chroot",
            self.config.mirror_main,
            "--bootstrap",
            "debootstrap",
            "--archive-areas",
            "main contrib non-free non-free-firmware",
            "--binary-images",
            "iso-hybrid",
            "--compression",
            "xz",
            "--bootappend-live",
            "boot=live quiet splash",
        ], cwd=self.live_dir)

        self._run([lb, "build"], cwd=self.live_dir)

        iso_files = sorted(self.live_dir.glob("**/*.iso"))
        if not iso_files:
            raise RuntimeError("No ISO was produced")

        iso_path = iso_files[0]
        destination = self.output_dir / iso_path.name
        destination.write_bytes(iso_path.read_bytes())
        self.logger.info(f"ISO created at {destination}")
        return destination
