use crate::error::Result;
use crate::exec::CommandRunner;
use crate::model::{
    BackendCapabilities, BackendInfo, DoctorFinding, OperationReport, PackageSummary, RepoEntry,
};

/// Options controlling a package operation.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub assume_yes: bool,
    pub dry_run: bool,
    pub purge: bool,
}

/// The contract every package backend implements.
///
/// Capabilities are explicit. A backend that cannot express an operation returns
/// an `Unsupported` error rather than pretending the operation succeeded.
pub trait PackageBackend {
    fn key(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> BackendCapabilities;
    fn detect(&self, runner: &dyn CommandRunner) -> bool;
    fn version(&self, _runner: &dyn CommandRunner) -> Option<String> {
        None
    }
    fn notes(&self) -> Vec<String> {
        Vec::new()
    }

    fn info_summary(&self, runner: &dyn CommandRunner) -> BackendInfo {
        BackendInfo {
            id: self.key().to_string(),
            name: self.name().to_string(),
            available: self.detect(runner),
            version: self.version(runner),
            capabilities: self.capabilities(),
            notes: self.notes(),
        }
    }

    fn search(&self, _runner: &dyn CommandRunner, _query: &str) -> Result<Vec<PackageSummary>>;
    fn info(&self, _runner: &dyn CommandRunner, _package: &str) -> Result<PackageSummary>;
    fn list_installed(&self, _runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>>;
    fn install(
        &self,
        _runner: &dyn CommandRunner,
        _packages: &[String],
        _options: &RunOptions,
    ) -> Result<OperationReport>;
    fn remove(
        &self,
        _runner: &dyn CommandRunner,
        _packages: &[String],
        _options: &RunOptions,
    ) -> Result<OperationReport>;
    fn update(&self, _runner: &dyn CommandRunner) -> Result<OperationReport>;
    fn upgrade(&self, _runner: &dyn CommandRunner) -> Result<OperationReport>;
    fn outdated(&self, _runner: &dyn CommandRunner) -> Result<Vec<PackageSummary>>;
    fn repositories(&self, _runner: &dyn CommandRunner) -> Result<Vec<RepoEntry>>;
    fn doctor(&self, _runner: &dyn CommandRunner) -> Result<Vec<DoctorFinding>> {
        Ok(Vec::new())
    }
}

pub fn unsupported(backend: &'static str, operation: &'static str) -> crate::error::TideError {
    crate::error::TideError::Unsupported { backend, operation }
}
