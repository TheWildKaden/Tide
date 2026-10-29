"""Release manifests, checksums, and verification."""

from __future__ import annotations

import hashlib
import json
import platform
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional

from builder.config import TideConfig
from builder.paths import BuildPaths

MANIFEST_SCHEMA = 1
CHUNK = 1024 * 1024


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(CHUNK), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _git(repo: Path, *args: str) -> Optional[str]:
    try:
        completed = subprocess.run(
            ["git", "-C", str(repo), *args],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if completed.returncode != 0:
        return None
    return completed.stdout.strip()


def _tool_version(command: str) -> Optional[str]:
    path = shutil.which(command)
    if not path:
        return None
    for args in (["--version"], ["version"]):
        try:
            completed = subprocess.run(
                [path, *args], capture_output=True, text=True, timeout=15, check=False
            )
        except (OSError, subprocess.TimeoutExpired):
            continue
        lines = (completed.stdout or completed.stderr).strip().splitlines()
        if lines:
            return lines[0].strip()
    return path


def generate_manifest(
    config: TideConfig,
    paths: BuildPaths,
    iso: Path,
    *,
    profile: str = "",
    boot_tested: bool = False,
    hardware_tested: bool = False,
) -> Dict[str, object]:
    if not iso.exists():
        raise FileNotFoundError(f"artifact does not exist: {iso}")
    paths.manifests.mkdir(parents=True, exist_ok=True)
    git_commit = _git(paths.repo, "rev-parse", "HEAD")
    git_dirty = bool(_git(paths.repo, "status", "--porcelain"))
    state = {}
    for stage in ("bootstrap", "configure", "livebuild"):
        state_path = paths.state_file(stage)
        if state_path.exists():
            try:
                state[stage] = json.loads(state_path.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                state[stage] = {"unreadable": True}

    artifact = {
        "name": iso.name,
        "path": iso.name,
        "size": iso.stat().st_size,
        "sha256": sha256_file(iso),
    }
    manifest: Dict[str, object] = {
        "schema_version": MANIFEST_SCHEMA,
        "product": config.name,
        "version": config.version,
        "channel": config.channel,
        "architecture": config.architecture,
        "version_codename": config.version_codename,
        "foundation": {
            "distribution": "Debian",
            "codename": config.codename,
            "mirror": config.mirror_main,
            "image_builder": "live-build",
            "debootstrap": _tool_version("debootstrap"),
            "live_build": _tool_version("lb"),
        },
        "build": {
            "timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "host_architecture": platform.machine(),
            "python": platform.python_version(),
            "git_commit": git_commit,
            "git_dirty": git_dirty,
            "build_dir": str(paths.work),
        },
        "profile": profile or "none",
        "packages": config.packages(profile),
        "stages": state,
        "artifacts": [artifact],
        "validation": {
            "compiled": True,
            "iso_generated": True,
            "linux_units_tested": False,
            "boot_tested": boot_tested,
            "installer_tested": False,
            "hardware_tested": hardware_tested,
            "notes": (
                "An ISO existing does not prove that it boots or installs. "
                "Boot and hardware validation must be performed separately."
            ),
        },
    }
    manifest_path = paths.manifests / f"{iso.stem}.manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    checksum_path = iso.with_suffix(iso.suffix + ".sha256")
    checksum_path.write_text(f"{artifact['sha256']}  {iso.name}\n", encoding="utf-8")

    sums = paths.output / "SHA256SUMS"
    lines = [f"{artifact['sha256']}  {iso.name}"]
    manifest_sum = sha256_file(manifest_path)
    lines.append(f"{manifest_sum}  manifests/{manifest_path.name}")
    sums.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return manifest


def parse_checksum_file(path: Path) -> List[tuple[str, str]]:
    entries = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 2:
            raise ValueError(f"malformed checksum line in {path}: {line}")
        digest, name = parts[0], parts[-1].lstrip("*")
        if not name or "/" in name or "\\" in name or name in {".", ".."}:
            # Allow manifests/ prefix only.
            if not (name.startswith("manifests/") and ".." not in name.split("/")):
                raise ValueError(f"unsafe checksum target in {path}: {name}")
        if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest.lower()):
            raise ValueError(f"invalid SHA-256 digest in {path}: {digest}")
        entries.append((digest, name))
    return entries


def verify(paths: BuildPaths, checksum_file: Optional[Path] = None) -> List[str]:
    """Return a list of verification errors. An empty list means verification passed."""
    errors: List[str] = []
    files: Iterable[Path]
    if checksum_file:
        files = [checksum_file]
    else:
        files = sorted(paths.output.glob("*.sha256")) + (
            [paths.output / "SHA256SUMS"] if (paths.output / "SHA256SUMS").exists() else []
        )
    files = list(files)
    if not files:
        return [f"no checksum files found in {paths.output}"]
    for file in files:
        try:
            entries = parse_checksum_file(file)
        except (OSError, ValueError) as exc:
            errors.append(str(exc))
            continue
        for digest, name in entries:
            target = paths.output / name
            if not target.is_file():
                errors.append(f"{file.name}: missing file {name}")
                continue
            actual = sha256_file(target)
            if actual != digest.lower():
                errors.append(f"{file.name}: checksum mismatch for {name}")
    return errors
