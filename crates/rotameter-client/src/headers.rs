#![forbid(unsafe_code)]
//! HTTP header construction, URL segment building, and shared request guards.

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use rotameter_core::id::SiteRef;
use rotameter_core::query::QueryFormat;
use url::Url;

use crate::config::ClientConfig;
use crate::error::ClientError;
use crate::types::{BatchEventsRequest, MAX_BATCH_EVENTS, QueryRequest};

static CF_ACCESS_CLIENT_ID: HeaderName = HeaderName::from_static("cf-access-client-id");
static CF_ACCESS_CLIENT_SECRET: HeaderName = HeaderName::from_static("cf-access-client-secret");

pub(crate) fn build_default_headers(config: &ClientConfig) -> Result<HeaderMap, ClientError> {
    let mut headers = HeaderMap::new();
    let ua = format!("rotameter-client/{}", rotameter_core::WORKSPACE_VERSION);
    let ua_val = HeaderValue::from_str(&ua)
        .map_err(|err| ClientError::InvalidRequest(format!("invalid User-Agent header: {err}")))?;
    headers.insert(USER_AGENT, ua_val);

    if let Some(token) = config
        .token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let auth = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|err| {
            ClientError::InvalidRequest(format!("invalid Authorization token header: {err}"))
        })?;
        headers.insert(AUTHORIZATION, auth);
    }

    insert_optional_header(
        &mut headers,
        &CF_ACCESS_CLIENT_ID,
        config.cf_access_client_id.as_deref(),
        "CF-Access-Client-Id",
    )?;
    insert_optional_header(
        &mut headers,
        &CF_ACCESS_CLIENT_SECRET,
        config.cf_access_client_secret.as_deref(),
        "CF-Access-Client-Secret",
    )?;

    Ok(headers)
}

fn insert_optional_header(
    headers: &mut HeaderMap,
    name: &HeaderName,
    raw: Option<&str>,
    label: &str,
) -> Result<(), ClientError> {
    if let Some(val) = raw.map(str::trim).filter(|s| !s.is_empty()) {
        let header_val = HeaderValue::from_str(val)
            .map_err(|err| ClientError::InvalidRequest(format!("invalid {label} header: {err}")))?;
        headers.insert(name.clone(), header_val);
    }
    Ok(())
}

pub(crate) fn endpoint_url(base: &Url, segments: &[&str]) -> Result<Url, ClientError> {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| ClientError::InvalidUrl("URL cannot be a base".to_string()))?;
        path.pop_if_empty().extend(segments);
    }
    Ok(url)
}

pub(crate) fn delete_site_url(
    base: &Url,
    site: &SiteRef,
    confirm: bool,
) -> Result<Url, ClientError> {
    let site_seg = site.to_string();
    let mut url = endpoint_url(base, &["v1", "admin", "sites", &site_seg])?;
    url.query_pairs_mut()
        .append_pair("confirm", if confirm { "true" } else { "false" });
    Ok(url)
}

pub(crate) fn query_with_format(req: &QueryRequest, format: QueryFormat) -> QueryRequest {
    let mut out = req.clone();
    out.format = format;
    out
}

pub(crate) fn validate_batch(batch: &BatchEventsRequest) -> Result<(), ClientError> {
    if batch.events.len() > MAX_BATCH_EVENTS {
        return Err(ClientError::InvalidRequest(format!(
            "batch contains {} events, exceeding limit of {MAX_BATCH_EVENTS}",
            batch.events.len()
        )));
    }
    Ok(())
}
