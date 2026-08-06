from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any, Dict, List, Optional

import yaml


class TideConfig:
    def __init__(self, data: Dict[str, Any]) -> None:
        self.data = data

    def get(self, *keys: str, default: Any = None) -> Any:
        value: Any = self.data
        for key in keys:
            if isinstance(value, dict) and key in value:
                value = value[key]
            else:
                return default
        return value

    @property
    def name(self) -> str:
        return str(self.get("os", "name", default="Tide OS"))

    @property
    def codename(self) -> str:
        return str(self.get("os", "codename", default="bookworm"))

    @property
    def architecture(self) -> str:
        return str(self.get("os", "architecture", default="amd64"))

    @property
    def mirror_main(self) -> str:
        return str(self.get("mirror", "main", default="http://deb.debian.org/debian"))

    @property
    def mirror_security(self) -> str:
        return str(self.get("mirror", "security", default="http://deb.debian.org/debian-security"))

    @property
    def base_packages(self) -> List[str]:
        return list(self.get("packages", "base", default=[]))

    @property
    def desktop_enabled(self) -> bool:
        return bool(self.get("desktop", "enabled", default=False))

    @property
    def desktop_environment(self) -> str:
        return str(self.get("desktop", "environment", default="kde"))

    @property
    def branding(self) -> Dict[str, Any]:
        return dict(self.get("branding", default={}))

    @property
    def custom_scripts(self) -> List[str]:
        return list(self.get("custom", "scripts", default=[]))


def load_config(path: Optional[Path] = None) -> TideConfig:
    if path is None:
        path = Path(__file__).resolve().parents[1] / "config" / "tide.yaml"

    path = Path(path)
    if not path.exists():
        raise FileNotFoundError(f"Configuration file not found: {path}")

    with path.open("r", encoding="utf-8") as handle:
        if path.suffix in {".yaml", ".yml"}:
            data = yaml.safe_load(handle) or {}
        elif path.suffix == ".toml":
            import tomllib

            with path.open("rb") as bin_handle:
                data = tomllib.load(bin_handle)
        else:
            raise ValueError(f"Unsupported config format: {path.suffix}")

    if not isinstance(data, dict):
        raise ValueError("Configuration root must be a mapping")

    return TideConfig(data)


def write_config_snapshot(config: TideConfig, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(config.data, handle, indent=2, sort_keys=True)
