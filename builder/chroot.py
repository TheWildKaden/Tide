"""Stage 3: apply Tide identity and packages inside the chroot."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

from builder.branding import BrandingStage
from builder.config import TideConfig
from builder.exec import ChrootMounts, chroot_run, remove_path, run
from builder.logger import BuildLogger
from builder.packages import PackagePlan
from builder.paths import BuildPaths
from builder.rusttools import RustToolsStage


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

        script_source = self.paths.repo / "scripts/rootfs-config.sh"
        if not script_source.exists():
            raise ChrootError("scripts/rootfs-config.sh is missing")

        # debootstrap produces a root-owned rootfs. Assemble everything Tide adds
        # in a user-owned staging tree, then rsync it in as root so file ownership
        # stays correct and no unprivileged write touches the rootfs.
        staging = self.paths.work / "staging" / "rootfs-overlay"
        remove_path(staging, self.logger, dry_run=self.dry_run)
        if not self.dry_run:
            staging.mkdir(parents=True, exist_ok=True)

        branding = BrandingStage(self.config, self.paths.repo, self.logger)
        branding.stage_rootfs(staging, self.profile, dry_run=self.dry_run)
        RustToolsStage(self.paths.repo, self.logger, dry_run=self.dry_run).stage(staging)
        if not self.dry_run:
            install_dir = staging / "tmp"
            install_dir.mkdir(parents=True, exist_ok=True)
            install_dir.chmod(0o1777)
            shutil.copy2(script_source, install_dir / "tide-rootfs-config.sh")
            (install_dir / "tide-rootfs-config.sh").chmod(0o755)
            branding.stage_custom_scripts(staging)
            run(
                ["rsync", "-a", "--chown=root:root", f"{staging}/", f"{self.rootfs}/"],
                self.logger,
                privileged=True,
            )

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
            remove_path(staging, self.logger)
            # Do not ship build-time scripts inside the image's /tmp.
            remove_path(self.rootfs / "tmp/tide-rootfs-config.sh", self.logger)
            for relative in self.config.custom_scripts:
                remove_path(self.rootfs / "tmp" / Path(relative).name, self.logger)
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
