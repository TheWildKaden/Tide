"""Build logging with a stable on-disk log file."""

from __future__ import annotations

import logging
from pathlib import Path


class BuildLogger:
    def __init__(self, log_dir: Path, name: str = "tide-build", verbose: bool = False) -> None:
        self.log_dir = log_dir
        self.log_dir.mkdir(parents=True, exist_ok=True)
        self.log_file = self.log_dir / f"{name}.log"
        self.logger = logging.getLogger(name)
        self.logger.setLevel(logging.DEBUG if verbose else logging.INFO)
        self.logger.handlers.clear()
        self.logger.propagate = False

        formatter = logging.Formatter("%(asctime)s %(levelname)-7s %(message)s")

        stream_handler = logging.StreamHandler()
        stream_handler.setFormatter(logging.Formatter("%(message)s"))
        self.logger.addHandler(stream_handler)

        file_handler = logging.FileHandler(self.log_file, encoding="utf-8")
        file_handler.setFormatter(formatter)
        self.logger.addHandler(file_handler)

    def debug(self, message: str) -> None:
        self.logger.debug(message)

    def info(self, message: str) -> None:
        self.logger.info(message)

    def warning(self, message: str) -> None:
        self.logger.warning(message)

    def error(self, message: str) -> None:
        self.logger.error(message)

    def stage(self, message: str) -> None:
        self.logger.info(f"\n=== {message} ===")

    def close(self) -> None:
        for handler in self.logger.handlers:
            handler.flush()
            handler.close()
        self.logger.handlers.clear()

    @property
    def path(self) -> Path:
        return self.log_file
