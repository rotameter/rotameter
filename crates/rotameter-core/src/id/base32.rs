#![forbid(unsafe_code)]
//! TypeID-compatible Crockford base32 encoding and decoding for 128-bit UUIDs.

const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
pub(crate) const ENCODED_LEN: usize = 26;

/// Encodes a 128-bit UUID byte array into a 26-character lowercase Crockford base32 string.
pub(crate) fn encode(bytes: [u8; 16]) -> String {
    let mut value = u128::from_be_bytes(bytes);
    let mut out = [b'0'; ENCODED_LEN];
    for slot in out.iter_mut().rev() {
        let digit = u8::try_from(value & 0x1f).unwrap_or(0);
        *slot = ALPHABET[usize::from(digit)];
        value >>= 5;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Decodes a 26-character lowercase Crockford base32 suffix into a 16-byte UUID array.
pub(crate) fn decode(suffix: &str) -> Option<[u8; 16]> {
    let raw = suffix.as_bytes();
    if raw.len() != ENCODED_LEN {
        return None;
    }
    let first = decode_digit(raw[0])?;
    if first > 7 {
        return None;
    }

    let mut acc = u128::from(first);
    for &byte in &raw[1..] {
        let digit = decode_digit(byte)?;
        acc = (acc << 5) | u128::from(digit);
    }
    Some(acc.to_be_bytes())
}

const fn decode_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0' => Some(0),
        b'1' => Some(1),
        b'2' => Some(2),
        b'3' => Some(3),
        b'4' => Some(4),
        b'5' => Some(5),
        b'6' => Some(6),
        b'7' => Some(7),
        b'8' => Some(8),
        b'9' => Some(9),
        b'a' => Some(10),
        b'b' => Some(11),
        b'c' => Some(12),
        b'd' => Some(13),
        b'e' => Some(14),
        b'f' => Some(15),
        b'g' => Some(16),
        b'h' => Some(17),
        b'j' => Some(18),
        b'k' => Some(19),
        b'm' => Some(20),
        b'n' => Some(21),
        b'p' => Some(22),
        b'q' => Some(23),
        b'r' => Some(24),
        b's' => Some(25),
        b't' => Some(26),
        b'v' => Some(27),
        b'w' => Some(28),
        b'x' => Some(29),
        b'y' => Some(30),
        b'z' => Some(31),
        _ => None,
    }
}
