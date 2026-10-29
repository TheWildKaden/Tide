# Release engineering

## Artifacts

A complete build produces:

| File | Purpose |
|---|---|
| `output/tide-<version>-<arch>.iso` | Hybrid live image |
| `output/tide-<version>-<arch>.iso.sha256` | Per-ISO checksum |
| `output/SHA256SUMS` | Combined checksums for ISO and manifest |
| `output/manifests/tide-<version>-<arch>.manifest.json` | Build provenance and validation flags |
| `.tide-build/logs/tide-build.log` | Full build log (not published by default) |

## Manifest contents

`builder/manifest.py` records:

- schema version, product, version, channel, architecture, codename,
- Debian foundation (codename, mirror, debootstrap/live-build versions),
- host architecture, Python version, UTC timestamp,
- git commit and dirty flag,
- stage state snapshots,
- desktop profile and resolved package list,
- artifact size and SHA-256,
- validation booleans.

Validation defaults:

```json
{
  "compiled": true,
  "iso_generated": true,
  "linux_units_tested": false,
  "boot_tested": false,
  "installer_tested": false,
  "hardware_tested": false
}
```

Only checks that actually ran should ever be set to `true`.

## Verifying

```bash
./build.sh verify
```

The verifier:

- parses each checksum file,
- rejects malformed lines and path traversal (`../`, absolute paths, backslashes),
- allows only files in the output directory (or `manifests/`),
- compares SHA-256 digests.

Any mismatch is an error and a non-zero exit.

## Signing status

**Release assets are not signed.** Checksums detect corruption and accidental modification; they do
not prove who produced the file. This is a known gap and must be stated in release notes.

Planned signing work:

1. Generate an offline release key (never committed).
2. Sign `SHA256SUMS` (detached signature) or use sigstore/cosign-style provenance.
3. Publish the public key and fingerprint in documentation.
4. Document key rotation and revocation.
5. Add verification instructions that fail closed.

Until then, users should obtain checksums from the same release page and treat them as integrity—not
authenticity—evidence.

## GitHub Actions

| Workflow | Trigger | Behavior |
|---|---|---|
| `ci.yml` | push to `main`, pull request, manual | Rust fmt/clippy/test/release build; Python tests; ShellCheck; asset parsing; doctor |
| `build-iso.yml` | manual | Builds an ISO for a chosen desktop profile, verifies, uploads artifact |
| `release.yml` | published release | Builds, verifies, uploads assets to the release |

Permissions are read-only except `release.yml`, which has `contents: write` to attach assets. No
secrets are exposed to pull-request workflows.

## Versioning

- `VERSION` and `config/tide.yaml` carry the version.
- `Cargo.toml` workspace version matches.
- Current channel: `development`.
- Version string format: `<major>.<minor>.<patch>-<stage>.<n>`.

When bumping the version, update:

1. `VERSION`
2. `config/tide.yaml` (`os.version`)
3. `Cargo.toml` (`workspace.package.version`)
4. `assets/branding/os-release` (`VERSION`, `PRETTY_NAME`)
5. `assets/branding/issue`, `issue.net`
6. `assets/calamares/branding/tide/branding.desc`

## Release checklist

- [ ] CI green on the release commit.
- [ ] `./scripts/validate.sh` passes.
- [ ] ISO built from a clean work directory.
- [ ] `./build.sh verify` passes.
- [ ] Manifest validation flags reviewed; no false `true` values.
- [ ] Boot tested in QEMU (BIOS and UEFI) — record results.
- [ ] Installer tested on a disposable disk — record results.
- [ ] First-boot diagnostic verified.
- [ ] Release notes state that assets are unsigned and list known gaps.
- [ ] Checksums published alongside assets.
