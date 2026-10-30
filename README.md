# Tide OS

Tide OS is a Linux distribution project currently built on a **Debian 12 (bookworm) foundation**.
It is not yet an independent distribution. Debian supplies the archive, kernel, bootloader, and
most of the userspace; Tide supplies its identity, reproducible build orchestration, package-manager
interface, system tools, desktop defaults, installer foundation, and release metadata.

The long-term direction is to let Tide own more of its distribution infrastructure while keeping
upstream Linux and well-maintained open-source components where replacing them would reduce security
or reliability. See [PLAN.md](PLAN.md) and [DEBIAN_DEPENDENCIES.md](DEBIAN_DEPENDENCIES.md).

## Current status

| Area | State |
|---|---|
| Distribution foundation | Debian 12 (bookworm) — inherited |
| Build workflow | Implemented: `debootstrap` + Debian `live-build`, orchestrated by Python |
| Tide identity and branding | Implemented assets: `os-release`, logo, wallpaper, KDE colour scheme, first-boot diagnostic |
| Desktop profiles | KDE Plasma (primary), XFCE (optional), minimal Openbox profile configured |
| Installer | Calamares configuration staged; **not boot-tested** |
| Package manager (`tide`) | Implemented Rust CLI with isolated APT, DNF5, pacman, apk, Flatpak, and managed AppImage backends |
| System tool (`tidectl`) | Implemented Rust CLI: info, doctor, services, updates, repositories, hardware, recovery status |
| Release engineering | Build manifests, SHA-256 checksums, verification, GitHub Actions |
| ISO generation | Pipeline implemented; not run in this Codespace (missing `live-build`/`debootstrap`, limited disk) |
| Boot / installer / hardware validation | **Not performed.** An ISO build does not prove any of these work. |
| Tide-owned package repositories | Not implemented |
| Tide-native package format | Not implemented |

## What works today

- `tide backends`, `tide search`, `tide info`, `tide list`, `tide repo list`, and `tide outdated`
  are implemented and were exercised against the real APT database on the development host.
- `tide install`, `remove`, `update`, and `upgrade` are implemented through native tools with argument
  validation, lock/permission error handling, a dry-run mode, and confirmation prompts. They are
  intentionally not run against the host during validation.
- `tidectl info`, `doctor`, `updates`, `repo list`, `hardware`, and `recover` read real system state.
- `tidectl services` refuses to pretend systemd is available in containers where it is not PID 1.
- The Python build system validates configuration, stages branding and Rust tools, runs `bootstrap`,
  `configure`, and `build-iso` stages, and writes a manifest that explicitly records what was **not**
  tested.
- Unit tests cover the Python builder, package-name validation, APT parsing, backend selection,
  AppImage lifecycle, checksum handling, and asset integrity.

## Quick start

Requirements: Debian/Ubuntu host, root or sudo, network access, several GiB of free disk, and
`debootstrap` + `live-build` for ISO stages.

```bash
make setup      # install build and development dependencies (Debian/Ubuntu)
make doctor     # check the environment; missing ISO dependencies are reported, not hidden
make tools      # build the `tide` package manager and `tidectl` system tool
make test       # run every check that doesn't require booting an image
make dry-run    # preview a full ISO build without changing anything
make build      # build a complete ISO (downloads packages; may take a long time)
```

Run `make` on its own to list every target. Each target is a thin wrapper around `./build.sh`,
`cargo`, or a script in `scripts/`, so you can call those directly too.

The primary build command is:

```bash
./build.sh all
```

Build artifacts:

```text
output/tide-<version>-<arch>.iso
output/tide-<version>-<arch>.iso.sha256
output/SHA256SUMS
output/manifests/tide-<version>-<arch>.manifest.json
.tide-build/logs/tide-build.log
```

Override the working directory with `TIDE_BUILD_DIR` or `--build-dir`. Select a desktop profile
with a global option:

```bash
./build.sh --desktop kde all
./build.sh --desktop xfce all
./build.sh --desktop none all
```

## Publishing a release

Pushing a tag that starts with `v` builds the ISO on GitHub Actions and creates or updates the
matching GitHub release with the ISO, checksums, and manifest:

```bash
git tag v0.1.0-alpha.1
git push origin v0.1.0-alpha.1
```

The release workflow can also be started manually with an existing tag. For an ISO built locally,
upload it with the GitHub CLI:

```bash
gh auth status
./scripts/publish-release.sh v0.1.0-alpha.1 --create --prerelease
# or through make:
make publish-release TAG=v0.1.0-alpha.1 CREATE=1
```

`scripts/publish-release.sh` verifies checksums first and refuses to publish a tag that does not
exist unless `--create` is passed. Assets are uploaded with `--clobber`, so re-running replaces
them. Release assets are checksummed but **not signed** yet.

## Package manager

```bash
tide install firefox
tide remove firefox
tide update
tide upgrade
tide search browser
tide info firefox
tide list
tide outdated
tide doctor
tide repo list
tide backends
tide --backend flatpak search firefox
tide --backend appimage install ./Application.AppImage
```

`tide` never builds a shell command string. Backends receive executable + argument vectors, and package
names are validated before use. Native signature verification, maintainer scripts, and lock handling
remain the responsibility of the native backend. See [docs/package-manager.md](docs/package-manager.md).

Tide is currently an **interface over native package managers**, not a replacement for APT's dependency
resolver or repository format. The backends do not share dependency semantics, and the documentation
does not claim they do.

## System tooling

```bash
tidectl info
tidectl doctor
tidectl services list
tidectl services status ssh.service
tidectl updates
tidectl repo list
tidectl hardware
tidectl recover
```

`tidectl recover` reports capabilities only. Snapshot creation and rollback are **not implemented**.

## Repository layout

```text
builder/            Python build orchestration
config/tide.yaml    Base system, desktop profiles, installer, and mirror configuration
crates/             Rust workspace
  tide-core/        Safe execution, config, backend contract and adapters
  tide-cli/         `tide` package manager
  tidectl/          Tide system administration tool
assets/             Branding, KDE defaults, Calamares, first-boot assets
live-build/         Isolated Debian live-build configuration
scripts/            Dependency installers, validation, release publishing,
                    and the in-chroot rootfs-config.sh
tests/              Python builder and asset tests
docs/               Development, package manager, release, migration, desktop guides
.github/workflows/  CI, manual ISO build, release build
```

## Honest limitations

- The ISO pipeline has not produced an image in this environment. `live-build` and `debootstrap`
  are absent and the workspace has limited free space.
- No generated image has been booted. No Calamares installation has been completed. No hardware
  compatibility claims are made.
- Release artifacts are checksummed but **not cryptographically signed**. Checksums detect corruption;
  they do not prove publisher identity.
- The KDE look-and-feel package and Calamares configuration are structurally valid assets, not
  visually or functionally validated on a running desktop.
- The `xfce` profile has no Tide-specific skel defaults yet.
- Tide-native repositories, package format, dependency resolver, and rollback tooling remain future work.

## Documentation

- [PLAN.md](PLAN.md) — staged independence roadmap
- [ARCHITECTURE.md](ARCHITECTURE.md) — subsystem boundaries and design
- [DEBIAN_DEPENDENCIES.md](DEBIAN_DEPENDENCIES.md) — inherited components and replacement analysis
- [CONTRIBUTING.md](CONTRIBUTING.md) — development workflow
- [SECURITY.md](SECURITY.md) — threat model and security policy
- [docs/development.md](docs/development.md)
- [docs/build-system.md](docs/build-system.md)
- [docs/package-manager.md](docs/package-manager.md)
- [docs/desktop.md](docs/desktop.md)
- [docs/installation.md](docs/installation.md)
- [docs/release-engineering.md](docs/release-engineering.md)
- [docs/debian-migration.md](docs/debian-migration.md)
- [docs/troubleshooting.md](docs/troubleshooting.md)

## License

MIT. See [LICENSE](LICENSE). Components inherited from Debian and other upstream projects retain
their own licenses.
