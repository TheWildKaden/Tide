use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::backend::{unsupported, PackageBackend, RunOptions};
use crate::error::{Result, TideError};
use crate::exec::CommandRunner;
use crate::model::{
    BackendCapabilities, CheckLevel, DoctorFinding, OperationReport, PackageSummary, RepoEntry,
};
use crate::util::{display_command, validate_package_spec};

use super::{ensure_program, planned, run, run_privileged};

/// Adapter for Debian's APT/dpkg stack.
///
/// This is Tide's bridge to the Debian foundation. It deliberately uses the native
/// tools (`apt-get`, `apt-cache`, `dpkg-query`) so signature verification, maintainer
/// scripts, conffile handling, and locking retain APT's established behaviour.
#[derive(Debug, Clone)]
pub struct AptBackend {
    etc_apt: PathBuf,
    install_recommends: bool,
}

impl AptBackend {
    pub fn new(etc_apt: PathBuf, install_recommends: bool) -> Self {
        Self {
            etc_apt,
            install_recommends,
        }
    }

    pub fn etc_apt(&self) -> &Path {
        &self.etc_apt
    }

    fn install_args(&self, packages: &[String]) -> Vec<String> {
        let mut args = vec!["install".to_string(), "--yes".to_string()];
        if !self.install_recommends {
            args.push("--no-install-recommends".to_string());
        }
        args.push("--".to_string());
        args.extend(packages.iter().cloned());
        args
    }
}

const APT_ENV: &[(&str, &str)] = &[
    ("DEBIAN_FRONTEND", "noninteractive"),
    ("APT_LISTCHANGES_FRONTEND", "none"),
];

impl PackageBackend for AptBackend {
    fn key(&self) -> &'static str {
        "apt"
    }

    fn name(&self) -> &'static str {
        "APT / dpkg (Debian family)"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            search: true,
            info: true,
            list: true,
            install: true,
            remove: true,
            update: true,
            upgrade: true,
            outdated: true,
            repositories: true,
        }
    }

    fn detect(&self, runner: &dyn CommandRunner) -> bool {
        runner.which("apt-get").is_some() && runner.which("dpkg-query").is_some()
    }

    fn version(&self, runner: &dyn CommandRunner) -> Option<String> {
        let output = runner
            .run("apt-get", &["--version".to_string()], &[])
            .ok()?;
        output.stdout.lines().next().map(|line| line.trim().to_string())
    }

    fn notes(&self) -> Vec<String> {
        vec![
            "Uses Debian APT/dpkg as the trusted installation engine.".into(),
            "Repository and package signatures remain enforced by APT.".into(),
        ]
    }

    fn search(&self, runner: &dyn CommandRunner, query: &str) -> Result<Vec<PackageSummary>> {
        ensure_program(runner, "apt-cache")?;
        let output = run(
            runner,
            "apt-cache",
            &["search".to_string(), "--".to_string(), query.to_string()],
            &[],
        )?;
        Ok(parse_search(&output, "apt"))
    }

    fn info(&self, runner: &dyn CommandRunner, package: &str) -> Result<PackageSummary> {
        validate_package_spec(package, true)?;
        ensure_program(runner, "apt-cache")?;
        let output = run(
            runner,
            "apt-cache",
            &["show".to_string(), "--".to_string(), package.to_string()],
            &[],
        )?;
        parse_show(&output, package, "apt")
    }

    fn list_installed(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        ensure_program(runner, "dpkg-query")?;
        let format = r#"${binary:Package}\t${Version}\t${db:Status-Abbrev}\n"#;
        let output = run(
            runner,
            "dpkg-query",
            &["-W".to_string(), format!("-f={format}")],
            &[],
        )?;
        Ok(parse_dpkg_list(&output))
    }

    fn install(
        &self,
        runner: &dyn CommandRunner,
        packages: &[String],
        options: &RunOptions,
    ) -> Result<OperationReport> {
        validate_packages(packages)?;
        let args = self.install_args(packages);
        if options.dry_run {
            return Ok(OperationReport::planned(
                format!("install {} package(s) with APT", packages.len()),
                planned("apt-get", &args),
            ));
        }
        let output = run_privileged(runner, "apt-get", &args, APT_ENV)?;
        Ok(OperationReport::completed(
            format!("installed {} package(s) with APT", packages.len()),
            collect_lines(&output),
        ))
    }

    fn remove(
        &self,
        runner: &dyn CommandRunner,
        packages: &[String],
        options: &RunOptions,
    ) -> Result<OperationReport> {
        validate_packages(packages)?;
        let mut args = vec!["remove".to_string(), "--yes".to_string()];
        if options.purge {
            args.push("--purge".to_string());
        }
        args.push("--".to_string());
        args.extend(packages.iter().cloned());
        if options.dry_run {
            return Ok(OperationReport::planned(
                format!("remove {} package(s) with APT", packages.len()),
                display_command("apt-get", &args),
            ));
        }
        let output = run_privileged(runner, "apt-get", &args, APT_ENV)?;
        Ok(OperationReport::completed(
            format!("removed {} package(s) with APT", packages.len()),
            collect_lines(&output),
        ))
    }

    fn update(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let output = run_privileged(runner, "apt-get", &["update".to_string()], APT_ENV)?;
        Ok(OperationReport::completed(
            "APT package metadata updated".to_string(),
            collect_lines(&output),
        ))
    }

    fn upgrade(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let args = vec![
            "upgrade".to_string(),
            "--yes".to_string(),
            "--with-new-pkgs".to_string(),
        ];
        let output = run_privileged(runner, "apt-get", &args, APT_ENV)?;
        Ok(OperationReport::completed(
            "APT packages upgraded".to_string(),
            collect_lines(&output),
        ))
    }

    fn outdated(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let args = vec![
            "-s".to_string(),
            "-o".to_string(),
            "Debug::NoLocking=1".to_string(),
            "upgrade".to_string(),
        ];
        let output = run(runner, "apt-get", &args, APT_ENV)?;
        Ok(parse_simulated_upgrade(&output))
    }

    fn repositories(&self, _runner: &dyn CommandRunner) -> Result<Vec<RepoEntry>> {
        let mut entries = parse_apt_sources(&self.etc_apt);
        entries.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(entries)
    }

    fn doctor(&self, runner: &dyn CommandRunner) -> Result<Vec<DoctorFinding>> {
        let mut findings = Vec::new();
        let missing = ["apt-get", "apt-cache", "dpkg-query"]
            .iter()
            .filter(|program| runner.which(program).is_none())
            .copied()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            findings.push(DoctorFinding::new(
                CheckLevel::Ok,
                "apt",
                "APT and dpkg-query are present",
            ));
        } else {
            findings.push(DoctorFinding::new(
                CheckLevel::Error,
                "apt",
                format!("missing commands: {}", missing.join(", ")),
            ));
        }
        let output = runner.run("apt-get", &["check".to_string()], &[]);
        match output {
            Ok(_) => findings.push(DoctorFinding::new(
                CheckLevel::Ok,
                "apt",
                "APT dependency check passed",
            )),
            Err(error) => findings.push(
                DoctorFinding::new(CheckLevel::Error, "apt", error.to_string())
                    .with_remediation("Inspect /var/lib/dpkg/status and /etc/apt/sources.list"),
            ),
        }
        let repos = self.repositories(runner)?;
        let unsigned = repos
            .iter()
            .filter(|repo| repo.enabled && repo.signed_by.is_none() && repo.manager == "apt")
            .count();
        if unsigned > 0 {
            findings.push(DoctorFinding::new(
                CheckLevel::Info,
                "apt",
                format!(
                    "{unsigned} APT source(s) rely on the system keyring rather than a per-source signed-by key"
                ),
            ));
        }
        if !runner.is_privileged() {
            findings.push(
                DoctorFinding::new(
                    CheckLevel::Info,
                    "apt",
                    "Tide is not running as root; install/remove will request sudo",
                )
                .with_remediation("Run read-only commands without sudo; approve the sudo prompt for changes"),
            );
        }
        Ok(findings)
    }
}

fn validate_packages(packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Err(TideError::InvalidInput {
            kind: "package list",
            message: "at least one package is required".into(),
        });
    }
    for package in packages {
        validate_package_spec(package, true)?;
    }
    Ok(())
}

fn collect_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub(crate) fn parse_search(output: &str, backend: &str) -> Vec<PackageSummary> {
    let mut packages = Vec::new();
    for line in output.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (name, description) = match line.split_once(" - ") {
            Some((name, description)) => (name.trim(), Some(description.trim().to_string())),
            None => (line.trim(), None),
        };
        if name.is_empty() {
            continue;
        }
        packages.push(PackageSummary {
            backend: backend.to_string(),
            name: name.to_string(),
            version: None,
            description,
            installed: None,
        });
    }
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    packages.dedup_by(|left, right| left.name == right.name);
    packages
}

fn parse_show(output: &str, requested: &str, backend: &str) -> Result<PackageSummary> {
    let stanza = output.split("\n\n").next().unwrap_or_default();
    let fields = parse_deb822_fields(stanza);
    let name = fields
        .get("Package")
        .cloned()
        .unwrap_or_else(|| requested.to_string());
    Ok(PackageSummary {
        backend: backend.to_string(),
        name,
        version: fields.get("Version").cloned(),
        description: fields.get("Description").cloned(),
        installed: None,
    })
}

fn parse_deb822_fields(stanza: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in stanza.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(key) = &current {
                let value = fields.entry(key.clone()).or_default();
                if !value.is_empty() {
                    value.push('\n');
                }
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            current = Some(key.trim().to_string());
            fields.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    fields
}

fn parse_dpkg_list(output: &str) -> Vec<PackageSummary> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.trim();
            let version = parts.next().map(str::trim).filter(|value| !value.is_empty());
            let status = parts.next().unwrap_or_default().trim();
            if name.is_empty() {
                return None;
            }
            let installed = status.starts_with("ii") || status.starts_with("hi");
            Some(PackageSummary {
                backend: "apt".to_string(),
                name: name.to_string(),
                version: version.map(str::to_string),
                description: None,
                installed: Some(installed),
            })
        })
        .filter(|package| package.installed == Some(true))
        .collect()
}

fn parse_simulated_upgrade(output: &str) -> Vec<PackageSummary> {
    let mut packages = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if !line.starts_with("Inst ") {
            continue;
        }
        let mut parts = line.split_whitespace();
        parts.next();
        if let Some(name) = parts.next() {
            if !name.is_empty() {
                packages.push(PackageSummary {
                    backend: "apt".to_string(),
                    name: name.to_string(),
                    version: None,
                    description: None,
                    installed: Some(true),
                });
            }
        }
    }
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    packages.dedup_by(|left, right| left.name == right.name);
    packages
}

/// Parse both one-line `.list` sources and deb822 `.sources` files.
pub fn parse_apt_sources(etc_apt: &Path) -> Vec<RepoEntry> {
    let mut entries = Vec::new();
    let list_file = etc_apt.join("sources.list");
    if let Ok(content) = std::fs::read_to_string(&list_file) {
        parse_list_sources(&content, &mut entries);
    }
    let source_dir = etc_apt.join("sources.list.d");
    if let Ok(read_dir) = std::fs::read_dir(&source_dir) {
        let mut files = read_dir
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                matches!(
                    path.extension().and_then(|value| value.to_str()),
                    Some("list" | "sources")
                )
            })
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if path.extension().and_then(|value| value.to_str()) == Some("sources") {
                    parse_deb822_sources(&content, &mut entries);
                } else {
                    parse_list_sources(&content, &mut entries);
                }
            }
        }
    }
    entries
}

fn parse_list_sources(content: &str, entries: &mut Vec<RepoEntry>) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let enabled = !line.starts_with('#');
        let mut parts = line.split_whitespace();
        let kind = match parts.next() {
            Some("deb") | Some("deb-src") => parts.next(),
            _ => None,
        };
        let Some(uri) = kind else { continue };
        let suite = parts.next().unwrap_or_default().to_string();
        let components = parts.map(str::to_string).collect::<Vec<_>>();
        entries.push(RepoEntry {
            id: format!("{}-{}", sanitize_id(uri), entries.len()),
            manager: "apt".to_string(),
            uri: Some(uri.to_string()),
            suites: vec![suite],
            components,
            enabled,
            signed_by: None,
        });
    }
}

fn parse_deb822_sources(content: &str, entries: &mut Vec<RepoEntry>) {
    for stanza in content.split("\n\n") {
        let fields = parse_deb822_fields(stanza);
        if fields.is_empty() {
            continue;
        }
        let enabled = fields
            .get("Enabled")
            .map(|value| value != "no")
            .unwrap_or(true);
        let uri = fields.get("URIs").cloned();
        if uri.is_none() {
            continue;
        }
        let uri_value = uri.clone().unwrap_or_default();
        let suites = fields
            .get("Suites")
            .map(|value| value.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        let components = fields
            .get("Components")
            .map(|value| value.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        entries.push(RepoEntry {
            id: format!("{}-{}", sanitize_id(&uri_value), entries.len()),
            manager: "apt".to_string(),
            uri,
            suites,
            components,
            enabled,
            signed_by: fields.get("Signed-By").cloned(),
        });
    }
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::MockRunner;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tide-apt-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn search_parser_handles_description_separator() {
        let packages = parse_search("firefox - web browser\ncurl - tool\n", "apt");
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "firefox");
        assert_eq!(packages[0].description.as_deref(), Some("web browser"));
    }

    #[test]
    fn show_parser_extracts_metadata() {
        let package = parse_show(
            "Package: firefox\nVersion: 1.2\nDescription: web browser\n long text\n\n",
            "firefox",
            "apt",
        )
        .unwrap();
        assert_eq!(package.version.as_deref(), Some("1.2"));
        assert!(package.description.unwrap().contains("long text"));
    }

    #[test]
    fn dpkg_parser_only_returns_installed() {
        let packages = parse_dpkg_list("bash\t5.2\tii \nold\t1.0\trc \n");
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "bash");
    }

    #[test]
    fn parsed_upgrade_inst_lines() {
        let packages = parse_simulated_upgrade("Inst bash [5.1] (5.2 Debian:stable [amd64])\n");
        assert_eq!(packages[0].name, "bash");
    }

    #[test]
    fn parses_list_and_deb822_sources() {
        let dir = temp_dir("sources");
        fs::write(
            dir.join("sources.list"),
            "deb http://deb.debian.org/debian bookworm main contrib\n# deb http://example.invalid/x stable main\n",
        )
        .unwrap();
        fs::create_dir_all(dir.join("sources.list.d")).unwrap();
        fs::write(
            dir.join("sources.list.d/tide.sources"),
            "Types: deb\nURIs: https://repo.tideos.example/tide\nSuites: stable\nComponents: main\nSigned-By: /usr/share/keyrings/tide.gpg\n",
        )
        .unwrap();
        let repos = parse_apt_sources(&dir);
        assert_eq!(repos.len(), 2);
        let debian = repos.iter().find(|repo| repo.uri.as_deref() == Some("http://deb.debian.org/debian")).unwrap();
        assert!(debian.enabled);
        let tide = repos.iter().find(|repo| repo.manager == "apt" && repo.suites == ["stable"]).unwrap();
        assert_eq!(tide.signed_by.as_deref(), Some("/usr/share/keyrings/tide.gpg"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn install_args_never_use_a_shell_and_reject_options() {
        let backend = AptBackend::new(PathBuf::from("/etc/apt"), false);
        let args = backend.install_args(&["firefox".to_string()]);
        assert!(args.contains(&"--no-install-recommends".to_string()));
        assert!(args.contains(&"--".to_string()));
        let runner = MockRunner::with_programs(&["apt-get", "dpkg-query"]);
        let error = backend
            .install(
                &runner,
                &["--allow-unauthenticated".to_string()],
                &RunOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(error, TideError::InvalidInput { .. }));
    }

    #[test]
    fn dry_run_does_not_execute() {
        let backend = AptBackend::new(PathBuf::from("/etc/apt"), true);
        let runner = MockRunner::with_programs(&["apt-get", "dpkg-query"]);
        let report = backend
            .install(
                &runner,
                &["firefox".to_string()],
                &RunOptions {
                    dry_run: true,
                    ..RunOptions::default()
                },
            )
            .unwrap();
        assert!(!report.changed);
        assert!(runner.calls.borrow().is_empty());
    }
}
