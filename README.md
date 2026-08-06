# Tide

Tide is a Debian-based OS scaffold for building a custom distribution.

## Getting started

1. Install build dependencies:
   - `debootstrap`
   - `live-build` (optional, for ISO creation)

2. Bootstrap the base system:
   ```bash
   ./build.sh bootstrap
   ```

3. Configure the root filesystem:
   ```bash
   ./build.sh configure
   ```

4. Build a live ISO (optional):
   ```bash
   ./build.sh build-iso
   ```

5. Clean generated artifacts:
   ```bash
   ./build.sh clean
   ```

## Notes

- The scaffold uses Debian Bookworm by default.
- A `tide` user is created with password `tide`.
- The default root password is `root`.
- Customize package selection with `PACKAGES`.
- Customize codename and architecture with `CODENAME` and `ARCH`.
