mod output;

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde::Serialize;
use tide_core::backends::BackendRegistry;
use tide_core::config::TideConfig;
use tide_core::error::{Result, TideError};
use tide_core::exec::{CommandRunner, SystemRunner};
use tide_core::model::{
    BackendInfo, CheckLevel, DoctorFinding, OperationReport, RepoEntry,
};
use tide_core::RunOptions;

#[derive(Debug, Parser)]
#[command(
    name = "tide",
    version,
    about = "Tide package manager — a stable interface over native Linux package backends",
    long_about = "Tide installs, removes, searches, and updates software through an isolated backend adapter. \
On Tide OS this is currently the Debian APT/dpkg stack; Fedora, Arch, Alpine, Flatpak, and managed AppImage \
backends are also available where detected. Native signature verification and package-script safeguards are \
never bypassed."
)]
struct Cli {
    /// Backend to use: auto, apt, dnf5, pacman, apk, flatpak, or appimage.
    #[arg(long, short = 'b', global = true, env = "TIDE_BACKEND")]
    backend: Option<String>,

    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

    /// Do not ask for confirmation before changing packages.
    #[arg(long, short = 'y', global = true)]
    yes: bool,

    /// Show what would be done without changing package state.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Path to the Tide configuration file.
    #[arg(long, global = true, env = "TIDE_CONFIG")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Install one or more packages using the selected backend.
    Install {
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
    },
    /// Remove one or more packages.
    Remove {
        #[arg(required = true, value_name = "PACKAGE")]
        packages: Vec<String>,
        /// Also remove configuration files where the backend supports it.
        #[arg(long)]
        purge: bool,
    },
    /// Refresh repository metadata without upgrading packages.
    Update,
    /// Upgrade installed packages.
    Upgrade,
    /// Search the selected backend for packages.
    Search {
        #[arg(required = true, value_name = "QUERY")]
        query: String,
    },
    /// Show information about a package or application.
    Info {
        #[arg(required = true, value_name = "PACKAGE")]
        package: String,
    },
    /// List installed packages.
    List,
    /// Check backend health, configuration, and signature policy.
    Doctor,
    /// Inspect software repositories.
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
    },
    /// Show all backends and their capabilities.
    Backends,
    /// Show packages with available upgrades.
    Outdated,
    /// Show Tide package-manager version details.
    Version,
}

#[derive(Debug, Subcommand)]
enum RepoCommands {
    /// List configured repositories for the selected backend.
    List,
}

#[derive(Serialize)]
struct DoctorReport {
    config_path: String,
    findings: Vec<DoctorFinding>,
    backends: Vec<BackendInfo>,
}

#[derive(Serialize)]
struct VersionReport {
    name: &'static str,
    version: &'static str,
    foundation: &'static str,
    backends: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tide: {error}");
            if let Some(hint) = error_hint(&error) {
                eprintln!("hint: {hint}");
            }
            ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(1))
        }
    }
}

fn error_hint(error: &TideError) -> Option<&'static str> {
    match error {
        TideError::Locked => Some("wait for the other package operation, then retry"),
        TideError::PermissionDenied => Some("run the command as root (for example with sudo)"),
        TideError::BackendUnavailable(_) => Some("run 'tide backends' to see detected package managers"),
        TideError::Unsupported { .. } => Some("run 'tide backends' to see what this backend supports"),
        _ => None,
    }
}

fn run(cli: &Cli) -> Result<()> {
    let config = TideConfig::load_optional(cli.config.as_ref())?;
    let runner = SystemRunner::new(true);
    let registry = BackendRegistry::new(&config);

    match &cli.command {
        Commands::Backends => {
            let statuses = registry.statuses(&runner);
            let infos = statuses.into_iter().map(|status| status.info).collect::<Vec<_>>();
            if cli.json {
                println!("{}", output::json(&infos));
            } else {
                output::print_backends(&infos);
            }
            Ok(())
        }
        Commands::Doctor => doctor(cli, &config, &runner, &registry),
        Commands::Repo { command } => match command {
            RepoCommands::List => {
                let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
                if !backend.capabilities().repositories {
                    return Err(TideError::Unsupported {
                        backend: backend.key(),
                        operation: "repository listing",
                    });
                }
                let repos: Vec<RepoEntry> = backend.repositories(&runner)?;
                if cli.json {
                    println!("{}", output::json(&repos));
                } else {
                    println!("Repositories managed by {}:", backend.name());
                    output::print_repos(&repos);
                }
                Ok(())
            }
        },
        Commands::Version => {
            let report = VersionReport {
                name: "Tide package manager",
                version: env!("CARGO_PKG_VERSION"),
                foundation: "Debian-based Tide OS foundation; backend-agnostic interface",
                backends: registry.ids().iter().map(|id| id.to_string()).collect(),
            };
            if cli.json {
                println!("{}", output::json(&report));
            } else {
                println!("{} {}", report.name, report.version);
                println!("Foundation: {}", report.foundation);
                println!("Backends:   {}", report.backends.join(", "));
            }
            Ok(())
        }
        Commands::Install { packages } => {
            validate_query_list(packages)?;
            let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
            let options = run_options(cli, &config, false);
            if !options.dry_run {
                confirm(
                    cli,
                    &config,
                    &format!(
                        "Install {} package(s) using {}?",
                        packages.len(),
                        backend.name()
                    ),
                )?;
            }
            let report = backend.install(&runner, packages, &options)?;
            print_operation(cli, &report);
            Ok(())
        }
        Commands::Remove { packages, purge } => {
            validate_query_list(packages)?;
            let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
            let options = run_options(cli, &config, *purge);
            if !options.dry_run {
                confirm(
                    cli,
                    &config,
                    &format!(
                        "Remove {} package(s) using {}?",
                        packages.len(),
                        backend.name()
                    ),
                )?;
            }
            let report = backend.remove(&runner, packages, &options)?;
            print_operation(cli, &report);
            Ok(())
        }
        Commands::Update => {
            let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
            if !backend.capabilities().update {
                return Err(TideError::Unsupported {
                    backend: backend.key(),
                    operation: "metadata update",
                });
            }
            if cli.dry_run {
                let report = OperationReport::planned(
                    format!("refresh metadata using {}", backend.name()),
                    "backend-specific metadata refresh",
                );
                print_operation(cli, &report);
                return Ok(());
            }
            confirm(
                cli,
                &config,
                &format!("Refresh package metadata using {}?", backend.name()),
            )?;
            let report = backend.update(&runner)?;
            print_operation(cli, &report);
            Ok(())
        }
        Commands::Upgrade => {
            let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
            if !backend.capabilities().upgrade {
                return Err(TideError::Unsupported {
                    backend: backend.key(),
                    operation: "upgrade",
                });
            }
            if cli.dry_run {
                let report = OperationReport::planned(
                    format!("upgrade packages using {}", backend.name()),
                    "backend-specific upgrade",
                );
                print_operation(cli, &report);
                return Ok(());
            }
            confirm(
                cli,
                &config,
                &format!("Upgrade installed packages using {}?", backend.name()),
            )?;
            let report = backend.upgrade(&runner)?;
            print_operation(cli, &report);
            Ok(())
        }
        Commands::Search { query } => {
            validate_query(query)?;
            let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
            if !backend.capabilities().search {
                return Err(TideError::Unsupported {
                    backend: backend.key(),
                    operation: "search",
                });
            }
            let packages = backend.search(&runner, query)?;
            if cli.json {
                println!("{}", output::json(&packages));
            } else {
                output::print_packages(&packages);
            }
            Ok(())
        }
        Commands::Info { package } => {
            validate_query(package)?;
            let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
            if !backend.capabilities().info {
                return Err(TideError::Unsupported {
                    backend: backend.key(),
                    operation: "package information",
                });
            }
            let package = backend.info(&runner, package)?;
            if cli.json {
                println!("{}", output::json(&package));
            } else {
                output::print_package(&package);
            }
            Ok(())
        }
        Commands::List => {
            let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
            if !backend.capabilities().list {
                return Err(TideError::Unsupported {
                    backend: backend.key(),
                    operation: "installed package listing",
                });
            }
            let packages = backend.list_installed(&runner)?;
            if cli.json {
                println!("{}", output::json(&packages));
            } else {
                output::print_packages(&packages);
            }
            Ok(())
        }
        Commands::Outdated => {
            let backend = registry.select(&config, cli.backend.as_deref(), &runner)?;
            if !backend.capabilities().outdated {
                return Err(TideError::Unsupported {
                    backend: backend.key(),
                    operation: "upgrade checking",
                });
            }
            let packages = backend.outdated(&runner)?;
            if cli.json {
                println!("{}", output::json(&packages));
            } else if packages.is_empty() {
                println!("All packages are up to date according to {}.", backend.name());
            } else {
                output::print_packages(&packages);
            }
            Ok(())
        }
    }
}

fn doctor(
    cli: &Cli,
    config: &TideConfig,
    runner: &dyn CommandRunner,
    registry: &BackendRegistry,
) -> Result<()> {
    let mut findings = Vec::new();
    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(TideConfig::default_path);
    if config_path.exists() {
        findings.push(DoctorFinding::new(
            CheckLevel::Ok,
            "config",
            format!("loaded {}", config_path.display()),
        ));
    } else {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "config",
            format!("{} not present; using safe defaults", config_path.display()),
        ));
    }
    if config.repositories.native_tide_repos {
        findings.push(
            DoctorFinding::new(
                CheckLevel::Warning,
                "repositories",
                "native_tide_repos is enabled, but Tide-native repositories are not implemented in this release",
            )
            .with_remediation("Set repositories.native_tide_repos = false until Tide repository support ships"),
        );
    } else {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "repositories",
            "Tide-native repositories are disabled; Debian APT repositories provide the current package source",
        ));
    }
    if runner.is_privileged() {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "privileges",
            "running as root; package changes will not require sudo",
        ));
    } else {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "privileges",
            "running as a normal user; privileged package operations use sudo",
        ));
    }

    let statuses = registry.statuses(runner);
    let selected = registry.select(config, cli.backend.as_deref(), runner);
    let mut backends = Vec::new();
    for status in &statuses {
        let mut info = status.info.clone();
        if !info.available {
            info.notes.push("not installed".to_string());
        }
        backends.push(info);
    }
    match &selected {
        Ok(backend) => {
            findings.push(DoctorFinding::new(
                CheckLevel::Ok,
                "backend",
                format!("selected backend: {} ({})", backend.name(), backend.key()),
            ));
            findings.extend(backend.doctor(runner)?);
            if backend.key() == "apt" {
                let caps = backend.capabilities();
                if caps.install && caps.repositories && caps.update {
                    findings.push(DoctorFinding::new(
                        CheckLevel::Ok,
                        "apt",
                        "APT backend exposes install, repository, and metadata operations",
                    ));
                }
            }
        }
        Err(error) => findings.push(
            DoctorFinding::new(CheckLevel::Error, "backend", error.to_string())
                .with_remediation("Install a supported package manager or select Flatpak explicitly"),
        ),
    }

    let has_error = findings
        .iter()
        .any(|finding| finding.level == CheckLevel::Error);
    if cli.json {
        let report = DoctorReport {
            config_path: config_path.display().to_string(),
            findings,
            backends,
        };
        println!("{}", output::json(&report));
    } else {
        println!("Tide package-manager doctor\n");
        output::print_findings(&findings);
        println!("\nDetected backends:");
        output::print_backends(&backends);
    }
    if has_error {
        return Err(TideError::Message(
            "doctor found errors; see findings above".into(),
        ));
    }
    Ok(())
}

fn run_options(cli: &Cli, config: &TideConfig, purge: bool) -> RunOptions {
    RunOptions {
        assume_yes: cli.yes || config.package.assume_yes,
        dry_run: cli.dry_run,
        purge,
    }
}

fn print_operation(cli: &Cli, report: &OperationReport) {
    if cli.json {
        println!("{}", output::json(report));
    } else {
        output::print_report(report);
    }
}

fn confirm(cli: &Cli, config: &TideConfig, prompt: &str) -> Result<()> {
    if cli.yes || config.package.assume_yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err(TideError::InvalidInput {
            kind: "confirmation",
            message: "refusing to change packages non-interactively without --yes".into(),
        });
    }
    print!("{prompt} [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| TideError::io("<stdout>", error))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| TideError::io("<stdin>", error))?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(TideError::Message("operation cancelled".into()))
    }
}

fn validate_query(query: &str) -> Result<()> {
    if query.trim().is_empty() || query.len() > 256 {
        return Err(TideError::InvalidInput {
            kind: "query",
            message: "must be between 1 and 256 characters".into(),
        });
    }
    if query.starts_with('-') || query.chars().any(char::is_control) {
        return Err(TideError::InvalidInput {
            kind: "query",
            message: "may not start with '-' or contain control characters".into(),
        });
    }
    Ok(())
}

fn validate_query_list(values: &[String]) -> Result<()> {
    if values.is_empty() {
        return Err(TideError::InvalidInput {
            kind: "package list",
            message: "at least one package is required".into(),
        });
    }
    for value in values {
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(TideError::InvalidInput {
                kind: "package",
                message: "contains an empty or invalid value".into(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dangerous_queries() {
        assert!(validate_query("--help").is_err());
        assert!(validate_query("fire\nfox").is_err());
        assert!(validate_query("web browser").is_ok());
    }

    #[test]
    fn accepts_package_lists() {
        validate_query_list(&["firefox".into(), "curl".into()]).unwrap();
        assert!(validate_query_list(&[]).is_err());
    }
}
