#![forbid(unsafe_code)]
//! Documented CLI exit codes and error mapping (`SPEC-INTERFACES.md` §2).

use std::fmt;

use rotameter_client::ClientError;
use rotameter_core::error::CoreError;

/// Command completed successfully.
pub const EXIT_OK: u8 = 0;
/// General runtime or HTTP execution failure.
pub const EXIT_RUNTIME: u8 = 1;
/// Invalid CLI arguments, flags, or unsupported feature configuration.
pub const EXIT_USAGE: u8 = 2;
/// Authentication or Cloudflare Access authorization error.
pub const EXIT_AUTH: u8 = 3;
/// `doctor` / health check failure.
pub const EXIT_HEALTH: u8 = 4;

/// Structured CLI error carrying one of the documented exit codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    /// Process exit status code (`1..=4`).
    pub code: u8,
    /// Diagnostic message emitted to `stderr`.
    pub message: String,
}

impl CliError {
    /// Creates a runtime failure (`exit 1`).
    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_RUNTIME,
            message: message.into(),
        }
    }

    /// Creates an argument/usage error (`exit 2`).
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_USAGE,
            message: message.into(),
        }
    }

    /// Creates an auth/Access failure (`exit 3`).
    pub fn auth(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_AUTH,
            message: message.into(),
        }
    }

    /// Creates a health/doctor gate failure (`exit 4`).
    pub fn health(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_HEALTH,
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

impl From<ClientError> for CliError {
    fn from(err: ClientError) -> Self {
        if err.is_auth_error() {
            Self::auth(err.to_string())
        } else {
            Self::runtime(err.to_string())
        }
    }
}

impl From<CoreError> for CliError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::InvalidId(_)
            | CoreError::InvalidSlug(_)
            | CoreError::InvalidUrl(_)
            | CoreError::InvalidEventName(_)
            | CoreError::InvalidProperties(_)
            | CoreError::InvalidQuery(_) => Self::usage(err.to_string()),
            CoreError::HostMismatch(_) | CoreError::Storage(_) | CoreError::SaltUnavailable(_) => {
                Self::runtime(err.to_string())
            }
        }
    }
}
