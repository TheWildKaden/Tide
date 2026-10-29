use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::backend::{PackageBackend, RunOptions};
use crate::error::{Result, TideError};
use crate::exec::CommandRunner;
use crate::model::{
    BackendCapabilities, CheckLevel, DoctorFinding, OperationReport, PackageSummary, RepoEntry,
};
use crate::util::{display_command, validate_package_spec};

use super::{ensure_program, planned, run, run_privileged};

const QUERY_FORMAT: &str = "%{name}\t%{version}\t%{summary}\\n";

/// Adapter for the DNF5/RPM family. It uses `dnf5` when present and falls back to
/// `dnf` on systems that still ship the DNF4 CLI.
#[derive(Debug, Clone)]
pub struct DnfBackend {
    repo_dir: PathBuf,
    program: &'static str,
}

impl DnfBackend {
    pub fn new(repo_dir: PathBuf) -> Self {
        Self {
            repo_dir,
            program: "dnf5",
        }
    }

    fn program(&self, runner: &dyn CommandRunner) -> &'static str {
        if runner.which("dnf5").is_some() {
            "dnf5"
        } else {
            "dnf"
        }
    }

    fn repoquery(&self, runner: &dyn CommandRunner, args: &[String]) -> Result<String> {
        let program = self.program(runner);
        ensure_program(runner, program)?;
        run(runner, program, args, &[])
    }
}

impl PackageBackend for DnfBackend {
    fn key(&self) -> &'static str {
        "dnf5"
    }

    fn name(&self) -> &'static str {
        "DNF5 / RPM (Fedora family)"
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
        runner.which("dnf5").is_some() || runner.which("dnf").is_some()
    }

    fn version(&self, runner: &dyn CommandRunner) -> Option<String> {
        let program = self.program(runner);
        let output = runner.run(program, &["--version".to_string()], &[]).ok()?;
        output.stdout.lines().next().map(|line| line.trim().to_string())
    }

    fn notes(&self) -> Vec<String> {
        vec![
            "DNF and RPM use different dependency and scriptlet semantics than APT.".into(),
            "Signature verification is delegated to DNF's configured policy.".into(),
        ]
    }

    fn search(&self, runner: &dyn CommandRunner, query: &str) -> Result<Vec<PackageSummary>> {
        let pattern = format!("*{query}*");
        let args = vec![
            "repoquery".to_string(),
            "--available".to_string(),
            "--queryformat".to_string(),
            QUERY_FORMAT.to_string(),
            pattern,
        ];
        let output = self.repoquery(runner, &args)?;
        Ok(parse_tabbed(&output, "dnf5"))
    }

    fn info(&self, runner: &dyn CommandRunner, package: &str) -> Result<PackageSummary> {
        validate_package_spec(package, false)?;
        let program = self.program(runner);
        ensure_program(runner, program)?;
        let output = run(
            runner,
            program,
            &["info".to_string(), "--".to_string(), package.to_string()],
            &[],
        )?;
        parse_info(&output, package)
    }

    fn list_installed(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let args = vec![
            "repoquery".to_string(),
            "--installed".to_string(),
            "--queryformat".to_string(),
            QUERY_FORMAT.to_string(),
        ];
        let output = self.repoquery(runner, &args)?;
        Ok(parse_tabbed(&output, "dnf5"))
    }

    fn install(
        &self,
        runner: &dyn CommandRunner,
        packages: &[String],
        options: &RunOptions,
    ) -> Result<OperationReport> {
        let program = self.program(runner);
        validate_packages(packages)?;
        let mut args = vec!["install".to_string(), "--assumeyes".to_string(), "--".to_string()];
        args.extend(packages.iter().cloned());
        if options.dry_run {
            return Ok(OperationReport::planned(
                format!("install {} package(s) with DNF", packages.len()),
                planned(program, &args),
            ));
        }
        let output = run_privileged(runner, program, &args, &[])?;
        Ok(OperationReport::completed(
            format!("installed {} package(s) with DNF", packages.len()),
            collect_lines(&output),
        ))
    }

    fn remove(
        &self,
        runner: &dyn CommandRunner,
        packages: &[String],
        options: &RunOptions,
    ) -> Result<OperationReport> {
        let program = self.program(runner);
        validate_packages(packages)?;
        let mut args = vec!["remove".to_string(), "--assumeyes".to_string(), "--".to_string()];
        args.extend(packages.iter().cloned());
        if options.dry_run {
            return Ok(OperationReport::planned(
                format!("remove {} package(s) with DNF", packages.len()),
                planned(program, &args),
            ));
        }
        let output = run_privileged(runner, program, &args, &[])?;
        Ok(OperationReport::completed(
            format!("removed {} package(s) with DNF", packages.len()),
            collect_lines(&output),
        ))
    }

    fn update(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let program = self.program(runner);
        let output = run_privileged(runner, program, &["makecache".to_string()], &[])?;
        Ok(OperationReport::completed(
            "DNF metadata cache refreshed".to_string(),
            collect_lines(&output),
        ))
    }

    fn upgrade(&self, runner: &dyn CommandRunner) -> Result<OperationReport> {
        let program = self.program(runner);
        let args = vec!["upgrade".to_string(), "--assumeyes".to_string()];
        let output = run_privileged(runner, program, &args, &[])?;
        Ok(OperationReport::completed(
            "DNF packages upgraded".to_string(),
            collect_lines(&output),
        ))
    }

    fn outdated(&self, runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>> {
        let args = vec![
            "repoquery".to_string(),
            "--upgrades".to_string(),
            "--queryformat".to_string(),
            QUERY_FORMAT.to_string(),
        ];
        let output = self.repoquery(runner, &args)?;
        Ok(parse_tabbed(&output, "dnf5"))
    }

    fn repositories(&self, _runner: &dyn CommandRunner) -> Result<Vec<RepoEntry>> {
        Ok(parse_yum_repos(&self.repo_dir))
    }

    fn doctor(&self, runner: &dyn CommandRunner) -> Result<Vec<DoctorFinding>> {
        let program = self.program(runner);
        let mut findings = Vec::new();
        if runner.which(program).is_some() {
            findings.push(DoctorFinding::new(
                CheckLevel::Ok,
                "dnf",
                format!("{program} is present"),
            ));
        } else {
            findings.push(DoctorFinding::new(
                CheckLevel::Error,
                "dnf",
                "neither dnf5 nor dnf was found",
            ));
        }
        if runner.which("rpm").is_none() {
            findings.push(DoctorFinding::new(
                CheckLevel::Error,
                "dnf",
                "rpm database tool is missing",
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

fn parse_tabbed(output: &str, backend: &str) -> Vec<PackageSummary> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            Some(PackageSummary {
                backend: backend.to_string(),
                name: name.to_string(),
                version: parts.next().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
                description: parts.next().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string),
                installed: None,
            })
        })
        .collect()
}

fn parse_info(output: &str, requested: &str) -> Result<PackageSummary> {
    let fields = parse_colon_fields(output);
    Ok(PackageSummary {
        backend: "dnf5".to_string(),
        name: fields
            .get("Name")
            .cloned()
            .unwrap_or_else(|| requested.to_string()),
        version: fields.get("Version").cloned(),
        description: fields.get("Summary").cloned(),
        installed: None,
    })
}

fn parse_colon_fields(content: &str) -> BTreeMap<String, String> {
    content
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

pub fn parse_yum_repos(repo_dir: &Path) -> Vec<RepoEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(repo_dir) else {
        return entries;
    };
    let mut files = read_dir
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("repo"))
        .collect::<Vec<_>>();
    files.sort();
    for path in files {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut section: Option<String> = None;
        let mut fields = BTreeMap::new();
        let mut flush = |section: &Option<String>, fields: &mut BTreeMap<String, String>| {
            if let Some(id) = section {
                let enabled = fields
                    .get("enabled")
                    .map(|value| value != "0")
                    .unwrap_or(true);
                entries.push(RepoEntry {
                    id: id.clone(),
                    manager: "dnf5".to_string(),
                    uri: fields.get("baseurl").cloned().or_else(|| fields.get("metalink").cloned()),
                    suites: Vec::new(),
                    components: Vec::new(),
                    enabled,
                    signed_by: fields.get("gpgkey").cloned(),
                });
            }
            fields.clear();
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                flush(&section, &mut fields);
                section = Some(line[1..line.len() - 1].trim().to_string());
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                fields.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
            }
        }
        flush(&section, &mut fields);
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_repo_files() {
        let dir = std::env::temp_dir().join(format!("tide-dnf-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("fedora.repo"),
            "[fedora]\nname=Fedora\nbaseurl=https://example.invalid/fedora\nenabled=1\ngpgkey=file:///etc/pki/key\n[disabled]\nbaseurl=https://example.invalid/off\nenabled=0\n",
        )
        .unwrap();
        let repos = parse_yum_repos(&dir);
        assert_eq!(repos.len(), 2);
        assert!(repos[0].enabled);
        assert!(!repos[1].enabled);
        assert_eq!(repos[0].signed_by.as_deref(), Some("file:///etc/pki/key"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn tabbed_parser_handles_summary() {
        let packages = parse_tabbed("bash\t5.2\tGNU shell\n", "dnf5");
        assert_eq!(packages[0].name, "bash");
        assert_eq!(packages[0].description.as_deref(), Some("GNU shell"));
    }

    #[test]
    fn rejects_injection_in_package() {
        let backend = DnfBackend::new(PathBuf::from("/nonexistent"));
        let runner = crate::exec::MockRunner::with_programs(&["dnf5"]);
        assert!(backend
            .install(&runner, &["--refresh".into()], &RunOptions::default())
            .is_err());
    }
}
