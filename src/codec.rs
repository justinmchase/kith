use crate::error::Error;
use crate::json_util::to_uffda_json;
use serde_json::Value as Json;
use uffda_runtime::codec::data::{DataDocument, DataLimits, DataNode};

pub fn encode(value: &Json) -> Result<String, Error> {
    let document = DataDocument::from_json(to_uffda_json(value)?)
        .map_err(|error| Error::new(error.to_string()))?;
    let mut text = document
        .encode(false, DataLimits::default())
        .map_err(|error| Error::new(error.to_string()))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

pub fn decode(text: &str) -> Result<Json, Error> {
    let document = DataDocument::from_codec(text).map_err(|error| Error::new(error.to_string()))?;
    node_to_json(&document, document.root())
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

fn node_to_json(document: &DataDocument, index: usize) -> Result<Json, Error> {
    match document
        .nodes()
        .get(index)
        .ok_or_else(|| Error::new("invalid codec node"))?
    {
        DataNode::Undefined | DataNode::Null => Ok(Json::Null),
        DataNode::Boolean(value) => Ok(Json::Bool(*value)),
        DataNode::Number(value) => Ok(Json::Number(
            serde_json::Number::from_f64(*value).ok_or_else(|| Error::new("non-finite number"))?,
        )),
        DataNode::String(value) => Ok(Json::String(
            value
                .to_utf8()
                .map_err(|error| Error::new(error.to_string()))?,
        )),
        DataNode::Array(items) => Ok(Json::Array(
            items
                .iter()
                .map(|index| node_to_json(document, *index))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        DataNode::Object(entries) => {
            let mut map = serde_json::Map::new();
            for (key, value) in entries {
                let name = match document.nodes().get(*key) {
                    Some(DataNode::String(text)) => text
                        .to_utf8()
                        .map_err(|error| Error::new(error.to_string()))?,
                    _ => return Err(Error::new("unsupported codec object key")),
                };
                map.insert(name, node_to_json(document, *value)?);
            }
            Ok(Json::Object(map))
        }
        other => Err(Error::new(format!(
            "unsupported codec value for Kith: {other:?}"
        ))),
    }
}
