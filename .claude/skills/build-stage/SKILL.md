---
name: build-stage
description: Add or change a stage/command in the Python ISO build system (build.sh / builder/). Use when modifying how Tide bootstraps, configures, or packages the image.
---

# Change the Tide build system

Layout: `builder/main.py` (CLI + `COMMANDS` table) → one module per stage
(`bootstrap.py`, `chroot.py`, `livebuild.py`, `branding.py`, `manifest.py`, `cleanup.py`),
with shared `config.py`, `paths.py`, `exec.py`, `logger.py`, `environment.py`.
Config comes from `config/tide.yaml`; in-chroot work happens in `scripts/rootfs-config.sh`.

## Rules

- Run external commands through `builder/exec.py` with argv lists; respect `dry_run`.
- New host dependencies go in `builder/environment.py` (so `doctor` reports them) and
  `scripts/install-dev-deps.sh`.
- Mark completed stages via `BuildPaths` so `all` can skip them; `--force` must rebuild.
- New command: add a subparser in `build_parser()` and an entry in `COMMANDS`.
- New config keys: validate them in `config.py` and document them in `docs/build-system.md`.
- Add tests in `tests/test_builder.py`.

## Testing

Real stages need root, network, `debootstrap`, and `live-build`, and are usually unavailable
here. Use `./build.sh --dry-run all` and unit tests. Never claim a stage works if it was only
dry-run; say exactly what was and wasn't exercised.
