# Development guide

## Prerequisites

Tide's build tooling targets Debian 12 or a compatible Ubuntu host. You need:

- Linux (x86_64 or arm64), root or `sudo`,
- Python 3.11+ with PyYAML,
- a Rust toolchain (`rustup` recommended),
- network access for package mirrors,
- `debootstrap`, `live-build`, and GRUB tooling to build an ISO,
- several GiB of free disk.

Install everything:

```bash
./scripts/install-dev-deps.sh
```

Or, in a Codespaces-style environment where you only need language toolchains:

```bash
INSTALL_QT=0 ./install_languages.sh
```

## First checks

```bash
./build.sh doctor        # environment report; exits 1 if ISO tools are missing
cargo test --workspace
python3 -m unittest discover -s tests -v
```

`./build.sh doctor` exiting non-zero on a machine without `debootstrap`/`live-build` is expected. It is
reporting reality, not a broken install.

## Rust workspace

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace --bins
```

Binaries:

- `target/release/tide`
- `target/release/tidectl`

Read-only commands can be exercised on the host:

```bash
./target/release/tide backends
./target/release/tide search curl
./target/release/tide info bash
./target/release/tide repo list
./target/release/tidectl info
./target/release/tidectl hardware
```

Do not run `tide install`, `remove`, or `upgrade` against a host you care about during development.

## Python build system

```bash
python3 -m compileall -q builder
python3 -m unittest discover -s tests -v
```

Tests use temporary directories and never require root. A new stage must expose a small class with a
`run()` method, use `builder.exec.run` for external commands, and write a state file only on success.

## Shell scripts

```bash
for script in build.sh rootfs-config.sh scripts/*.sh assets/firstboot/*.sh; do
    bash -n "$script"
done
shellcheck build.sh rootfs-config.sh scripts/*.sh assets/firstboot/*.sh
```

## Full validation

```bash
./scripts/validate.sh
```

This runs Python tests, `cargo fmt --check`, Clippy, Rust tests, a release build, shell syntax checks,
ShellCheck when available, and asset/config parsing. It does **not** boot an image.

## Working with the build system without root

```bash
./build.sh --dry-run all
```

A dry run prints the commands and stages assets into the working tree without executing the
privileged steps. Mounts and chroot calls are skipped.

## Where to make changes

| Change | Location |
|---|---|
| Backend behavior | `crates/tide-core/src/backends/` |
| CLI surface | `crates/tide-cli/src/` |
| System tooling | `crates/tidectl/src/` |
| Build stage | `builder/` |
| Package/profile set | `config/tide.yaml` |
| Identity/branding | `assets/branding/` |
| Desktop defaults | `assets/desktop/` |
| Installer | `assets/calamares/` |
| In-chroot setup | `rootfs-config.sh` |
| ISO configuration | `live-build/` |
| CI | `.github/workflows/` |

## See also

- [build-system.md](build-system.md)
- [package-manager.md](package-manager.md)
- [troubleshooting.md](troubleshooting.md)
- [CONTRIBUTING.md](../CONTRIBUTING.md)
