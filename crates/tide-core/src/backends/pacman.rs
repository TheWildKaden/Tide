use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::backend::{PackageBackend, RunOptions};
use crate::error::{Result, TideError};
use crate::exec::CommandRunner;
use crate::model::{
    BackendCapabilities, CheckLevel, DoctorFinding, OperationReport, PackageSummary, RepoEntry,
};
use crate::util::validate_package_spec;

use super::{ensure_program, planned, run, run_privileged};

/// Adapter for Arch Linux's pacman/libalpm frontend.
#[derive(Debug, Clone)]
pub struct PacmanBackend {
    config_file: PathBuf,
}

impl PacmanBackend {
    pub fn new(config_file: PathBuf) -> Self {
        Self { config_file }
    }
}

impl PackageBackend for PacmanBackend {
    fn key(&self) -> &'static str {
        "pacman"
    }

    fn name(&self) -> &'static str {
        "pacman / libalpm (Arch family)"
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
        runner.which("pacman").is_some()
    }

    fn version(&self, runner: &dyn CommandRunner) -> Option<String> {
        let output = runner.run("pacman", &["--version".to_string()], &[]).ok()?;
        output.stdout.lines().next().map(|line| line.trim().to_string())
    }

    fn notes(&self) -> Vec<String> {
        vec![
            "pacman uses its own dependency model and partial-upgrade constraints.".into(),
            "Tide does not run -Sy without -u as part of normal upgrades.".into(),
        ]
    }

    fn search(&self, runner: &dyn CommandRunner, query: &str) -> Result<Vec<PackageSummary>> {
        ensure_program(runner, "pacman")?;
        let output = run(
            runner,
            "pacman",
            &["-Ss".to_string(), "--".to_string(), query.to_string()],
            &[],
        )?;
        Ok(parse_search(&output))
    }

    fn info(&self, runner: &dyn CommandRunner, package: &str) -> Result<PackageSummary> {
        validate_package_spec(package, false)?;
        ensure_program(runner, "pacman")?;
        let output = run(
            runner,
            "pacman",
            &["-Si".to_string(), "--".to_string(), package.to_string()],
            &[],
        )?;
        let fields = parse_colon_fields(&output);
        Ok(PackageSummary {
            backend: "pacman".to_string(),
            name: fields.get("Name").cloned().unwrap_or_else(|| package.to_string()),
            version: fields.get("Version").cloned(),
            description: fields.get("Description").cloned(),
            installed: None,
        })
    }

    fn list_installed(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let output = run(runner, "pacman", &["-Q".to_string()], &[])?;
        Ok(output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let name = parts.next()?;
                Some(PackageSummary {
                    backend: "pacman".to_string(),
                    name: name.to_string(),
                    version: parts.next().map(str::to_string),
                    description: None,
                    installed: Some(true),
                })
            })
            .collect())
    }

    fn install(
        &self,
        runner: &dyn CommandRunner,
        packages: &[String],
        options: &RunOptions,
    ) -> Result<OperationReport> {
        validate_packages(packages)?;
        let mut args = vec![
            "-S".to_string(),
            "--noconfirm".to_string(),
            "--needed".to_string(),
            "--".to_string(),
        ];
        args.extend(packages.iter().cloned());
        if options.dry_run {
            return Ok(OperationReport::planned(
                format!("install {} package(s) with pacman", packages.len()),
                planned("pacman", &args),
            ));
        }
        let output = run_privileged(runner, "pacman", &args, &[])?;
        Ok(OperationReport::completed(
            format!("installed {} package(s) with pacman", packages.len()),
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
        let mut args = vec!["-R".to_string(), "--noconfirm".to_string(), "--".to_string()];
        if options.purge {
            args = vec![
                "-Rns".to_string(),
                "--noconfirm".to_string(),
                "--".to_string(),
            ];
        }
        args.extend(packages.iter().cloned());
        if options.dry_run {
            return Ok(OperationReport::planned(
                format!("remove {} package(s) with pacman", packages.len()),
                planned("pacman", &args),
            ));
        }
        let output = run_privileged(runner, "pacman", &args, &[])?;
        Ok(OperationReport::completed(
            format!("removed {} package(s) with pacman", packages.len()),
            collect_lines(&output),
        ))
    }

    fn update(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let output = run_privileged(runner, "pacman", &["-Sy".to_string()], &[])?;
        Ok(OperationReport::completed(
            "pacman databases synchronized".to_string(),
            collect_lines(&output),
        ))
    }

    fn upgrade(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let args = vec![
            "-Syu".to_string(),
            "--noconfirm".to_string(),
        ];
        let output = run_privileged(runner, "pacman", &args, &[])?;
        Ok(OperationReport::completed(
            "pacman packages upgraded".to_string(),
            collect_lines(&output),
        ))
    }

    fn outdated(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let output = run(runner, "pacman", &["-Qu".to_string()], &[])?;
        Ok(output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let name = parts.next()?;
                Some(PackageSummary {
                    backend: "pacman".to_string(),
                    name: name.to_string(),
                    version: parts.next().map(str::to_string),
                    description: None,
                    installed: Some(true),
                })
            })
            .collect())
    }

    fn repositories(&self, _runner: &dyn CommandRunner) -> Result<Vec<RepoEntry>> {
        Ok(parse_pacman_conf(&self.config_file))
    }

    fn doctor(&self, runner: &dyn CommandRunner) -> Result<Vec<DoctorFinding>> {
        let mut findings = Vec::new();
        if runner.which("pacman").is_some() {
            findings.push(DoctorFinding::new(CheckLevel::Ok, "pacman", "pacman is present"));
        } else {
            findings.push(DoctorFinding::new(CheckLevel::Error, "pacman", "pacman was not found"));
        }
        let repos = self.repositories(runner)?;
        if repos.is_empty() {
            findings.push(DoctorFinding::new(
                CheckLevel::Warning,
                "pacman",
                "no pacman repositories were found",
            ));
        }
        Ok(findings)
    }
}

fn collect_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(8)
        .map(str::to_string)
        .collect()
}

fn validate_packages(packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Err(TideError::InvalidInput {
            kind: "package list",
            message: "at least one package is required".into(),
        });
    }
    for package in packages {
        validate_package_spec(package, false)?;
    }
    Ok(())
}

fn parse_search(output: &str) -> Vec<PackageSummary> {
    let mut packages = Vec::new();
    let mut pending: Option<PackageSummary> = None;
    for line in output.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(mut package) = pending.take() {
                package.description = Some(line.trim().to_string());
                packages.push(package);
            }
            continue;
        }
        if let Some(package) = pending.take() {
            packages.push(package);
        }
        // repo/name version
        let mut parts = line.split_whitespace();
        let full_name = parts.next().unwrap_or_default();
        let name = full_name.rsplit('/').next().unwrap_or(full_name);
        if name.is_empty() {
            continue;
        }
        pending = Some(PackageSummary {
            backend: "pacman".to_string(),
            name: name.to_string(),
            version: parts.next().map(str::to_string),
            description: None,
            installed: None,
        });
    }
    if let Some(package) = pending {
        packages.push(package);
    }
    packages
}

fn parse_colon_fields(content: &str) -> BTreeMap<String, String> {
    content
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

pub fn parse_pacman_conf(path: &std::path::Path) -> Vec<RepoEntry> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    let mut section: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = Some(line[1..line.len() - 1].to_string());
            continue;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == "Server" {
                if let Some(id) = &section {
                    entries.push(RepoEntry {
                        id: id.clone(),
                        manager: "pacman".to_string(),
                        uri: Some(value.replace("$repo", id).replace("$arch", "x86_64").trim().to_string()),
                        suites: Vec::new(),
                        components: Vec::new(),
                        enabled: true,
                        signed_by: None,
                    });
                }
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_pacman_conf_servers() {
        let path = std::env::temp_dir().join(format!("tide-pacman-{}.conf", std::process::id()));
        fs::write(
            &path,
            "[core]\nServer = https://mirror.example/$repo/os/$arch\n# Server = disabled\n",
        )
        .unwrap();
        let repos = parse_pacman_conf(&path);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, "core");
        assert!(repos[0].uri.as_deref().unwrap().contains("/core/os/x86_64"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn parses_search_blocks() {
        let packages = parse_search("extra/firefox 120.0-1\n    Standalone web browser\n");
        assert_eq!(packages[0].name, "firefox");
        assert_eq!(packages[0].description.as_deref(), Some("Standalone web browser"));
    }
}
