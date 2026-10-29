# Tide OS independence roadmap

This plan is a working engineering document, not a claim that each stage is complete.
The current release is a **Debian-based development distribution** with Tide-owned interfaces
and a migration path. Debian remains the source of packages, the kernel, bootloader, and most
system libraries.

Legend: **Done** = implemented and tested where the environment allows; **Partial** = implemented
foundation with explicit gaps; **Planned** = designed but not implemented; **Not started**.

## Guiding rules

1. Independence means Tide controls its distribution-specific decisions, not that Tide rewrites
   security-critical upstream software.
2. Never weaken signature verification, privilege separation, or package-script safeguards to make a
   build pass.
3. Keep Debian-specific assumptions behind adapters, build configuration, or documented boundaries.
4. Every stage must leave a working, testable system behind.
5. Validation claims must match what was actually executed.

## Stage 1 — Tide distribution on Debian

**State: Partial.** The build foundation, identity, profiles, package-manager interface, manifests, and
CI exist. Boot and installer validation do not.

| Deliverable | State | Evidence / gap |
|---|---|---|
| Tide branding and system identity | Done | `assets/branding/os-release`, issue files, logo, wallpaper |
| Minimal live-system build configuration | Partial | Python stages + `live-build/`; ISO not produced here |
| Configurable base system and desktop profiles | Done | `config/tide.yaml`, `builder/packages.py` |
| KDE Plasma primary, XFCE optional | Partial | package profiles and KDE defaults; XFCE branding minimal |
| Networking, storage, audio, graphics, users, hardware detection | Partial | Debian packages + calibration; no hardware matrix |
| Calamares installer foundation | Partial | `assets/calamares/`; not run |
| Reproducible build scripts | Partial | pinned config, state, logs; byte-for-byte reproducibility not proven |
| Versioning, checksums, manifests | Done | `builder/manifest.py`, `VERSION`, release workflow |
| Clean first-boot experience | Partial | first-boot diagnostic service; no setup wizard |
| Automated build and validation workflows | Done | `.github/workflows/ci.yml`, `build-iso.yml`, `release.yml` |

### Next Stage 1 work

1. Produce an ISO in a machine with `live-build`, `debootstrap`, network, and ≥10 GiB free space.
2. Boot BIOS and UEFI images in QEMU; record kernel, display, audio, and network results.
3. Run Calamares end-to-end on a disposable virtual disk; fix partition/bootloader configuration.
4. Add a hardware test matrix for common GPUs, Wi-Fi, and audio devices.
5. Improve XFCE defaults or remove the profile from release configurations until it is branded.
6. Add package-list validation against the Debian archive before long builds.
7. Add a boot-smoke-test GitHub runner using QEMU with KVM where available.

## Stage 2 — Tide system tools

**State: Partial.** `tide` and `tidectl` are implemented as Rust binaries and staged into images.
Privileged installation, rollback, and first-boot configuration remain incomplete.

| Tool area | State | Notes |
|---|---|---|
| Package manager CLI | Done | install/remove/update/upgrade/search/info/list/doctor/repo/outdated/backends |
| System information and diagnostics | Done | `tidectl info`, `doctor`, `hardware` |
| Service management abstraction | Partial | systemd wrapper with unit validation; no alternative init backend |
| Software repository configuration | Partial | read-only inspection; no safe add/remove of sources |
| Update management | Partial | check and upgrade through native backend; no staged/atomic updates |
| Recovery and rollback | Not started | `tidectl recover` reports capabilities only |
| Hardware/driver management | Partial | inventory and tool detection; no driver installation |
| Installation/first-boot configuration | Partial | Calamares config + first-boot diagnostic; no first-boot wizard |
| Desktop integration | Partial | KDE defaults and `tide`/`tidectl`; no QML settings application |

### Next Stage 2 work

1. Implement atomic update policy and a pre-upgrade snapshot hook where a snapshot filesystem is available.
2. Add `tide repo add/remove` with signed-by enforcement and dry-run output.
3. Add a Tide update service with explicit opt-in and no silent background changes.
4. Implement first-boot setup through a small QML/Qt application or a documented CLI flow.
5. Add a driver information interface that recommends firmware packages without installing silently.
6. Add a QML Tide Settings application over `tidectl` JSON output.
7. Test all privileged operations in disposable VMs.

## Stage 3 — Tide universal package manager

**State: Partial foundation.** The Rust architecture, backend contract, CLI, and six adapters exist.
There is no Tide-native package format, resolver, repository, or signature infrastructure yet.

| Capability | State |
|---|---|
| Stable CLI across backends | Done |
| APT/dpkg adapter | Done, read-only commands exercised |
| DNF5/RPM adapter | Implemented, not exercised on Fedora |
| pacman adapter | Implemented, not exercised on Arch |
| apk adapter | Implemented, not exercised on Alpine |
| Flatpak adapter | Implemented, not exercised |
| Managed AppImage integration | Done with local-file install/remove tests |
| Capability detection and explicit unsupported errors | Done |
| Safe argument-vector execution | Done |
| Lock/permission classification | Done |
| Tide-native repository format | Not started |
| Tide-native package format | Not started |
| Dependency resolver | Not started |
| Package signing and verification | Not started |
| Transaction rollback | Not started |

### Next Stage 3 work

1. Define a versioned repository metadata format with detached signatures and a keyring model.
2. Define a package archive format that records file hashes, permissions, and declared conflicts.
3. Implement a resolver as a separately testable library, without pretending its semantics match APT.
4. Add a `tide-native` backend that installs into a controlled root/prefix for tests first.
5. Add signature verification tests with generated test keys; never commit private keys.
6. Implement transactional staging, fsync, and rollback for native installs.
7. Keep compatible external sources available through the existing adapters.

## Stage 4 — Replace Debian-specific infrastructure

**State: Analysis and interface foundation.** See [DEBIAN_DEPENDENCIES.md](DEBIAN_DEPENDENCIES.md)
for per-subsystem decisions.

Priority order:

1. **Tide-controlled repositories and release metadata.** High value, low kernel-level risk.
2. **Tide-owned system configuration and update policy.** Already started with `tidectl`.
3. **Tide image builder.** Replace live-build behind the existing `builder/` interface.
4. **Installer branding and flow.** Keep Calamares initially; replace Debian integration details.
5. **Package management.** Move from APT-facing workflows to Tide-native repositories and packages
   incrementally; keep APT as an optional compatibility backend.
6. **Boot/initramfs pipeline.** Reuse Debian initramfs-tools or dracut until a Tide pipeline is tested;
   then control configuration without rewriting boot fundamentals.
7. **Desktop experience.** Continue Tide defaults and settings; do not fork Plasma wholesale.
8. **Foundational libc/coreutils replacement.** Low priority and high risk. Retain glibc and GNU
   coreutils unless a specific technical reason justifies a change.

## Stage 5 — Independent distribution infrastructure

Long-term target architecture:

- Tide-controlled signed repositories and mirrors.
- Tide package format, resolver, verifier, and transactional installer.
- Tide image builder replacing live-build behind a stable Python/Rust interface.
- Tide release keys, reproducible-build policy, and upgrade channels.
- Tide installer and first-boot experience.
- Stable Linux kernel and upstream driver/firmware compatibility.
- Documented source build from a minimal bootstrap environment.

The kernel is not replaced by distribution independence. Tide can remain compatible with upstream
Linux while owning its packaging, image, and release infrastructure.

## Milestones and exit criteria

| Milestone | Exit criteria |
|---|---|
| M1: Buildable | ISO generated twice from clean workspaces; manifests and checksums verify |
| M2: Bootable | BIOS and UEFI boot in QEMU; install media reaches live desktop |
| M3: Installable | Calamares completes on BIOS and UEFI VMs; installed system boots |
| M4: Supportable | Update/rollback path tested; security response documented |
| M5: Tide repositories | Signed Tide repository serves at least Tide-owned packages |
| M6: Tide-native packages | Native format installs/removes/upgrades with verified signatures |
| M7: Independent image | Image builder and installer no longer require Debian-specific assumptions |

## Tracking incomplete work

Incomplete work must stay visible in this file, in `README.md`, and in release manifests. A feature is
not complete because code exists; it is complete when its tests and validation have actually run.
