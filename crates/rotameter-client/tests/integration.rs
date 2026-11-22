#![forbid(unsafe_code)]
//! Integration tests against live `rotameter-server`.

mod common;

use std::time::Duration;

use rotameter_client::config::ClientConfig;
use rotameter_client::types::{
    CreateKeyRequest, EventPayload, Granularity, KeyScope, Metric, QueryRequest, RangePreset,
    SiteRef, UpsertSiteRequest, WireTimeRange,
};
use rotameter_client::{BlockingRotameterClient, ClientError, RotameterClient};

use common::spawn_live_server;

#[tokio::test]
async fn live_server_admin_and_blocking_roundtrip() {
    let admin_tok = "rm_adm_client_e2e_bootstrap_token_987";
    let (base_url, state, srv_handle) = spawn_live_server(admin_tok).await.unwrap();

    let admin_cfg = ClientConfig::new(&base_url).unwrap().with_token(admin_tok);
    let admin = RotameterClient::new(admin_cfg.clone()).unwrap();
    assert_eq!(admin.health().await.unwrap().status, "ok");

    let created = admin
        .create_site(&UpsertSiteRequest {
            slug: Some("colinwolff.dev".to_string()),
            display_name: Some("Initial".to_string()),
            ..UpsertSiteRequest::default()
        })
        .await
        .unwrap();
    let site_ref = SiteRef::Slug("colinwolff.dev".to_string());
    let updated = admin
        .upsert_site(
            &site_ref,
            &UpsertSiteRequest {
                display_name: Some("Colin Wolff".to_string()),
                ..UpsertSiteRequest::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.display_name, "Colin Wolff");
    assert_eq!(admin.list_sites().await.unwrap().len(), 1);
    assert_eq!(
        admin.get_site(&site_ref).await.unwrap().site_id,
        created.site_id
    );

    let (b_health, b_sites) = tokio::task::spawn_blocking(move || {
        let blocking = BlockingRotameterClient::new(admin_cfg).unwrap();
        (blocking.health().unwrap(), blocking.list_sites().unwrap())
    })
    .await
    .unwrap();
    assert_eq!((b_health.status.as_str(), b_sites.len()), ("ok", 1));

    let err = admin.delete_site(&site_ref, false).await.unwrap_err();
    assert!(matches!(
        err,
        ClientError::Api {
            status: 400,
            ref code,
            ..
        } if code == "confirm_required"
    ));
    assert!(admin.delete_site(&site_ref, true).await.unwrap().deleted);

    state.ingest.shutdown(Duration::from_secs(2)).await;
    srv_handle.abort();
}

#[tokio::test]
async fn live_server_ingest_query_csv_realtime_and_key_revocation() {
    let admin_tok = "rm_adm_client_e2e_bootstrap_token_654";
    let (base_url, state, srv_handle) = spawn_live_server(admin_tok).await.unwrap();
    let admin =
        RotameterClient::new(ClientConfig::new(&base_url).unwrap().with_token(admin_tok)).unwrap();

    let site_ref = SiteRef::Slug("colinwolff.dev".to_string());
    admin
        .create_site(&UpsertSiteRequest {
            slug: Some("colinwolff.dev".to_string()),
            ..UpsertSiteRequest::default()
        })
        .await
        .unwrap();
    let rd_key = admin
        .create_key(&CreateKeyRequest {
            scope: KeyScope::Read,
            site: Some(site_ref.clone()),
            label: "reader".to_string(),
        })
        .await
        .unwrap();
    let ing_key = admin
        .create_key(&CreateKeyRequest {
            scope: KeyScope::Ingest,
            site: Some(site_ref.clone()),
            label: "ingestor".to_string(),
        })
        .await
        .unwrap();
    assert!(admin.list_keys().await.unwrap().len() >= 3);

    let ing = RotameterClient::new(
        ClientConfig::new(&base_url)
            .unwrap()
            .with_token(ing_key.token),
    )
    .unwrap();
    let accepted = ing
        .post_event(&EventPayload {
            site_id: site_ref.clone(),
            event: None,
            url: "https://colinwolff.dev/projects/rotameter".to_string(),
            referrer: Some("https://news.ycombinator.com/".to_string()),
            viewport_width: Some(1440),
            props: None,
        })
        .await
        .unwrap();
    assert_eq!(accepted.accepted, 1);
    state.ingest.flush_now().await;

    let reader = RotameterClient::new(
        ClientConfig::new(&base_url)
            .unwrap()
            .with_token(rd_key.token),
    )
    .unwrap();
    let mut req = QueryRequest::new(
        site_ref.clone(),
        WireTimeRange::Preset {
            preset: RangePreset::Hours24,
        },
        vec![Metric::Visitors, Metric::Pageviews],
    );
    req.granularity = Some(Granularity::Hour);
    let q = reader.query(&req).await.unwrap();
    assert_eq!((q.summary.visitors, q.summary.pageviews), (1, 1));
    assert!(reader.query_csv(&req).await.unwrap().contains("visitors"));
    assert_eq!(reader.realtime(&site_ref).await.unwrap().active_visitors, 1);

    assert!(admin.revoke_key(rd_key.key_id).await.unwrap().revoked);
    assert!(
        reader
            .realtime(&site_ref)
            .await
            .unwrap_err()
            .is_auth_error()
    );

    state.ingest.shutdown(Duration::from_secs(2)).await;
    srv_handle.abort();
}
