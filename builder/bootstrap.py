"""Stage 2: bootstrap a Debian foundation with debootstrap."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

from builder.config import TideConfig
from builder.exec import run
from builder.logger import BuildLogger
from builder.packages import PackagePlan
from builder.paths import BuildPaths


class BootstrapError(RuntimeError):
    pass


class BootstrapStage:
    def __init__(
        self,
        config: TideConfig,
        logger: BuildLogger,
        paths: BuildPaths,
        profile: str = "",
        *,
        force: bool = False,
        dry_run: bool = False,
    ) -> None:
        self.config = config
        self.logger = logger
        self.paths = paths
        self.profile = profile
        self.force = force
        self.dry_run = dry_run
        self.rootfs = paths.rootfs

    def _prepare_rootfs(self) -> None:
        if self.rootfs.exists() and any(self.rootfs.iterdir()):
            if not self.force:
                raise BootstrapError(
                    f"{self.rootfs} already contains a root filesystem; "
                    "use --force to replace it or run 'build.sh clean'"
                )
            self._safe_remove(self.rootfs)
        self.rootfs.mkdir(parents=True, exist_ok=True)

    def _safe_remove(self, path: Path) -> None:
        resolved = path.resolve()
        work = self.paths.work.resolve()
        if resolved == work or work not in resolved.parents:
            raise BootstrapError(f"refusing to remove path outside build directory: {resolved}")
        shutil.rmtree(resolved)

    def run(self) -> Path:
        self.logger.stage("Stage 2: Bootstrap Debian foundation")
        if not shutil.which("debootstrap") and not self.dry_run:
            raise BootstrapError("debootstrap is not installed")
        self._prepare_rootfs()

        plan = PackagePlan(self.config, self.profile)
        packages = ",".join(plan.bootstrap_packages())
        components = ",".join(self.config.archive_areas)
        command = [
            "debootstrap",
            f"--arch={self.config.architecture}",
            "--variant=minbase",
            f"--components={components}",
            f"--include={packages}",
            self.config.codename,
            str(self.rootfs),
            self.config.mirror_main,
        ]
        self.logger.info(
            f"Bootstrapping Debian {self.config.codename} ({self.config.architecture}) into {self.rootfs}"
        )
        run(command, self.logger, privileged=True, dry_run=self.dry_run)

        if not self.dry_run:
            sources = self.rootfs / "etc/apt/sources.list"
            sources.parent.mkdir(parents=True, exist_ok=True)
            sources.write_text(self.config.sources_list(), encoding="utf-8")
            self.paths.mark_done(
                "bootstrap",
                json.dumps(
                    {
                        "codename": self.config.codename,
                        "architecture": self.config.architecture,
                        "mirror": self.config.mirror_main,
                        "packages": plan.bootstrap_packages(),
                    },
                    indent=2,
                ),
            )
        self.logger.info("Bootstrap complete")
        return self.rootfs
