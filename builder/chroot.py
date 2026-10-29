"""Stage 3: apply Tide identity and packages inside the chroot."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

from builder.branding import BrandingStage
from builder.config import TideConfig
from builder.exec import ChrootMounts, chroot_run
from builder.logger import BuildLogger
from builder.packages import PackagePlan
from builder.paths import BuildPaths


class ChrootError(RuntimeError):
    pass


class _NullMounts:
    def __enter__(self) -> "_NullMounts":
        return self

    def __exit__(self, *_: object) -> None:
        return None


class ChrootStage:
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
        self.rootfs = paths.rootfs

    def run(self) -> None:
        self.logger.stage("Stage 3: Configure Tide root filesystem")
        if not self.rootfs.is_dir() and not self.dry_run:
            raise ChrootError(f"root filesystem not found: {self.rootfs}; run 'build.sh bootstrap' first")
        if not shutil.which("chroot") and not self.dry_run:
            raise ChrootError("chroot command is not available")

        branding = BrandingStage(self.config, self.paths.repo, self.logger)
        branding.stage_rootfs(self.rootfs, self.profile, dry_run=self.dry_run)
        script_source = self.paths.repo / "rootfs-config.sh"
        if not script_source.exists():
            raise ChrootError("rootfs-config.sh is missing")
        if not self.dry_run:
            shutil.copy2(script_source, self.rootfs / "tmp/tide-rootfs-config.sh")
            (self.rootfs / "tmp/tide-rootfs-config.sh").chmod(0o755)
            branding.stage_custom_scripts(self.rootfs)

        plan = PackagePlan(self.config, self.profile)
        env = {
            "TIDE_CODENAME": self.config.codename,
            "TIDE_VERSION": self.config.version,
            "TIDE_MIRROR_MAIN": self.config.mirror_main,
            "TIDE_MIRROR_SECURITY": self.config.mirror_security,
            "TIDE_ARCHIVE_AREAS": " ".join(self.config.archive_areas),
            "TIDE_PACKAGES": " ".join(plan.install_packages()),
            "TIDE_DESKTOP_PROFILE": self.profile or "none",
            "DEBIAN_FRONTEND": "noninteractive",
        }
        with ChrootMounts(self.rootfs, self.logger) if not self.dry_run else _NullMounts():
            chroot_run(
                self.rootfs,
                ["/bin/bash", "/tmp/tide-rootfs-config.sh"],
                self.logger,
                env=env,
                dry_run=self.dry_run,
            )
            for relative in self.config.custom_scripts:
                script = f"/tmp/{Path(relative).name}"
                self.logger.info(f"Running custom script: {relative}")
                chroot_run(self.rootfs, ["/bin/bash", script], self.logger, dry_run=self.dry_run)

        if not self.dry_run:
            for relative in self.config.custom_scripts:
                (self.rootfs / "tmp" / Path(relative).name).unlink(missing_ok=True)
            (self.rootfs / "tmp/tide-rootfs-config.sh").unlink(missing_ok=True)
            self.paths.mark_done(
                "configure",
                json.dumps(
                    {
                        "profile": self.profile,
                        "packages": plan.install_packages(),
                        "identity": self.config.os_id,
                    },
                    indent=2,
                ),
            )
        self.logger.info("Chroot configuration complete")
