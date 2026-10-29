"""Stage Tide branding, desktop defaults, installer, and first-boot assets."""

from __future__ import annotations

import os
import shutil
from pathlib import Path
from typing import Optional

from builder.config import TideConfig
from builder.logger import BuildLogger


class BrandingError(RuntimeError):
    """Raised when a required branding asset is missing."""


class BrandingStage:
    def __init__(self, config: TideConfig, repo: Path, logger: BuildLogger) -> None:
        self.config = config
        self.repo = repo
        self.assets = repo / "assets"
        self.logger = logger
        self._required = [
            self.assets / "branding/os-release",
            self.assets / "branding/issue",
            self.assets / "branding/issue.net",
            self.assets / "branding/logo/tide-logo.svg",
            self.assets / "branding/wallpaper/tide-default.svg",
            self.assets / "branding/tide.toml",
            self.assets / "firstboot/tide-firstboot.sh",
            self.assets / "firstboot/tide-firstboot.service",
        ]

    def validate(self) -> None:
        missing = [str(path.relative_to(self.repo)) for path in self._required if not path.exists()]
        if missing:
            raise BrandingError("missing branding assets: " + ", ".join(missing))

    def _copy_file(self, source: Path, destination: Path, mode: Optional[int] = None) -> None:
        if not source.exists():
            raise BrandingError(f"missing asset: {source}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        if mode is not None:
            destination.chmod(mode)

    def _copy_tree(self, source: Path, destination: Path) -> None:
        if not source.exists():
            raise BrandingError(f"missing asset directory: {source}")
        shutil.copytree(source, destination, dirs_exist_ok=True)

    def stage_rootfs(
        self,
        root: Path,
        profile: str = "kde",
        *,
        include_installer: bool = False,
        dry_run: bool = False,
    ) -> None:
        """Install Tide identity and defaults into a root filesystem or live include tree."""
        self.validate()
        if dry_run:
            self.logger.info(f"Would stage Tide branding into {root}")
            return
        root.mkdir(parents=True, exist_ok=True)

        self._copy_file(self.assets / "branding/os-release", root / "etc/os-release", 0o644)
        self._copy_file(self.assets / "branding/issue", root / "etc/issue", 0o644)
        self._copy_file(self.assets / "branding/issue.net", root / "etc/issue.net", 0o644)
        self._copy_file(self.assets / "branding/tide.toml", root / "etc/tide/tide.toml", 0o644)

        brand_dir = root / "usr/share/tide/branding"
        self._copy_file(
            self.assets / "branding/logo/tide-logo.svg",
            brand_dir / "tide-logo.svg",
            0o644,
        )
        self._copy_file(
            self.assets / "branding/logo/tide-logo.svg",
            root / "usr/share/pixmaps/tide-logo.svg",
            0o644,
        )
        self._copy_file(
            self.assets / "branding/wallpaper/tide-default.svg",
            root / "usr/share/backgrounds/tide/tide-default.svg",
            0o644,
        )
        self._copy_file(
            self.assets / "branding/os-release",
            brand_dir / "os-release",
            0o644,
        )

        self._copy_file(
            self.assets / "desktop/kde/color-schemes/Tide.colors",
            root / "usr/share/color-schemes/Tide.colors",
            0o644,
        )
        self._copy_tree(
            self.assets / "desktop/plasma/look-and-feel/org.tide.desktop",
            root / "usr/share/plasma/look-and-feel/org.tide.desktop",
        )
        if profile == "kde":
            self._copy_tree(
                self.assets / "desktop/kde/skel",
                root / "etc/skel",
            )
        elif profile == "xfce":
            xfce_skel = self.assets / "desktop/xfce/skel"
            if xfce_skel.exists():
                self._copy_tree(xfce_skel, root / "etc/skel")
            else:
                self.logger.warning("XFCE profile has no Tide skel defaults yet; using package defaults")

        self._copy_file(
            self.assets / "firstboot/tide-firstboot.sh",
            root / "usr/lib/tide/tide-firstboot.sh",
            0o755,
        )
        self._copy_file(
            self.assets / "firstboot/tide-firstboot.service",
            root / "usr/lib/systemd/system/tide-firstboot.service",
            0o644,
        )
        wants = root / "etc/systemd/system/multi-user.target.wants"
        wants.mkdir(parents=True, exist_ok=True)
        link = wants / "tide-firstboot.service"
        if not link.exists():
            link.symlink_to("/usr/lib/systemd/system/tide-firstboot.service")

        if include_installer and self.config.installer_enabled:
            self._copy_tree(self.assets / "calamares", root / "etc/calamares")
        self.logger.info(f"Staged Tide branding into {root}")

    def stage_live_includes(self, live_dir: Path, profile: str, dry_run: bool = False) -> None:
        includes = live_dir / "config/includes.chroot"
        self.stage_rootfs(
            includes,
            profile,
            include_installer=True,
            dry_run=dry_run,
        )

    def stage_custom_scripts(self, rootfs: Path) -> None:
        """Copy repository customization scripts into the chroot for the configure stage."""
        for relative in self.config.custom_scripts:
            source = (self.repo / relative).resolve()
            if not source.is_file() or self.repo not in source.parents:
                raise BrandingError(f"custom script is missing or outside the repository: {relative}")
            destination = rootfs / "tmp" / Path(relative).name
            self._copy_file(source, destination, 0o755)


def ensure_directory(path: Path, mode: int = 0o755) -> None:
    path.mkdir(parents=True, exist_ok=True)
    os.chmod(path, mode)
