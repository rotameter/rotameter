#![forbid(unsafe_code)]
//! Typed column lookup and fixed-binary decoding helpers shared across Arrow codecs.

use std::collections::BTreeMap;

use arrow_array::{
    Array, FixedSizeBinaryArray, Float64Array, Int32Array, Int64Array, RecordBatch, StringArray,
    UInt8Array, UInt32Array, UInt64Array,
};
use rotameter_core::id::SiteId;
use rotameter_core::types::EventProps;

use crate::ArrowError;

fn downcast_col<'a, T: 'static>(batch: &'a RecordBatch, name: &str) -> Result<&'a T, ArrowError> {
    let col = batch
        .column_by_name(name)
        .ok_or_else(|| ArrowError::InvalidExchange(format!("missing Arrow column `{name}`")))?;
    col.as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| ArrowError::InvalidExchange(format!("unexpected type for column `{name}`")))
}

pub(crate) fn col_bin16<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a FixedSizeBinaryArray, ArrowError> {
    let arr = downcast_col::<FixedSizeBinaryArray>(batch, name)?;
    if arr.value_length() != 16 {
        return Err(ArrowError::InvalidExchange(format!(
            "column `{name}` must have byte width 16, got {}",
            arr.value_length()
        )));
    }
    Ok(arr)
}

pub(crate) fn col_i64<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a Int64Array, ArrowError> {
    downcast_col::<Int64Array>(batch, name)
}

pub(crate) fn col_i32<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a Int32Array, ArrowError> {
    downcast_col::<Int32Array>(batch, name)
}

pub(crate) fn col_u32<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a UInt32Array, ArrowError> {
    downcast_col::<UInt32Array>(batch, name)
}

pub(crate) fn col_u64<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a UInt64Array, ArrowError> {
    downcast_col::<UInt64Array>(batch, name)
}

pub(crate) fn col_u8<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a UInt8Array, ArrowError> {
    downcast_col::<UInt8Array>(batch, name)
}

pub(crate) fn col_f64<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a Float64Array, ArrowError> {
    downcast_col::<Float64Array>(batch, name)
}

pub(crate) fn col_str<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a StringArray, ArrowError> {
    downcast_col::<StringArray>(batch, name)
}

pub(crate) fn bytes16(
    arr: &FixedSizeBinaryArray,
    row: usize,
    name: &str,
) -> Result<[u8; 16], ArrowError> {
    if arr.is_null(row) {
        return Err(ArrowError::InvalidExchange(format!(
            "null value in required column `{name}` at row {row}"
        )));
    }
    arr.value(row)
        .try_into()
        .map_err(|_| ArrowError::InvalidExchange(format!("invalid 16-byte ID in `{name}`")))
}

pub(crate) fn site_id_at(arr: &FixedSizeBinaryArray, row: usize) -> Result<SiteId, ArrowError> {
    Ok(SiteId::from_bytes(bytes16(arr, row, "site_id")?))
}

pub(crate) fn opt_str(arr: &StringArray, row: usize) -> Option<String> {
    (!arr.is_null(row)).then(|| arr.value(row).to_string())
}

pub(crate) fn decode_props(arr: &StringArray, row: usize) -> Result<EventProps, ArrowError> {
    if arr.is_null(row) {
        return Ok(EventProps::default());
    }
    let raw = arr.value(row);
    if raw.is_empty() {
        return Ok(EventProps::default());
    }
    let map: BTreeMap<String, String> = serde_json::from_str(raw)?;
    Ok(EventProps::from_pairs(map)?)
}
