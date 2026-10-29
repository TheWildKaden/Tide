# Tide OS architecture

## System context

Tide is currently a Debian-based distribution. The architecture separates three concerns:

1. **Inherited foundation** — Debian archive, APT/dpkg, Linux kernel, systemd, boot components,
   firmware, and most desktop packages.
2. **Tide-owned integration** — build orchestration, identity, desktop defaults, installer
   configuration, package-manager interface, system tooling, and release metadata.
3. **Future Tide-owned infrastructure** — native repositories, package format, resolver, image
   builder, installer flow, update transactions.

The boundary is deliberate: Debian-specific behavior is concentrated in the APT adapter, the
Debian live-build stage, and the `config/tide.yaml` foundation settings. Rust tools do not assume
Debian outside the APT backend.

```text
                 +---------------------------------------+
                 |   Tide CLI: `tide` / `tidectl`        |
                 |   Rust, no shell command construction |
                 +-------------------+-------------------+
                                     |
                 +-------------------v-------------------+
                 |            tide-core                  |
                 | config | safe exec | backend contract |
                 +--+----+----+----+----+----+----+------+
                    |    |    |    |    |    |
                  APT  DNF  pacman apk Flatpak AppImage
                    |    |    |    |    |    |
                 native package managers / local files
```

## Repository boundaries

| Path | Responsibility | May depend on Debian? |
|---|---|---|
| `crates/tide-core` | Execution, validation, config, backend trait, adapters | Only the APT adapter |
| `crates/tide-cli` | `tide` command-line surface and output | Through adapters only |
| `crates/tidectl` | System info, services, updates, hardware, recovery status | Through adapters/systemd interfaces |
| `builder/` | Build orchestration, branding staging, manifests, verification | Yes, isolated in stages |
| `live-build/` | Current image builder configuration | Yes, explicitly replaceable |
| `assets/` | Branding, desktop defaults, installer, first boot | Mostly no |
| `config/tide.yaml` | Foundation, packages, profiles, mirrors | Yes |
| `rootfs-config.sh` | In-chroot system setup | Yes, current foundation |

## Package-manager architecture

### Contract

Every backend implements `PackageBackend`:

- `key()` / `name()` — stable identifier and human name.
- `capabilities()` — explicit operation support. Unsupported operations return an error.
- `detect()` — presence check based on real executables.
- `search`, `info`, `list_installed`, `install`, `remove`, `update`, `upgrade`, `outdated`,
  `repositories`, `doctor` — operations.

Backends are registered in `BackendRegistry`. Selection is:

1. Explicit `--backend` or `TIDE_BACKEND`.
2. `package.preferred_backend` from `/etc/tide/tide.toml`, unless `auto`.
3. First available native backend in order: APT, DNF5, pacman, apk.

Flatpak and AppImage are never auto-selected as system package managers.

### Safe execution

`SystemRunner` builds `std::process::Command` from an executable and argument vector. There is no
shell evaluation. Controls:

- Package names are validated by `validate_package_spec`; leading `-`, whitespace, control
  characters, and shell metacharacters are rejected.
- Argument separators (`--`) are used where the native tool supports them.
- Environment is sanitised with `LC_ALL=C` for parse stability.
- Apt uses `DEBIAN_FRONTEND=noninteractive` and `APT_LISTCHANGES_FRONTEND=none`.
- Privileged operations are prefixed with `sudo` only when not running as root.
- One package manager lock or permission failure maps to a clear Tide error.
- No signature checks or package-script safeguards are disabled.

### Backend semantics

The adapters are not a shared dependency model:

| Backend | Native engine | Dependency/script semantics |
|---|---|---|
| `apt` | apt-get, apt-cache, dpkg-query | Debian dependency resolution, maintainer scripts, conffiles |
| `dnf5` | dnf5 or dnf, rpm | RPM scriptlets and DNF transaction semantics |
| `pacman` | pacman/libalpm | Arch package semantics and partial-upgrade constraints |
| `apk` | apk-tools | Alpine versioning, musl ABI, apk scripts |
| `flatpak` | flatpak | Runtimes, sandboxing, remote apps; user scope by default |
| `appimage` | managed local files | No repository, no dependency resolution, no mandatory signatures |

The CLI and docs must never imply these models are interchangeable.

### Package trust

Tide delegates verification to the native backend. For APT this means APT's configured keyring and
`Signed-By` policy. Tide does not pass `--allow-unauthenticated`. AppImage has no equivalent
repository signature; `tide doctor` warns about this. Tide-native signing is planned, not implemented.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Operation or general failure |
| 2 | Invalid input or configuration |
| 3 | Backend unavailable or operation unsupported |
| 4 | Permission denied |
| 5 | Package database locked |

## Build architecture

```text
build.sh -> builder.main
   |
   +-- doctor        environment checks
   +-- bootstrap     debootstrap -> .tide-build/rootfs
   +-- configure     branding + Rust tools + chroot script + mounts
   +-- build-iso     live-build config + package list + includes -> ISO
   +-- manifest      SHA-256 + build manifest + SHA256SUMS
   +-- verify        checksum verification
```

### Working directories

Defaults:

- Work directory: `$TIDE_BUILD_DIR` or `<repo>/.tide-build`
- Root filesystem: `<work>/rootfs`
- Logs: `<work>/logs/tide-build.log`
- State: `<work>/state/*.json`
- Output: `<repo>/output`

No hard-coded `/tmp/tide-builder` or machine-specific paths are used. The external build directory is
supported but cleanup refuses unsafe targets.

### Stage isolation

- `bootstrap` installs a minimal Debian rootfs with a constrained package set.
- `configure` stages identity, desktop defaults, first-boot units, installer configuration, and the
  compiled Rust tools, then runs `rootfs-config.sh` in a chroot with `/proc`, `/sys`, `/dev`, and
  `/run` mounted. Mounts are skipped in dry-run mode.
- `build-iso` copies the tracked `live-build/` configuration, generates a Tide package list and
  `includes.chroot`, then invokes `lb config` and `lb build`. live-build is replaceable behind this
  stage.

### Manifests and validation

`generate_manifest` records:

- Product/version/channel/architecture and Debian foundation.
- Git commit and dirty state.
- Stage state snapshots.
- Package list and desktop profile.
- Artifact size and SHA-256.
- Explicit validation booleans: compiled, ISO generated, boot tested, installer tested,
  hardware tested, Linux unit tests.

The build command only sets `iso_generated=true`. It never sets boot or hardware flags. The manifest
notes that ISO existence is not proof of bootability.

## Identity and desktop

- `/etc/os-release` declares `ID=tide`, `ID_LIKE=debian`, and a `TIDE_FOUNDATION` field.
- KDE is the primary desktop; package profiles are declarative in `config/tide.yaml`.
- Tide desktop assets are KDE colour scheme, look-and-feel metadata/defaults, skel config, and
  wallpaper. They reuse Breeze and Plasma rather than forking the desktop.
- The visual direction is dark glass/acrylic with teal accents, but effects default to
  compositor-managed settings; low-end hardware is not required to run expensive effects.
- XFCE is a package profile only; Tide-specific XFCE defaults are not yet implemented.

## System tooling

`tidectl` wraps standard Linux interfaces:

- Identity: `/etc/os-release`, `/proc`, `/etc/hostname`.
- Services: `systemctl` with unit-name validation. Refuses when systemd is not PID 1.
- Updates: native backend's outdated query.
- Repositories: read-only native repository inspection.
- Hardware: `/proc/cpuinfo`, `/proc/meminfo`, `/proc/mounts`, `/sys/class/drm`, `lspci`, `lsusb`.
- Recovery: capability report only; no snapshot or rollback mutation.

## Security boundaries

- Release assets are checksummed but not signed. See `SECURITY.md`.
- The live user is created with a locked password and no passwordless sudo.
- The first-boot service writes diagnostics once and does not apply hidden configuration.
- CI builds releases only from reviewed code and does not expose secrets to pull-request workflows.
- Future Tide repositories must require signed metadata and preserve key rotation.

## Decisions and trade-offs

| Decision | Rationale |
|---|---|
| Rust for `tide`/`tidectl` | Memory safety, explicit errors, single static binaries, testable execution layer |
| Keep APT/dpkg for Debian foundation | Reusing a mature, signed package ecosystem is safer than a rushed replacement |
| Keep live-build initially | Fastest path to a real image; isolated so it can be replaced |
| Keep systemd initially | Stable service interface; tools do not encode Debian-specific service policy |
| Do not fork Plasma | Branding and defaults deliver identity without a maintenance-heavy fork |
| No Tide-native format yet | Repository/signing/transaction design must come before installation code |
| Explicit capability errors | Prevents a fake success path across incompatible package ecosystems |
