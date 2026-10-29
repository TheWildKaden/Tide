use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde::Serialize;
use tide_core::backends::BackendRegistry;
use tide_core::config::TideConfig;
use tide_core::error::{Result, TideError};
use tide_core::exec::{CommandRunner, SystemRunner};
use tide_core::model::{
    BackendInfo, CheckLevel, DoctorFinding, PackageSummary, RepoEntry,
};
use tide_core::util::{parse_key_value_file, validate_unit_name};

#[derive(Debug, Parser)]
#[command(
    name = "tidectl",
    version,
    about = "Tide OS system administration and diagnostics",
    long_about = "Tide-owned administration interface for system identity, diagnostics, services, updates, \
repositories, hardware inventory, and recovery status. It uses standard Linux interfaces and does not replace \
systemd or the kernel."
)]
struct Cli {
    #[arg(long, short = 'b', global = true, env = "TIDE_BACKEND")]
    backend: Option<String>,

    #[arg(long, global = true)]
    json: bool,

    #[arg(long, global = true, env = "TIDE_CONFIG")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Show Tide OS, kernel, and backend information.
    Info,
    /// Diagnose system identity, services, repositories, and package backends.
    Doctor,
    /// Inspect or control systemd services.
    Services {
        #[command(subcommand)]
        action: Option<ServiceAction>,
    },
    /// Check for available package upgrades.
    Updates,
    /// Inspect configured software repositories.
    Repo {
        #[command(subcommand)]
        action: RepoAction,
    },
    /// Show CPU, memory, PCI, USB, and graphics inventory.
    Hardware,
    /// Report recovery and rollback capabilities without changing the system.
    Recover,
}

#[derive(Debug, Subcommand)]
enum ServiceAction {
    /// List services.
    List,
    /// Show the status of a service.
    Status { unit: String },
    /// Start a service.
    Start { unit: String },
    /// Stop a service.
    Stop { unit: String },
    /// Restart a service.
    Restart { unit: String },
    /// Enable a service at boot.
    Enable { unit: String },
    /// Disable a service at boot.
    Disable { unit: String },
}

#[derive(Debug, Subcommand)]
enum RepoAction {
    /// List repositories for the selected backend.
    List,
}

#[derive(Serialize)]
struct SystemInfo {
    tide_version: &'static str,
    os_name: String,
    os_id: String,
    os_version: String,
    foundation: String,
    kernel: String,
    architecture: String,
    hostname: String,
    init: String,
    selected_backend: Option<String>,
    backends: Vec<BackendInfo>,
}

#[derive(Serialize)]
struct ServiceEntry {
    unit: String,
    load: String,
    active: String,
    sub: String,
    description: String,
}

#[derive(Serialize)]
struct HardwareInfo {
    cpu_model: Option<String>,
    cpu_cores: usize,
    memory_kib: Option<u64>,
    pci: Vec<String>,
    usb: Vec<String>,
    graphics_devices: Vec<String>,
    driver_tools: Vec<String>,
}

#[derive(Serialize)]
struct RecoveryStatus {
    root_filesystem: Option<String>,
    btrfs: bool,
    snapper: bool,
    timeshift: bool,
    findings: Vec<DoctorFinding>,
}

#[derive(Serialize)]
struct DoctorReport {
    findings: Vec<DoctorFinding>,
    backends: Vec<BackendInfo>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tidectl: {error}");
            ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(1))
        }
    }
}

fn run(cli: &Cli) -> Result<()> {
    let config = TideConfig::load_optional(cli.config.as_ref())?;
    let runner = SystemRunner::new(true);
    let registry = BackendRegistry::new(&config);

    match &cli.command {
        Commands::Info => {
            let info = system_info(&config, &runner, &registry, cli.backend.as_deref());
            if cli.json {
                print_json(&info);
            } else {
                println!("Tide OS {}", info.tide_version);
                println!("Distribution:  {} {} (ID: {})", info.os_name, info.os_version, info.os_id);
                println!("Foundation:    {}", info.foundation);
                println!("Kernel:        {}", info.kernel);
                println!("Architecture:  {}", info.architecture);
                println!("Hostname:      {}", info.hostname);
                println!("Init system:   {}", info.init);
                println!(
                    "Backend:       {}",
                    info.selected_backend.as_deref().unwrap_or("none detected")
                );
                println!("\nAvailable package backends:");
                for backend in &info.backends {
                    println!(
                        "  {:<10} {}",
                        backend.id,
                        if backend.available { "available" } else { "unavailable" }
                    );
                }
            }
            Ok(())
        }
        Commands::Doctor => doctor(cli, &config, &runner, &registry),
        Commands::Services { action } => services(cli, &runner, action.as_ref()),
        Commands::Updates => updates(cli, &config, &runner, &registry),
        Commands::Repo { action } => match action {
            RepoAction::List => repo_list(cli, &config, &runner, &registry),
        },
        Commands::Hardware => hardware(cli, &runner),
        Commands::Recover => recover(cli, &runner),
    }
}

fn system_info(
    config: &TideConfig,
    runner: &dyn CommandRunner,
    registry: &BackendRegistry,
    explicit: Option<&str>,
) -> SystemInfo {
    let os_release = std::fs::read_to_string("/etc/os-release")
        .map(|content| parse_key_value_file(&content))
        .unwrap_or_default();
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let init = if runner.which("systemctl").is_some() {
        "systemd".to_string()
    } else {
        "not detected".to_string()
    };
    let statuses = registry.statuses(runner);
    let backends = statuses.into_iter().map(|status| status.info).collect();
    let selected_backend = registry
        .select(config, explicit, runner)
        .ok()
        .map(|backend| backend.key().to_string());
    SystemInfo {
        tide_version: env!("CARGO_PKG_VERSION"),
        os_name: os_release
            .get("NAME")
            .cloned()
            .unwrap_or_else(|| "Tide OS".to_string()),
        os_id: os_release.get("ID").cloned().unwrap_or_else(|| "tide".to_string()),
        os_version: os_release
            .get("VERSION")
            .cloned()
            .or_else(|| os_release.get("VERSION_ID").cloned())
            .unwrap_or_else(|| "development".to_string()),
        foundation: os_release
            .get("TIDE_FOUNDATION")
            .cloned()
            .unwrap_or_else(|| "Debian live-build foundation (not yet independent)".to_string()),
        kernel,
        architecture: std::env::consts::ARCH.to_string(),
        hostname,
        init,
        selected_backend,
        backends,
    }
}

fn doctor(
    cli: &Cli,
    config: &TideConfig,
    runner: &dyn CommandRunner,
    registry: &BackendRegistry,
) -> Result<()> {
    let info = system_info(config, runner, registry, cli.backend.as_deref());
    let mut findings = Vec::new();
    if info.os_id == "tide" {
        findings.push(DoctorFinding::new(
            CheckLevel::Ok,
            "identity",
            "system identity reports ID=tide",
        ));
    } else {
        findings.push(
            DoctorFinding::new(
                CheckLevel::Warning,
                "identity",
                format!("system identity reports ID={}, not tide", info.os_id),
            )
            .with_remediation("Install Tide branding from assets/branding/os-release"),
        );
    }
    if info.init == "systemd" {
        findings.push(DoctorFinding::new(
            CheckLevel::Ok,
            "init",
            "systemd is available for service management",
        ));
    } else {
        findings.push(DoctorFinding::new(
            CheckLevel::Warning,
            "init",
            "systemctl was not found; service management is limited",
        ));
    }
    match registry.select(config, cli.backend.as_deref(), runner) {
        Ok(backend) => {
            findings.push(DoctorFinding::new(
                CheckLevel::Ok,
                "packages",
                format!("using {} ({})", backend.name(), backend.key()),
            ));
            findings.extend(backend.doctor(runner)?);
        }
        Err(error) => findings.push(DoctorFinding::new(
            CheckLevel::Error,
            "packages",
            error.to_string(),
        )),
    }
    if let Ok(load) = std::fs::read_to_string("/proc/loadavg") {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "system",
            format!("load average: {}", load.trim().split_whitespace().take(3).collect::<Vec<_>>().join(" ")),
        ));
    }
    if runner.which("fwupdmgr").is_some() {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "firmware",
            "fwupd is available for firmware updates",
        ));
    }
    let has_error = findings
        .iter()
        .any(|finding| finding.level == CheckLevel::Error);
    let backends = info.backends.clone();
    if cli.json {
        print_json(&DoctorReport { findings, backends });
    } else {
        println!("Tide OS doctor\n");
        print_findings(&findings);
        println!("\nBackends:");
        for backend in &backends {
            println!(
                "  {:<10} {:<12} {}",
                backend.id,
                if backend.available { "available" } else { "unavailable" },
                backend.version.as_deref().unwrap_or("")
            );
        }
    }
    if has_error {
        return Err(TideError::Message("doctor found errors".into()));
    }
    Ok(())
}

fn services(cli: &Cli, runner: &dyn CommandRunner, action: Option<&ServiceAction>) -> Result<()> {
    if runner.which("systemctl").is_none() {
        return Err(TideError::Message(
            "systemctl was not found; Tide service management currently uses systemd".into(),
        ));
    }
    match action.unwrap_or(&ServiceAction::List) {
        ServiceAction::List => {
            let output = runner.run(
                "systemctl",
                &[
                    "list-units".into(),
                    "--type=service".into(),
                    "--all".into(),
                    "--no-pager".into(),
                    "--no-legend".into(),
                ],
                &[],
            )?;
            let entries = parse_units(&output.stdout);
            if cli.json {
                print_json(&entries);
            } else {
                for entry in &entries {
                    println!(
                        "{:<40} {:<9} {:<10} {}",
                        entry.unit, entry.load, entry.active, entry.description
                    );
                }
            }
            Ok(())
        }
        ServiceAction::Status { unit } => {
            validate_unit_name(unit)?;
            let output = runner.run(
                "systemctl",
                &["status".into(), unit.clone(), "--no-pager".into()],
                &[],
            );
            // `systemctl status` exits non-zero for inactive units, which is still a
            // useful status report rather than a Tide failure.
            match output {
                Ok(output) => {
                    print!("{}", output.stdout);
                    eprint!("{}", output.stderr);
                    Ok(())
                }
                Err(TideError::CommandFailed { stderr, .. }) => {
                    eprintln!("{stderr}");
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
        ServiceAction::Start { unit } => run_service(runner, "start", unit),
        ServiceAction::Stop { unit } => run_service(runner, "stop", unit),
        ServiceAction::Restart { unit } => run_service(runner, "restart", unit),
        ServiceAction::Enable { unit } => run_service(runner, "enable", unit),
        ServiceAction::Disable { unit } => run_service(runner, "disable", unit),
    }
}

fn run_service(runner: &dyn CommandRunner, verb: &str, unit: &str) -> Result<()> {
    validate_unit_name(unit)?;
    let output = runner.run_privileged(
        "systemctl",
        &[verb.to_string(), unit.to_string(), "--no-pager".to_string()],
        &[],
    )?;
    if !output.stdout.trim().is_empty() {
        print!("{}", output.stdout);
    }
    println!("systemctl {verb} {unit}: ok");
    Ok(())
}

fn parse_units(output: &str) -> Vec<ServiceEntry> {
    output
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 4 {
                return None;
            }
            Some(ServiceEntry {
                unit: fields[0].to_string(),
                load: fields[1].to_string(),
                active: fields[2].to_string(),
                sub: fields[3].to_string(),
                description: fields[4..].join(" "),
            })
        })
        .collect()
}

fn updates(
    cli: &Cli,
    config: &TideConfig,
    runner: &dyn CommandRunner,
    registry: &BackendRegistry,
) -> Result<()> {
    let backend = registry.select(config, cli.backend.as_deref(), runner)?;
    if !backend.capabilities().outdated {
        return Err(TideError::Unsupported {
            backend: backend.key(),
            operation: "upgrade checking",
        });
    }
    let packages: Vec<PackageSummary> = backend.outdated(runner)?;
    if cli.json {
        print_json(&packages);
    } else if packages.is_empty() {
        println!("No upgrades are available from {}.", backend.name());
    } else {
        println!(
            "{} upgrade(s) available from {}:",
            packages.len(),
            backend.name()
        );
        for package in &packages {
            println!(
                "  {:<40} {}",
                package.name,
                package.version.as_deref().unwrap_or("")
            );
        }
        println!("\nApply upgrades with: tide upgrade");
    }
    Ok(())
}

fn repo_list(
    cli: &Cli,
    config: &TideConfig,
    runner: &dyn CommandRunner,
    registry: &BackendRegistry,
) -> Result<()> {
    let backend = registry.select(config, cli.backend.as_deref(), runner)?;
    if !backend.capabilities().repositories {
        return Err(TideError::Unsupported {
            backend: backend.key(),
            operation: "repository listing",
        });
    }
    let repos: Vec<RepoEntry> = backend.repositories(runner)?;
    if cli.json {
        print_json(&repos);
        return Ok(());
    }
    for repo in &repos {
        println!(
            "{:<9} {:<40} {}",
            if repo.enabled { "enabled" } else { "disabled" },
            repo.id,
            repo.uri.as_deref().unwrap_or("-")
        );
    }
    if repos.is_empty() {
        println!("No repositories found for {}.", backend.name());
    }
    Ok(())
}

fn hardware(cli: &Cli, runner: &dyn CommandRunner) -> Result<()> {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let cpu_model = cpuinfo
        .lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim().to_string());
    let cpu_cores = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(0);
    let memory_kib = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("MemTotal:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse::<u64>().ok())
        });
    let pci = command_lines(runner, "lspci", &["-nnk"]);
    let usb = command_lines(runner, "lsusb", &[]);
    let graphics_devices = graphics_devices();
    let driver_tools = ["fwupdmgr", "ubuntu-drivers", "pkcon", "flatpak"]
        .iter()
        .filter(|program| runner.which(program).is_some())
        .map(|program| program.to_string())
        .collect::<Vec<_>>();
    let info = HardwareInfo {
        cpu_model,
        cpu_cores,
        memory_kib,
        pci,
        usb,
        graphics_devices,
        driver_tools,
    };
    if cli.json {
        print_json(&info);
        return Ok(());
    }
    println!(
        "CPU:      {} ({} logical cores)",
        info.cpu_model.as_deref().unwrap_or("unknown"),
        info.cpu_cores
    );
    if let Some(memory) = info.memory_kib {
        println!("Memory:   {:.1} GiB", memory as f64 / 1024.0 / 1024.0);
    }
    println!(
        "Graphics: {}",
        if info.graphics_devices.is_empty() {
            "not reported".to_string()
        } else {
            info.graphics_devices.join(", ")
        }
    );
    println!(
        "Driver tools: {}",
        if info.driver_tools.is_empty() {
            "none detected".to_string()
        } else {
            info.driver_tools.join(", ")
        }
    );
    if !info.pci.is_empty() {
        println!("\nPCI devices:");
        for line in &info.pci {
            println!("  {line}");
        }
    }
    if !info.usb.is_empty() {
        println!("\nUSB devices:");
        for line in &info.usb {
            println!("  {line}");
        }
    }
    Ok(())
}

fn command_lines(runner: &dyn CommandRunner, program: &str, args: &[&str]) -> Vec<String> {
    if runner.which(program).is_none() {
        return Vec::new();
    }
    let args = args.iter().map(|value| value.to_string()).collect::<Vec<_>>();
    runner
        .run(program, &args, &[])
        .map(|output| {
            output
                .stdout
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn graphics_devices() -> Vec<String> {
    let mut devices = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return devices;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let driver = std::fs::read_link(entry.path().join("device/driver"))
            .ok()
            .and_then(|path| path.file_name().map(|value| value.to_string_lossy().to_string()));
        devices.push(match driver {
            Some(driver) => format!("{name} ({driver})"),
            None => name,
        });
    }
    devices.sort();
    devices
}

fn recover(cli: &Cli, runner: &dyn CommandRunner) -> Result<()> {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let root_filesystem = mounts.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let _device = fields.next()?;
        let mountpoint = fields.next()?;
        let filesystem = fields.next()?;
        (mountpoint == "/").then(|| filesystem.to_string())
    });
    let btrfs = root_filesystem.as_deref() == Some("btrfs");
    let snapper = runner.which("snapper").is_some();
    let timeshift = runner.which("timeshift").is_some();
    let mut findings = Vec::new();
    if btrfs {
        findings.push(DoctorFinding::new(
            CheckLevel::Ok,
            "recovery",
            "root filesystem is btrfs; snapshot support is technically possible",
        ));
    } else {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "recovery",
            format!(
                "root filesystem is {}; snapshot-based rollback requires a snapshot-capable filesystem",
                root_filesystem.as_deref().unwrap_or("unknown")
            ),
        ));
    }
    if snapper {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "recovery",
            "snapper is installed; Tide rollback integration is not yet implemented",
        ));
    }
    if timeshift {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "recovery",
            "timeshift is installed but is not a Tide-managed recovery mechanism",
        ));
    }
    if !snapper && !timeshift {
        findings.push(DoctorFinding::new(
            CheckLevel::Info,
            "recovery",
            "no snapshot tool is installed; Tide recovery/rollback is planned, not implemented",
        ));
    }
    let status = RecoveryStatus {
        root_filesystem,
        btrfs,
        snapper,
        timeshift,
        findings,
    };
    if cli.json {
        print_json(&status);
    } else {
        println!(
            "Root filesystem: {}",
            status.root_filesystem.as_deref().unwrap_or("unknown")
        );
        println!("btrfs:           {}", status.btrfs);
        println!("snapper:         {}", status.snapper);
        println!("timeshift:       {}", status.timeshift);
        println!();
        print_findings(&status.findings);
    }
    Ok(())
}

fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(rendered) => println!("{rendered}"),
        Err(error) => eprintln!("tidectl: could not encode JSON: {error}"),
    }
}

fn print_findings(findings: &[DoctorFinding]) {
    for finding in findings {
        println!(
            "[{:<7}] {:<10} {}",
            finding.level.label(),
            finding.area,
            finding.message
        );
        if let Some(remediation) = &finding.remediation {
            println!("          fix: {remediation}");
        }
    }
}

#[allow(dead_code)]
fn _keep_map_type_exercised(values: &BTreeMap<String, String>) -> usize {
    values.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_systemd_unit_list() {
        let units = parse_units("sshd.service loaded active running OpenSSH server\n");
        assert_eq!(units[0].unit, "sshd.service");
        assert_eq!(units[0].active, "active");
        assert_eq!(units[0].description, "OpenSSH server");
    }

    #[test]
    fn ignores_malformed_unit_lines() {
        assert!(parse_units("garbage\n\n").is_empty());
    }
}
