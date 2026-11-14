use super::data::{is_blob, is_buffer};
use super::time::{date_time_of, zone_of};
use super::value::{is_bytes, is_instant};
use crate::error::Error;
use crate::host::match_message;
use serde_json::Value as Json;
use std::collections::HashMap;

pub fn uses_rule(pattern: &Json, rules: &[String]) -> bool {
    match pattern {
        Json::Object(map) => {
            if map.get("kind").and_then(Json::as_str) == Some("resolve")
                && map
                    .get("name")
                    .and_then(Json::as_str)
                    .is_some_and(|name| rules.iter().any(|rule| rule == name))
            {
                return true;
            }
            map.values().any(|value| uses_rule(value, rules))
        }
        Json::Array(items) => items.iter().any(|value| uses_rule(value, rules)),
        _ => false,
    }
}

pub fn match_with_rules(
    pattern: &Json,
    message: &Json,
    rules: &[String],
) -> Result<Option<HashMap<String, Json>>, Error> {
    if pattern.get("kind").and_then(Json::as_str) == Some("then") {
        let Some(patterns) = pattern.get("patterns").and_then(Json::as_array) else {
            return Ok(None);
        };
        let Some(values) = message.as_array() else {
            return Ok(None);
        };
        if values.len() != patterns.len() {
            return Ok(None);
        }
        let mut bindings = HashMap::new();
        for (pattern, value) in patterns.iter().zip(values) {
            let Some(matched) = match_node(pattern, value, rules)? else {
                return Ok(None);
            };
            bindings.extend(matched);
        }
        return Ok(Some(bindings));
    }
    match_node(pattern, message, rules)
}

fn match_node(
    pattern: &Json,
    value: &Json,
    rules: &[String],
) -> Result<Option<HashMap<String, Json>>, Error> {
    if !uses_rule(pattern, rules) {
        return match_message(pattern, value);
    }
    let kind = pattern.get("kind").and_then(Json::as_str).unwrap_or("");
    match kind {
        "resolve" => {
            let Some(name) = pattern.get("name").and_then(Json::as_str) else {
                return Ok(None);
            };
            let args = pattern.get("args").and_then(Json::as_array);
            if args.is_some_and(|args| !args.is_empty()) {
                return Ok(None);
            }
            Ok(rule_matches(name, value).then_some(HashMap::new()))
        }
        "variable" => {
            let Some(name) = pattern.get("name").and_then(Json::as_str) else {
                return Ok(None);
            };
            let inner = pattern.get("pattern").unwrap_or(&Json::Null);
            let Some(mut bindings) = match_node(inner, value, rules)? else {
                return Ok(None);
            };
            bindings.insert(name.to_string(), value.clone());
            Ok(Some(bindings))
        }
        "and" => {
            let Some(patterns) = pattern.get("patterns").and_then(Json::as_array) else {
                return Ok(None);
            };
            let mut bindings = HashMap::new();
            for pattern in patterns {
                let Some(matched) = match_node(pattern, value, rules)? else {
                    return Ok(None);
                };
                bindings.extend(matched);
            }
            Ok(Some(bindings))
        }
        "or" => {
            let Some(patterns) = pattern.get("patterns").and_then(Json::as_array) else {
                return Ok(None);
            };
            for pattern in patterns {
                if let Some(bindings) = match_node(pattern, value, rules)? {
                    return Ok(Some(bindings));
                }
            }
            Ok(None)
        }
        "not" => {
            let inner = pattern.get("pattern").unwrap_or(&Json::Null);
            Ok(match_node(inner, value, rules)?
                .is_none()
                .then_some(HashMap::new()))
        }
        "maybe" => {
            if value.is_null() {
                return Ok(Some(HashMap::new()));
            }
            let inner = pattern.get("pattern").unwrap_or(&Json::Null);
            match_node(inner, value, rules)
        }
        "any" => Ok(Some(HashMap::new())),
        "equal" => Ok(equal(pattern.get("value"), value).then_some(HashMap::new())),
        "type" => {
            let expected = pattern.get("type").and_then(Json::as_str).unwrap_or("");
            Ok(type_matches(expected, value).then_some(HashMap::new()))
        }
        "over" => match_over(pattern, value, rules),
        "then" => match_with_rules(pattern, value, rules),
        _ => Err(Error::new(format!("unsupported pattern {kind}"))),
    }
}

fn rule_matches(name: &str, value: &Json) -> bool {
    match name {
        "Bytes" => is_bytes(value),
        "Blob" => is_blob(value),
        "Buffer" => is_buffer(value),
        "Chunk" => value.as_str() == Some("Chunk"),
        "End" => value.as_str() == Some("End"),
        "Instant" => is_instant(value),
        "DateTime" => date_time_of(value).is_some(),
        "Zone" => zone_of(value).is_some(),
        _ => false,
    }
}

fn equal(expected: Option<&Json>, value: &Json) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    let literal = if expected.get("kind").and_then(Json::as_str) == Some("value.literal") {
        expected.get("value").unwrap_or(&Json::Null)
    } else {
        expected
    };
    same_value(literal, value)
}

fn same_value(left: &Json, right: &Json) -> bool {
    match (left.as_f64(), right.as_f64()) {
        (Some(left), Some(right)) => left == right,
        _ => left == right,
    }
}

fn type_matches(expected: &str, value: &Json) -> bool {
    match expected {
        "Number" => value.is_number(),
        "String" => value.is_string(),
        "Boolean" => value.is_boolean(),
        "Array" => value.is_array(),
        "Null" => value.is_null(),
        "Object" => value.is_object() && !is_bytes(value) && !is_instant(value),
        "Date" => is_instant(value),
        _ => false,
    }
}

fn match_over(
    pattern: &Json,
    value: &Json,
    rules: &[String],
) -> Result<Option<HashMap<String, Json>>, Error> {
    if is_bytes(value) || is_instant(value) {
        return Ok(None);
    }
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let Some(keys) = pattern.get("keys").and_then(Json::as_object) else {
        return Ok(Some(HashMap::new()));
    };
    let mut bindings = HashMap::new();
    for (name, pattern) in keys {
        let Some(field) = object.get(name) else {
            return Ok(None);
        };
        let Some(matched) = match_node(pattern, field, rules)? else {
            return Ok(None);
        };
        bindings.extend(matched);
    }
    Ok(Some(bindings))
}
