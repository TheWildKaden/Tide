"""Build directory layout for Tide images."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

WORKDIR_ENV = "TIDE_BUILD_DIR"


@dataclass(frozen=True)
class BuildPaths:
    repo: Path
    work: Path
    output: Path

    @classmethod
    def resolve(cls, repo: Path, override: Optional[Path] = None) -> "BuildPaths":
        repo = repo.resolve()
        if override is None:
            env_value = os.environ.get(WORKDIR_ENV)
            override = Path(env_value) if env_value else repo / ".tide-build"
        work = Path(override).expanduser()
        if not work.is_absolute():
            work = (repo / work).resolve()
        return cls(repo=repo, work=work, output=repo / "output")

    @property
    def rootfs(self) -> Path:
        return self.work / "rootfs"

    @property
    def logs(self) -> Path:
        return self.work / "logs"

    @property
    def state(self) -> Path:
        return self.work / "state"

    @property
    def live(self) -> Path:
        return self.work / "live-build"

    @property
    def manifests(self) -> Path:
        return self.output / "manifests"

    @property
    def assets(self) -> Path:
        return self.repo / "assets"

    @property
    def live_source(self) -> Path:
        return self.repo / "live-build"

    def ensure(self) -> None:
        for directory in (self.work, self.logs, self.state, self.output):
            directory.mkdir(parents=True, exist_ok=True)

    def state_file(self, stage: str) -> Path:
        return self.state / f"{stage}.json"

    def is_done(self, stage: str) -> bool:
        return self.state_file(stage).exists()

    def mark_done(self, stage: str, payload: str = "{}") -> None:
        self.state.mkdir(parents=True, exist_ok=True)
        self.state_file(stage).write_text(payload + "\n", encoding="utf-8")
