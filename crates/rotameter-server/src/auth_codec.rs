#![forbid(unsafe_code)]
//! Constant-time byte comparison, hex/base64url codecs, cookie parsing, and JWT payload decoding.

use axum::http::HeaderMap;
use axum::http::header::COOKIE;
use hmac_sha256::HMAC;
use serde_json::Value;

use crate::auth_config::OperatorAuthConfig;
use crate::config::ServerError;
use crate::error::ApiError;
use crate::state::fill_random;

#[derive(Clone)]
pub(crate) struct PasswordVerifier {
    pub(crate) salt: Vec<u8>,
    pub(crate) digest: [u8; 32],
}

pub(crate) fn build_verifier(
    auth: &OperatorAuthConfig,
) -> Result<Option<PasswordVerifier>, ServerError> {
    if let Some(raw_hash) = auth
        .admin_password_hash
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let rest = raw_hash
            .strip_prefix("sha256$")
            .ok_or_else(|| ServerError::new("admin_password_hash must start with `sha256$`"))?;
        let (salt_hex, digest_hex) = rest
            .split_once('$')
            .ok_or_else(|| ServerError::new("invalid `sha256$<salt>$<hex>` format"))?;
        let salt = hex_decode(salt_hex)
            .ok_or_else(|| ServerError::new("invalid hex salt in admin_password_hash"))?;
        let digest_vec = hex_decode(digest_hex)
            .ok_or_else(|| ServerError::new("invalid hex digest in admin_password_hash"))?;
        let digest: [u8; 32] = digest_vec
            .try_into()
            .map_err(|_| ServerError::new("admin_password_hash digest must be 32 bytes"))?;
        return Ok(Some(PasswordVerifier { salt, digest }));
    }
    if let Some(pw) = auth.admin_password.as_deref().filter(|s| !s.is_empty()) {
        let salt = fill_random::<16>()
            .map_err(|e| ServerError::new(format!("cannot salt admin password: {e}")))?
            .to_vec();
        let digest = HMAC::mac(pw.as_bytes(), &salt);
        return Ok(Some(PasswordVerifier { salt, digest }));
    }
    Ok(None)
}

pub(crate) fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    for raw in headers.get_all(COOKIE) {
        let Ok(s) = raw.to_str() else { continue };
        for part in s.split(';') {
            let trimmed = part.trim();
            if let Some((k, v)) = trimmed.split_once('=')
                && k.trim() == name
            {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

pub(crate) fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for pair in s.as_bytes().as_chunks::<2>().0 {
        let text = std::str::from_utf8(pair).ok()?;
        out.push(u8::from_str_radix(text, 16).ok()?);
    }
    Some(out)
}

pub(crate) fn b64url_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(char::from(TABLE[((triple >> 18) & 0x3f) as usize]));
        out.push(char::from(TABLE[((triple >> 12) & 0x3f) as usize]));
        if chunk.len() > 1 {
            out.push(char::from(TABLE[((triple >> 6) & 0x3f) as usize]));
        }
        if chunk.len() > 2 {
            out.push(char::from(TABLE[(triple & 0x3f) as usize]));
        }
    }
    out
}

pub(crate) fn b64url_decode(input: &str) -> Option<Vec<u8>> {
    let trimmed = input.trim_end_matches('=');
    let mut out = Vec::with_capacity((trimmed.len() * 3) / 4);
    let mut buf = 0_u32;
    let mut bits = 0_u8;
    for &b in trimmed.as_bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

pub(crate) fn decode_jwt_payload(jwt: &str) -> Result<Value, ApiError> {
    let payload_b64 = jwt
        .split('.')
        .nth(1)
        .ok_or_else(|| ApiError::unauthorized("malformed JWT"))?;
    let bytes = b64url_decode(payload_b64)
        .ok_or_else(|| ApiError::unauthorized("invalid JWT base64url payload"))?;
    serde_json::from_slice(&bytes).map_err(|_| ApiError::unauthorized("invalid JWT JSON payload"))
}
