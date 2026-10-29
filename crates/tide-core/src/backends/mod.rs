use std::path::PathBuf;

use crate::backend::PackageBackend;
use crate::config::TideConfig;
use crate::error::{Result, TideError};
use crate::exec::CommandRunner;
use crate::model::BackendInfo;
use crate::util::display_command;

pub mod apk;
pub mod appimage;
pub mod apt;
pub mod dnf;
pub mod flatpak;
pub mod pacman;

pub use apk::ApkBackend;
pub use appimage::AppImageBackend;
pub use apt::AptBackend;
pub use dnf::DnfBackend;
pub use flatpak::FlatpakBackend;
pub use pacman::PacmanBackend;

/// Backend ids in the order used for automatic selection on an unknown host.
/// Flatpak and AppImage are intentionally not automatic: they complement a
/// system package manager rather than replacing it.
pub const PRIMARY_ORDER: &[&str] = &["apt", "dnf5", "pacman", "apk"];

#[derive(Debug, Clone)]
pub struct BackendStatus {
    pub info: BackendInfo,
    pub primary_candidate: bool,
}

pub struct BackendRegistry {
    backends: Vec<Box<dyn PackageBackend>>,
}

impl BackendRegistry {
    pub fn new(config: &TideConfig) -> Self {
        let appimage_dir = config
            .appimage
            .managed_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(crate::paths::default_appimage_dir);
        Self {
            backends: vec![
                Box::new(AptBackend::new(
                    PathBuf::from("/etc/apt"),
                    config.apt.install_recommends,
                )),
                Box::new(DnfBackend::new(PathBuf::from("/etc/yum.repos.d"))),
                Box::new(PacmanBackend::new(PathBuf::from("/etc/pacman.conf"))),
                Box::new(ApkBackend::new(PathBuf::from("/etc/apk/repositories"))),
                Box::new(FlatpakBackend::new(config.flatpak.scope.clone())),
                Box::new(AppImageBackend::new(
                    appimage_dir,
                    crate::paths::default_appimage_desktop_dir(),
                )),
            ],
        }
    }

    pub fn statuses(&self, runner: &dyn CommandRunner) -> Vec<BackendStatus> {
        self.backends
            .iter()
            .map(|backend| BackendStatus {
                info: backend.info_summary(runner),
                primary_candidate: PRIMARY_ORDER.contains(&backend.key()),
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&dyn PackageBackend> {
        let normalized = if id == "dnf" { "dnf5" } else { id };
        self.backends
            .iter()
            .find(|backend| backend.key() == normalized)
            .map(|backend| backend.as_ref())
    }

    /// Resolve the backend for an operation.
    ///
    /// Explicit selection must name an available backend. Automatic selection uses
    /// the configured preference, then the first available native package manager.
    pub fn select(
        &self,
        config: &TideConfig,
        explicit: Option<&str>,
        runner: &dyn CommandRunner,
    ) -> Result<&dyn PackageBackend> {
        let requested = explicit
            .filter(|value| !value.is_empty() && *value != "auto")
            .or_else(|| {
                (config.package.preferred_backend != "auto")
                    .then_some(config.package.preferred_backend.as_str())
            });

        if let Some(requested) = requested {
            let backend = self.get(requested).ok_or_else(|| {
                TideError::BackendUnavailable(format!(
                    "{requested} (known backends: {})",
                    self.ids().join(", ")
                ))
            })?;
            if !backend.detect(runner) {
                return Err(TideError::BackendUnavailable(backend.key().to_string()));
            }
            return Ok(backend);
        }

        for id in PRIMARY_ORDER {
            if let Some(backend) = self.get(id) {
                if backend.detect(runner) {
                    return Ok(backend);
                }
            }
        }

        Err(TideError::Message(
            "no supported native package manager was detected; install apt, dnf5, pacman, or apk, or select flatpak explicitly with --backend flatpak".into(),
        ))
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.backends.iter().map(|backend| backend.key()).collect()
    }
}

pub(crate) fn run(
    runner: &dyn CommandRunner,
    program: &str,
    args: &[String],
    env: &[(&str, &str)],
) -> Result<String> {
    let output = runner.run(program, args, env)?;
    Ok(output.stdout)
}

pub(crate) fn run_privileged(
    runner: &dyn CommandRunner,
    program: &str,
    args: &[String],
    env: &[(&str, &str)],
) -> Result<String> {
    let output = runner.run_privileged(program, args, env)?;
    Ok(output.stdout)
}

pub(crate) fn planned(program: &str, args: &[String]) -> String {
    display_command(program, args)
}

pub(crate) fn ensure_program(runner: &dyn CommandRunner, program: &str) -> Result<()> {
    if runner.which(program).is_none() {
        return Err(TideError::Message(format!(
            "required command '{program}' was not found on PATH"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::MockRunner;

    #[test]
    fn registry_contains_expected_backends() {
        let registry = BackendRegistry::new(&TideConfig::default());
        let ids = registry.ids();
        for expected in ["apt", "dnf5", "pacman", "apk", "flatpak", "appimage"] {
            assert!(ids.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn selects_apt_when_available() {
        let registry = BackendRegistry::new(&TideConfig::default());
        let runner = MockRunner::with_programs(&["apt-get"]);
        let backend = registry
            .select(&TideConfig::default(), None, &runner)
            .unwrap();
        assert_eq!(backend.key(), "apt");
    }

    #[test]
    fn explicit_missing_backend_fails_clearly() {
        let registry = BackendRegistry::new(&TideConfig::default());
        let runner = MockRunner::default();
        let error = registry
            .select(&TideConfig::default(), Some("pacman"), &runner)
            .unwrap_err();
        assert!(error.to_string().contains("not available"));
    }
}
