//! Uffda data-codec documents. Built-in values, messages, pipe state, and
//! bindings are `DataDocument`s. JSON appears only in the functions marked as
//! an existing external edge.

use crate::error::Error;
use serde_json::Value as Json;
use time::OffsetDateTime;
use uffda_runtime::codec::data::{DataDocument, DataLimits, DataNode};
use uffda_runtime::text::RuntimeString;

pub type Doc = DataDocument;

pub fn null() -> Doc {
    leaf(DataNode::Null)
}

pub fn boolean(value: bool) -> Doc {
    leaf(DataNode::Boolean(value))
}

pub fn number(value: f64) -> Doc {
    leaf(DataNode::Number(value))
}

pub fn string(value: impl AsRef<str>) -> Doc {
    leaf(DataNode::String(
        RuntimeString::from_utf16(value.as_ref().encode_utf16().collect::<Vec<_>>())
            .codec_text()
            .clone(),
    ))
}

pub fn bytes(value: Vec<u8>) -> Doc {
    leaf(DataNode::Bytes(value))
}

pub fn date(value: OffsetDateTime) -> Doc {
    leaf(DataNode::Date(value))
}

pub fn array(items: impl IntoIterator<Item = Doc>) -> Doc {
    let mut nodes = Vec::new();
    let mut indexes = Vec::new();
    for item in items {
        indexes.push(append(&mut nodes, item));
    }
    let root = nodes.len();
    nodes.push(DataNode::Array(indexes));
    DataDocument::from_nodes(nodes, root, DataLimits::default()).expect("array document")
}

pub fn object<K: AsRef<str>>(entries: impl IntoIterator<Item = (K, Doc)>) -> Doc {
    let mut nodes = Vec::new();
    let mut pairs = Vec::new();
    for (key, value) in entries {
        let key_index = append(&mut nodes, string(key));
        let value_index = append(&mut nodes, value);
        pairs.push((key_index, value_index));
    }
    let root = nodes.len();
    nodes.push(DataNode::Object(pairs));
    DataDocument::from_nodes(nodes, root, DataLimits::default()).expect("object document")
}

pub fn is_null(doc: &Doc) -> bool {
    matches!(root(doc), DataNode::Null | DataNode::Undefined)
}

pub fn as_bool(doc: &Doc) -> Option<bool> {
    match root(doc) {
        DataNode::Boolean(value) => Some(*value),
        _ => None,
    }
}

pub fn as_number(doc: &Doc) -> Option<f64> {
    match root(doc) {
        DataNode::Number(value) => Some(*value),
        _ => None,
    }
}

pub fn as_str(doc: &Doc) -> Option<String> {
    match root(doc) {
        DataNode::String(value) => value.to_utf8().ok(),
        _ => None,
    }
}

pub fn as_bytes(doc: &Doc) -> Option<Vec<u8>> {
    match root(doc) {
        DataNode::Bytes(value) => Some(value.clone()),
        _ => None,
    }
}

pub fn as_date(doc: &Doc) -> Option<OffsetDateTime> {
    match root(doc) {
        DataNode::Date(value) => Some(*value),
        _ => None,
    }
}

pub fn is_bytes(doc: &Doc) -> bool {
    matches!(root(doc), DataNode::Bytes(_))
}

pub fn is_date(doc: &Doc) -> bool {
    matches!(root(doc), DataNode::Date(_))
}

pub fn as_i64(doc: &Doc) -> Option<i64> {
    let number = as_number(doc)?;
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

pub fn as_u64(doc: &Doc) -> Option<u64> {
    let number = as_number(doc)?;
    if number.is_finite() && number >= 0.0 && number.fract() == 0.0 && number <= u64::MAX as f64 {
        let int = number as u64;
        (int as f64 == number).then_some(int)
    } else {
        None
    }
}

pub fn items(doc: &Doc) -> Option<Vec<Doc>> {
    let DataNode::Array(indexes) = root(doc) else {
        return None;
    };
    indexes
        .clone()
        .into_iter()
        .map(|index| subdocument(doc, index))
        .collect()
}

pub fn get(doc: &Doc, key: &str) -> Option<Doc> {
    let DataNode::Object(entries) = root(doc) else {
        return None;
    };
    let index = entries
        .iter()
        .find_map(|(name, value)| match doc.nodes().get(*name) {
            Some(DataNode::String(text)) if text.to_utf8().is_ok_and(|text| text == key) => {
                Some(*value)
            }
            _ => None,
        })?;
    subdocument(doc, index)
}

pub fn entries(doc: &Doc) -> Option<Vec<(String, Doc)>> {
    let DataNode::Object(entries) = root(doc) else {
        return None;
    };
    entries
        .clone()
        .into_iter()
        .map(|(key, value)| {
            let name = match doc.nodes().get(key) {
                Some(DataNode::String(text)) => text.to_utf8().ok()?,
                _ => return None,
            };
            Some((name, subdocument(doc, value)?))
        })
        .collect()
}

pub fn encode(doc: &Doc) -> Result<String, Error> {
    let mut text = doc
        .encode(false, DataLimits::default())
        .map_err(|error| Error::new(error.to_string()))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

pub fn decode(text: &str) -> Result<Doc, Error> {
    DataDocument::from_codec(text).map_err(|error| Error::new(error.to_string()))
}

/// External edge: `kith run --message`, the Kith grammar's JSON syntax tree,
/// and existing callers pass JSON. The stored value is a data-codec document.
pub fn from_json(value: &Json) -> Result<Doc, Error> {
    DataDocument::from_json(crate::json_util::to_uffda_json(value)?)
        .map_err(|error| Error::new(error.to_string()))
}

/// External edge: `World::field`, `World::prints`, and `codec::decode` observe
/// JSON. Byte and date nodes, which JSON cannot hold, project to their codec
/// text. Callers that need the value use the document.
pub fn to_json(doc: &Doc) -> Result<Json, Error> {
    node_to_json(doc, doc.root())
}

fn node_to_json(doc: &Doc, index: usize) -> Result<Json, Error> {
    match doc
        .nodes()
        .get(index)
        .ok_or_else(|| Error::new("invalid codec node"))?
    {
        DataNode::Undefined | DataNode::Null => Ok(Json::Null),
        DataNode::Boolean(value) => Ok(Json::Bool(*value)),
        DataNode::Number(value) => Ok(Json::Number(json_number(*value)?)),
        DataNode::String(value) => Ok(Json::String(
            value
                .to_utf8()
                .map_err(|error| Error::new(error.to_string()))?,
        )),
        DataNode::Array(items) => Ok(Json::Array(
            items
                .iter()
                .map(|index| node_to_json(doc, *index))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        DataNode::Object(entries) => {
            let mut map = serde_json::Map::new();
            for (key, value) in entries {
                let name = match doc.nodes().get(*key) {
                    Some(DataNode::String(text)) => text
                        .to_utf8()
                        .map_err(|error| Error::new(error.to_string()))?,
                    _ => return Err(Error::new("unsupported codec object key")),
                };
                map.insert(name, node_to_json(doc, *value)?);
            }
            Ok(Json::Object(map))
        }
        DataNode::Bytes(_) | DataNode::Date(_) => {
            let sub = subdocument(doc, index).ok_or_else(|| Error::new("invalid codec node"))?;
            let text = sub
                .encode(false, DataLimits::default())
                .map_err(|error| Error::new(error.to_string()))?;
            Ok(Json::String(text.trim_end().to_string()))
        }
        _ => Err(Error::new("unsupported codec node for JSON")),
    }
}

fn json_number(value: f64) -> Result<serde_json::Number, Error> {
    serde_json::Number::from_f64(value).ok_or_else(|| Error::new("non-finite number"))
}

fn leaf(node: DataNode) -> Doc {
    DataDocument::from_nodes(vec![node], 0, DataLimits::default()).expect("leaf document")
}

fn root(doc: &Doc) -> &DataNode {
    &doc.nodes()[doc.root()]
}

fn append(nodes: &mut Vec<DataNode>, doc: Doc) -> usize {
    let base = nodes.len();
    let root = doc.root() + base;
    for node in doc.nodes() {
        nodes.push(shift(node.clone(), base));
    }
    root
}

fn shift(node: DataNode, by: usize) -> DataNode {
    match node {
        DataNode::Array(items) => {
            DataNode::Array(items.into_iter().map(|index| index + by).collect())
        }
        DataNode::Object(entries) => DataNode::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key + by, value + by))
                .collect(),
        ),
        DataNode::Set(items) => DataNode::Set(items.into_iter().map(|index| index + by).collect()),
        DataNode::Error {
            name,
            message,
            stack,
            cause,
        } => DataNode::Error {
            name,
            message,
            stack,
            cause: cause.map(|index| index + by),
        },
        other => other,
    }
}

fn subdocument(doc: &Doc, index: usize) -> Option<Doc> {
    let nodes = doc.nodes();
    let mut seen = vec![false; nodes.len()];
    let mut order = Vec::new();
    let mut pending = vec![index];
    while let Some(current) = pending.pop() {
        if seen.get(current).copied().unwrap_or(true) {
            continue;
        }
        seen[current] = true;
        order.push(current);
        match &nodes[current] {
            DataNode::Array(items) | DataNode::Set(items) => pending.extend(items.iter().rev()),
            DataNode::Object(entries) => {
                for (key, value) in entries.iter().rev() {
                    pending.push(*value);
                    pending.push(*key);
                }
            }
            DataNode::Error { cause, .. } => {
                if let Some(cause) = cause {
                    pending.push(*cause);
                }
            }
            _ => {}
        }
    }
    order.sort_unstable();
    let mut map = vec![0; nodes.len()];
    for (next, old) in order.iter().copied().enumerate() {
        map[old] = next;
    }
    let compact = order
        .iter()
        .map(|old| shift_to(nodes[*old].clone(), &map))
        .collect();
    DataDocument::from_nodes(compact, map[index], DataLimits::default()).ok()
}

fn shift_to(node: DataNode, map: &[usize]) -> DataNode {
    match node {
        DataNode::Array(items) => {
            DataNode::Array(items.into_iter().map(|index| map[index]).collect())
        }
        DataNode::Object(entries) => DataNode::Object(
            entries
                .into_iter()
                .map(|(key, value)| (map[key], map[value]))
                .collect(),
        ),
        DataNode::Set(items) => DataNode::Set(items.into_iter().map(|index| map[index]).collect()),
        DataNode::Error {
            name,
            message,
            stack,
            cause,
        } => DataNode::Error {
            name,
            message,
            stack,
            cause: cause.map(|index| map[index]),
        },
        other => other,
    }
}
