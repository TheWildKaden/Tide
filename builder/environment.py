"""Environment diagnostics for building Tide images."""

from __future__ import annotations

import json
import platform
import shutil
import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable, List, Optional

from builder.config import TideConfig
from builder.paths import BuildPaths

STAGE_COMMANDS = {
    "bootstrap": ["debootstrap", "chroot", "mount", "umount"],
    "configure": ["cargo", "chroot", "mount", "umount"],
    "iso": ["cargo", "lb", "debootstrap", "grub-mkrescue", "rsync", "mount", "umount", "sha256sum"],
}

MIN_FREE_BYTES = 8 * 1024**3


@dataclass
class Check:
    level: str
    area: str
    message: str
    remediation: Optional[str] = None


def _command_version(command: str) -> Optional[str]:
    path = shutil.which(command)
    if not path:
        return None
    for args in (["--version"], ["version"]):
        try:
            completed = subprocess.run(
                [path, *args],
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired):
            continue
        line = (completed.stdout or completed.stderr).strip().splitlines()
        if line:
            return line[0].strip()
    return path


def doctor(
    config: TideConfig,
    paths: BuildPaths,
    stage: Optional[str] = None,
) -> List[Check]:
    checks: List[Check] = []
    checks.append(
        Check(
            "ok" if config.os_id == "tide" else "warning",
            "identity",
            f"{config.name} {config.version} (ID={config.os_id}, foundation={config.foundation}, Debian {config.codename})",
        )
    )
    checks.append(Check("info", "platform", f"{platform.system()} {platform.machine()} ({platform.release()})"))
    checks.append(Check("info", "python", platform.python_version()))

    if platform.machine().lower() in {"x86_64", "amd64"}:
        host_arch = "amd64"
    elif platform.machine().lower() in {"aarch64", "arm64"}:
        host_arch = "arm64"
    else:
        host_arch = platform.machine().lower()
    if host_arch != config.architecture:
        checks.append(
            Check(
                "warning",
                "architecture",
                f"target is {config.architecture} but host is {host_arch}; cross-bootstrap may require qemu",
                "Build on a matching architecture or install binfmt/qemu support",
            )
        )
    else:
        checks.append(Check("ok", "architecture", f"target architecture matches host ({host_arch})"))

    if paths.work == Path("/"):
        checks.append(Check("error", "build-dir", "build directory resolves to /", "Use TIDE_BUILD_DIR or --build-dir"))
    else:
        checks.append(Check("info", "build-dir", str(paths.work)))

    try:
        usage = shutil.disk_usage(paths.work if paths.work.exists() else paths.repo)
        free = usage.free
        if free < MIN_FREE_BYTES:
            checks.append(
                Check(
                    "warning",
                    "disk",
                    f"{free / 1024**3:.1f} GiB free; ISO builds usually need several GiB",
                    "Free disk space or choose another build directory",
                )
            )
        else:
            checks.append(Check("ok", "disk", f"{free / 1024**3:.1f} GiB free"))
    except OSError as exc:
        checks.append(Check("warning", "disk", f"could not inspect free space: {exc}"))

    stages = [stage] if stage else ["bootstrap", "configure", "iso"]
    required = sorted({command for name in stages for command in STAGE_COMMANDS.get(name, [])})
    for command in required:
        version = _command_version(command)
        if version:
            checks.append(Check("ok", "dependency", f"{command}: {version}"))
        else:
            checks.append(
                Check(
                    "error",
                    "dependency",
                    f"missing build dependency: {command}",
                    "Install Debian live-build and debootstrap, or run scripts/install-dev-deps.sh",
                )
            )

    if config.installer_enabled and not shutil.which("lb"):
        checks.append(
            Check(
                "warning",
                "installer",
                "Calamares configuration will be staged, but the Calamares package is only available inside the built image",
            )
        )

    checks.append(
        Check(
            "ok" if _is_root() else "info",
            "privileges",
            "running as root" if _is_root() else "running as a normal user; stages that need root will use sudo",
        )
    )
    return checks


def require_stage_dependencies(stage: str) -> None:
    missing = [command for command in STAGE_COMMANDS.get(stage, []) if not shutil.which(command)]
    if missing:
        raise RuntimeError(
            "missing required commands: "
            + ", ".join(missing)
            + ". Install live-build/debootstrap or run scripts/install-dev-deps.sh"
        )
    if not _is_root():
        # sudo is required for chroot/live-build operations.
        if not shutil.which("sudo"):
            raise RuntimeError("this stage requires root and sudo is not available")


def _is_root() -> bool:
    import os

    return os.geteuid() == 0


def checks_as_json(checks: Iterable[Check]) -> str:
    return json.dumps([asdict(check) for check in checks], indent=2)
