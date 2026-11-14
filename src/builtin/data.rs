use super::value::{bytes_json, bytes_of, whole_number};
use crate::error::Error;
use serde_json::{json, Value as Json};

pub fn call(name: &str, args: &[Json]) -> Result<Option<Json>, Error> {
    let value = match name {
        "bytes" => {
            expect_arity(name, args, 1)?;
            bytes_json(&octets(&args[0])?)
        }
        "byte_at" => {
            expect_arity(name, args, 2)?;
            let bytes = expect_bytes("byte_at", &args[0])?;
            let index = expect_index("byte_at", &args[1])?;
            let byte = bytes
                .get(index)
                .ok_or_else(|| Error::new("byte_at index out of range"))?;
            json!(*byte as f64)
        }
        "byte_length" => {
            expect_arity(name, args, 1)?;
            json!(expect_bytes("byte_length", &args[0])?.len() as f64)
        }
        "bytes_concat" => {
            if args.len() < 2 {
                return Err(Error::new("bytes_concat expected at least 2 arguments"));
            }
            let mut out = Vec::new();
            for arg in args {
                out.extend(expect_bytes("bytes_concat", arg)?);
            }
            bytes_json(&out)
        }
        "bytes_slice" => {
            expect_arity(name, args, 3)?;
            let bytes = expect_bytes("bytes_slice", &args[0])?;
            let start = expect_index("bytes_slice", &args[1])?;
            let end = expect_index("bytes_slice", &args[2])?;
            if start > end || end > bytes.len() {
                return Err(Error::new("bytes_slice range out of bounds"));
            }
            bytes_json(&bytes[start..end])
        }
        "blob" => {
            expect_arity(name, args, 2)?;
            let media = args[0]
                .as_str()
                .ok_or_else(|| Error::new("blob expected a media type string"))?;
            let bytes = expect_bytes("blob", &args[1])?;
            json!({ "type": media, "bytes": bytes_json(&bytes) })
        }
        "blob_bytes" => {
            expect_arity(name, args, 1)?;
            bytes_json(&blob_bytes(&args[0])?)
        }
        "blob_type" => {
            expect_arity(name, args, 1)?;
            json!(blob_type(&args[0])?)
        }
        "buffer" => {
            expect_arity(name, args, 1)?;
            let bytes = expect_bytes("buffer", &args[0])?;
            buffer_json(&bytes, 0, bytes.len())
        }
        "buffer_slice" => {
            expect_arity(name, args, 3)?;
            let (bytes, window_start, window_end) = expect_buffer(&args[0])?;
            let start = expect_index("buffer_slice", &args[1])?;
            let end = expect_index("buffer_slice", &args[2])?;
            let window = window_end - window_start;
            if start > end || end > window {
                return Err(Error::new("buffer_slice range out of bounds"));
            }
            buffer_json(&bytes, window_start + start, window_start + end)
        }
        "buffer_bytes" => {
            expect_arity(name, args, 1)?;
            let (bytes, start, end) = expect_buffer(&args[0])?;
            bytes_json(&bytes[start..end])
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

pub fn is_blob(value: &Json) -> bool {
    blob_parts(value).is_some()
}

pub fn is_buffer(value: &Json) -> bool {
    buffer_parts(value).is_some()
}

fn octets(value: &Json) -> Result<Vec<u8>, Error> {
    let items = value
        .as_array()
        .ok_or_else(|| Error::new("bytes expected an array of octets"))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let number = item
            .as_f64()
            .filter(|number| number.is_finite() && number.fract() == 0.0)
            .ok_or_else(|| Error::new("bytes expected an array of octets"))?;
        if !(0.0..=255.0).contains(&number) {
            return Err(Error::new("bytes octet out of range"));
        }
        out.push(number as u8);
    }
    Ok(out)
}

fn expect_bytes(name: &str, value: &Json) -> Result<Vec<u8>, Error> {
    bytes_of(value).ok_or_else(|| Error::new(format!("{name} expected bytes")))
}

fn expect_index(name: &str, value: &Json) -> Result<usize, Error> {
    let number =
        whole_number(value).ok_or_else(|| Error::new(format!("{name} expected an index")))?;
    usize::try_from(number).map_err(|_| Error::new(format!("{name} expected an index")))
}

fn expect_arity(name: &str, args: &[Json], arity: usize) -> Result<(), Error> {
    if args.len() == arity {
        Ok(())
    } else {
        Err(Error::new(format!("{name} expected {arity} arguments")))
    }
}

fn blob_parts(value: &Json) -> Option<(String, Vec<u8>)> {
    let object = value.as_object()?;
    let media = object.get("type")?.as_str()?.to_string();
    let bytes = bytes_of(object.get("bytes")?)?;
    Some((media, bytes))
}

fn blob_bytes(value: &Json) -> Result<Vec<u8>, Error> {
    blob_parts(value)
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| Error::new("blob_bytes expected a blob"))
}

fn blob_type(value: &Json) -> Result<String, Error> {
    blob_parts(value)
        .map(|(media, _)| media)
        .ok_or_else(|| Error::new("blob_type expected a blob"))
}

fn buffer_json(bytes: &[u8], start: usize, end: usize) -> Json {
    json!({
        "bytes": bytes_json(bytes),
        "start": start as f64,
        "end": end as f64,
    })
}

fn buffer_parts(value: &Json) -> Option<(Vec<u8>, usize, usize)> {
    let object = value.as_object()?;
    let bytes = bytes_of(object.get("bytes")?)?;
    let start = whole_number(object.get("start")?)?;
    let end = whole_number(object.get("end")?)?;
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    if start > end || end > bytes.len() {
        return None;
    }
    Some((bytes, start, end))
}

fn expect_buffer(value: &Json) -> Result<(Vec<u8>, usize, usize), Error> {
    buffer_parts(value).ok_or_else(|| Error::new("expected a buffer"))
}
