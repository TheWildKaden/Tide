# Tide OS package-manager and system tools

This page documents the implemented behavior of `tide` and `tidectl`. It describes what the binaries do
today, including gaps. Nothing here should be read as a claim that a backend has been validated on its
native distribution unless stated.

## Building

```bash
cargo build --release --workspace --bins
# target/release/tide
# target/release/tidectl
```

The build system stages these binaries into `/usr/bin` in the image
(`builder/rusttools.py`).

## `tide` CLI

```text
tide [--backend <id>] [--json] [--yes] [--dry-run] [--config <path>] <command>
```

| Command | Behavior |
|---|---|
| `install <pkg>...` | Install via selected backend |
| `remove <pkg>... [--purge]` | Remove; `--purge` where supported |
| `update` | Refresh repository metadata |
| `upgrade` | Upgrade installed packages |
| `search <query>` | Search package names/descriptions |
| `info <pkg>` | Show package metadata |
| `list` | List installed packages |
| `outdated` | List packages with available upgrades |
| `repo list` | List configured repositories |
| `backends` | Show detected backends and capabilities |
| `doctor` | Diagnose config, backend, signatures, privileges |
| `version` | Print version and backend list |

Global flags:

- `--backend, -b` / `TIDE_BACKEND` — explicitly select a backend.
- `--json` — machine-readable output.
- `--yes, -y` — skip confirmation. Non-interactive changes require this.
- `--dry-run` — print what would be done without changing package state.
- `--config` / `TIDE_CONFIG` — path to `tide.toml`.

### Backend selection

1. `--backend` or `TIDE_BACKEND`.
2. `package.preferred_backend` in config, if not `auto`.
3. First available native backend in order: `apt`, `dnf5`, `pacman`, `apk`.

`flatpak` and `appimage` are never selected automatically. Choose them explicitly.

If an explicitly named backend is unavailable, Tide returns an error rather than silently falling back.

### Configuration

`/etc/tide/tide.toml` (or `--config`) is optional; safe defaults apply when absent.

```toml
[package]
preferred_backend = "auto"
assume_yes = false

[apt]
install_recommends = true

[flatpak]
scope = "user"

[repositories]
native_tide_repos = false
```

`repositories.native_tide_repos = true` currently produces a `tide doctor` warning: Tide-native
repositories are not implemented.

### Backends

| ID | Native tools | Notes |
|---|---|---|
| `apt` | `apt-get`, `apt-cache`, `dpkg-query` | Debian/Ubuntu. Signature policy remains APT's. |
| `dnf5` | `dnf5` or `dnf`, `rpm` | Fedora family. |
| `pacman` | `pacman` | Arch family. `upgrade` uses `-Syu`. |
| `apk` | `apk` | Alpine family. musl ABI is not glibc-compatible. |
| `flatpak` | `flatpak` | Sandboxed desktop apps; user scope default. |
| `appimage` | managed local files | No repository, no dependency resolution. |

Capabilities are explicit. Requesting an unsupported operation (for example AppImage `search`) returns
an “unsupported operation” error, never a fake empty success.

### Execution safety

- Commands are `executable + argument vector`; no shell is invoked.
- Package names are validated: no leading `-`, whitespace, control characters, or shell
  metacharacters.
- `--` is inserted before package lists where the native tool supports it.
- `LC_ALL=C` is set for stable parsing.
- APT runs with `DEBIAN_FRONTEND=noninteractive`.
- Privileged commands use `sudo` only when not already root.
- No signature bypass flags are ever added.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General failure |
| 2 | Invalid input/configuration |
| 3 | Backend unavailable or unsupported operation |
| 4 | Permission denied |
| 5 | Package database locked |

### AppImage behavior

`tide --backend appimage install ./App.AppImage`:

1. Requires a regular file whose name ends in `.AppImage` and is not a symlink-style option.
2. Checks the file begins with the ELF magic bytes.
3. Copies it to the managed directory (default under `~/.local/share/tide/appimages`).
4. Marks it executable and writes a desktop entry.
5. Does **not** execute the AppImage during installation.

AppImages have no repository signature. `tide doctor` warns about this. There is no update mechanism.

## `tidectl` CLI

```text
tidectl [--backend <id>] [--json] [--config <path>] <command>
```

| Command | Behavior |
|---|---|
| `info` | OS identity, kernel, architecture, init, backends |
| `doctor` | Identity, init, backend, load average, firmware tool checks |
| `services list` | List systemd services |
| `services status <unit>` | Show unit status |
| `services start/stop/restart/enable/disable <unit>` | Control units |
| `updates` | Outdated packages from the selected backend |
| `repo list` | Repositories for the selected backend |
| `hardware` | CPU, memory, PCI, USB, DRM graphics, driver tools |
| `recover` | Report snapshot/rollback capability only |

### Service safety

- Unit names are validated (`validate_unit_name`) before use.
- If `/run/systemd/system` is absent, `tidectl services` refuses with a clear message:
  “systemd is not running as PID 1 (container or chroot detected)”.
- Commands are argument vectors, not shell strings.

### Recovery

`tidectl recover` never creates, deletes, or restores snapshots. It reports:

- root filesystem type,
- whether it is btrfs,
- whether `snapper` or `timeshift` is installed,
- an explicit finding that Tide rollback is not implemented.

## Known gaps

- Only the APT adapter has been exercised against a real package database in this repository's
  development environment.
- DNF5, pacman, apk, and Flatpak parsers are unit-tested against sample output, not live systems.
- Non-APT signature policy is delegated to the native tool; Tide has not audited each policy.
- There is no Tide-native package format, resolver, repository, or signing yet.
- AppImage installs are local-only and unsigned.
- `tide repo` is read-only; adding/removing repositories safely is future work.
