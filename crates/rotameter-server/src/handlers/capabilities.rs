#![forbid(unsafe_code)]
//! Unauthenticated `GET /v1/capabilities` boot handshake endpoint (`SPEC-UI.md` §4.1).

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::auth_config::UiMode;
use crate::state::AppState;

#[allow(clippy::struct_excessive_bools)]
#[derive(Serialize)]
pub(crate) struct CapabilitiesFeatures {
    ui: bool,
    arrow_export: bool,
    oidc: bool,
    streams: bool,
}

#[derive(Serialize)]
pub(crate) struct CapabilitiesEndpoints {
    query: &'static str,
    stream: &'static str,
}

#[derive(Serialize)]
pub(crate) struct CapabilitiesResponse {
    topology: &'static str,
    node_role: &'static str,
    features: CapabilitiesFeatures,
    auth_methods: Vec<&'static str>,
    endpoints: CapabilitiesEndpoints,
    #[serde(skip_serializing_if = "Option::is_none")]
    self_site_id: Option<String>,
}

pub(crate) async fn get_capabilities(State(state): State<AppState>) -> Json<CapabilitiesResponse> {
    let oidc_enabled = state.operator_auth.oidc.is_some();
    let session_enabled = state.operator_auth.has_local_admin() || oidc_enabled;

    let mut auth_methods = Vec::new();
    if session_enabled {
        auth_methods.push("session");
    }
    if oidc_enabled {
        auth_methods.push("oidc");
    }
    auth_methods.push("bearer");
    if state.operator_auth.cf_access_trusted {
        auth_methods.push("cf_access");
    }

    Json(CapabilitiesResponse {
        topology: "single",
        node_role: state.role.as_str(),
        features: CapabilitiesFeatures {
            ui: state.ui.mode == UiMode::On,
            arrow_export: state.backup_exporter.is_some(),
            oidc: oidc_enabled,
            streams: false,
        },
        auth_methods,
        endpoints: CapabilitiesEndpoints {
            query: "/v1",
            stream: "/v1",
        },
        self_site_id: state.self_site_id.map(|id| id.to_string()),
    })
}
