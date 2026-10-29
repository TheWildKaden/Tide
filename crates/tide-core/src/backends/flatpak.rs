use crate::backend::{PackageBackend, RunOptions};
use crate::error::{Result, TideError};
use crate::exec::CommandRunner;
use crate::model::{
    BackendCapabilities, CheckLevel, DoctorFinding, OperationReport, PackageSummary, RepoEntry,
};
use crate::util::validate_package_spec;

use super::{ensure_program, planned, run, run_privileged};

/// Adapter for Flatpak. User scope is the default so desktop applications do not
/// require elevating the entire package manager.
#[derive(Debug, Clone)]
pub struct FlatpakBackend {
    scope: String,
}

impl FlatpakBackend {
    pub fn new(scope: String) -> Self {
        Self { scope }
    }

    fn scope_flag(&self) -> String {
        if self.scope == "system" {
            "--system".to_string()
        } else {
            "--user".to_string()
        }
    }

    fn invoke(&self, runner: &dyn CommandRunner, args: &[String]) -> Result<String> {
        ensure_program(runner, "flatpak")?;
        let mut full = vec![self.scope_flag()];
        full.extend(args.iter().cloned());
        run(runner, "flatpak", &full, &[])
    }

    fn invoke_privileged(&self, runner: &dyn CommandRunner, args: &[String]) -> Result<String> {
        ensure_program(runner, "flatpak")?;
        let mut full = vec![self.scope_flag()];
        full.extend(args.iter().cloned());
        if self.scope == "system" {
            run_privileged(runner, "flatpak", &full, &[])
        } else {
            run(runner, "flatpak", &full, &[])
        }
    }
}

impl PackageBackend for FlatpakBackend {
    fn key(&self) -> &'static str {
        "flatpak"
    }

    fn name(&self) -> &'static str {
        "Flatpak (sandboxed desktop applications)"
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
        runner.which("flatpak").is_some()
    }

    fn version(&self, runner: &dyn CommandRunner) -> Option<String> {
        let output = runner.run("flatpak", &["--version".to_string()], &[]).ok()?;
        output.stdout.lines().next().map(|line| line.trim().to_string())
    }

    fn notes(&self) -> Vec<String> {
        vec![
            format!("Using {} installation scope.", self.scope),
            "Flatpak applications are sandboxed and use runtimes, not distribution dependencies.".into(),
        ]
    }

    fn search(&self, runner: &dyn CommandRunner, query: &str) -> Result<Vec<PackageSummary>> {
        let output = self.invoke(
            runner,
            &[
                "search".to_string(),
                "--columns=application,name,description".to_string(),
                query.to_string(),
            ],
        )?;
        Ok(parse_columns(&output))
    }

    fn info(&self, runner: &dyn CommandRunner, package: &str) -> Result<PackageSummary> {
        validate_flatpak_ref(package)?;
        let output = self.invoke_if_available(runner, &["info".to_string(), package.to_string()])?;
        let fields = output
            .lines()
            .filter_map(|line| line.split_once(':'))
            .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
            .collect::<std::collections::BTreeMap<_, _>>();
        Ok(PackageSummary {
            backend: "flatpak".to_string(),
            name: fields
                .get("ref")
                .cloned()
                .unwrap_or_else(|| package.to_string()),
            version: fields.get("version").cloned(),
            description: None,
            installed: Some(true),
        })
    }

    fn list_installed(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let output = self.invoke(
            runner,
            &[
                "list".to_string(),
                "--app".to_string(),
                "--columns=application,version".to_string(),
            ],
        )?;
        Ok(output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let name = parts.next()?.trim();
                if name.is_empty() {
                    return None;
                }
                Some(PackageSummary {
                    backend: "flatpak".to_string(),
                    name: name.to_string(),
                    version: parts.next().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
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
        if packages.is_empty() {
            return Err(TideError::InvalidInput {
                kind: "application list",
                message: "at least one Flatpak application ID is required".into(),
            });
        }
        for package in packages {
            validate_flatpak_ref(package)?;
        }
        let mut args = vec![
            "install".to_string(),
            "-y".to_string(),
            "--noninteractive".to_string(),
        ];
        args.extend(packages.iter().cloned());
        if options.dry_run {
            let mut shown = vec![self.scope_flag()];
            shown.extend(args.iter().cloned());
            return Ok(OperationReport::planned(
                format!("install {} Flatpak application(s)", packages.len()),
                planned("flatpak", &shown),
            ));
        }
        let output = self.invoke_privileged(runner, &args)?;
        Ok(OperationReport::completed(
            format!("installed {} Flatpak application(s)", packages.len()),
            collect_lines(&output),
        ))
    }

    fn remove(
        &self,
        runner: &dyn CommandRunner,
        packages: &[String],
        options: &RunOptions,
    ) -> Result<OperationReport> {
        if packages.is_empty() {
            return Err(TideError::InvalidInput {
                kind: "application list",
                message: "at least one Flatpak application ID is required".into(),
            });
        }
        for package in packages {
            validate_flatpak_ref(package)?;
        }
        let mut args = vec![
            "uninstall".to_string(),
            "-y".to_string(),
            "--noninteractive".to_string(),
        ];
        args.extend(packages.iter().cloned());
        if options.dry_run {
            let mut shown = vec![self.scope_flag()];
            shown.extend(args.iter().cloned());
            return Ok(OperationReport::planned(
                format!("remove {} Flatpak application(s)", packages.len()),
                planned("flatpak", &shown),
            ));
        }
        let output = self.invoke_privileged(runner, &args)?;
        Ok(OperationReport::completed(
            format!("removed {} Flatpak application(s)", packages.len()),
            collect_lines(&output),
        ))
    }

    fn update(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let output = self.invoke_privileged(
            runner,
            &[
                "update".to_string(),
                "-y".to_string(),
                "--noninteractive".to_string(),
            ],
        )?;
        Ok(OperationReport::completed(
            "Flatpak metadata and applications updated".to_string(),
            collect_lines(&output),
        ))
    }

    fn upgrade(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        self.update(runner)
    }

    fn outdated(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let output = self.invoke(
            runner,
            &[
                "remote-ls".to_string(),
                "--updates".to_string(),
                "--columns=application,version".to_string(),
            ],
        )?;
        Ok(parse_columns(&output))
    }

    fn repositories(&self, runner: &dyn CommandRunner) -> Result<Vec<RepoEntry>> {
        let output = self.invoke(
            runner,
            &["remotes".to_string(), "--columns=name,url".to_string()],
        )?;
        Ok(output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let name = parts.next()?.trim();
                if name.is_empty() {
                    return None;
                }
                Some(RepoEntry {
                    id: name.to_string(),
                    manager: "flatpak".to_string(),
                    uri: parts.next().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
                    suites: Vec::new(),
                    components: Vec::new(),
                    enabled: true,
                    signed_by: None,
                })
            })
            .collect())
    }

    fn doctor(&self, runner: &dyn CommandRunner) -> Result<Vec<DoctorFinding>> {
        let mut findings = Vec::new();
        if runner.which("flatpak").is_none() {
            findings.push(DoctorFinding::new(
                CheckLevel::Error,
                "flatpak",
                "flatpak was not found",
            ));
            return Ok(findings);
        }
        findings.push(DoctorFinding::new(CheckLevel::Ok, "flatpak", "flatpak is present"));
        match self.repositories(runner) {
            Ok(repos) if repos.is_empty() => findings.push(
                DoctorFinding::new(
                    CheckLevel::Warning,
                    "flatpak",
                    format!("no Flatpak remotes configured in {} scope", self.scope),
                )
                .with_remediation("flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo"),
            ),
            Ok(_) => findings.push(DoctorFinding::new(
                CheckLevel::Ok,
                "flatpak",
                "Flatpak remotes are configured",
            )),
            Err(error) => findings.push(DoctorFinding::new(
                CheckLevel::Warning,
                "flatpak",
                format!("could not read remotes: {error}"),
            )),
        }
        Ok(findings)
    }
}

impl FlatpakBackend {
    fn invoke_if_available(&self, runner: &dyn CommandRunner, args: &[String]) -> Result<String> {
        self.invoke(runner, args)
    }
}

fn validate_flatpak_ref(reference: &str) -> Result<()> {
    if reference.len() > 255
        || reference.starts_with('-')
        || !reference
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
    {
        return Err(TideError::InvalidInput {
            kind: "Flatpak application ID",
            message: format!("'{reference}' is not a valid application ID"),
        });
    }
    Ok(())
}

fn parse_columns(output: &str) -> Vec<PackageSummary> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            let description = parts.next().and_then(|second| {
                // Search output is application, name, description; remotes/list output
                // is application, version. Keep the extra column as a description only
                // when a third column exists.
                if parts.next().is_some() {
                    Some(second.trim().to_string())
                } else {
                    None
                }
            });
            Some(PackageSummary {
                backend: "flatpak".to_string(),
                name: name.to_string(),
                version: None,
                description,
                installed: None,
            })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_option_injection() {
        assert!(validate_flatpak_ref("--user").is_err());
        assert!(validate_flatpak_ref("org.mozilla.firefox").is_ok());
        assert!(validate_flatpak_ref("org.mozilla.firefox;rm").is_err());
    }

    #[test]
    fn scopes_are_explicit() {
        assert_eq!(FlatpakBackend::new("user".into()).scope_flag(), "--user");
        assert_eq!(FlatpakBackend::new("system".into()).scope_flag(), "--system");
    }
}
