use crate::error::Error;
use serde_json::Value as Json;
use uffda_runtime::json::{JsonValue, ObjectMap};
use uffda_runtime::value::{AssociationKey, Value};

pub fn to_uffda_json(value: &Json) -> Result<JsonValue, Error> {
    Ok(match value {
        Json::Null => JsonValue::Null,
        Json::Bool(value) => JsonValue::Bool(*value),
        Json::Number(value) => JsonValue::Number(value.clone()),
        Json::String(value) => JsonValue::String(value.as_str().into()),
        Json::Array(values) => JsonValue::Array(
            values
                .iter()
                .map(to_uffda_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Json::Object(values) => {
            let mut map = ObjectMap::new();
            for (key, value) in values {
                map.insert(key.as_str().into(), to_uffda_json(value)?);
            }
            JsonValue::Object(map)
        }
    })
}

pub fn from_uffda_value(value: Value<'_>) -> Result<Json, Error> {
    match value.raw() {
        Value::Undefined | Value::Null => Ok(Json::Null),
        Value::Boolean(value) => Ok(Json::Bool(value)),
        Value::Number(value) => Ok(Json::Number(
            serde_json::Number::from_f64(value).ok_or_else(|| Error::new("non-finite number"))?,
        )),
        Value::String(value) => Ok(Json::String(
            value
                .to_utf8()
                .map_err(|error| Error::new(error.to_string()))?,
        )),
        Value::Match(matched) => from_uffda_value(matched.value),
        Value::Array(values) => Ok(Json::Array(
            values
                .borrow()
                .iter()
                .copied()
                .map(from_uffda_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Value::Object(object) => {
            let mut map = serde_json::Map::new();
            for (key, value) in object.borrow().entries() {
                let name = match key {
                    AssociationKey::String(text) => text
                        .to_utf8()
                        .map_err(|error| Error::new(error.to_string()))?,
                    AssociationKey::Symbol(_) | AssociationKey::Value(_) => {
                        return Err(Error::new("unsupported Uffda object key for Kith JSON"));
                    }
                };
                map.insert(name, from_uffda_value(*value)?);
            }
            Ok(Json::Object(map))
        }
        other => Err(Error::new(format!(
            "unsupported Uffda value for Kith JSON: {}",
            other.runtime_type()
        ))),
    }
}
