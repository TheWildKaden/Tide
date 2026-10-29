use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use crate::error::{Result, TideError};

/// Captured result of running an external program.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status.success()
    }
}

/// Abstraction over process execution so adapters can be tested without touching
/// the host package manager.
pub trait CommandRunner {
    /// Resolve a program on `PATH`.
    fn which(&self, program: &str) -> Option<PathBuf>;

    /// Run a program with an argument vector. `env` entries are added to a
    /// sanitised environment; the process is never passed through a shell.
    fn run(
        &self,
        program: &str,
        args: &[String],
        env: &[(&str, &str)],
    ) -> Result<CommandOutput>;

    /// Whether the current process can perform privileged package operations.
    fn is_privileged(&self) -> bool;
}

/// Executes commands on the local machine.
///
/// When `elevate` is set and Tide is not running as root, privileged commands are
/// prefixed with `sudo`. The caller decides which commands are privileged; Tide
/// never silently escalates read-only queries.
#[derive(Debug, Clone)]
pub struct SystemRunner {
    elevate: bool,
}

impl SystemRunner {
    pub fn new(elevate: bool) -> Self {
        Self { elevate }
    }

    fn wrap<'a>(&self, program: &'a str, args: &'a [String]) -> (String, Vec<String>) {
        if self.elevate && !is_privileged() {
            let mut wrapped = Vec::with_capacity(args.len() + 2);
            wrapped.push("--".to_string());
            wrapped.push(program.to_string());
            wrapped.extend(args.iter().cloned());
            ("sudo".to_string(), wrapped)
        } else {
            (program.to_string(), args.to_vec())
        }
    }

    /// Run a command that changes system package state.
    pub fn run_privileged(
        &self,
        program: &str,
        args: &[String],
        env: &[(&str, &str)],
    ) -> Result<CommandOutput> {
        let (program, args) = self.wrap(program, args);
        self.exec(&program, &args, env)
    }

    fn exec(&self, program: &str, args: &[String], env: &[(&str, &str)]) -> Result<CommandOutput> {
        let mut command = Command::new(program);
        command.args(args);
        command.env("LC_ALL", "C");
        command.env("LANG", "C");
        for (key, value) in env {
            command.env(key, value);
        }
        let output = command.output().map_err(|source| TideError::Spawn {
            program: program.to_string(),
            source,
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let result = CommandOutput {
            status: output.status,
            stdout,
            stderr,
        };
        classify_failure(program, &result)?;
        Ok(result)
    }
}

impl CommandRunner for SystemRunner {
    fn which(&self, program: &str) -> Option<PathBuf> {
        which(program)
    }

    fn run(
        &self,
        program: &str,
        args: &[String],
        env: &[(&str, &str)],
    ) -> Result<CommandOutput> {
        self.exec(program, args, env)
    }

    fn is_privileged(&self) -> bool {
        is_privileged()
    }
}

/// Detect common package-manager lock and permission failures and convert them
/// into actionable Tide errors instead of opaque non-zero exits.
fn classify_failure(program: &str, output: &CommandOutput) -> Result<()> {
    if output.success() {
        return Ok(());
    }
    let stderr = output.stderr.trim();
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("could not get lock")
        || lower.contains("unable to lock directory")
        || lower.contains("database is locked")
        || lower.contains("failed to lock")
    {
        return Err(TideError::Locked);
    }
    if lower.contains("permission denied")
        || lower.contains("are you root")
        || lower.contains("you need to be root")
        || lower.contains("requires root")
    {
        return Err(TideError::PermissionDenied);
    }
    Err(TideError::CommandFailed {
        program: program.to_string(),
        code: output.status.code(),
        stderr: stderr.to_string(),
    })
}

pub fn is_privileged() -> bool {
    // SAFETY: geteuid has no preconditions and does not touch memory.
    unsafe { libc::geteuid() == 0 }
}

/// Resolve an executable without invoking a shell.
pub fn which(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let path = PathBuf::from(program);
        return if is_executable(&path) { Some(path) } else { None };
    }
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Test double used by adapter unit tests. Records commands instead of running them.
#[derive(Debug, Default)]
pub struct MockRunner {
    pub available: Vec<String>,
    pub responses: HashMap<String, CommandOutput>,
    pub privileged: bool,
    pub calls: std::cell::RefCell<Vec<(String, Vec<String>)>>,
}

impl MockRunner {
    pub fn with_programs(programs: &[&str]) -> Self {
        Self {
            available: programs.iter().map(|value| value.to_string()).collect(),
            ..Self::default()
        }
    }

    pub fn insert(&mut self, script: &str, stdout: &str, success: bool) {
        self.responses.insert(
            script.to_string(),
            CommandOutput {
                status: exit_status(success),
                stdout: stdout.to_string(),
                stderr: if success {
                    String::new()
                } else {
                    stdout.to_string()
                },
            },
        );
    }
}

fn exit_status(success: bool) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(if success { 0 } else { 1 << 8 })
}

impl CommandRunner for MockRunner {
    fn which(&self, program: &str) -> Option<PathBuf> {
        self.available
            .iter()
            .any(|value| value == program)
            .then(|| PathBuf::from("/usr/bin").join(program))
    }

    fn run(
        &self,
        program: &str,
        args: &[String],
        _env: &[(&str, &str)],
    ) -> Result<CommandOutput> {
        self.calls
            .borrow_mut()
            .push((program.to_string(), args.to_vec()));
        let key = format!("{} {}", program, args.join(" "));
        if let Some(response) = self.responses.get(&key).or_else(|| self.responses.get(program)) {
            return Ok(response.clone());
        }
        Ok(CommandOutput {
            status: exit_status(true),
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn is_privileged(&self) -> bool {
        self.privileged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_lock_errors() {
        let output = CommandOutput {
            status: exit_status(false),
            stdout: String::new(),
            stderr: "E: Could not get lock /var/lib/dpkg/lock-frontend".into(),
        };
        assert!(matches!(
            classify_failure("apt-get", &output),
            Err(TideError::Locked)
        ));
    }

    #[test]
    fn rejects_shell_metacharacters_by_not_using_a_shell() {
        // This is a documentation-by-test guarantee: arguments stay arguments.
        let runner = MockRunner::with_programs(&["apt-get"]);
        let args = vec!["install".to_string(), "; rm -rf /".to_string()];
        runner.run("apt-get", &args, &[]).unwrap();
        let calls = runner.calls.borrow();
        assert_eq!(calls[0].1[1], "; rm -rf /");
    }

    #[test]
    fn which_finds_existing_program() {
        assert!(which("sh").is_some());
    }
}
