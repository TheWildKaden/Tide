# Contributing to Tide OS

Thanks for your interest. Tide is an early-stage, Debian-based distribution project. The most useful
contributions right now are honest fixes, tests, documentation corrections, and validation on real
hardware or virtual machines.

Please read [SECURITY.md](SECURITY.md) before changing privileged code, build scripts, packaging, or
authentication.

## Ground rules

1. **Do not overclaim.** Tide is not yet independent from Debian. Do not describe the ISO as bootable,
   the installer as working, or a backend as validated unless you personally ran that check and can
   say how.
2. **Do not fake success.** A command that prints “done” without doing the work is a bug.
3. **Do not weaken security to make a build pass.** Never add `--allow-unauthenticated`, disable
   signature checks, grant passwordless sudo, or commit secrets.
4. **No shell-command construction from untrusted input.** Pass argument vectors to
   `std::process::Command` / `subprocess.run`.
5. **Keep Tide-owned code separate from inherited Debian behavior.** Put Debian-specific assumptions
   behind the APT adapter, the `builder/` stages, or `config/tide.yaml`.

## Development environment

Requirements: a Debian/Ubuntu host (or the provided dev container), Python 3.11+, a Rust toolchain, and
network access.

```bash
./scripts/install-dev-deps.sh   # system packages + Rust toolchain
./scripts/validate.sh           # everything that does not require booting an image
```

`install_languages.sh` is a lighter alternative for Codespaces-style environments when you only need
language toolchains. It installs Rust, C/C++, Python, Node/TypeScript, and optional Qt6 development
packages.

### Toolchain versions

- `rust-toolchain.toml` pins `stable` with `rustfmt` and `clippy`.
- Python dependencies are listed in `requirements-dev.txt` (currently PyYAML).
- Cargo dependencies are locked in `Cargo.lock`; keep it committed.

## Repository layout

| Path | Contents |
|---|---|
| `crates/tide-core` | Safe execution, config, backend contract, adapters |
| `crates/tide-cli` | `tide` package-manager CLI |
| `crates/tidectl` | `tidectl` system tool |
| `builder/` | Python build orchestration |
| `config/tide.yaml` | Foundation, package, desktop, and installer configuration |
| `assets/` | Branding, KDE defaults, Calamares config, first-boot assets |
| `live-build/` | Debian live-build configuration (replaceable) |
| `rootfs-config.sh` | In-chroot configuration |
| `scripts/` | Dependency installation and validation |
| `tests/` | Python builder and asset tests |
| `docs/` | Long-form documentation |

## Making changes

1. Create a focused branch.
2. Add or update tests with the change.
3. Run `./scripts/validate.sh`.
4. If your change affects build output, run a dry run:
   `./build.sh --dry-run all`.
5. Update the relevant documentation, including the limitations section if applicable.
6. Open a pull request describing what you changed, what you ran, and what remains unverified.

### Rust

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Backend changes should include parser tests and a test proving option-injection is rejected. Commands
must be constructed as an executable plus arguments—never interpolated into a shell string.

### Python build system

```bash
python3 -m compileall -q builder
python3 -m unittest discover -s tests -v
```

Tests must not require root, a network, or a full ISO build. Use `tempfile.TemporaryDirectory()` and
`BuildPaths.resolve(..., override)` so tests never touch the real build directory.

### Shell scripts

```bash
bash -n script.sh
shellcheck script.sh
```

Scripts must start with `set -Eeuo pipefail`, quote expansions, use arrays for argument lists, and
avoid `eval`. Destructive operations must validate their target is inside an expected directory.

### Assets

- Configuration files must parse: YAML for Calamares and `config/tide.yaml`, TOML for
  `tide.toml`, JSON for Plasma metadata, XML for SVG.
- Do not add binary blobs that cannot be reviewed or regenerated.
- Branding must keep `ID=tide` and `ID_LIKE=debian` until the project is genuinely independent.

## Configuration changes

`config/tide.yaml` is validated at load time:

- Package names must match `^[a-z0-9][a-z0-9+.:-]*$`.
- Desktops must exist before being referenced as the default.
- Mirrors must be HTTP(S).
- Custom scripts must stay inside the repository.

If you add a package, confirm it exists in the target Debian suite. If you add a desktop profile,
update `docs/desktop.md` and the builder tests.

## Commit messages

Use an imperative summary line, for example:

```text
builder: reject checksum entries outside output/
tide-core: add pacman repository parsing test
docs: correct release signing status
```

Explain *why* in the body when the change is not obvious. Avoid claiming validation you did not run.

## What needs help

- Producing and boot-testing an ISO on a machine with enough disk and `live-build`.
- Running Calamares on a disposable virtual disk (BIOS and UEFI).
- Testing non-APT backends on Fedora, Arch, and Alpine.
- Improving XFCE branding defaults.
- Designing Tide repository metadata and signing.
- Adding an SBOM and provenance attestations.

## Licensing

By contributing, you agree that your contribution is licensed under the MIT License. Do not submit
code you cannot license. Upstream components keep their own licenses.
