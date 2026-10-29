# Debian migration notes

Tide is a Debian-based distribution that intends to own more of its own infrastructure over time.
This page summarizes the practical migration strategy and the boundaries that already exist in the
code. Detailed per-subsystem analysis is in [DEBIAN_DEPENDENCIES.md](../DEBIAN_DEPENDENCIES.md).

## What Debian provides today

- package archive and security updates,
- APT/dpkg and repository signatures,
- Linux kernel and firmware packages,
- GRUB/syslinux, initramfs-tools, systemd,
- most desktop and userspace packages,
- Calamares (Tide supplies configuration and branding).

## What Tide owns today

- distribution identity (`os-release`, issue, logo, wallpaper),
- build orchestration (`builder/`) and configuration (`config/tide.yaml`),
- `tide` package-manager interface and adapters,
- `tidectl` diagnostics and system inspection,
- desktop defaults and package profiles,
- Calamares configuration and first-boot diagnostic,
- versioning, manifests, checksums, CI.

## Boundary rules

1. Debian assumptions live in:
   - `crates/tide-core/src/backends/apt.rs`,
   - `builder/bootstrap.py`, `builder/chroot.py`, `builder/livebuild.py`,
   - `rootfs-config.sh`, `live-build/`, `config/tide.yaml`.
2. The rest of `tide-core`, `tide-cli`, and `tidectl` must not hard-code Debian-only paths or tools
   outside the APT adapter.
3. Documentation must say “Debian-based” not “independent” until the image builder, repositories, and
   package format no longer depend on Debian.

## Migration sequence

### 1. Tide-owned repositories and release metadata

Highest value, lowest risk. Start with Tide metadata and a small set of Tide packages while Debian
continues to supply the base.

Requirements: signed metadata, key rotation, rollback protection, freshness checks, and mirror
compromise handling.

### 2. Tide configuration and update policy

Extend `tidectl` and add a settings surface. Do not silently change the system. Updates should be
explicit and recoverable.

### 3. Tide image builder

Replace `live-build` behind the existing stage contract. A new builder must:

- bootstrap a rootfs,
- apply an immutable package plan,
- configure identity and defaults,
- create the root filesystem image,
- assemble boot metadata,
- verify and record provenance.

Keep live-build until the replacement passes the same boot tests.

### 4. Installer

Keep Calamares as the engine, replace Debian-specific defaults, then add Tide first-boot setup. Do not
write a new installer before the current flow is validated on real virtual disks.

### 5. Package management

Introduce a Tide-native backend only after:

- repository metadata is signed,
- package hashes are verified,
- transactions are recoverable,
- script policy is defined,
- rollback is tested.

Keep the APT adapter for compatibility. Do not claim the backends share dependency semantics.

### 6. Boot chain

Owning GRUB/initramfs configuration is feasible. Replacing the kernel or bootloader wholesale is not a
near-term goal.

## What should stay upstream

- Linux kernel and drivers,
- glibc and core libraries,
- systemd and udev,
- GRUB/syslinux and initramfs generators,
- Debian security advisories for inherited packages,
- KDE Plasma, Qt, and desktop libraries,
- firmware packages,
- Calamares until a validated replacement exists.

## Anti-goals

- Rebranding Debian and calling it independent.
- Forking Plasma without a maintenance plan.
- Writing a package resolver without signature and transaction tests.
- Replacing glibc/coreutils for branding reasons.
- Disabling signature or privilege checks to make a build pass.
