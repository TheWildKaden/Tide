# Tide OS Security Policy

**Project:** Tide OS  
**Security status:** Actively maintained  
**Current foundation:** Debian-based Linux distribution

## 1. Security Philosophy

Tide OS prioritizes security, integrity, privacy, transparency, and system reliability.

Tide currently builds upon Debian infrastructure, but its long-term goal is to progressively replace Debian-specific components with independently maintained Tide implementations.

Security must remain a requirement throughout this transition.

The project follows these principles:

- Secure by default.
- Least-privilege access.
- Defense in depth.
- Verified software and dependencies.
- Minimal attack surface.
- Transparent vulnerability handling.
- Reproducible and auditable builds.
- No unnecessary collection of user information.
- No silent security degradation.

Security must never be sacrificed simply to make a feature easier to implement.

## 2. Supported Versions

Security fixes are prioritized in this order:

1. Latest stable Tide release.
2. Latest development branch.
3. Older releases when a critical vulnerability warrants a backport.

Development builds are not guaranteed to receive the same level of security maintenance as stable releases.

Users should update to the latest supported release whenever possible.

## 3. Reporting Security Vulnerabilities

**Do not disclose security vulnerabilities through public GitHub issues, discussions, or pull requests.**

Use GitHub's private vulnerability reporting feature when available:

1. Open the Tide OS repository.
2. Navigate to the Security tab.
3. Select Advisories.
4. Choose Report a vulnerability.
5. Submit a private report with reproduction details.

If private vulnerability reporting is unavailable, contact the repository maintainer privately through GitHub.

Please include:

- Vulnerability description.
- Affected component.
- Affected Tide version or commit.
- Reproduction steps.
- Potential impact.
- Suggested mitigation, if known.
- Proof of concept, if necessary.

Do not include real credentials, personal information, or sensitive production data in reports.

### Response expectations

The project aims to:

- Acknowledge reports within 72 hours.
- Perform initial triage within 7 days.
- Keep the reporter informed of significant progress.
- Prioritize actively exploited and critical vulnerabilities.
- Coordinate a responsible disclosure timeline.

These are response targets, not guaranteed service-level agreements.

## 4. Security Scope

The following components are considered security-sensitive:

### Operating system

- Kernel configuration and integration.
- Bootloader and boot process.
- Initramfs generation.
- System initialization.
- Service management.
- User and permission management.
- Privileged system utilities.
- System configuration.
- Network services.

### Package management

- Tide package manager.
- APT/dpkg integration.
- Package installation and removal.
- Dependency resolution.
- Repository metadata.
- Package signatures.
- Repository signing keys.
- Download and update mechanisms.
- Package scripts and installation hooks.

### Desktop environment

- KDE Plasma integration.
- Tide-specific desktop utilities.
- Display manager configuration.
- Authentication interfaces.
- Settings applications.
- Inter-process communication.
- Privileged desktop operations.

### Build and distribution infrastructure

- ISO generation.
- Build scripts.
- GitHub Actions workflows.
- Release automation.
- Artifact integrity.
- Update distribution.
- Download infrastructure.
- Installer components.

## 5. Package and Dependency Security

All software incorporated into Tide must be evaluated for its origin, integrity, licensing, maintenance status, and security implications.

Requirements:

- Prefer official and verifiable software repositories.
- Verify package signatures and repository metadata.
- Never silently disable signature verification.
- Avoid unmaintained or unnecessary dependencies.
- Pin dependencies where appropriate.
- Review dependency changes before merging.
- Do not execute untrusted installation scripts.
- Validate downloaded artifacts.
- Keep dependency lockfiles committed where applicable.
- Investigate known vulnerabilities before release.

### Tide-native package management

As Tide develops its own package infrastructure, the following requirements apply:

- Cryptographically signed packages.
- Trusted repository metadata.
- Secure key management and rotation.
- Integrity verification before installation.
- Safe dependency resolution.
- Protection against package substitution.
- Protection against downgrade attacks where appropriate.
- Atomic or recoverable updates where feasible.
- Clear reporting of verification failures.

Package installation must not silently continue when integrity verification fails.

## 6. Privileged Operations

Operations requiring root privileges must be carefully restricted.

Requirements:

- Use least-privilege execution.
- Avoid running ordinary applications as root.
- Validate user input before privileged operations.
- Never construct unsafe shell commands from untrusted input.
- Avoid unnecessary setuid executables.
- Validate filesystem paths before modifying system files.
- Protect sensitive configuration files.
- Never expose unrestricted privileged APIs to desktop applications.

Privileged operations must fail safely when validation or authorization fails.

## 7. Secrets and Credentials

The following must never be committed to the repository:

- Passwords.
- API keys.
- Access tokens.
- Private signing keys.
- SSH private keys.
- Database credentials.
- Personal access tokens.
- Production secrets.
- Authentication cookies.
- Private environment configuration.

Use environment variables, secure secret storage, or GitHub Actions secrets where appropriate.

Never place signing private keys inside public build artifacts or release archives.

If a credential is accidentally committed:

1. Revoke or rotate it immediately.
2. Identify potentially affected systems.
3. Remove it from the repository.
4. Assess whether Git history needs remediation.
5. Document the incident without republishing the secret.

Deleting a secret from the latest commit does not invalidate a previously exposed credential.

## 8. Build and Release Security

Every official Tide release should have a verifiable origin and integrity record.

Requirements:

- Build from reviewed source code.
- Use trusted build dependencies.
- Validate build scripts.
- Protect release credentials.
- Generate SHA-256 checksums.
- Publish release manifests.
- Verify generated artifacts before distribution.
- Restrict release permissions.
- Avoid executing untrusted pull-request code with privileged credentials.
- Keep build logs available for auditing.

Where practical, implement:

- Reproducible builds.
- Signed release manifests.
- Signed ISO images or detached signatures.
- Software bill of materials (SBOM).
- Build provenance and artifact attestations.

A checksum verifies file integrity against a trusted checksum source; it does not independently prove authenticity.

## 9. GitHub Repository Security

The repository should use the following protections:

- Dependabot alerts.
- Dependabot security updates.
- Secret scanning.
- Push protection.
- Code scanning.
- Dependency graph.
- Protected release branches.
- Pull request review.
- Required CI checks.
- Restricted GitHub Actions permissions.

GitHub Actions should use the minimum required permissions.

Third-party Actions should be reviewed and pinned to immutable commit SHAs where practical.

Do not expose secrets to untrusted pull-request workflows.

## 10. Security Testing

Security-sensitive components should receive appropriate testing before release.

Testing should include:

- Static analysis.
- Dependency vulnerability checks.
- Input validation tests.
- Permission and authorization tests.
- Package signature verification tests.
- Installation and rollback tests.
- Filesystem safety tests.
- Privilege escalation testing.
- Network security testing.
- Build artifact integrity checks.

Rust components should use Clippy and appropriate Rust security practices.

C and C++ components should use compiler warnings, sanitizers, and static analysis where practical.

Shell scripts should be checked with ShellCheck.

Security testing must be performed in controlled development environments. Do not test destructive installation or partitioning behavior on systems containing important data.

## 11. Privacy and Telemetry

Tide aims to minimize unnecessary collection of user information.

Requirements:

- No undisclosed telemetry.
- No hidden data collection.
- No transmission of credentials or personal files.
- No unnecessary network requests.
- Clear disclosure of any diagnostic reporting.
- User control over optional telemetry.
- Secure handling of diagnostic information.

Network connections and update checks should be documented.

## 12. Vulnerability Severity

Security issues should be prioritized based on impact, exploitability, and affected users.

| Severity | Examples | Priority |
|---|---|---|
| Critical | Remote root compromise, widespread package-signing compromise | Immediate |
| High | Privilege escalation, authentication bypass, arbitrary privileged execution | Urgent |
| Medium | Significant information disclosure, limited privilege bypass | High |
| Low | Limited security impact or defense-in-depth weakness | Normal |

Severity classifications may be adjusted as more information becomes available.

## 13. Responsible Disclosure

Security researchers are expected to:

- Avoid accessing other people's data.
- Avoid disrupting services.
- Avoid destructive testing.
- Avoid public disclosure before coordinated remediation.
- Provide sufficient reproduction information.
- Respect user privacy.

The Tide maintainers will make reasonable efforts to investigate valid reports and coordinate fixes.

## 14. Security Development Rules

Every contributor and automated coding agent working on Tide must follow these rules:

1. Never introduce hardcoded credentials.
2. Never disable security checks simply to make tests pass.
3. Never bypass package signature verification.
4. Never execute untrusted code with unnecessary privileges.
5. Never introduce undocumented network communication.
6. Never silently weaken system permissions.
7. Never conceal security failures.
8. Never mark an untested security feature as complete.
9. Never introduce dependencies without evaluating their trustworthiness.
10. Always document security-sensitive architectural decisions.

Automated agents must not assume that generated code is secure simply because it compiles or passes ordinary tests.

## 15. Security Roadmap

Tide's security infrastructure should evolve alongside its independence from Debian.

Long-term objectives:

- Tide-controlled package signing infrastructure.
- Tide-native secure repository metadata.
- Independent package integrity verification.
- Signed system updates.
- Secure boot integration where practical.
- Verified installation media.
- Reproducible release builds.
- Automated vulnerability monitoring.
- Software bill of materials for releases.
- System recovery and rollback mechanisms.
- Independent security audits of critical Tide components.

Replacing Debian infrastructure must not weaken existing security guarantees.

## 16. Policy Changes

This policy may evolve as Tide develops.

Security-related changes should be documented and reviewed. Any significant reduction in security protections must be explicitly justified and disclosed.

**Tide OS — Security through verification, transparency, and control.**
