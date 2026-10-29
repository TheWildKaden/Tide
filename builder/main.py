"""Tide OS build orchestrator.

Primary command:
    ./build.sh all

Stages:
    doctor      environment and dependency checks
    bootstrap   debootstrap a Debian foundation
    configure   apply Tide identity, packages, users, and defaults
    build-iso   generate a live ISO with Debian live-build
    manifest    write/refresh checksums and a release manifest
    verify      verify generated artifacts
    clean       remove local build artifacts
"""

from __future__ import annotations

import argparse
import json
import sys
import traceback
from pathlib import Path
from typing import Optional

from builder.bootstrap import BootstrapStage
from builder.branding import BrandingStage
from builder.chroot import ChrootStage
from builder.cleanup import CleanupStage
from builder.config import ConfigError, load_config, write_config_snapshot
from builder.environment import checks_as_json, doctor, require_stage_dependencies
from builder.livebuild import LiveBuildStage
from builder.logger import BuildLogger
from builder.manifest import generate_manifest, verify
from builder.paths import BuildPaths

REPO = Path(__file__).resolve().parents[1]


class TideBuilder:
    def __init__(self, args: argparse.Namespace) -> None:
        self.args = args
        self.config = load_config(args.config)
        self.paths = BuildPaths.resolve(REPO, args.build_dir)
        self.paths.ensure()
        self.logger = BuildLogger(self.paths.logs, verbose=args.verbose)
        self.profile = self.config.desktop_profile(args.desktop)
        write_config_snapshot(self.config, self.paths.state / "config.json")

    def close(self) -> None:
        self.logger.close()

    def _require(self, stage: str) -> None:
        require_stage_dependencies(stage)

    def bootstrap(self) -> None:
        self._require("bootstrap")
        BootstrapStage(
            self.config,
            self.logger,
            self.paths,
            self.profile,
            force=self.args.force,
            dry_run=self.args.dry_run,
        ).run()

    def configure(self) -> None:
        self._require("configure")
        ChrootStage(
            self.config,
            self.logger,
            self.paths,
            self.profile,
            dry_run=self.args.dry_run,
        ).run()

    def build_iso(self) -> Path:
        self._require("iso")
        if self.paths.is_done("configure"):
            branding = BrandingStage(self.config, self.paths.repo, self.logger)
            branding.validate()
        return LiveBuildStage(
            self.config,
            self.logger,
            self.paths,
            self.profile,
            dry_run=self.args.dry_run,
        ).run()

    def manifest(self, iso: Optional[Path] = None) -> dict:
        iso = iso or self._latest_iso()
        manifest = generate_manifest(self.config, self.paths, iso, profile=self.profile)
        self.logger.info(f"Manifest: {self.paths.manifests / (iso.stem + '.manifest.json')}")
        self.logger.info(f"Checksum: {iso}.sha256")
        return manifest

    def _latest_iso(self) -> Path:
        isos = sorted(
            self.paths.output.glob("*.iso"),
            key=lambda path: path.stat().st_mtime,
            reverse=True,
        )
        if not isos:
            raise RuntimeError(f"no ISO found in {self.paths.output}; run 'build.sh build-iso' first")
        return isos[0]

    def all(self) -> None:
        self.logger.stage("Tide OS build")
        self.logger.info(
            f"{self.config.name} {self.config.version} | Debian {self.config.codename} "
            f"| {self.config.architecture} | profile={self.profile or 'none'}"
        )
        for check in doctor(self.config, self.paths):
            if check.level == "error":
                raise RuntimeError(f"{check.area}: {check.message} ({check.remediation or ''})".strip())
            if check.level in {"warning", "info"}:
                self.logger.info(f"{check.level}: {check.area}: {check.message}")

        if self.args.dry_run or not self.paths.is_done("bootstrap") or self.args.force:
            self.bootstrap()
        else:
            self.logger.info("Bootstrap stage already complete; skipping (use --force to rebuild)")
        if self.args.dry_run or not self.paths.is_done("configure") or self.args.force:
            self.configure()
        else:
            self.logger.info("Configure stage already complete; skipping (use --force to rebuild)")

        iso = self.build_iso()
        if self.args.dry_run:
            self.logger.info("Dry run complete: no ISO was generated and no manifest was written")
            return
        manifest = self.manifest(iso)
        self._print_summary(iso, manifest)

    def _print_summary(self, iso: Path, manifest: dict) -> None:
        self.logger.stage("Build summary")
        self.logger.info(f"ISO:       {iso}")
        self.logger.info(f"Size:      {iso.stat().st_size / 1024**3:.2f} GiB")
        self.logger.info(f"SHA-256:   {manifest['artifacts'][0]['sha256']}")
        self.logger.info(f"Manifest:  {self.paths.manifests / (iso.stem + '.manifest.json')}")
        self.logger.info(f"Log:       {self.logger.path}")
        self.logger.info(
            "Validation: ISO generation only. Boot, installer, and hardware behaviour "
            "have NOT been tested by this command."
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="build.sh",
        description="Build Tide OS images from a Debian foundation.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="The environment cannot prove that an ISO boots. Validate images in a VM or on hardware.",
    )
    parser.add_argument("--config", type=Path, default=REPO / "config/tide.yaml", help="configuration file")
    parser.add_argument(
        "--build-dir",
        type=Path,
        default=None,
        help="working directory (default: $TIDE_BUILD_DIR or .tide-build)",
    )
    parser.add_argument("--desktop", choices=["kde", "xfce", "minimal", "none"], default=None)
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument("--dry-run", action="store_true", help="print commands without changing the system")

    subparsers = parser.add_subparsers(dest="command", required=True)

    doctor_parser = subparsers.add_parser("doctor", help="check the build environment")
    doctor_parser.add_argument("--json", action="store_true")
    doctor_parser.add_argument("--stage", choices=["bootstrap", "configure", "iso"], default=None)

    bootstrap_parser = subparsers.add_parser("bootstrap", help="create the Debian root filesystem")
    bootstrap_parser.add_argument("--force", action="store_true", help="replace an existing rootfs")

    subparsers.add_parser("configure", help="apply Tide configuration in the chroot")

    iso_parser = subparsers.add_parser("build-iso", help="build a live ISO")
    iso_parser.add_argument("--force", action="store_true", help="accepted for consistency; ISO always rebuilds")

    all_parser = subparsers.add_parser("all", help="bootstrap, configure, build ISO, and write a manifest")
    all_parser.add_argument("--force", action="store_true", help="rebuild bootstrap and configure stages")

    clean_parser = subparsers.add_parser("clean", help="remove build artifacts")
    clean_parser.add_argument("--keep-output", action="store_true", help="keep generated images")
    clean_parser.add_argument("--force", action="store_true", help="allow removing an external build directory")

    manifest_parser = subparsers.add_parser("manifest", help="write checksums and a release manifest")
    manifest_parser.add_argument("--iso", type=Path, default=None)

    verify_parser = subparsers.add_parser("verify", help="verify generated artifacts")
    verify_parser.add_argument("--checksum", type=Path, default=None)

    subparsers.add_parser("version", help="print the Tide build-system version")
    return parser


def main(argv: Optional[list[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command == "version":
        version_file = REPO / "VERSION"
        print(version_file.read_text(encoding="utf-8").strip() if version_file.exists() else "unknown")
        return 0

    builder: Optional[TideBuilder] = None
    try:
        if args.command == "doctor":
            config = load_config(args.config)
            paths = BuildPaths.resolve(REPO, args.build_dir)
            checks = doctor(config, paths, args.stage)
            if args.json:
                print(checks_as_json(checks))
            else:
                print(f"Tide build doctor: {config.name} {config.version}")
                for check in checks:
                    print(f"[{check.level:<7}] {check.area:<12} {check.message}")
                    if check.remediation:
                        print(f"          fix: {check.remediation}")
            return 1 if any(check.level == "error" for check in checks) else 0

        builder = TideBuilder(args)
        if args.command == "bootstrap":
            builder.bootstrap()
        elif args.command == "configure":
            builder.configure()
        elif args.command == "build-iso":
            iso = builder.build_iso()
            if not args.dry_run:
                builder.manifest(iso)
        elif args.command == "all":
            builder.all()
        elif args.command == "manifest":
            builder.manifest(args.iso)
        elif args.command == "verify":
            errors = verify(builder.paths, args.checksum)
            if errors:
                for error in errors:
                    builder.logger.error(error)
                return 1
            builder.logger.info("All checksums verified")
        elif args.command == "clean":
            CleanupStage(builder.logger, builder.paths, dry_run=args.dry_run).clean(
                args.keep_output, args.force
            )
        return 0
    except (ConfigError, RuntimeError, FileNotFoundError, ValueError) as exc:
        if builder:
            builder.logger.error(f"Build failed: {exc}")
            if args.verbose:
                builder.logger.debug(traceback.format_exc())
        else:
            print(f"build.sh: {exc}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("\nInterrupted.", file=sys.stderr)
        return 130
    finally:
        if builder:
            builder.close()


if __name__ == "__main__":
    sys.exit(main())
