from __future__ import annotations

import shutil
from pathlib import Path
from typing import Optional


class CleanupStage:
    def __init__(self, logger, workdir: Path) -> None:
        self.logger = logger
        self.workdir = workdir
        self.rootfs_dir = workdir / "rootfs"
        self.output_dir = workdir / "output"
        self.state_dir = workdir / ".state"

    def clean(self, keep_output: bool = False) -> None:
        self.logger.stage("Cleanup")
        if self.rootfs_dir.exists():
            shutil.rmtree(self.rootfs_dir)
        if not keep_output and self.output_dir.exists():
            shutil.rmtree(self.output_dir)
        if self.state_dir.exists():
            shutil.rmtree(self.state_dir)
        self.logger.info("Cleanup complete")
