# Tide OS — working notes

Debian 12 (bookworm)–based distro project by TheWildKaden. Tide adds identity, build
orchestration, a Rust package-manager front end, system tooling, and desktop/installer defaults.
Nothing has been boot-tested; docs are deliberately honest about that — keep it that way.

## Map

| Path | What it is |
|---|---|
| `build.sh` → `builder/main.py` | Python build CLI. Stages: doctor, bootstrap, configure, build-iso, manifest, verify, clean, all |
| `builder/*.py` | One module per stage (`bootstrap`, `chroot`, `livebuild`, `branding`, `manifest`, `cleanup`) plus `config`, `paths`, `exec`, `logger`, `environment` |
| `config/tide.yaml` | Single source of truth: OS identity, mirrors, package sets, desktop profiles, installer |
| `scripts/rootfs-config.sh` | Runs **inside the chroot** during `configure` (copied in by `builder/chroot.py`) |
| `scripts/validate.sh` | Full local check suite (`make test`), mirrors CI |
| `scripts/install-dev-deps.sh` / `install-languages.sh` | Host setup (full ISO deps / toolchains only) |
| `crates/tide-core` | Backend trait + adapters (apt, dnf, pacman, apk, flatpak, appimage), safe exec |
| `crates/tide-cli` | `tide` binary · `crates/tidectl` → `tidectl` binary |
| `assets/` | Branding, KDE/Plasma defaults, Calamares config, first-boot service |
| `live-build/` | Debian live-build config used by `build-iso` |
| `docs/` | Per-subsystem guides; `PLAN.md`, `ARCHITECTURE.md`, `DEBIAN_DEPENDENCIES.md` at root |

## Commands

- `make` lists targets. Common: `make test`, `make lint`, `make tools`, `make dry-run`.
- Python tests alone: `python3 -m unittest discover -s tests`
- Build state lives in `.tide-build/` (override `TIDE_BUILD_DIR`); artifacts in `output/`.

## Conventions

- Never build shell command strings — pass argv lists (both Python `builder/exec.py` and Rust `tide-core/src/exec.rs`).
- Package names are validated before use; keep that when adding backends.
- Adding a build command: add a subparser in `build_parser()` and an entry in `COMMANDS` in `builder/main.py`.
- Moving any shell script: update `scripts/validate.sh`, `.github/workflows/ci.yml`, `tests/test_builder.py`, and docs.
- CI: `.github/workflows/ci.yml` (checks), `build-iso.yml` (manual), `release.yml` (on `v*` tags).

## Open issues noticed

- `LICENSE` is the Unlicense but `Cargo.toml` declares `license = "MIT"` — pick one.
- Release artifacts are checksummed, not signed.
- XFCE profile has no Tide skel defaults.

## Claude setup (`.claude/`)

- `settings.json` — auto-allows tests/lint/dry-runs; asks before real builds, `sudo`, installs,
  tags, pushes, and releases; blocks force-push. Personal overrides go in `settings.local.json` (gitignored).
- `hooks/format.sh` — after edits: rustfmt on `.rs`, `bash -n` on `.sh`.
- Skills: `/validate`, `/add-backend`, `/build-stage`, `/release` (manual only).
- Agent: `tide-reviewer` — checks diffs against the conventions above.
