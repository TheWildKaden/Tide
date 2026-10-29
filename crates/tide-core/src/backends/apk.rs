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

/// Adapter for Alpine's apk-tools.
#[derive(Debug, Clone)]
pub struct ApkBackend {
    repositories_file: PathBuf,
}

impl ApkBackend {
    pub fn new(repositories_file: PathBuf) -> Self {
        Self { repositories_file }
    }
}

impl PackageBackend for ApkBackend {
    fn key(&self) -> &'static str {
        "apk"
    }

    fn name(&self) -> &'static str {
        "apk-tools (Alpine family)"
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
        runner.which("apk").is_some()
    }

    fn version(&self, runner: &dyn CommandRunner) -> Option<String> {
        let output = runner.run("apk", &["--version".to_string()], &[]).ok()?;
        output
            .stdout
            .lines()
            .next()
            .map(|line| line.trim().to_string())
    }

    fn notes(&self) -> Vec<String> {
        vec![
            "apk has its own versioning and dependency semantics; it is not interchangeable with APT.".into(),
            "The musl libc base is not ABI-compatible with glibc packages.".into(),
        ]
    }

    fn search(&self, runner: &dyn CommandRunner, query: &str) -> Result<Vec<PackageSummary>> {
        ensure_program(runner, "apk")?;
        let output = run(
            runner,
            "apk",
            &["search".to_string(), "-v".to_string(), query.to_string()],
            &[],
        )?;
        Ok(parse_apk_search(&output))
    }

    fn info(&self, runner: &dyn CommandRunner, package: &str) -> Result<PackageSummary> {
        validate_package_spec(package, false)?;
        ensure_program(runner, "apk")?;
        let output = run(
            runner,
            "apk",
            &["info".to_string(), "-a".to_string(), package.to_string()],
            &[],
        )?;
        let fields = parse_colon_fields(&output);
        Ok(PackageSummary {
            backend: "apk".to_string(),
            name: fields
                .get("name")
                .cloned()
                .unwrap_or_else(|| package.to_string()),
            version: fields.get("version").cloned(),
            description: fields.get("description").cloned(),
            installed: None,
        })
    }

    fn list_installed(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let output = run(runner, "apk", &["info".to_string(), "-v".to_string()], &[])?;
        Ok(output
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                let (name, version) = split_apk_version(line);
                Some(PackageSummary {
                    backend: "apk".to_string(),
                    name: name.to_string(),
                    version: version.map(str::to_string),
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
        let mut args = vec!["add".to_string(), "--".to_string()];
        args.extend(packages.iter().cloned());
        if options.dry_run {
            return Ok(OperationReport::planned(
                format!("install {} package(s) with apk", packages.len()),
                planned("apk", &args),
            ));
        }
        let output = run_privileged(runner, "apk", &args, &[])?;
        Ok(OperationReport::completed(
            format!("installed {} package(s) with apk", packages.len()),
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
        let mut args = vec!["del".to_string(), "--".to_string()];
        args.extend(packages.iter().cloned());
        if options.dry_run {
            return Ok(OperationReport::planned(
                format!("remove {} package(s) with apk", packages.len()),
                planned("apk", &args),
            ));
        }
        let output = run_privileged(runner, "apk", &args, &[])?;
        Ok(OperationReport::completed(
            format!("removed {} package(s) with apk", packages.len()),
            collect_lines(&output),
        ))
    }

    fn update(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let output = run_privileged(runner, "apk", &["update".to_string()], &[])?;
        Ok(OperationReport::completed(
            "apk indexes updated".to_string(),
            collect_lines(&output),
        ))
    }

    fn upgrade(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let output = run_privileged(runner, "apk", &["upgrade".to_string()], &[])?;
        Ok(OperationReport::completed(
            "apk packages upgraded".to_string(),
            collect_lines(&output),
        ))
    }

    fn outdated(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let output = run(
            runner,
            "apk",
            &["version".to_string(), "-l".to_string(), "<".to_string()],
            &[],
        )?;
        Ok(output
            .lines()
            .filter_map(|line| {
                let (name, version) = line.trim().split_once('<')?;
                Some(PackageSummary {
                    backend: "apk".to_string(),
                    name: name.trim().to_string(),
                    version: Some(version.trim().to_string()),
                    description: None,
                    installed: Some(true),
                })
            })
            .collect())
    }

    fn repositories(&self, _runner: &dyn CommandRunner) -> Result<Vec<RepoEntry>> {
        let content = match std::fs::read_to_string(&self.repositories_file) {
            Ok(content) => content,
            Err(_) => return Ok(Vec::new()),
        };
        Ok(content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .enumerate()
            .map(|(index, line)| RepoEntry {
                id: format!("apk-{index}"),
                manager: "apk".to_string(),
                uri: Some(line.to_string()),
                suites: Vec::new(),
                components: Vec::new(),
                enabled: true,
                signed_by: None,
            })
            .collect())
    }

    fn doctor(&self, runner: &dyn CommandRunner) -> Result<Vec<DoctorFinding>> {
        let mut findings = Vec::new();
        if runner.which("apk").is_some() {
            findings.push(DoctorFinding::new(CheckLevel::Ok, "apk", "apk is present"));
        } else {
            findings.push(DoctorFinding::new(
                CheckLevel::Error,
                "apk",
                "apk was not found",
            ));
        }
        if self.repositories(runner)?.is_empty() {
            findings.push(DoctorFinding::new(
                CheckLevel::Warning,
                "apk",
                format!(
                    "no repositories found in {}",
                    self.repositories_file.display()
                ),
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

fn parse_apk_search(output: &str) -> Vec<PackageSummary> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (spec, description) = match line.split_once(" - ") {
                Some((spec, description)) => (spec, Some(description.to_string())),
                None => (line, None),
            };
            let (name, version) = split_apk_version(spec);
            Some(PackageSummary {
                backend: "apk".to_string(),
                name: name.to_string(),
                version: version.map(str::to_string),
                description,
                installed: None,
            })
        })
        .collect()
}

/// apk prints `name-version` without a separator. Split at the last hyphen that is
/// followed by a digit, which is the common package/version boundary.
fn split_apk_version(spec: &str) -> (&str, Option<&str>) {
    let bytes = spec.as_bytes();
    for index in (1..bytes.len()).rev() {
        if bytes[index - 1] == b'-' && bytes[index].is_ascii_digit() {
            return (&spec[..index - 1], Some(&spec[index..]));
        }
    }
    (spec, None)
}

fn parse_colon_fields(content: &str) -> BTreeMap<String, String> {
    content
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_apk_versions() {
        assert_eq!(
            split_apk_version("firefox-120.0-r0"),
            ("firefox", Some("120.0-r0"))
        );
        assert_eq!(split_apk_version("musl"), ("musl", None));
    }

    #[test]
    fn parses_search_output() {
        let packages = parse_apk_search("firefox-120.0-r0 - Web browser\n");
        assert_eq!(packages[0].name, "firefox");
        assert_eq!(packages[0].version.as_deref(), Some("120.0-r0"));
    }
}
