# Build system

Tide's build system is a Python orchestrator (`builder/`) around `debootstrap` and Debian
`live-build`. The primary command is:

```bash
./build.sh all
```

`build.sh` is a thin wrapper that runs `python3 -m builder.main`.

## Stages

| Stage | Command | What it does |
|---|---|---|
| Doctor | `./build.sh doctor` | Checks identity, architecture, disk, privileges, required commands |
| Bootstrap | `./build.sh bootstrap` | `debootstrap` a minimal Debian rootfs |
| Configure | `./build.sh configure` | Stage branding/tools, run `scripts/rootfs-config.sh` in a chroot |
| Build ISO | `./build.sh build-iso` | Run `lb config` and `lb build`, copy the ISO |
| Manifest | `./build.sh manifest` | Write SHA-256 checksums and a release manifest |
| Verify | `./build.sh verify` | Verify all checksum files |
| Clean | `./build.sh clean` | Remove the work directory and output |

`all` runs bootstrap, configure, build-iso, and manifest in order, skipping bootstrap/configure when
their state files exist unless `--force` is given.

## Options

Global options come **before** the subcommand:

```bash
./build.sh --desktop kde all
./build.sh --desktop xfce --dry-run all
./build.sh --build-dir /var/tmp/tide-build all
./build.sh --config config/tide.yaml --verbose all
```

| Option | Meaning |
|---|---|
| `--config <path>` | Configuration file (default `config/tide.yaml`) |
| `--build-dir <path>` | Work directory (default `$TIDE_BUILD_DIR` or `.tide-build`) |
| `--desktop kde|xfce|minimal|none` | Override the desktop profile |
| `--verbose` | Verbose logging |
| `--dry-run` | Print commands, skip privileged/network actions |

Subcommand options:

- `bootstrap --force` — replace an existing rootfs.
- `all --force` — rebuild bootstrap and configure.
- `clean [--keep-output] [--force]` — remove artifacts; `--force` allows removing an external build
  directory.
- `manifest --iso <path>` — manifest a specific ISO.
- `verify --checksum <path>` — verify a specific checksum file.

## Directory layout

```text
.tide-build/
  rootfs/                 Debian root filesystem
  logs/tide-build.log     full build log
  state/bootstrap.json    stage metadata
  state/configure.json
  state/livebuild.json
  state/config.json       resolved configuration snapshot
  live-build/             copied live-build tree, package lists, includes
output/
  tide-<version>-<arch>.iso
  tide-<version>-<arch>.iso.sha256
  SHA256SUMS
  manifests/tide-<version>-<arch>.manifest.json
```

The work directory can be external. Cleanup refuses `/`, the repository root, and the user's home
directory.

## Stage details

### Bootstrap

`debootstrap --variant=minbase --components=main,contrib,non-free,non-free-firmware` with a small
bootstrap package set. It then writes a controlled `/etc/apt/sources.list` using the configured mirrors
and suites. State is recorded in `state/bootstrap.json`.

### Configure

1. Stage branding, KDE defaults, first-boot units, and Calamares config into the rootfs.
2. Build and stage `tide` and `tidectl` (`builder/rusttools.py`).
3. Copy `scripts/rootfs-config.sh` into the chroot.
4. Mount `/proc`, `/sys`, `/dev`, `/run`.
5. Run the script with `TIDE_*` environment variables.
6. Unmount and remove temporary scripts.

`scripts/rootfs-config.sh` validates package names, installs the configured package set, sets locale and
timezone, creates the `tide` user with a **locked password** and no passwordless sudo, and enables
NetworkManager.

### Build ISO

1. Copy `live-build/` into `.tide-build/live-build`.
2. Generate `config/package-lists/tide.list.chroot`.
3. Stage `config/includes.chroot` with branding and Rust tools.
4. Run `lb config` with Tide product strings.
5. Run `lb build` with `SOURCE_DATE_EPOCH`.
6. Copy the newest ISO into `output/`.

live-build is isolated here so it can be replaced later behind the same stage contract.

## Manifests

`builder/manifest.py` writes a manifest with:

- product, version, channel, architecture,
- Debian foundation, mirror, tool versions,
- git commit and dirty state,
- stage state,
- package list and profile,
- artifact size and SHA-256,
- explicit validation flags.

Validation flags are conservative: `boot_tested`, `installer_tested`, and `hardware_tested` default to
`false`. Nothing in the build sets them to `true`. ISO existence is not proof of bootability.

## Reproducibility

`SOURCE_DATE_EPOCH` is set from the last git commit time when available. `Cargo.lock` is committed and
the release tool build uses `--locked`. Full byte-for-byte reproducibility has not been proven;
package mirror contents and live-build behavior still vary.
