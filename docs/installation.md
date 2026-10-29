# Installation and image validation

> **Status:** The build pipeline and Calamares configuration exist. **No Tide image has been booted or
> installed in this repository's validation environment.** This page describes the intended flow and
> the checks required before any compatibility claim can be made.

## Producing an image

On a Debian/Ubuntu host with enough disk and the ISO dependencies:

```bash
./scripts/install-dev-deps.sh
./build.sh doctor
./build.sh all
```

Outputs:

```text
output/tide-<version>-<arch>.iso
output/tide-<version>-<arch>.iso.sha256
output/SHA256SUMS
output/manifests/tide-<version>-<arch>.manifest.json
```

Verify checksums:

```bash
./build.sh verify
sha256sum -c output/SHA256SUMS
```

## Before you boot anything

1. Read the manifest's `validation` block. `boot_tested`, `installer_tested`, and `hardware_tested`
   are `false` unless someone ran and recorded those checks.
2. Confirm the ISO size is plausible (multiple GiB for a KDE image).
3. Record the checksum and git commit.

## Booting in QEMU (BIOS)

Builds are `iso-hybrid`. A minimal smoke test:

```bash
qemu-system-x86_64 \
  -m 4096 \
  -smp 2 \
  -enable-kvm \
  -cdrom output/tide-<version>-amd64.iso \
  -boot d
```

Without KVM, drop `-enable-kvm` (much slower).

## Booting in QEMU (UEFI)

You need OVMF firmware:

```bash
qemu-system-x86_64 \
  -m 4096 \
  -smp 2 \
  -enable-kvm \
  -bios /usr/share/OVMF/OVMF_CODE.fd \
  -cdrom output/tide-<version>-amd64.iso \
  -boot d
```

Exact OVMF paths vary by distribution. If firmware is missing, install the distribution's OVMF package.

## Testing the installer safely

Never point Calamares at a disk containing important data.

1. Create a disposable virtual disk:

   ```bash
   qemu-img create -f qcow2 /tmp/tide-test.qcow2 24G
   ```

2. Boot with both the ISO and the disk:

   ```bash
   qemu-system-x86_64 \
     -m 4096 -smp 2 -enable-kvm \
     -cdrom output/tide-<version>-amd64.iso \
     -drive file=/tmp/tide-test.qcow2,format=qcow2,if=virtio \
     -boot d
   ```

3. Start Calamares from the live session and complete partitioning with the virtual disk only.
4. After install, boot the virtual disk without the ISO and confirm it reaches a login/desktop.
5. Record what worked: bootloader (BIOS/UEFI), filesystem, encryption, user creation, first-boot
   service result.

## What to record

- ISO checksum and git commit.
- Host and VM firmware (BIOS or UEFI), QEMU version.
- Kernel version reached.
- Desktop profile and whether SDDM/Plasma started.
- Display, audio, network, and input behavior.
- Calamares modules that failed.
- `systemctl --failed` and `journalctl -b -p err` output.
- `/var/log/tide/firstboot-doctor.json`.

Do not describe an image as bootable or the installer as working until these checks have run and the
results are recorded.

## Hardware testing

Hardware validation is separate from VM validation. At minimum check:

- UEFI and legacy boot,
- integrated and discrete graphics,
- Wi-Fi and Ethernet,
- suspend/resume,
- audio output and input,
- firmware update tooling (`fwupdmgr`),
- secure boot state (Tide does not claim secure boot support).

## Current limitations

- The installer has not run.
- Partition, encryption, and bootloader defaults are unvalidated.
- The default live user `tide` has a locked password and no passwordless sudo; confirm the intended
  live-session login path before distributing an image.
- Release assets are checksummed but not signed.
