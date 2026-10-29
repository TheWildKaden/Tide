"""Build and stage Tide's Rust tools into an image."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

from builder.exec import find_command, run
from builder.logger import BuildLogger


class RustToolsError(RuntimeError):
    pass


class RustToolsStage:
    def __init__(self, repo: Path, logger: BuildLogger, *, dry_run: bool = False) -> None:
        self.repo = repo
        self.logger = logger
        self.dry_run = dry_run
        self.target_dir = Path(os.environ.get("CARGO_TARGET_DIR", repo / "target"))
        self.binaries = ["tide", "tidectl"]

    def ensure_built(self) -> Path:
        self.logger.stage("Building Tide Rust tools")
        cargo = find_command("cargo")
        if cargo is None and not self.dry_run:
            raise RustToolsError(
                "cargo is required to build the Tide package manager and tidectl; "
                "run scripts/install-dev-deps.sh"
            )
        lock = self.repo / "Cargo.lock"
        command = [
            cargo or "cargo",
            "build",
            "--release",
            "--workspace",
            "--bins",
        ]
        if lock.exists():
            command.insert(2, "--locked")
        run(command, self.logger, cwd=self.repo, dry_run=self.dry_run)
        for binary in self.binaries:
            path = self.target_dir / "release" / binary
            if not self.dry_run and not path.exists():
                raise RustToolsError(f"cargo did not produce {path}")
        return self.target_dir / "release"

    def stage(self, root: Path) -> None:
        release = self.ensure_built()
        destination = root / "usr/bin"
        if self.dry_run:
            self.logger.info(f"Would install Tide tools into {destination}")
            return
        destination.mkdir(parents=True, exist_ok=True)
        for binary in self.binaries:
            source = release / binary
            target = destination / binary
            shutil.copy2(source, target)
            target.chmod(0o755)
            self.logger.info(f"Installed {target}")
        version_file = self.repo / "VERSION"
        if version_file.exists():
            share = root / "usr/share/tide"
            share.mkdir(parents=True, exist_ok=True)
            shutil.copy2(version_file, share / "VERSION")
