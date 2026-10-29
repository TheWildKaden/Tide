# Debian dependencies and replacement analysis

Tide currently inherits the Debian 12 (bookworm) distribution foundation. This document identifies
what is inherited, what Tide owns, whether replacement is feasible, and what should remain upstream.
It is intentionally conservative: replacing security-critical infrastructure is not a goal by itself.

Status terms:

- **Inherited** — supplied by Debian or another upstream.
- **Tide-owned interface** — Tide code/config controls behavior, upstream engine remains.
- **Tide-owned implementation** — implemented in this repository.
- **Planned** — designed but not implemented.

## Summary matrix

| Subsystem | Currently inherited | Tide owns now | Replacement feasibility | Recommended direction |
|---|---|---|---|---|
| Package archive | Debian archive | Backend adapter, signatures policy visibility | High for Tide repos; low for re-signing Debian | Add Tide repositories; keep Debian as source |
| Package manager | APT/dpkg | `tide` CLI, adapters | Medium-high | Tide-native backend later; APT compatibility remains |
| Repository infrastructure | deb.debian.org, Debian keys | Read-only inspection | High | Tide-controlled signed repository first |
| Kernel | Debian linux-image | Selection/config only | Low value | Keep upstream kernel |
| Bootloader | GRUB/syslinux from Debian | ISO stage configuration | Medium | Keep GRUB; own configuration |
| initramfs | initramfs-tools | None | Medium | Reuse dracut/initramfs-tools; own config later |
| Init/service management | systemd | `tidectl services` abstraction | Low value | Keep systemd; avoid Debian-specific assumptions |
| libc | glibc (Debian build) | None | Low value/high risk | Keep glibc or evaluate musl only with ABI analysis |
| Core utilities | GNU coreutils etc. | None | Low value | Keep upstream |
| Desktop | KDE Plasma from Debian | Branding/defaults/profiles | Low for fork; high for defaults | Keep Plasma; own Tide experience |
| Installer | Calamares (Debian package) | Calamares config/branding | High for flow/config | Keep Calamares; own settings and first boot |
| Image builder | live-build/debootstrap | Python orchestration | High | Replace live-build behind builder interface |
| Configuration tools | Debian/systemd defaults | `tidectl`, rootfs script | High | Grow Tide config ownership |
| Release engineering | Debian versioning | Tide version, manifests, CI | High | Tide channels, signed metadata |
| Security updates | Debian Security Team | Policy and notification only | Low for advisory data | Keep Debian advisories initially |
| Firmware | Debian non-free-firmware | Package selection | Medium | Keep firmware packages; own hardware UI |
| Networking/storage/audio/graphics | Debian packages | Defaults/integration | Low | Keep upstream stacks |

## Package management

**Inherited:** `apt`, `dpkg`, Debian dependency metadata, maintainer scripts, conffile handling,
keyrings, archive signatures, package database, policy-rc.d conventions.

**Tide-owned now:** `crates/tide-core` backend contract, `crates/tide-cli` command surface, APT
adapter, capability detection, validation, lock/permission errors, dry-run, JSON output, config.

**Feasibility:** Replacing the *interface* is feasible now. Replacing APT's dependency resolution
and installation semantics safely is a multi-stage project.

**Security implications:** APT already enforces signatures, verifies package hashes, handles
maintainer scripts, and protects the dpkg database. A Tide-native installer must reproduce these
properties, including transaction recovery and script policy, before replacing APT.

**Migration steps:**

1. Keep the APT adapter as the production path for the Debian-based image.
2. Define Tide repository metadata and a signed package format in parallel.
3. Implement a resolver library with differential tests against known APT outcomes.
4. Install Tide-native packages into disposable roots first.
5. Introduce a `tide-native` backend only after signature, transaction, and rollback tests pass.
6. Keep APT available for Debian-compatible sources.

**Validation:** package-name injection tests, signature-failure tests, lock tests, interrupted
transaction tests, downgrade tests, conffile behavior tests, and VM install/remove/upgrade cycles.

**Remain upstream:** APT/dpkg remain the safest choice for Debian packages until Tide's own
implementation is demonstrably equivalent for the supported package subset.

## Repository infrastructure

**Inherited:** Debian mirrors, `Release`/`InRelease` signatures, Debian archive keys, snapshot service.

**Tide-owned now:** repository listing, `Signed-By` visibility, doctor findings. No Tide repository.

**Feasibility:** High. A Tide repository can start with Tide-owned metadata and a small package set,
while Debian continues to provide the base.

**Security implications:** Key generation, offline signing, rotation, revocation, mirror compromise,
rollback protection, and metadata freshness all need explicit design. Private keys must never enter
the repository or CI logs.

**Migration:** define schema -> generate test keys -> sign metadata -> publish a staging repository ->
add `tide repo add` with mandatory signed-by -> test key rotation and failure paths.

**Remain upstream:** Debian archive signatures for inherited packages.

## Image creation

**Inherited:** `live-build`, `debootstrap`, `lb`, GRUB/syslinux templates, Debian package lists.

**Tide-owned now:** Python stages, config, branding staging, package profile generation, manifests,
verification, state, logs.

**Feasibility:** High. The builder interface already isolates live-build. A Tide image tool can:
bootstrap a rootfs, apply an immutable package plan, configure, create squashfs/erofs, and assemble
boot metadata.

**Security implications:** image builder must not disable signatures, must verify artifacts, and must
record package provenance. Build reproducibility requires pinned packages and timestamps.

**Migration:** add a second image backend behind `builder/livebuild.py`'s stage contract; compare
artifacts in CI; retire live-build only after boot tests pass.

## Installer

**Inherited:** Calamares package, Debian partition/bootloader defaults, Debian live install integration.

**Tide-owned now:** branding, settings sequence, welcome/partition/users/finished overrides.

**Feasibility:** High for configuration and branding; medium for replacing Calamares itself. Building
a new installer is not justified before the Calamares flow works.

**Security implications:** partitioning, disk encryption, bootloader installation, user creation, and
root password policy are high-risk. The current config disables setting a root password and creates a
sudo user; this must be validated.

**Migration:** validate Calamares in VMs, replace Debian-specific module defaults, then add Tide first-boot
setup. Keep Calamares as the engine until a replacement has equivalent disk-safety tests.

## Kernel, boot, initramfs, init

**Inherited:** Debian kernel package, initramfs-tools, GRUB/syslinux, systemd, udev.

**Tide-owned now:** ISO-stage configuration and service inspection.

**Feasibility:** Kernel replacement provides little distribution-independence value and high risk.
Bootloader/initramfs configuration ownership is feasible; wholesale replacement is not recommended.
systemd remains the init system.

**Security implications:** secure boot, kernel module signing, initramfs integrity, and bootloader
password policy need careful handling. Tide should not claim secure boot support until implemented.

**Migration:** own kernel package selection and cmdline policy -> own initramfs configuration ->
evaluate UKI/secure boot -> keep upstream kernel.

## libc, core utilities, foundational libraries

**Inherited:** glibc, GNU coreutils, util-linux, OpenSSL, etc.

**Tide-owned now:** none.

**Feasibility:** Technically possible, practically low value. Replacing glibc or coreutils would
create ABI risk, security maintenance burden, and enormous validation cost.

**Recommendation:** remain upstream dependencies. If musl is ever evaluated, it requires a separate
ABI, package, and security analysis. It should not be presented as a branding exercise.

## Desktop

**Inherited:** KDE Plasma, Qt, KWin, SDDM, Breeze, XFCE, PipeWire, NetworkManager.

**Tide-owned now:** wallpaper, logo, colour scheme, look-and-feel metadata/defaults, skel config,
desktop package profiles, settings/tools integration points.

**Feasibility:** High for defaults and identity; low value for a full Plasma fork.

**Security implications:** display manager authentication, screen locking, portal permissions, and
sandboxed app integration remain upstream responsibilities. Tide-specific QML must not expose
privileged APIs.

**Migration:** validate the look-and-feel in a booted image, add Tide settings app, add accessibility
and low-end hardware profiles, and only then consider deeper shell customization.

## Security updates and advisories

**Inherited:** Debian Security Team advisories and fixed packages.

**Tide-owned now:** policy, release manifests, checksum verification, CI.

**Feasibility:** Tide can own notification and release process; owning vulnerability triage for all
Debian packages is not realistic initially.

**Migration:** subscribe to Debian advisories, map affected Tide images, publish Tide security notes,
add SBOM/provenance, then gradually own advisories for Tide-owned packages.

**Remain upstream:** Debian advisory data and package fixes.

## What should remain an upstream dependency

- Linux kernel and drivers.
- glibc and core system libraries.
- systemd and udev.
- GRUB/syslinux and initramfs generators until a tested replacement exists.
- Debian package archive and security updates for inherited packages.
- KDE Plasma, Qt, and desktop libraries.
- Firmware packages and upstream driver projects.
- Calamares until a validated Tide installer exists.

Tide independence is measured by control over its own distribution decisions, not by the percentage
of upstream code rewritten.
