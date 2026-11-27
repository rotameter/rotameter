#![forbid(unsafe_code)]
//! Uniform JSON `/v1` error envelope implementing [`axum::response::IntoResponse`].

use std::fmt;

use axum::Json;
use axum::http::header::RETRY_AFTER;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use rotameter_core::error::CoreError;
use serde::Serialize;

/// Uniform HTTP JSON error payload (`{ "error": { "code", "message", "details" } }`).
#[derive(Debug, Clone)]
pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    details: Option<serde_json::Value>,
    retry_after_secs: Option<u64>,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: &'a Option<serde_json::Value>,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
            retry_after_secs: None,
        }
    }

    pub(crate) fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub(crate) fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", message)
    }

    pub(crate) fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message)
    }

    pub(crate) fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub(crate) fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub(crate) fn too_many_requests(
        code: &'static str,
        message: impl Into<String>,
        retry_after_secs: u64,
    ) -> Self {
        let mut err = Self::new(StatusCode::TOO_MANY_REQUESTS, code, message);
        err.retry_after_secs = Some(retry_after_secs.max(1));
        err
    }

    pub(crate) fn service_unavailable(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message)
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: &self.message,
                details: &self.details,
            },
        };
        let mut resp = (self.status, Json(body)).into_response();
        if let Some(secs) = self.retry_after_secs
            && let Ok(val) = HeaderValue::from_str(&secs.to_string())
        {
            resp.headers_mut().insert(RETRY_AFTER, val);
        }
        resp
    }
}

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        let msg = err.to_string();
        match err {
            CoreError::InvalidId(_) => Self::bad_request("invalid_id", msg),
            CoreError::InvalidSlug(_) => Self::bad_request("invalid_slug", msg),
            CoreError::InvalidUrl(_) => Self::bad_request("invalid_url", msg),
            CoreError::InvalidEventName(_) => Self::bad_request("invalid_event_name", msg),
            CoreError::InvalidProperties(_) => Self::bad_request("invalid_properties", msg),
            CoreError::InvalidQuery(_) => Self::bad_request("invalid_query", msg),
            CoreError::HostMismatch(_) => Self::forbidden("host_mismatch", msg),
            CoreError::Storage(_) => Self::internal(msg),
            CoreError::SaltUnavailable(_) => Self::service_unavailable("salt_unavailable", msg),
        }
    }
}
