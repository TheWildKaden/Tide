---
name: add-backend
description: Add a new package-manager backend to the `tide` CLI (crates/tide-core). Use when asked to support another package manager such as zypper, xbps, nix, or snap.
---

# Add a `tide` backend

Read first: `crates/tide-core/src/backend.rs` (the `PackageBackend` trait) and the closest
existing adapter in `crates/tide-core/src/backends/` (e.g. `dnf.rs` for a system manager,
`flatpak.rs` for an app-level one). Also `docs/package-manager.md`.

## Steps

1. Create `crates/tide-core/src/backends/<name>.rs` implementing `PackageBackend`.
   - Build commands as executable + argv via the `CommandRunner`. **Never** build a shell string.
   - Validate package names with `util::validate_package_spec` before passing them on.
   - Honour `RunOptions` (`dry_run`, `assume_yes`, `purge`). Dry runs must not change the system.
   - Unsupported operations return `TideError::Unsupported`, never a fake success.
   - Declare accurate `capabilities()`.
2. Register it in `backends/mod.rs`: `pub mod`, `pub use`, and `BackendRegistry::new`.
   Add it to `PRIMARY_ORDER` only if it is a host's primary system package manager.
3. Add unit tests using `exec::MockRunner`: argv shape,
   output parsing, dry-run behaviour, invalid names rejected.
4. Document it in `docs/package-manager.md` and the README backend list.
5. Run the `validate` skill.

Do not run install/remove/upgrade against the real host to test.
