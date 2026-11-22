#![forbid(unsafe_code)]
//! Asynchronous typed HTTP client for the Rotameter `/v1/*` API.

mod admin;

use std::fmt;

use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;

use crate::config::ClientConfig;
use crate::error::{ClientError, error_from_status};
use crate::headers::{build_default_headers, endpoint_url, query_with_format, validate_batch};
use crate::types::{
    BatchEventsRequest, BatchEventsResponse, EventAcceptedResponse, EventPayload, HealthResponse,
    QueryFormat, QueryRequest, QueryResponse, RealtimeResponse, SiteRef,
};

/// Asynchronous typed client targeting a Rotameter `/v1/*` server.
#[derive(Clone)]
pub struct RotameterClient {
    config: ClientConfig,
    http: Client,
}

impl fmt::Debug for RotameterClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RotameterClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl RotameterClient {
    /// Builds an asynchronous [`RotameterClient`] configured with default auth and Access headers.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if header construction or underlying TLS/HTTP setup fails.
    pub fn new(config: ClientConfig) -> Result<Self, ClientError> {
        let headers = build_default_headers(&config)?;
        let http = Client::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .build()?;
        Ok(Self { config, http })
    }

    /// Returns the resolved [`ClientConfig`] used by this client.
    pub const fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Fetches `/v1/health` liveness and queue telemetry.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network failure or non-success HTTP status.
    pub async fn health(&self) -> Result<HealthResponse, ClientError> {
        let url = endpoint_url(&self.config.base_url, &["v1", "health"])?;
        send_json(self.http.get(url)).await
    }

    /// Executes a typed analytics query (`POST /v1/query`, `format = json`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network failure, auth failure, or invalid query parameters.
    pub async fn query(&self, req: &QueryRequest) -> Result<QueryResponse, ClientError> {
        let url = endpoint_url(&self.config.base_url, &["v1", "query"])?;
        let payload = query_with_format(req, QueryFormat::Json);
        send_json(self.http.post(url).json(&payload)).await
    }

    /// Executes an analytics query returning raw `RFC 4180` CSV (`POST /v1/query`, `format = csv`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network failure, auth failure, or invalid query parameters.
    pub async fn query_csv(&self, req: &QueryRequest) -> Result<String, ClientError> {
        let url = endpoint_url(&self.config.base_url, &["v1", "query"])?;
        let payload = query_with_format(req, QueryFormat::Csv);
        send_text(self.http.post(url).json(&payload)).await
    }

    /// Fetches the trailing 5-minute active snapshot and 30-minute pulse for `site`.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network failure, missing site, or unauthorized access.
    pub async fn realtime(&self, site: &SiteRef) -> Result<RealtimeResponse, ClientError> {
        let site_seg = site.to_string();
        let url = endpoint_url(
            &self.config.base_url,
            &["v1", "sites", &site_seg, "realtime"],
        )?;
        send_json(self.http.get(url)).await
    }

    /// Submits a single event to `POST /v1/events`.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on network failure or server rejection.
    pub async fn post_event(
        &self,
        event: &EventPayload,
    ) -> Result<EventAcceptedResponse, ClientError> {
        let url = endpoint_url(&self.config.base_url, &["v1", "events"])?;
        send_json(self.http.post(url).json(event)).await
    }

    /// Submits up to `500` historical/import events to `POST /v1/events/batch`.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidRequest`] if `batch.events.len() > 500`, or [`ClientError`]
    /// on transport/HTTP errors.
    pub async fn post_events_batch(
        &self,
        batch: &BatchEventsRequest,
    ) -> Result<BatchEventsResponse, ClientError> {
        validate_batch(batch)?;
        let url = endpoint_url(&self.config.base_url, &["v1", "events", "batch"])?;
        send_json(self.http.post(url).json(batch)).await
    }
}

pub(super) async fn send_json<T: DeserializeOwned>(req: RequestBuilder) -> Result<T, ClientError> {
    let text = send_text(req).await?;
    serde_json::from_str(&text).map_err(|err| {
        ClientError::InvalidRequest(format!("invalid JSON response from server: {err}"))
    })
}

async fn send_text(req: RequestBuilder) -> Result<String, ClientError> {
    let resp = req.send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(error_from_status(status.as_u16(), body));
    }
    Ok(body)
}
