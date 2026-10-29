"""Stage 4: build a hybrid live ISO with Debian live-build.

live-build is intentionally isolated in this stage. It is the current image builder,
not the permanent Tide image architecture.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from pathlib import Path

from builder.branding import BrandingStage
from builder.config import TideConfig
from builder.exec import run
from builder.logger import BuildLogger
from builder.packages import PackagePlan
from builder.paths import BuildPaths
from builder.rusttools import RustToolsStage


class LiveBuildError(RuntimeError):
    pass


def _tool_version(command: str) -> str:
    path = shutil.which(command)
    if not path:
        return "missing"
    try:
        completed = subprocess.run(
            [path, "--version"],
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return "unknown"
    lines = (completed.stdout or completed.stderr).strip().splitlines()
    return lines[0].strip() if lines else "unknown"


class LiveBuildStage:
    def __init__(
        self,
        config: TideConfig,
        logger: BuildLogger,
        paths: BuildPaths,
        profile: str = "",
        *,
        dry_run: bool = False,
    ) -> None:
        self.config = config
        self.logger = logger
        self.paths = paths
        self.profile = profile
        self.dry_run = dry_run
        self.live_dir = paths.live
        self.iso_name = f"{config.output_name}.iso"

    def _prepare_tree(self) -> None:
        source = self.paths.live_source
        if not source.is_dir():
            raise LiveBuildError(f"live-build configuration directory not found: {source}")
        if self.live_dir.exists():
            shutil.rmtree(self.live_dir)
        shutil.copytree(source, self.live_dir)
        for directory in ("config/includes.chroot", "config/package-lists", "config/hooks/live"):
            (self.live_dir / directory).mkdir(parents=True, exist_ok=True)

        plan = PackagePlan(self.config, self.profile)
        package_list = self.live_dir / "config/package-lists/tide.list.chroot"
        package_list.write_text(plan.package_list_text(), encoding="utf-8")

        branding = BrandingStage(self.config, self.paths.repo, self.logger)
        branding.stage_live_includes(self.live_dir, self.profile, dry_run=self.dry_run)
        RustToolsStage(self.paths.repo, self.logger, dry_run=self.dry_run).stage(self.live_dir / "config/includes.chroot")

    def _config_command(self) -> list[str]:
        return [
            "lb",
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
            "--mirror-chroot-security",
            self.config.mirror_security,
            "--bootstrap",
            "debootstrap",
            "--archive-areas",
            " ".join(self.config.archive_areas),
            "--binary-images",
            "iso-hybrid",
            "--compression",
            self.config.compression,
            "--bootappend-live",
            self.config.boot_append,
            "--debian-installer",
            "none",
            "--iso-application",
            "Tide OS",
            "--iso-publisher",
            "Tide OS project; https://github.com/Tide-OS/Tide",
            "--iso-volume",
            f"Tide OS {self.config.version} {self.config.architecture}",
            "--linux-flavours",
            self._linux_flavour(),
        ]

    def _linux_flavour(self) -> str:
        return {"amd64": "amd64", "arm64": "arm64", "armhf": "armmp", "i386": "686", "riscv64": "riscv64"}.get(
            self.config.architecture,
            self.config.architecture,
        )

    def run(self) -> Path:
        self.logger.stage("Stage 4: Build live ISO")
        if not self.dry_run and shutil.which("lb") is None:
            raise LiveBuildError("live-build is not installed")
        if not self.dry_run and os.geteuid() != 0 and shutil.which("sudo") is None:
            raise LiveBuildError("live-build requires root and sudo is not available")

        self._prepare_tree()
        config_command = self._config_command()
        self.logger.info("Configuring live-build")
        run(config_command, self.logger, cwd=self.live_dir, privileged=True, dry_run=self.dry_run)

        env = {
            "SOURCE_DATE_EPOCH": os.environ.get("SOURCE_DATE_EPOCH", self._source_date_epoch()),
            "TIDE_VERSION": self.config.version,
        }
        self.logger.info("Running live-build (this can take a long time)")
        started = time.time()
        run(["lb", "build"], self.logger, cwd=self.live_dir, env=env, privileged=True, dry_run=self.dry_run)
        elapsed = time.time() - started

        if self.dry_run:
            self.logger.info("Dry run complete; no ISO was produced")
            self.logger.info(f"Build log: {self.logger.path}")
            return self.paths.output / self.iso_name

        candidates = sorted(
            self.live_dir.glob("**/*.iso"),
            key=lambda path: path.stat().st_mtime,
            reverse=True,
        )
        if not candidates:
            raise LiveBuildError(
                "live-build completed without producing an ISO; "
                f"inspect {self.logger.path} and {self.live_dir / 'build.log'}"
            )
        produced = candidates[0]
        destination = self.paths.output / self.iso_name
        self.paths.output.mkdir(parents=True, exist_ok=True)
        self.logger.info(f"Copying {produced} -> {destination}")
        shutil.copy2(produced, destination)

        metadata = {
            "source_iso": produced.name,
            "iso": destination.name,
            "seconds": round(elapsed, 1),
            "live_build": _tool_version("lb"),
            "debootstrap": _tool_version("debootstrap"),
            "profile": self.profile,
            "packages": PackagePlan(self.config, self.profile).install_packages(),
        }
        self.paths.mark_done("livebuild", json.dumps(metadata, indent=2))
        self.logger.info(f"ISO created at {destination}")
        self.logger.info(f"Build log: {self.logger.path}")
        return destination

    def _source_date_epoch(self) -> str:
        try:
            completed = subprocess.run(
                ["git", "-C", str(self.paths.repo), "log", "-1", "--format=%ct"],
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
            value = completed.stdout.strip()
            if value.isdigit():
                return value
        except (OSError, subprocess.TimeoutExpired):
            pass
        return str(int(time.time()))
