use crate::builtin::{
    bytes_json, bytes_of, encode_base64url, instant_json, instant_millis, instant_text,
};
use crate::error::Error;
use crate::json_util::to_uffda_json;
use serde_json::Value as Json;
use time::OffsetDateTime;
use uffda_runtime::codec::data::{DataDocument, DataLimits, DataNode};

pub fn encode(value: &Json) -> Result<String, Error> {
    let document = if contains_kith_value(value) {
        document_from_json(value)?
    } else {
        DataDocument::from_json(to_uffda_json(value)?)
            .map_err(|error| Error::new(error.to_string()))?
    };
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
        DataNode::Bytes(value) => Ok(bytes_json(value)),
        DataNode::Date(value) => Ok(instant_json(millis_of(*value)?)),
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

fn contains_kith_value(value: &Json) -> bool {
    bytes_of(value).is_some()
        || instant_millis(value).is_some()
        || match value {
            Json::Array(items) => items.iter().any(contains_kith_value),
            Json::Object(map) => map.values().any(contains_kith_value),
            _ => false,
        }
}

fn document_from_json(value: &Json) -> Result<DataDocument, Error> {
    let (nodes, root) = build_nodes(value)?;
    DataDocument::from_nodes(nodes, root, DataLimits::default())
        .map_err(|error| Error::new(error.to_string()))
}

fn build_nodes(value: &Json) -> Result<(Vec<DataNode>, usize), Error> {
    if let Some(bytes) = bytes_of(value) {
        let text = value
            .get("base64url")
            .and_then(Json::as_str)
            .ok_or_else(|| Error::new("bytes missing base64url"))?;
        if encode_base64url(&bytes) != text {
            return Err(Error::new("bytes base64url is not canonical"));
        }
        return take_nodes(&format!("@bytes\"{text}\""));
    }
    if let Some(millis) = instant_millis(value) {
        let text = instant_text(millis)?;
        return take_nodes(&format!("@date\"{text}\""));
    }
    match value {
        Json::Array(items) => {
            let mut nodes = Vec::new();
            let mut children = Vec::new();
            for item in items {
                let (sub, root) = build_nodes(item)?;
                children.push(append_nodes(&mut nodes, sub, root));
            }
            let root = nodes.len();
            nodes.push(DataNode::Array(children));
            Ok((nodes, root))
        }
        Json::Object(map) => {
            let mut nodes = Vec::new();
            let mut entries = Vec::new();
            for (key, item) in map {
                let (key_nodes, key_root) = take_plain(&Json::String(key.clone()))?;
                let key_index = append_nodes(&mut nodes, key_nodes, key_root);
                let (sub, root) = build_nodes(item)?;
                let value_index = append_nodes(&mut nodes, sub, root);
                entries.push((key_index, value_index));
            }
            let root = nodes.len();
            nodes.push(DataNode::Object(entries));
            Ok((nodes, root))
        }
        other => take_plain(other),
    }
}

fn take_plain(value: &Json) -> Result<(Vec<DataNode>, usize), Error> {
    let document = DataDocument::from_json(to_uffda_json(value)?)
        .map_err(|error| Error::new(error.to_string()))?;
    Ok((document.nodes().to_vec(), document.root()))
}

fn take_nodes(source: &str) -> Result<(Vec<DataNode>, usize), Error> {
    let document =
        DataDocument::from_codec(source).map_err(|error| Error::new(error.to_string()))?;
    Ok((document.nodes().to_vec(), document.root()))
}

fn append_nodes(nodes: &mut Vec<DataNode>, sub: Vec<DataNode>, root: usize) -> usize {
    let shift = nodes.len();
    nodes.extend(sub.into_iter().map(|node| shift_node(node, shift)));
    root + shift
}

fn shift_node(node: DataNode, shift: usize) -> DataNode {
    match node {
        DataNode::Array(items) => {
            DataNode::Array(items.into_iter().map(|index| index + shift).collect())
        }
        DataNode::Object(entries) => DataNode::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key + shift, value + shift))
                .collect(),
        ),
        other => other,
    }
}

fn millis_of(value: OffsetDateTime) -> Result<i64, Error> {
    let nanos = value.unix_timestamp_nanos();
    if nanos % 1_000_000 != 0 {
        return Err(Error::new("instant is not millisecond UTC"));
    }
    i64::try_from(nanos / 1_000_000).map_err(|_| Error::new("instant overflow"))
}
