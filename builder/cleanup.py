"""Build artifact cleanup with path safety checks."""

from __future__ import annotations

import shutil
from pathlib import Path

from builder.logger import BuildLogger
from builder.paths import BuildPaths


class CleanupStage:
    def __init__(self, logger: BuildLogger, paths: BuildPaths) -> None:
        self.logger = logger
        self.paths = paths

    def _safe_target(self, path: Path, *, allow_outside_repo: bool = False) -> bool:
        resolved = path.resolve()
        if resolved in {Path("/"), self.paths.repo, Path.home().resolve()}:
            return False
        if resolved == self.paths.work.resolve() or self.paths.work.resolve() in resolved.parents:
            return True
        if not allow_outside_repo and self.paths.repo.resolve() not in resolved.parents:
            return False
        return True

    def clean(self, keep_output: bool = False, force: bool = False) -> None:
        self.logger.stage("Cleanup")
        work = self.paths.work
        if work.exists():
            if not self._safe_target(work, allow_outside_repo=force):
                raise RuntimeError(f"refusing to remove unsafe build directory: {work}")
            self.logger.info(f"Removing build directory {work}")
            shutil.rmtree(work)
        if not keep_output and self.paths.output.exists():
            if not self._safe_target(self.paths.output):
                raise RuntimeError(f"refusing to remove unsafe output directory: {self.paths.output}")
            self.logger.info(f"Removing output directory {self.paths.output}")
            shutil.rmtree(self.paths.output)
        self.logger.info("Cleanup complete")
