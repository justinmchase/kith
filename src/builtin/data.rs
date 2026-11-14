use crate::doc::{self, Doc};
use crate::error::Error;

pub fn call(name: &str, args: &[Doc]) -> Result<Option<Doc>, Error> {
    let value = match name {
        "bytes" => {
            expect_arity(name, args, 1)?;
            doc::bytes(octets(&args[0])?)
        }
        "byte_at" => {
            expect_arity(name, args, 2)?;
            let bytes = expect_bytes("byte_at", &args[0])?;
            let index = expect_index("byte_at", &args[1])?;
            let byte = bytes
                .get(index)
                .ok_or_else(|| Error::new("byte_at index out of range"))?;
            doc::number(f64::from(*byte))
        }
        "byte_length" => {
            expect_arity(name, args, 1)?;
            doc::number(expect_bytes("byte_length", &args[0])?.len() as f64)
        }
        "bytes_concat" => {
            if args.len() < 2 {
                return Err(Error::new("bytes_concat expected at least 2 arguments"));
            }
            let mut out = Vec::new();
            for arg in args {
                out.extend(expect_bytes("bytes_concat", arg)?);
            }
            doc::bytes(out)
        }
        "bytes_slice" => {
            expect_arity(name, args, 3)?;
            let bytes = expect_bytes("bytes_slice", &args[0])?;
            let start = expect_index("bytes_slice", &args[1])?;
            let end = expect_index("bytes_slice", &args[2])?;
            if start > end || end > bytes.len() {
                return Err(Error::new("bytes_slice range out of bounds"));
            }
            doc::bytes(bytes[start..end].to_vec())
        }
        "blob" => {
            expect_arity(name, args, 2)?;
            let media = doc::as_str(&args[0])
                .ok_or_else(|| Error::new("blob expected a media type string"))?;
            let bytes = expect_bytes("blob", &args[1])?;
            doc::object([("type", doc::string(media)), ("bytes", doc::bytes(bytes))])
        }
        "blob_bytes" => {
            expect_arity(name, args, 1)?;
            doc::bytes(blob_bytes(&args[0])?)
        }
        "blob_type" => {
            expect_arity(name, args, 1)?;
            doc::string(blob_type(&args[0])?)
        }
        "buffer" => {
            expect_arity(name, args, 1)?;
            let bytes = expect_bytes("buffer", &args[0])?;
            buffer_doc(&bytes, 0, bytes.len())
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
            buffer_doc(&bytes, window_start + start, window_start + end)
        }
        "buffer_bytes" => {
            expect_arity(name, args, 1)?;
            let (bytes, start, end) = expect_buffer(&args[0])?;
            doc::bytes(bytes[start..end].to_vec())
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

pub fn buffer_in_window(value: &Doc) -> bool {
    buffer_parts(value).is_some()
}

fn octets(value: &Doc) -> Result<Vec<u8>, Error> {
    let items = doc::items(value).ok_or_else(|| Error::new("bytes expected an array of octets"))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let number = doc::as_number(&item)
            .filter(|number| number.is_finite() && number.fract() == 0.0)
            .ok_or_else(|| Error::new("bytes expected an array of octets"))?;
        if !(0.0..=255.0).contains(&number) {
            return Err(Error::new("bytes octet out of range"));
        }
        out.push(number as u8);
    }
    Ok(out)
}

fn expect_bytes(name: &str, value: &Doc) -> Result<Vec<u8>, Error> {
    doc::as_bytes(value).ok_or_else(|| Error::new(format!("{name} expected bytes")))
}

fn expect_index(name: &str, value: &Doc) -> Result<usize, Error> {
    let number =
        doc::as_i64(value).ok_or_else(|| Error::new(format!("{name} expected an index")))?;
    usize::try_from(number).map_err(|_| Error::new(format!("{name} expected an index")))
}

fn expect_arity(name: &str, args: &[Doc], arity: usize) -> Result<(), Error> {
    if args.len() == arity {
        Ok(())
    } else {
        Err(Error::new(format!("{name} expected {arity} arguments")))
    }
}

fn blob_parts(value: &Doc) -> Option<(String, Vec<u8>)> {
    let media = doc::as_str(&doc::get(value, "type")?)?;
    let bytes = doc::as_bytes(&doc::get(value, "bytes")?)?;
    Some((media, bytes))
}

fn blob_bytes(value: &Doc) -> Result<Vec<u8>, Error> {
    blob_parts(value)
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| Error::new("blob_bytes expected a blob"))
}

fn blob_type(value: &Doc) -> Result<String, Error> {
    blob_parts(value)
        .map(|(media, _)| media)
        .ok_or_else(|| Error::new("blob_type expected a blob"))
}

fn buffer_doc(bytes: &[u8], start: usize, end: usize) -> Doc {
    doc::object([
        ("bytes", doc::bytes(bytes.to_vec())),
        ("start", doc::number(start as f64)),
        ("end", doc::number(end as f64)),
    ])
}

fn buffer_parts(value: &Doc) -> Option<(Vec<u8>, usize, usize)> {
    let bytes = doc::as_bytes(&doc::get(value, "bytes")?)?;
    let start = usize::try_from(doc::as_i64(&doc::get(value, "start")?)?).ok()?;
    let end = usize::try_from(doc::as_i64(&doc::get(value, "end")?)?).ok()?;
    if start > end || end > bytes.len() {
        return None;
    }
    Some((bytes, start, end))
}

fn expect_buffer(value: &Doc) -> Result<(Vec<u8>, usize, usize), Error> {
    buffer_parts(value).ok_or_else(|| Error::new("expected a buffer"))
}
