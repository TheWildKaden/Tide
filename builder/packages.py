from __future__ import annotations

from typing import List


class PackagePlan:
    def __init__(self, config) -> None:
        self.config = config

    def base_packages(self) -> List[str]:
        packages = list(self.config.base_packages)
        if self.config.desktop_enabled:
            packages.extend(["task-kde-desktop"])
        return packages

    def install_packages(self) -> List[str]:
        packages = self.base_packages()
        packages.extend(["locales", "tzdata", "ca-certificates", "curl", "wget", "rsync", "net-tools"])
        return sorted(set(packages))
