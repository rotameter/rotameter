#![forbid(unsafe_code)]
//! Arrow `RecordBatch` <-> `SessionUpsert` codec (`sessions` schema v1).

use std::sync::Arc;

use arrow_array::builder::{
    FixedSizeBinaryBuilder, Int64Builder, StringBuilder, UInt8Builder, UInt32Builder,
};
use arrow_array::{ArrayRef, RecordBatch};
use rotameter_core::storage::SessionUpsert;
use rotameter_core::types::{Channel, DeviceType, UtmParams, ViewportClass};

use crate::ArrowError;
use crate::col_ext::{bytes16, col_bin16, col_i64, col_str, col_u8, col_u32, opt_str, site_id_at};
use crate::schema::sessions_schema;

/// Encodes `sessions` into a canonical [`RecordBatch`] matching [`sessions_schema`].
///
/// # Errors
///
/// Returns [`ArrowError`] if Arrow construction fails.
pub fn sessions_to_record_batch(sessions: &[SessionUpsert]) -> Result<RecordBatch, ArrowError> {
    let n = sessions.len();
    let mut site_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut session_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut visitor_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut started = Int64Builder::with_capacity(n);
    let mut last_ev = Int64Builder::with_capacity(n);
    let mut ev_cnt = UInt32Builder::with_capacity(n);
    let mut pv_cnt = UInt32Builder::with_capacity(n);
    let mut entry = StringBuilder::with_capacity(n, n * 24);
    let mut exit = StringBuilder::with_capacity(n, n * 24);
    let mut ref_host = StringBuilder::with_capacity(n, n * 16);
    let mut channel = StringBuilder::with_capacity(n, n * 16);
    let mut utm_src = StringBuilder::with_capacity(n, n * 8);
    let mut utm_med = StringBuilder::with_capacity(n, n * 8);
    let mut utm_camp = StringBuilder::with_capacity(n, n * 8);
    let mut utm_cont = StringBuilder::with_capacity(n, n * 8);
    let mut utm_term = StringBuilder::with_capacity(n, n * 8);
    let mut country = StringBuilder::with_capacity(n, n * 8);
    let mut region = StringBuilder::with_capacity(n, n * 8);
    let mut browser = StringBuilder::with_capacity(n, n * 16);
    let mut os = StringBuilder::with_capacity(n, n * 16);
    let mut device = UInt8Builder::with_capacity(n);
    let mut viewport = UInt8Builder::with_capacity(n);

    for s in sessions {
        site_ids.append_value(s.site_id.as_bytes())?;
        session_ids.append_value(s.session_id)?;
        visitor_ids.append_value(s.visitor_id)?;
        started.append_value(s.started_at_ms);
        last_ev.append_value(s.last_event_ms);
        ev_cnt.append_value(s.event_count);
        pv_cnt.append_value(s.pageview_count);
        entry.append_value(&s.entry_path);
        exit.append_value(&s.exit_path);
        ref_host.append_value(&s.ref_host);
        channel.append_value(s.channel.as_str());
        utm_src.append_option(s.utm.source.as_deref());
        utm_med.append_option(s.utm.medium.as_deref());
        utm_camp.append_option(s.utm.campaign.as_deref());
        utm_cont.append_option(s.utm.content.as_deref());
        utm_term.append_option(s.utm.term.as_deref());
        country.append_value(&s.country);
        region.append_value(&s.region);
        browser.append_value(&s.browser);
        os.append_value(&s.os);
        device.append_value(s.device_type.as_u8());
        viewport.append_value(s.viewport_class.as_u8());
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(site_ids.finish()),
        Arc::new(session_ids.finish()),
        Arc::new(visitor_ids.finish()),
        Arc::new(started.finish()),
        Arc::new(last_ev.finish()),
        Arc::new(ev_cnt.finish()),
        Arc::new(pv_cnt.finish()),
        Arc::new(entry.finish()),
        Arc::new(exit.finish()),
        Arc::new(ref_host.finish()),
        Arc::new(channel.finish()),
        Arc::new(utm_src.finish()),
        Arc::new(utm_med.finish()),
        Arc::new(utm_camp.finish()),
        Arc::new(utm_cont.finish()),
        Arc::new(utm_term.finish()),
        Arc::new(country.finish()),
        Arc::new(region.finish()),
        Arc::new(browser.finish()),
        Arc::new(os.finish()),
        Arc::new(device.finish()),
        Arc::new(viewport.finish()),
    ];
    Ok(RecordBatch::try_new(sessions_schema(), columns)?)
}

/// Decodes a [`RecordBatch`] produced with [`sessions_schema`] back into [`SessionUpsert`] rows.
///
/// # Errors
///
/// Returns [`ArrowError`] if required columns are missing or invalid.
pub fn record_batch_to_sessions(batch: &RecordBatch) -> Result<Vec<SessionUpsert>, ArrowError> {
    let site_ids = col_bin16(batch, "site_id")?;
    let session_ids = col_bin16(batch, "session_id")?;
    let visitor_ids = col_bin16(batch, "visitor_id")?;
    let started = col_i64(batch, "started_at_ms")?;
    let last_ev = col_i64(batch, "last_event_ms")?;
    let ev_cnt = col_u32(batch, "event_count")?;
    let pv_cnt = col_u32(batch, "pageview_count")?;
    let entry = col_str(batch, "entry_path")?;
    let exit = col_str(batch, "exit_path")?;
    let ref_host = col_str(batch, "ref_host")?;
    let channel = col_str(batch, "channel")?;
    let utm_src = col_str(batch, "utm_source")?;
    let utm_med = col_str(batch, "utm_medium")?;
    let utm_camp = col_str(batch, "utm_campaign")?;
    let utm_cont = col_str(batch, "utm_content")?;
    let utm_term = col_str(batch, "utm_term")?;
    let country = col_str(batch, "country")?;
    let region = col_str(batch, "region")?;
    let browser = col_str(batch, "browser")?;
    let os = col_str(batch, "os")?;
    let device = col_u8(batch, "device_type")?;
    let viewport = col_u8(batch, "viewport_class")?;

    let mut out = Vec::with_capacity(batch.num_rows());
    for row in 0..batch.num_rows() {
        let ch_raw = channel.value(row);
        let parsed_ch = Channel::parse(ch_raw).ok_or_else(|| {
            ArrowError::InvalidExchange(format!("unknown session channel `{ch_raw}`"))
        })?;
        out.push(SessionUpsert {
            site_id: site_id_at(site_ids, row)?,
            session_id: bytes16(session_ids, row, "session_id")?,
            visitor_id: bytes16(visitor_ids, row, "visitor_id")?,
            started_at_ms: started.value(row),
            last_event_ms: last_ev.value(row),
            event_count: ev_cnt.value(row),
            pageview_count: pv_cnt.value(row),
            entry_path: entry.value(row).to_string(),
            exit_path: exit.value(row).to_string(),
            ref_host: ref_host.value(row).to_string(),
            channel: parsed_ch,
            utm: UtmParams {
                source: opt_str(utm_src, row),
                medium: opt_str(utm_med, row),
                campaign: opt_str(utm_camp, row),
                content: opt_str(utm_cont, row),
                term: opt_str(utm_term, row),
            },
            country: country.value(row).to_string(),
            region: region.value(row).to_string(),
            browser: browser.value(row).to_string(),
            os: os.value(row).to_string(),
            device_type: DeviceType::from_u8(device.value(row)),
            viewport_class: ViewportClass::from_u8(viewport.value(row)),
        });
    }
    Ok(out)
}
