#![forbid(unsafe_code)]
//! Integration tests for `rotameter-core` UIDs, slugs, domain models, and query validation.

use std::str::FromStr;

use rotameter_core::error::{CoreError, ErrorCode};
use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef, validate_slug};
use rotameter_core::query::{
    Dimension, FilterOp, FilterValue, Granularity, Metric, QueryFilter, QueryFormat, QuerySpec,
    RangePreset, TimeRange,
};
use rotameter_core::types::{
    DeviceType, EventKind, EventProps, KeyScope, MAX_PROP_VALUE_BYTES, SaltRotation, ViewportClass,
};

#[test]
fn prefixed_uuidv7_round_trips_and_sorts_by_time() {
    let earlier = SiteId::from_uuidv7_parts(1_763_670_000_000, [1; 10]);
    let later = SiteId::from_uuidv7_parts(1_763_670_005_000, [1; 10]);
    assert!(earlier < later);
    assert_eq!(earlier.timestamp_ms(), 1_763_670_000_000);

    let encoded = earlier.to_string();
    assert!(encoded.starts_with("site_"));
    assert_eq!(encoded.len(), 31);
    assert_eq!(SiteId::from_str(&encoded).unwrap(), earlier);

    let org = OrgId::from_uuidv7_parts(42, [0xaa; 10]);
    let key = KeyId::from_bytes([0xff; 16]);
    assert_eq!(OrgId::from_str(&org.to_string()).unwrap(), org);
    assert_eq!(KeyId::from_str(&key.to_string()).unwrap(), key);
    assert_eq!(
        serde_json::from_str::<SiteId>(&serde_json::to_string(&earlier).unwrap()).unwrap(),
        earlier
    );
}

#[test]
fn invalid_ids_and_slugs_are_rejected() {
    assert_eq!(
        OrgId::from_str("site_00000000000000000000000000")
            .unwrap_err()
            .code(),
        ErrorCode::InvalidId
    );
    assert_eq!(
        SiteId::from_str("site_8zzzzzzzzzzzzzzzzzzzzzzzzz")
            .unwrap_err()
            .code(),
        ErrorCode::InvalidId
    );
    assert_eq!(validate_slug(" _SELF ").unwrap(), "_self");
    assert_eq!(validate_slug("ColinWolff.Dev").unwrap(), "colinwolff.dev");
    assert_eq!(
        validate_slug("-bad").unwrap_err().code(),
        ErrorCode::InvalidSlug
    );
    assert_eq!(
        validate_slug("bad slug").unwrap_err().code(),
        ErrorCode::InvalidSlug
    );
}

#[test]
fn site_ref_parses_uid_and_slug() {
    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [9; 10]);
    assert_eq!(
        SiteRef::parse(&site_id.to_string()).unwrap(),
        SiteRef::Id(site_id)
    );
    assert_eq!(
        SiteRef::parse("colinwolff.dev").unwrap(),
        SiteRef::Slug("colinwolff.dev".to_string())
    );
}

#[test]
fn event_kind_props_and_viewport_enforce_caps() {
    assert_eq!(EventKind::parse("pageview").unwrap(), EventKind::Pageview);
    assert_eq!(
        EventKind::parse("signup.completed:v1").unwrap(),
        EventKind::Custom("signup.completed:v1".to_string())
    );
    assert_eq!(
        EventKind::parse("bad event!").unwrap_err().code(),
        ErrorCode::InvalidEventName
    );

    let long_val = "é".repeat(100);
    let props = EventProps::from_pairs([("plan", long_val.as_str())]).unwrap();
    assert!(props.as_map()["plan"].len() <= MAX_PROP_VALUE_BYTES);

    let too_many: Vec<(String, String)> = (0..11)
        .map(|i| (format!("k{i}"), "v".to_string()))
        .collect();
    assert_eq!(
        EventProps::from_pairs(too_many).unwrap_err().code(),
        ErrorCode::InvalidProperties
    );

    assert_eq!(ViewportClass::from_width(None), ViewportClass::Unknown);
    assert_eq!(ViewportClass::from_width(Some(375)), ViewportClass::Xs);
    assert_eq!(ViewportClass::from_width(Some(1440)), ViewportClass::Xl);
    assert_eq!(DeviceType::Mobile.as_u8(), 2);
    assert_eq!(KeyScope::Read.token_prefix(), "rm_rd_");
    assert_eq!(SaltRotation::Weekly.as_str(), "weekly");
}

#[test]
fn query_spec_validates_semantic_invariants() {
    let valid = QuerySpec {
        site_id: SiteRef::Slug("colinwolff.dev".to_string()),
        range: TimeRange::Preset {
            preset: RangePreset::Days30,
        },
        granularity: Some(Granularity::Day),
        metrics: vec![Metric::Visitors, Metric::Pageviews],
        group_by: vec![Dimension::Path],
        filters: vec![QueryFilter {
            dimension: Dimension::Country,
            op: FilterOp::In,
            value: FilterValue::List(vec!["US".to_string(), "CA".to_string()]),
        }],
        compare_previous: true,
        sort: None,
        limit: 20,
        offset: 0,
        format: QueryFormat::default(),
    };
    assert!(valid.validate().is_ok());

    let mut bad = valid.clone();
    bad.range = TimeRange::Explicit {
        from_ms: 100,
        to_ms: 100,
    };
    let err: CoreError = bad.validate().unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidQuery);
}
