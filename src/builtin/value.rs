//! In-memory form of Uffda byte values and millisecond UTC instants.
//!
//! Codec records use `@bytes` and `@date`. Between those boundaries a byte
//! value is `{"$kith":"bytes","base64url":...}` and an instant is
//! `{"$kith":"instant","millis":...}`. Pattern rules and host functions
//! recognize only that exact shape.

use serde_json::{json, Value as Json};

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn bytes_json(bytes: &[u8]) -> Json {
    json!({
        "$kith": "bytes",
        "base64url": encode_base64url(bytes),
    })
}

pub fn instant_json(millis: i64) -> Json {
    json!({
        "$kith": "instant",
        "millis": millis,
    })
}

pub fn bytes_of(value: &Json) -> Option<Vec<u8>> {
    let object = value.as_object()?;
    if object.len() != 2 {
        return None;
    }
    if object.get("$kith")?.as_str()? != "bytes" {
        return None;
    }
    decode_base64url(object.get("base64url")?.as_str()?)
}

pub fn instant_millis(value: &Json) -> Option<i64> {
    let object = value.as_object()?;
    if object.len() != 2 {
        return None;
    }
    if object.get("$kith")?.as_str()? != "instant" {
        return None;
    }
    whole_number(object.get("millis")?)
}

pub fn is_bytes(value: &Json) -> bool {
    bytes_of(value).is_some()
}

pub fn is_instant(value: &Json) -> bool {
    instant_millis(value).is_some()
}

pub fn whole_number(value: &Json) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    let number = value.as_f64()?;
    if number.is_finite()
        && number.fract() == 0.0
        && number >= i64::MIN as f64
        && number <= i64::MAX as f64
    {
        Some(number as i64)
    } else {
        None
    }
}

pub fn encode_base64url(data: &[u8]) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= data.len() {
        let chunk =
            ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8) | data[index + 2] as u32;
        out.push(ALPHABET[(chunk >> 18) as usize] as char);
        out.push(ALPHABET[((chunk >> 12) & 63) as usize] as char);
        out.push(ALPHABET[((chunk >> 6) & 63) as usize] as char);
        out.push(ALPHABET[(chunk & 63) as usize] as char);
        index += 3;
    }
    if index < data.len() {
        let rest = data.len() - index;
        let mut chunk = (data[index] as u32) << 16;
        if rest == 2 {
            chunk |= (data[index + 1] as u32) << 8;
        }
        out.push(ALPHABET[(chunk >> 18) as usize] as char);
        out.push(ALPHABET[((chunk >> 12) & 63) as usize] as char);
        if rest == 2 {
            out.push(ALPHABET[((chunk >> 6) & 63) as usize] as char);
        }
    }
    out
}

pub fn decode_base64url(text: &str) -> Option<Vec<u8>> {
    if text.len() % 4 == 1 || text.bytes().any(|byte| byte == b'=') {
        return None;
    }
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    if bits > 0 && buffer != 0 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_round_trip_is_canonical_and_unpadded() {
        assert_eq!(encode_base64url(&[]), "");
        assert_eq!(encode_base64url(&[1, 2, 3]), "AQID");
        assert_eq!(decode_base64url("AQID"), Some(vec![1, 2, 3]));
        assert_eq!(decode_base64url(""), Some(vec![]));
        assert!(decode_base64url("AQID=").is_none());
        assert!(decode_base64url("A").is_none());
        let mut all = (0u8..=255).collect::<Vec<_>>();
        all.extend_from_slice(&[0, 128, 255]);
        assert_eq!(decode_base64url(&encode_base64url(&all)), Some(all));
    }
}
