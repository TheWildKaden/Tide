from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import sys
import time
from pathlib import Path
from typing import Optional

from builder.bootstrap import BootstrapStage
from builder.chroot import ChrootStage
from builder.cleanup import CleanupStage
from builder.config import load_config, write_config_snapshot
from builder.livebuild import LiveBuildStage
from builder.logger import BuildLogger
from builder.packages import PackagePlan


class TideBuilder:
    def __init__(self, repo_dir: Path) -> None:
        self.repo_dir = repo_dir.resolve()
        self.workdir = Path("/tmp/tide-builder")
        self.workdir.mkdir(parents=True, exist_ok=True)
        self.state_dir = self.workdir / ".state"
        self.logs_dir = self.workdir / "logs"
        self.output_dir = self.repo_dir / "output"
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.logger = BuildLogger(self.logs_dir, "tide-build")
        self.config = load_config(self.repo_dir / "config" / "tide.yaml")
        self.state_dir.mkdir(parents=True, exist_ok=True)

    def validate_environment(self) -> None:
        self.logger.stage("Stage 1: Environment checks")
        self.repo_dir.joinpath("rootfs").mkdir(parents=True, exist_ok=True)
        self.repo_dir.joinpath("output").mkdir(parents=True, exist_ok=True)
        if os.geteuid() != 0:
            self.logger.warning("Running without root privileges; some steps may fail")
        required_commands = ["debootstrap", "lb", "grub-mkrescue", "rsync", "mount", "umount"]
        missing = [cmd for cmd in required_commands if not shutil.which(cmd)]
        if missing:
            raise RuntimeError(f"Missing required commands: {', '.join(missing)}")

        mount_info = shutil.which("mount")
        if mount_info:
            self.logger.info("Environment checks passed")

        if self.workdir.exists() and self.workdir.is_dir():
            self._check_restricted_mounts()

    def _check_restricted_mounts(self) -> None:
        mount_output = shutil.which("mount")
        if not mount_output:
            return
        try:
            import subprocess
            result = subprocess.run(["mount"], text=True, capture_output=True, check=True)
            if "nodev" in result.stdout and "/workspaces" in result.stdout:
                self.logger.warning("Workspace appears mounted with nodev restrictions; build may fail")
        except Exception as exc:
            self.logger.warning(f"Unable to inspect mount table: {exc}")

    def _write_state(self, name: str) -> None:
        state_path = self.state_dir / f"{name}.state"
        state_path.write_text(str(time.time()), encoding="utf-8")

    def _state_exists(self, name: str) -> bool:
        return (self.state_dir / f"{name}.state").exists()

    def run(self) -> Path:
        self.validate_environment()
        write_config_snapshot(self.config, self.state_dir / "config.json")

        if self._state_exists("bootstrap"):
            self.logger.info("Bootstrap stage already completed; skipping")
        else:
            BootstrapStage(self.config, self.logger, self.workdir).run()
            self._write_state("bootstrap")

        if self._state_exists("chroot"):
            self.logger.info("Chroot stage already completed; skipping")
        else:
            ChrootStage(self.config, self.logger, self.workdir).run()
            self._write_state("chroot")

        if self._state_exists("livebuild"):
            self.logger.info("Live-build stage already completed; skipping")
        else:
            iso_path = LiveBuildStage(self.config, self.logger, self.workdir, self.repo_dir).run()
            self._write_state("livebuild")
            self._write_checksum(iso_path)

        self.logger.stage("Stage 5: Validation")
        iso_path = self.output_dir / self._latest_iso_name()
        if not iso_path.exists():
            raise RuntimeError("ISO was not produced")
        self._write_build_report(iso_path)
        self.logger.info("Build completed successfully")
        return iso_path

    def _latest_iso_name(self) -> str:
        isos = sorted(self.output_dir.glob("*.iso"))
        if not isos:
            raise RuntimeError("No ISO available in output directory")
        return isos[-1].name

    def _write_checksum(self, iso_path: Path) -> None:
        sha256 = hashlib.sha256(iso_path.read_bytes()).hexdigest()
        checksum_path = self.output_dir / f"{iso_path.stem}.sha256"
        checksum_path.write_text(f"{sha256}  {iso_path.name}\n", encoding="utf-8")

    def _write_build_report(self, iso_path: Path) -> None:
        report_path = self.output_dir / "build-report.txt"
        report_path.write_text(
            f"Tide OS build report\n"
            f"Name: {self.config.name}\n"
            f"Codename: {self.config.codename}\n"
            f"Architecture: {self.config.architecture}\n"
            f"ISO: {iso_path.name}\n",
            encoding="utf-8",
        )

    def close(self) -> None:
        self.logger.close()


def main() -> int:
    parser = argparse.ArgumentParser(description="Build Tide OS")
    parser.add_argument("--config", type=Path, default=Path(__file__).resolve().parents[1] / "config" / "tide.yaml")
    parser.add_argument("--clean", action="store_true", help="Remove previous build artifacts before starting")
    args = parser.parse_args()

    repo_dir = Path(__file__).resolve().parents[1]
    builder = TideBuilder(repo_dir)
    try:
        if args.clean:
            CleanupStage(builder.logger, builder.workdir).clean(keep_output=False)
        builder.run()
        return 0
    except Exception as exc:
        builder.logger.error(f"Build failed: {exc}")
        return 1
    finally:
        builder.close()


if __name__ == "__main__":
    sys.exit(main())
