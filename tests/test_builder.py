"""Tests for the Tide build system. These do not require root or an ISO build."""

from __future__ import annotations

import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from builder.branding import BrandingStage
from builder.cleanup import CleanupStage
from builder.config import ConfigError, TideConfig, load_config, validate_packages
from builder.environment import doctor
from builder.exec import run
from builder.logger import BuildLogger
from builder.manifest import generate_manifest, parse_checksum_file, sha256_file, verify
from builder.packages import PackagePlan
from builder.paths import BuildPaths

REPO = Path(__file__).resolve().parents[1]


def isolated_paths(temp: str) -> BuildPaths:
    """Build paths entirely under a temporary directory.

    `BuildPaths.output` normally points at the repository-level `output/`
    directory, so tests must override it to avoid touching real artifacts.
    """
    paths = replace(
        BuildPaths.resolve(REPO, Path(temp) / "build"),
        output=Path(temp) / "output",
    )
    paths.ensure()
    return paths


class ConfigTests(unittest.TestCase):
    def test_repository_config_is_valid(self) -> None:
        config = load_config(REPO / "config/tide.yaml")
        self.assertEqual(config.os_id, "tide")
        self.assertEqual(config.foundation, "debian")
        self.assertEqual(config.codename, "bookworm")
        self.assertIn("kde", config.desktop_profiles)
        self.assertEqual(config.desktop_profile(), "kde")

    def test_profiles_select_packages(self) -> None:
        config = load_config(REPO / "config/tide.yaml")
        kde = config.packages("kde")
        xfce = config.packages("xfce")
        none = config.packages("none")
        self.assertIn("kde-plasma-desktop", kde)
        self.assertIn("task-xfce-desktop", xfce)
        self.assertNotIn("task-xfce-desktop", kde)
        self.assertFalse(any("desktop" in package for package in none))
        self.assertEqual(len(kde), len(set(kde)))

    def test_invalid_package_is_rejected(self) -> None:
        with self.assertRaises(ConfigError):
            validate_packages(["--allow-unauthenticated"])
        with self.assertRaises(ConfigError):
            validate_packages(["foo bar"])

    def test_unknown_profile_fails(self) -> None:
        config = load_config(REPO / "config/tide.yaml")
        with self.assertRaises(ConfigError):
            config.profile_packages("gnome")

    def test_installer_can_be_disabled(self) -> None:
        config = load_config(REPO / "config/tide.yaml")
        data = config.snapshot()
        data["installer"]["enabled"] = False
        disabled = TideConfig(data)
        disabled.validate()
        self.assertNotIn("calamares", disabled.packages("none"))


class PackagePlanTests(unittest.TestCase):
    def test_plan_is_deterministic_and_unique(self) -> None:
        config = load_config(REPO / "config/tide.yaml")
        plan = PackagePlan(config, "kde")
        packages = plan.install_packages()
        self.assertEqual(packages, list(dict.fromkeys(packages)))
        self.assertIn("systemd", plan.bootstrap_packages())
        self.assertIn("kde-plasma-desktop", packages)
        text = plan.package_list_text()
        self.assertIn("# Profile: kde", text)
        self.assertIn("kde-plasma-desktop", text)


class BrandingTests(unittest.TestCase):
    def test_branding_stages_identity_and_installer(self) -> None:
        config = load_config(REPO / "config/tide.yaml")
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            logger = BuildLogger(temp_path / "logs", "branding-test")
            stage = BrandingStage(config, REPO, logger)
            root = temp_path / "rootfs"
            stage.stage_rootfs(root, "kde", include_installer=True)
            os_release = (root / "etc/os-release").read_text(encoding="utf-8")
            self.assertIn("ID=tide", os_release)
            self.assertTrue((root / "usr/share/backgrounds/tide/tide-default.svg").exists())
            self.assertTrue((root / "usr/share/plasma/look-and-feel/org.tide.desktop/metadata.json").exists())
            self.assertTrue((root / "etc/skel/.config/kdeglobals").exists())
            self.assertTrue((root / "etc/calamares/settings.conf").exists())
            firstboot = root / "usr/lib/tide/tide-firstboot.sh"
            self.assertTrue(firstboot.exists())
            self.assertTrue(firstboot.stat().st_mode & 0o111)
            link = root / "etc/systemd/system/multi-user.target.wants/tide-firstboot.service"
            self.assertTrue(link.is_symlink())
            logger.close()


class ManifestTests(unittest.TestCase):
    def test_manifest_and_checksum_round_trip(self) -> None:
        config = load_config(REPO / "config/tide.yaml")
        with tempfile.TemporaryDirectory() as temp:
            paths = isolated_paths(temp)
            iso = paths.output / "tide-test.iso"
            iso.write_bytes(b"not a real iso, only an artifact for testing\n")
            manifest = generate_manifest(config, paths, iso, profile="none")
            self.assertEqual(manifest["artifacts"][0]["sha256"], sha256_file(iso))
            self.assertFalse(manifest["validation"]["boot_tested"])
            self.assertFalse(manifest["validation"]["installer_tested"])
            self.assertEqual(verify(paths), [])
            iso.write_bytes(b"tampered\n")
            self.assertTrue(verify(paths))

    def test_checksum_parser_rejects_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "bad.sha256"
            path.write_text("0" * 64 + "  ../etc/passwd\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                parse_checksum_file(path)


class EnvironmentTests(unittest.TestCase):
    def test_doctor_reports_identity_and_dependencies(self) -> None:
        config = load_config(REPO / "config/tide.yaml")
        with tempfile.TemporaryDirectory() as temp:
            paths = BuildPaths.resolve(REPO, Path(temp))
            checks = doctor(config, paths)
            areas = {check.area for check in checks}
            self.assertIn("identity", areas)
            self.assertIn("dependency", areas)
            identity = next(check for check in checks if check.area == "identity")
            self.assertEqual(identity.level, "ok")
            self.assertIn("Debian bookworm", identity.message)


class ExecTests(unittest.TestCase):
    def test_privileged_env_is_passed_through_sudo(self) -> None:
        import os

        if os.geteuid() == 0:
            self.skipTest("running as root does not use sudo")
        with tempfile.TemporaryDirectory() as temp:
            logger = BuildLogger(Path(temp) / "logs", "exec-test")
            completed = run(
                ["true"],
                logger,
                env={"TIDE_EXAMPLE": "value", "SOURCE_DATE_EPOCH": "0"},
                privileged=True,
                dry_run=True,
            )
            argv = list(completed.args)
            self.assertEqual(argv[:2], ["sudo", "env"])
            self.assertIn("TIDE_EXAMPLE=value", argv)
            self.assertIn("SOURCE_DATE_EPOCH=0", argv)
            self.assertEqual(argv[-1], "true")
            logger.close()

    def test_cleanup_dry_run_keeps_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            paths = isolated_paths(temp)
            marker = paths.work / "keep-me"
            marker.write_text("x", encoding="utf-8")
            logger = BuildLogger(Path(temp) / "logs", "cleanup-test")
            # keep_output avoids touching the repository-level output directory.
            CleanupStage(logger, paths, dry_run=True).clean(keep_output=True)
            self.assertTrue(marker.exists())
            logger.close()


class AssetTests(unittest.TestCase):
    def test_shell_scripts_parse(self) -> None:
        import subprocess

        for script in [
            REPO / "install_languages.sh",
            REPO / "rootfs-config.sh",
            REPO / "assets/firstboot/tide-firstboot.sh",
        ]:
            completed = subprocess.run(["bash", "-n", str(script)], check=False)
            self.assertEqual(completed.returncode, 0, f"{script} has a syntax error")

    def test_calamares_configuration_is_tide_branded(self) -> None:
        settings = (REPO / "assets/calamares/settings.conf").read_text(encoding="utf-8")
        self.assertIn("branding: tide", settings)
        branding = (REPO / "assets/calamares/branding/tide/branding.desc").read_text(encoding="utf-8")
        self.assertIn("Tide OS", branding)
        self.assertTrue((REPO / "assets/calamares/branding/tide/tide-logo.svg").exists())

    def test_os_release_does_not_overclaim_independence(self) -> None:
        content = (REPO / "assets/branding/os-release").read_text(encoding="utf-8")
        self.assertIn("ID=tide", content)
        self.assertIn("ID_LIKE=debian", content)
        self.assertIn("TIDE_FOUNDATION=", content)
        self.assertNotIn("independent distribution", content)


if __name__ == "__main__":
    unittest.main()
