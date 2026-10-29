//! Shared types, safe process execution, and package-backend adapters for Tide OS.
//!
//! The crate deliberately has no shell-evaluation layer. Every external command is
//! expressed as an executable plus an argument vector, which removes shell injection
//! as a class of vulnerability from Tide's package-management path.

pub mod backend;
pub mod backends;
pub mod config;
pub mod error;
pub mod exec;
pub mod model;
pub mod paths;
pub mod util;

pub use backend::{BackendStatus, PackageBackend, RunOptions};
pub use error::{Result, TideError};
pub use exec::{CommandOutput, CommandRunner, SystemRunner};
pub use model::{
    BackendCapabilities, BackendInfo, CheckLevel, DoctorFinding, OperationReport, PackageSummary,
    RepoEntry,
};
