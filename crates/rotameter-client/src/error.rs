#![forbid(unsafe_code)]
//! Client error types and `/v1/*` error envelope decoding.

use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

/// Error returned by [`crate::config`] resolution or HTTP client calls.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Structured `/v1/*` JSON error envelope (`{ "error": { "code", "message", "details" } }`).
    #[error("API error {status} ({code}): {message}")]
    Api {
        /// HTTP status code returned by the server.
        status: u16,
        /// Stable machine-readable error code.
        code: String,
        /// Human-readable error explanation.
        message: String,
        /// Optional structured field metadata.
        details: Option<serde_json::Value>,
    },
    /// Non-success HTTP response without a structured `/v1/*` error envelope.
    #[error("HTTP status {status}: {body}")]
    HttpStatus {
        /// HTTP status code returned by the server or proxy.
        status: u16,
        /// Raw response payload text.
        body: String,
    },
    /// Underlying `reqwest` transport or JSON serialization/deserialization error.
    #[error("HTTP transport error: {0}")]
    Transport(#[from] reqwest::Error),
    /// Configured base URL is malformed or uses an unsupported scheme.
    #[error("invalid Rotameter URL: {0}")]
    InvalidUrl(String),
    /// No base URL was found in flags, environment variables, or `config.toml`.
    #[error("missing Rotameter URL: provide --url, ROTAMETER_URL, or a profile url in config.toml")]
    MissingUrl,
    /// Reading the client configuration file failed.
    #[error("failed to read config `{}`: {source}", path.display())]
    ConfigIo {
        /// Path of the unreadable configuration file.
        path: PathBuf,
        /// Underlying filesystem I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Parsing `config.toml` failed.
    #[error("failed to parse config `{}`: {source}", path.display())]
    ConfigParse {
        /// Path of the invalid TOML configuration file.
        path: PathBuf,
        /// Underlying TOML parser error.
        #[source]
        source: toml::de::Error,
    },
    /// Named profile was requested but does not exist in `config.toml`.
    #[error("profile `{0}` not found in config.toml")]
    ProfileNotFound(String),
    /// Client-side request validation failed before dispatching HTTP I/O.
    #[error("invalid client request: {0}")]
    InvalidRequest(String),
}

impl ClientError {
    /// Returns `true` when the failure reflects an authentication or authorization error (`401`/`403`).
    pub const fn is_auth_error(&self) -> bool {
        matches!(
            self,
            Self::Api {
                status: 401 | 403,
                ..
            } | Self::HttpStatus {
                status: 401 | 403,
                ..
            }
        )
    }
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
    #[serde(default)]
    details: Option<serde_json::Value>,
}

pub(crate) fn error_from_status(status: u16, body: String) -> ClientError {
    if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&body) {
        ClientError::Api {
            status,
            code: env.error.code,
            message: env.error.message,
            details: env.error.details,
        }
    } else {
        ClientError::HttpStatus { status, body }
    }
}
