"""Configuration loading and validation for the Tide build system."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional

import yaml

PACKAGE_RE = re.compile(r"^[a-z0-9][a-z0-9+.:-]*$")


class ConfigError(ValueError):
    """Raised when tide.yaml is missing required data or contains unsafe values."""


class TideConfig:
    def __init__(self, data: Dict[str, Any], source: Optional[Path] = None) -> None:
        if not isinstance(data, dict):
            raise ConfigError("configuration root must be a mapping")
        self.data = data
        self.source = source

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
    def os_id(self) -> str:
        return str(self.get("os", "id", default="tide"))

    @property
    def version(self) -> str:
        return str(self.get("os", "version", default="0.1.0-alpha.1"))

    @property
    def channel(self) -> str:
        return str(self.get("release", "channel", default="development"))

    @property
    def codename(self) -> str:
        return str(self.get("os", "codename", default="bookworm"))

    @property
    def version_codename(self) -> str:
        return str(self.get("os", "version_codename", default="tidepool"))

    @property
    def foundation(self) -> str:
        return str(self.get("os", "foundation", default="debian"))

    @property
    def architecture(self) -> str:
        return str(self.get("os", "architecture", default="amd64"))

    @property
    def mirror_main(self) -> str:
        return str(self.get("mirror", "main", default="http://deb.debian.org/debian"))

    @property
    def mirror_security(self) -> str:
        return str(self.get("mirror", "security", default="http://security.debian.org/debian-security"))

    @property
    def archive_areas(self) -> List[str]:
        areas = self.get("mirror", "areas", default=["main", "contrib", "non-free", "non-free-firmware"])
        return [str(area) for area in areas]

    @property
    def base_packages(self) -> List[str]:
        return [str(package) for package in self.get("packages", "base", default=[])]

    @property
    def firmware_packages(self) -> List[str]:
        return [str(package) for package in self.get("packages", "firmware", default=[])]

    @property
    def desktop_enabled(self) -> bool:
        return bool(self.get("desktop", "enabled", default=False))

    @property
    def desktop_default(self) -> str:
        return str(self.get("desktop", "default", default="kde"))

    @property
    def desktop_profiles(self) -> Dict[str, Dict[str, Any]]:
        profiles = self.get("desktop", "profiles", default={})
        return profiles if isinstance(profiles, dict) else {}

    @property
    def installer_enabled(self) -> bool:
        return bool(self.get("installer", "enabled", default=True))

    @property
    def installer_packages(self) -> List[str]:
        return [str(package) for package in self.get("installer", "packages", default=[])]

    @property
    def custom_scripts(self) -> List[str]:
        return [str(path) for path in self.get("custom", "scripts", default=[])]

    @property
    def boot_append(self) -> str:
        return str(self.get("boot", "append", default="boot=live quiet splash"))

    @property
    def compression(self) -> str:
        return str(self.get("build", "compression", default="xz"))

    @property
    def output_name(self) -> str:
        return f"tide-{self.version}-{self.architecture}"

    def desktop_profile(self, override: Optional[str] = None) -> str:
        if override:
            return override
        if not self.desktop_enabled:
            return ""
        return self.desktop_default

    def profile_packages(self, profile: str) -> List[str]:
        if not profile or profile == "none":
            return []
        entry = self.desktop_profiles.get(profile)
        if entry is None:
            raise ConfigError(
                f"unknown desktop profile '{profile}'; available: {', '.join(sorted(self.desktop_profiles)) or 'none'}"
            )
        packages: Iterable[Any] = entry.get("packages", []) if isinstance(entry, dict) else []
        return [str(package) for package in packages]

    def packages(self, profile: str = "") -> List[str]:
        packages = self.base_packages + self.firmware_packages + self.profile_packages(profile)
        if self.installer_enabled:
            packages += self.installer_packages
        return validate_packages(packages)

    def sources_list(self) -> str:
        areas = " ".join(self.archive_areas)
        return (
            f"deb {self.mirror_main} {self.codename} {areas}\n"
            f"deb {self.mirror_main} {self.codename}-updates {areas}\n"
            f"deb {self.mirror_security} {self.codename}-security {areas}\n"
        )

    def validate(self) -> None:
        for key in ("name", "os_id", "version", "codename", "architecture"):
            if not getattr(self, key):
                raise ConfigError(f"os.{key} must not be empty")
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", self.os_id):
            raise ConfigError("os.id must contain only lowercase letters, digits, and hyphens")
        if not re.fullmatch(r"[a-z0-9-]+", self.codename):
            raise ConfigError("os.codename must contain only lowercase letters, digits, and hyphens")
        if self.architecture not in {"amd64", "arm64", "armhf", "i386", "riscv64"}:
            raise ConfigError(f"unsupported architecture: {self.architecture}")
        for label, mirror in (("main", self.mirror_main), ("security", self.mirror_security)):
            if not mirror.startswith(("http://", "https://")):
                raise ConfigError(f"mirror.{label} must be an HTTP(S) URL")
        validate_packages(self.base_packages)
        validate_packages(self.firmware_packages)
        validate_packages(self.installer_packages)
        for profile in self.desktop_profiles:
            self.profile_packages(profile)
        if not self.desktop_profiles and self.desktop_enabled:
            raise ConfigError("desktop.enabled is true but no desktop profiles are defined")
        if self.desktop_enabled and self.desktop_default not in self.desktop_profiles:
            raise ConfigError(
                f"desktop.default '{self.desktop_default}' is not defined in desktop.profiles"
            )
        for script in self.custom_scripts:
            if Path(script).is_absolute() or ".." in Path(script).parts:
                raise ConfigError(f"custom script must stay inside the repository: {script}")

    def snapshot(self) -> Dict[str, Any]:
        return json.loads(json.dumps(self.data))


def validate_packages(packages: Iterable[str]) -> List[str]:
    seen = set()
    result = []
    for package in packages:
        package = str(package).strip()
        if not package:
            continue
        if package.startswith("-") or not PACKAGE_RE.fullmatch(package):
            raise ConfigError(f"invalid package name in configuration: {package!r}")
        if package not in seen:
            seen.add(package)
            result.append(package)
    return result


def load_config(path: Optional[Path] = None) -> TideConfig:
    if path is None:
        path = Path(__file__).resolve().parents[1] / "config" / "tide.yaml"
    path = Path(path)
    if not path.exists():
        raise ConfigError(f"configuration file not found: {path}")
    try:
        with path.open("r", encoding="utf-8") as handle:
            if path.suffix in {".yaml", ".yml"}:
                data = yaml.safe_load(handle) or {}
            elif path.suffix == ".toml":
                import tomllib

                with path.open("rb") as binary:
                    data = tomllib.load(binary)
            else:
                raise ConfigError(f"unsupported configuration format: {path.suffix}")
    except (OSError, yaml.YAMLError) as exc:
        raise ConfigError(f"could not read {path}: {exc}") from exc
    config = TideConfig(data, source=path)
    config.validate()
    return config


def write_config_snapshot(config: TideConfig, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(config.snapshot(), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
