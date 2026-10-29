use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, TideError>;

#[derive(Debug, Error)]
pub enum TideError {
    #[error("backend '{0}' is not available on this system")]
    BackendUnavailable(String),

    #[error("backend '{backend}' does not support {operation}")]
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },

    #[error("invalid {kind}: {message}")]
    InvalidInput { kind: &'static str, message: String },

    #[error("command '{program}' failed with exit code {code:?}: {stderr}")]
    CommandFailed {
        program: String,
        code: Option<i32>,
        stderr: String,
    },

    #[error("command '{program}' could not be started: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("the package manager database is locked; wait for the other package operation to finish")]
    Locked,

    #[error("permission denied for a privileged operation; run as root or via sudo")]
    PermissionDenied,

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("configuration error: {0}")]
    Config(String),

    #[error("{0}")]
    Message(String),
}

impl TideError {
    /// Process exit code used by the `tide` and `tidectl` command-line tools.
    pub fn exit_code(&self) -> i32 {
        match self {
            TideError::BackendUnavailable(_) | TideError::Unsupported { .. } => 3,
            TideError::InvalidInput { .. } | TideError::Config(_) => 2,
            TideError::PermissionDenied => 4,
            TideError::Locked => 5,
            TideError::CommandFailed { .. }
            | TideError::Spawn { .. }
            | TideError::Io { .. }
            | TideError::Message(_) => 1,
        }
    }

    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        TideError::Io {
            path: path.into(),
            source,
        }
    }
}
