# Troubleshooting

## Build environment

### `./build.sh doctor` exits 1

Expected when `debootstrap`, `lb`, or `grub-mkrescue` are missing. Read the `[error]` lines. Install
the dependencies:

```bash
./scripts/install-dev-deps.sh
```

The doctor also reports low disk space and architecture mismatches.

### `cargo: command not found`

Rust is usually installed in `~/.cargo/bin`. The builder checks that location and, under `sudo`, the
invoking user's home. For an interactive shell:

```bash
source "$HOME/.cargo/env"
```

For `sudo`, preserve the environment or install the toolchain system-wide:

```bash
sudo -E ./build.sh all
```

### `missing required commands: ...`

The stage you asked for needs tools the host lacks. `bootstrap` needs `debootstrap`; `configure` needs
`cargo`, `chroot`, `mount`, `umount`; `build-iso` additionally needs `lb`, `grub-mkrescue`, and
`rsync`.

### `root filesystem ... already contains a root filesystem`

Use `./build.sh bootstrap --force` or clean first:

```bash
./build.sh clean
```

### Disk space warnings

ISO builds typically need more than 10 GiB. Move the work directory to a larger volume:

```bash
TIDE_BUILD_DIR=/var/tmp/tide-build ./build.sh all
```

## Package manager

### `no supported native package manager was detected`

Tide found no APT/DNF/pacman/apk. Install one, or select Flatpak explicitly:

```bash
tide --backend flatpak search firefox
```

### `systemd is not running as PID 1`

You are in a container or chroot. `tidectl services` refuses because `systemctl` cannot control a real
service manager there. Run it on a booted system.

### `doctor` reports an APT permission error

`apt-get check` needs root. When run unprivileged, Tide falls back to a readable `dpkg-query` check and
labels the full check as skipped. Re-run with `sudo tide doctor` for the complete check.

### “1 APT source(s) rely on the system keyring…”

Informational. The source does not use a per-source `Signed-By`. APT still verifies signatures via the
system keyring; Tide does not disable verification.

### `refusing to change packages non-interactively without --yes`

Package changes in scripts need explicit confirmation:

```bash
tide --yes install firefox
```

Use `--dry-run` first.

### AppImage install fails

Tide requires:

- a regular file (not a directory),
- a filename ending in `.AppImage` (not starting with `-`),
- ELF magic bytes at the start.

AppImages are unsigned and have no update mechanism; verify publisher checksums yourself.

## Desktop / assets

### Looks wrong in a live session

The KDE look-and-feel package has not been visually validated. Check that
`/usr/share/plasma/look-and-feel/org.tide.desktop` and `/usr/share/color-schemes/Tide.colors` exist,
then select “Tide” under System Settings → Appearance.

### Calamares does not appear

Calamares is staged to `/etc/calamares`. Confirm the package is installed in the image and that the
live session can start it. This flow has not been validated.

### First-boot report missing

Check:

```bash
systemctl status tide-firstboot.service
ls -l /var/lib/tide/firstboot-done /var/log/tide/
```

If the marker exists, the service already ran. Remove the marker to run it again manually:

```bash
sudo rm /var/lib/tide/firstboot-done
sudo systemctl start tide-firstboot.service
```

## Manifests and checksums

### `verify` reports a mismatch

An artifact changed after its checksum was written, or the checksum file is stale. Rebuild and
regenerate the manifest. Do not edit checksum files to match tampered artifacts.

### `no ISO found in output`

Run `./build.sh build-iso` first, or pass `--iso <path>` to `manifest`.

## Tests

### Python tests fail importing `yaml`

```bash
python3 -m pip install -r requirements-dev.txt
```

### Clippy fails with `-D warnings`

Fix the lint; do not add `#[allow(...)]` unless the alternative is materially worse. CI treats warnings
as errors.

### ShellCheck not installed

`./scripts/validate.sh` skips ShellCheck with a notice when it is absent. CI installs it.
