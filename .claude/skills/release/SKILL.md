---
name: release
description: Prepare a Tide OS release — bump version, update docs, tag. Use when the user asks to cut, tag, or publish a release.
disable-model-invocation: true
---

# Cut a Tide release

The version lives in **three** places; keep them identical:
`VERSION`, `config/tide.yaml` (`os.version`), and `Cargo.toml` (`workspace.package.version`,
which uses plain semver without the pre-release suffix unless the user says otherwise).

1. Ask the user for the version (e.g. `0.1.0-alpha.2`) if not given.
2. Update the three files, then run the `validate` skill.
3. Summarise changes since the last tag: `git log $(git describe --tags --abbrev=0)..HEAD --oneline`.
4. Commit on a branch / as the user prefers.
5. **Stop and confirm** before `git tag v<version>` and `git push origin v<version>`.
   Pushing the tag triggers `.github/workflows/release.yml`, which builds and publishes the ISO
   publicly. This is outward-facing and hard to undo.
6. Remind the user: artifacts are checksummed but not signed, and the ISO is not boot-tested by CI.
