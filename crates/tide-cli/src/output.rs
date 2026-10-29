use serde::Serialize;
use tide_core::model::{BackendInfo, DoctorFinding, OperationReport, PackageSummary, RepoEntry};

pub fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| format!("{{\"error\":\"failed to encode JSON: {error}\"}}"))
}

pub fn print_packages(packages: &[PackageSummary]) {
    if packages.is_empty() {
        println!("No packages found.");
        return;
    }
    let name_width = packages
        .iter()
        .map(|package| package.name.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 48);
    let version_width = packages
        .iter()
        .filter_map(|package| package.version.as_ref())
        .map(|version| version.chars().count())
        .max()
        .unwrap_or(7)
        .clamp(7, 32);
    println!(
        "{:<name_width$}  {:<version_width$}  DESCRIPTION",
        "NAME", "VERSION"
    );
    for package in packages {
        println!(
            "{:<name_width$}  {:<version_width$}  {}",
            package.name,
            package.version.as_deref().unwrap_or("-"),
            package.description.as_deref().unwrap_or("")
        );
    }
}

pub fn print_package(package: &PackageSummary) {
    println!("Name:        {}", package.name);
    println!("Backend:     {}", package.backend);
    println!(
        "Version:     {}",
        package.version.as_deref().unwrap_or("unknown")
    );
    if let Some(description) = &package.description {
        println!("Description: {description}");
    }
    if let Some(installed) = package.installed {
        println!("Installed:   {}", if installed { "yes" } else { "no" });
    }
}

pub fn print_report(report: &OperationReport) {
    println!("{}", report.summary);
    for detail in &report.details {
        println!("  {detail}");
    }
}

pub fn print_backends(backends: &[BackendInfo]) {
    if backends.is_empty() {
        println!("No backends registered.");
        return;
    }
    let id_width = backends
        .iter()
        .map(|backend| backend.id.len())
        .max()
        .unwrap_or(2);
    for backend in backends {
        let state = if backend.available {
            "available"
        } else {
            "unavailable"
        };
        println!("{:<id_width$}  {:<12}  {}", backend.id, state, backend.name);
        if let Some(version) = &backend.version {
            println!("{:<id_width$}  version: {version}", "");
        }
        println!(
            "{:<id_width$}  capabilities: {}",
            "",
            backend.capabilities.names().join(", ")
        );
        for note in &backend.notes {
            println!("{:<id_width$}  note: {note}", "");
        }
    }
}

pub fn print_repos(repos: &[RepoEntry]) {
    if repos.is_empty() {
        println!("No repositories found.");
        return;
    }
    let id_width = repos
        .iter()
        .map(|repo| repo.id.len())
        .max()
        .unwrap_or(2)
        .clamp(2, 40);
    for repo in repos {
        let state = if repo.enabled { "enabled" } else { "disabled" };
        println!(
            "{:<id_width$}  {:<9}  {}",
            repo.id,
            state,
            repo.uri.as_deref().unwrap_or("-")
        );
        let mut detail = Vec::new();
        if !repo.suites.is_empty() {
            detail.push(format!("suites={}", repo.suites.join(",")));
        }
        if !repo.components.is_empty() {
            detail.push(format!("components={}", repo.components.join(",")));
        }
        if let Some(signed_by) = &repo.signed_by {
            detail.push(format!("signed-by={signed_by}"));
        }
        if !detail.is_empty() {
            println!("{:<id_width$}  {:<9}  {}", "", "", detail.join(" "));
        }
    }
}

pub fn print_findings(findings: &[DoctorFinding]) {
    if findings.is_empty() {
        println!("No checks were run.");
        return;
    }
    for finding in findings {
        println!(
            "[{:<7}] {:<12} {}",
            finding.level.label(),
            finding.area,
            finding.message
        );
        if let Some(remediation) = &finding.remediation {
            println!("          fix: {remediation}");
        }
    }
}
