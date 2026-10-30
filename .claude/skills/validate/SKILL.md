---
name: validate
description: Run Tide's full check suite (Python tests, rustfmt, clippy, cargo test, shell syntax/ShellCheck, config and asset parsing) and fix what fails. Use after changes and before committing.
---

# Validate Tide

1. Run `./scripts/validate.sh` (same as `make test`). It mirrors `.github/workflows/ci.yml`.
2. If it fails, fix the root cause, not the check. Common fixes:
   - rustfmt: `cargo fmt --all`
   - clippy: fix the warning; do not add `#[allow]` without a comment explaining why
   - Python tests: `python3 -m unittest discover -s tests -v` for detail
3. Re-run until green.
4. Report the result honestly. Passing validation does **not** mean an ISO boots or installs;
   never say it does.

For a quick inner loop on one area:
- Rust only: `cargo test -p tide-core` / `-p tide-cli` / `-p tidectl`
- Python only: `python3 -m unittest discover -s tests`
