//! Pure text and byte conversions. `to_codec` / `from_codec` stay globals.

use crate::doc::{self, Doc};
use crate::error::Error;

pub fn call(name: &str, args: &[Doc]) -> Result<Option<Doc>, Error> {
    let value = match name {
        "utf8_encode" => utf8_encode(args)?,
        "utf8_decode" => utf8_decode(args)?,
        "base64_encode" => base64_encode(args)?,
        "base64_decode" => base64_decode(args)?,
        "hex_encode" => hex_encode(args)?,
        "hex_decode" => hex_decode(args)?,
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn one<'a>(name: &str, args: &'a [Doc]) -> Result<&'a Doc, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!("{name} expected one argument")));
    }
    Ok(&args[0])
}

fn text_arg(name: &str, args: &[Doc]) -> Result<String, Error> {
    doc::as_str(one(name, args)?).ok_or_else(|| Error::new(format!("{name} expected text")))
}

fn bytes_arg(name: &str, args: &[Doc]) -> Result<Vec<u8>, Error> {
    doc::as_bytes(one(name, args)?).ok_or_else(|| Error::new(format!("{name} expected bytes")))
}

fn utf8_encode(args: &[Doc]) -> Result<Doc, Error> {
    Ok(doc::bytes(text_arg("utf8_encode", args)?.into_bytes()))
}

fn utf8_decode(args: &[Doc]) -> Result<Doc, Error> {
    let bytes = bytes_arg("utf8_decode", args)?;
    let text = String::from_utf8(bytes).map_err(|_| Error::new("utf8_decode invalid utf-8"))?;
    Ok(doc::string(text))
}

fn base64_encode(args: &[Doc]) -> Result<Doc, Error> {
    Ok(doc::string(b64_encode(&bytes_arg("base64_encode", args)?)))
}

fn base64_decode(args: &[Doc]) -> Result<Doc, Error> {
    let text = text_arg("base64_decode", args)?;
    Ok(doc::bytes(b64_decode(&text)?))
}

fn hex_encode(args: &[Doc]) -> Result<Doc, Error> {
    let bytes = bytes_arg("hex_encode", args)?;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(hex_digit(byte >> 4));
        out.push(hex_digit(byte & 0xf));
    }
    Ok(doc::string(out))
}

fn hex_decode(args: &[Doc]) -> Result<Doc, Error> {
    let text = text_arg("hex_decode", args)?;
    if text.len() % 2 != 0 {
        return Err(Error::new("hex_decode odd length"));
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut index = 0;
    while index < bytes.len() {
        let hi = hex_value(bytes[index])?;
        let lo = hex_value(bytes[index + 1])?;
        out.push((hi << 4) | lo);
        index += 2;
    }
    Ok(doc::bytes(out))
}

fn hex_digit(nibble: u8) -> char {
    char::from(b"0123456789abcdef"[nibble as usize])
}

fn hex_value(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(Error::new("hex_decode non-hex")),
    }
}

fn b64_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let n = ((bytes[index] as u32) << 16)
            | ((bytes[index + 1] as u32) << 8)
            | bytes[index + 2] as u32;
        out.push(ALPHA[((n >> 18) & 63) as usize] as char);
        out.push(ALPHA[((n >> 12) & 63) as usize] as char);
        out.push(ALPHA[((n >> 6) & 63) as usize] as char);
        out.push(ALPHA[(n & 63) as usize] as char);
        index += 3;
    }
    let rest = bytes.len() - index;
    if rest == 1 {
        let n = (bytes[index] as u32) << 16;
        out.push(ALPHA[((n >> 18) & 63) as usize] as char);
        out.push(ALPHA[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let n = ((bytes[index] as u32) << 16) | ((bytes[index + 1] as u32) << 8);
        out.push(ALPHA[((n >> 18) & 63) as usize] as char);
        out.push(ALPHA[((n >> 12) & 63) as usize] as char);
        out.push(ALPHA[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn b64_decode(text: &str) -> Result<Vec<u8>, Error> {
    fn val(byte: u8) -> Result<u8, Error> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(Error::new("base64_decode invalid")),
        }
    }
    let bytes = text.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(Error::new("base64_decode invalid"));
    }
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let pad = (bytes[index + 2] == b'=') as usize + (bytes[index + 3] == b'=') as usize;
        if bytes[index] == b'=' || bytes[index + 1] == b'=' {
            return Err(Error::new("base64_decode invalid"));
        }
        let a = val(bytes[index])?;
        let b = val(bytes[index + 1])?;
        let c = if bytes[index + 2] == b'=' {
            0
        } else {
            val(bytes[index + 2])?
        };
        let d = if bytes[index + 3] == b'=' {
            0
        } else {
            val(bytes[index + 3])?
        };
        let n = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6) | d as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
        index += 4;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // req:edges-005
    fn hi_is_padded_standard_base64() {
        let encoded = call("base64_encode", &[doc::bytes(b"hi".to_vec())])
            .unwrap()
            .unwrap();
        assert_eq!(doc::as_str(&encoded).as_deref(), Some("aGk="));
        let decoded = call("base64_decode", &[doc::string("aGk=")])
            .unwrap()
            .unwrap();
        assert_eq!(doc::as_bytes(&decoded).unwrap(), b"hi");
        assert!(call("base64_decode", &[doc::string("aGk")]).is_err());
        assert!(call("hex_decode", &[doc::string("abc")]).is_err());
        assert!(call("hex_decode", &[doc::string("zz")]).is_err());
        let hex = call("hex_encode", &[doc::bytes(vec![0x0a, 0xff])])
            .unwrap()
            .unwrap();
        assert_eq!(doc::as_str(&hex).as_deref(), Some("0aff"));
        assert!(call("utf8_decode", &[doc::bytes(vec![0xff])]).is_err());
    }
}
