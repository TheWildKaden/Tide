"""Safe process execution helpers for build stages.

Commands are always passed as argument lists. A shell is only used inside a chroot
to execute a reviewed script that is part of this repository.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Mapping, Optional, Sequence

from builder.logger import BuildLogger


class CommandError(RuntimeError):
    """Raised when an external build command fails."""


def is_root() -> bool:
    return os.geteuid() == 0


def run(
    command: Sequence[str],
    logger: BuildLogger,
    *,
    cwd: Optional[Path] = None,
    env: Optional[Mapping[str, str]] = None,
    privileged: bool = False,
    dry_run: bool = False,
    capture: bool = False,
) -> subprocess.CompletedProcess:
    argv = [str(part) for part in command]
    if privileged and not is_root():
        argv = ["sudo", *argv]
    logger.info("Running: " + " ".join(argv))
    if dry_run:
        return subprocess.CompletedProcess(argv, 0, "", "")
    merged_env = os.environ.copy()
    if env:
        merged_env.update({str(key): str(value) for key, value in env.items()})
    try:
        completed = subprocess.run(
            argv,
            cwd=str(cwd) if cwd else None,
            env=merged_env,
            text=True,
            capture_output=capture,
            check=False,
        )
    except FileNotFoundError as exc:
        raise CommandError(f"required command not found: {argv[0]}") from exc
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()
        message = f"command failed ({completed.returncode}): {' '.join(argv)}"
        if detail:
            message += f"\n{detail}"
        raise CommandError(message)
    return completed


def require_commands(commands: Iterable[str]) -> List[str]:
    return [command for command in commands if find_command(command) is None]


def find_command(command: str) -> Optional[str]:
    """Find a command on PATH, including the standard rustup install location."""
    found = shutil.which(command)
    if found:
        return found
    candidates = [Path.home() / ".cargo" / "bin" / command]
    sudo_user = os.environ.get("SUDO_USER")
    if sudo_user:
        candidates.append(Path("/home") / sudo_user / ".cargo" / "bin" / command)
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


def chroot_run(
    rootfs: Path,
    command: Sequence[str],
    logger: BuildLogger,
    *,
    env: Optional[Mapping[str, str]] = None,
    dry_run: bool = False,
) -> None:
    run(
        ["chroot", str(rootfs), *command],
        logger,
        env=env,
        privileged=True,
        dry_run=dry_run,
    )


@dataclass
class ChrootMounts:
    """Mount the virtual filesystems required by APT and systemd tooling."""

    rootfs: Path
    logger: BuildLogger

    _mounted: List[Path] = None  # type: ignore[assignment]

    def __post_init__(self) -> None:
        if self._mounted is None:
            self._mounted = []

    def _mount(self, args: Sequence[str], target: Path) -> None:
        target.mkdir(parents=True, exist_ok=True)
        run(["mount", *args], self.logger, privileged=True, capture=True)
        self._mounted.append(target)

    def __enter__(self) -> "ChrootMounts":
        self._mount(["-t", "proc", "proc", str(self.rootfs / "proc")], self.rootfs / "proc")
        self._mount(["--rbind", "/sys", str(self.rootfs / "sys")], self.rootfs / "sys")
        self._mount(["--rbind", "/dev", str(self.rootfs / "dev")], self.rootfs / "dev")
        self._mount(["--rbind", "/run", str(self.rootfs / "run")], self.rootfs / "run")
        return self

    def __exit__(self, *_: object) -> None:
        for target in reversed(self._mounted):
            try:
                run(["umount", "-l", str(target)], self.logger, privileged=True, capture=True)
            except CommandError as exc:
                self.logger.warning(f"could not unmount {target}: {exc}")
        self._mounted.clear()
