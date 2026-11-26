#![forbid(unsafe_code)]
//! Wire JSON DTOs for `POST /v1/events/batch`.

use std::collections::BTreeMap;

use serde::Deserialize;

pub(crate) const MAX_BATCH_EVENTS: usize = 500;

#[derive(Deserialize)]
pub(crate) struct BatchEventsRequest {
    pub(crate) events: Vec<BatchEventItem>,
}

#[derive(Deserialize)]
pub(crate) struct BatchEventItem {
    pub(crate) site_id: String,
    pub(crate) event: Option<String>,
    pub(crate) url: String,
    pub(crate) referrer: Option<String>,
    pub(crate) viewport_width: Option<u16>,
    #[serde(default)]
    pub(crate) props: Option<BTreeMap<String, serde_json::Value>>,
    pub(crate) ts: Option<String>,
    pub(crate) visitor_hint: Option<String>,
    pub(crate) user_agent: Option<String>,
    pub(crate) country: Option<String>,
    pub(crate) ip: Option<String>,
    #[serde(default)]
    pub(crate) allow_automated: bool,
}
