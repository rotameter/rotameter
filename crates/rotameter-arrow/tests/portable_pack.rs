#![forbid(unsafe_code)]
//! Parquet Zstd streaming, manifest SHA-256 verification, and `.tar.zst` archive tests.

use std::collections::BTreeMap;
use std::fs;

use rotameter_arrow::{
    ARROW_SCHEMA_VERSION, ArrowError, PORTABLE_FORMAT_VERSION, PortableManifest, PortableRowCounts,
    events_schema, events_to_record_batch, pack_tar_zst, record_batch_to_events, sha256_file,
    stream_parquet_batches, unpack_tar_zst, verify_manifest, write_manifest, write_parquet_batches,
};
use rotameter_core::id::SiteId;
use rotameter_core::storage::IngestEvent;
use rotameter_core::types::{EventKind, EventProps};

#[test]
fn parquet_manifest_and_tar_zst_roundtrip_and_tamper_detection() {
    let ts = 1_700_000_000_000_i64;
    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [9; 10]);
    let events = vec![IngestEvent {
        site_id,
        ts_ms: ts,
        session_id: [1; 16],
        seq_in_session: 1,
        visitor_id: [2; 16],
        event_kind: EventKind::Pageview,
        path: "/".to_string(),
        props: EventProps::default(),
    }];
    let ev_batch = events_to_record_batch(&events).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let export_dir = dir.path().join("export");
    let ev_pq = export_dir.join("events/day=2023-11-14.parquet");
    assert_eq!(
        write_parquet_batches(&ev_pq, events_schema(), [Ok(ev_batch)]).unwrap(),
        1
    );

    let mut streamed = Vec::new();
    stream_parquet_batches(&ev_pq, 1, |batch| {
        streamed.extend(record_batch_to_events(&batch)?);
        Ok(())
    })
    .unwrap();
    assert_eq!(streamed, events);

    let files = BTreeMap::from([(
        "events/day=2023-11-14.parquet".to_string(),
        sha256_file(&ev_pq).unwrap(),
    )]);
    let manifest = PortableManifest {
        format_version: PORTABLE_FORMAT_VERSION,
        rotameter_version: "0.1.1".to_string(),
        schema_version: 4,
        arrow_schema_version: ARROW_SCHEMA_VERSION,
        created_at_ms: ts,
        row_counts: PortableRowCounts {
            events: 1,
            ..PortableRowCounts::default()
        },
        files,
    };
    write_manifest(&export_dir, &manifest).unwrap();
    assert_eq!(verify_manifest(&export_dir).unwrap(), manifest);

    let archive = dir.path().join("backup.tar.zst");
    pack_tar_zst(&export_dir, &archive).unwrap();
    let unpacked = dir.path().join("unpacked");
    unpack_tar_zst(&archive, &unpacked).unwrap();
    assert_eq!(verify_manifest(&unpacked).unwrap(), manifest);

    fs::write(unpacked.join("events/day=2023-11-14.parquet"), b"tampered").unwrap();
    assert!(matches!(
        verify_manifest(&unpacked),
        Err(ArrowError::InvalidExchange(_))
    ));
}
