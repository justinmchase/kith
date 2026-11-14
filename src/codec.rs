use crate::doc;
use crate::error::Error;
use crate::json_util::to_uffda_json;
use serde_json::Value as Json;
use uffda_runtime::codec::data::DataDocument;

/// External edge: the compiler syntax tree and other JSON observations are
/// encoded as data-codec text. Runtime byte and date values use [`doc::encode`].
pub fn encode(value: &Json) -> Result<String, Error> {
    let document = DataDocument::from_json(to_uffda_json(value)?)
        .map_err(|error| Error::new(error.to_string()))?;
    doc::encode(&document)
}

/// External edge: `kith parse` and tests read data-codec text back as JSON.
/// Byte and date nodes project to their codec text.
pub fn decode(text: &str) -> Result<Json, Error> {
    doc::to_json(&doc::decode(text)?)
}

pub fn json_u64(value: &Json) -> Option<u64> {
    if let Some(value) = value.as_u64() {
        return Some(value);
    }
    let value = value.as_f64()?;
    if value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value <= u64::MAX as f64 {
        let int = value as u64;
        (int as f64 == value).then_some(int)
    } else {
        None
    }
}
