use crate::doc::{self, Doc};
use crate::error::Error;
use serde_json::Value as Json;
use std::collections::HashMap;
use std::rc::Rc;
use uffda_heap::{Arena, Rootable};
use uffda_runtime::codec::data::{DataDocument, DataLimits};
use uffda_runtime::environment::Environment;
use uffda_runtime::evaluate::Evaluation;
use uffda_runtime::expression::Expression;
use uffda_runtime::json;
use uffda_runtime::pattern::{MatchKind, Pattern, PatternMatch};
use uffda_runtime::runtime::RuleExecution;

/// The Kith grammar embeds Uffda expression syntax as JSON. This parses that
/// tree into the native expression AST.
pub fn parse_expression(ast: &Json) -> Result<Expression, Error> {
    json::from_str(&ast.to_string()).map_err(|error| Error::new(error.to_string()))
}

/// The Kith grammar embeds Uffda pattern syntax as JSON. This parses that
/// tree into the native pattern AST.
pub fn parse_pattern(ast: &Json) -> Result<Pattern, Error> {
    json::from_str(&ast.to_string()).map_err(|error| Error::new(error.to_string()))
}

/// External edge: existing callers pass JSON bindings. The evaluation itself
/// runs on data-codec documents.
pub fn evaluate(expression: &Json, bindings: &HashMap<String, Json>) -> Result<Json, Error> {
    let bindings = json_bindings(bindings)?;
    doc::to_json(&evaluate_document(expression, &bindings)?)
}

pub fn evaluate_document(expression: &Json, bindings: &HashMap<String, Doc>) -> Result<Doc, Error> {
    let expression = parse_expression(expression)?;
    let captured = bindings.clone();
    let mut evaluation = Evaluation::with_environment(expression, |mc| {
        let values = captured
            .iter()
            .map(|(name, value)| (Rc::<str>::from(name.as_str()), value.materialize(mc)))
            .collect();
        Environment::new(mc, None, values)
    });
    let result = evaluation
        .finish()
        .map_err(|error| Error::new(error.to_string()))?;
    evaluation
        .read(&result, |value| {
            DataDocument::from_value(value, DataLimits::default())
                .map_err(|error| Error::new(error.to_string()))
        })
        .map_err(|error| Error::new(error.to_string()))?
}

/// External edge: existing callers pass a JSON message. Matching runs on a
/// data-codec document.
pub fn match_message(
    pattern: &Json,
    message: &Json,
) -> Result<Option<HashMap<String, Json>>, Error> {
    let matched = match_document(pattern, &doc::from_json(message)?)?;
    match matched {
        None => Ok(None),
        Some(bindings) => Ok(Some(json_map(bindings)?)),
    }
}

/// `@Name` in a handler is `{ kind: "symbol", name }`. Uffda's equal pattern
/// does not match a symbol, so the name is checked here and the node is
/// rewritten to `{ kind: "type", type: "symbol" }` before `parse_pattern`.
pub fn uffda_pattern(pattern: &Json) -> Json {
    let mut pattern = pattern.clone();
    rewrite_symbol_patterns(&mut pattern);
    pattern
}

fn rewrite_symbol_patterns(value: &mut Json) {
    if value.get("kind").and_then(Json::as_str) == Some("symbol") {
        *value = serde_json::json!({"kind": "type", "type": "symbol"});
        return;
    }
    match value {
        Json::Array(items) => {
            for item in items {
                rewrite_symbol_patterns(item);
            }
        }
        Json::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                if let Some(child) = map.get_mut(&key) {
                    rewrite_symbol_patterns(child);
                }
            }
        }
        _ => {}
    }
}

/// A `{ kind: "symbol", name }` prefix matches only that registry symbol.
/// Patterns with no symbol prefix agree with any message.
pub fn symbol_names_match(pattern: &Json, message: &Doc) -> bool {
    symbol_names_agree(pattern, message)
}

fn symbol_names_agree(pattern: &Json, message: &Doc) -> bool {
    let Some(kind) = pattern.get("kind").and_then(Json::as_str) else {
        return true;
    };
    match kind {
        "symbol" => {
            pattern.get("name").and_then(Json::as_str) == doc::as_symbol(message).as_deref()
        }
        "then" => {
            let Some(patterns) = pattern.get("patterns").and_then(Json::as_array) else {
                return true;
            };
            let Some(items) = doc::items(message) else {
                return false;
            };
            if items.len() != patterns.len() {
                return false;
            }
            patterns
                .iter()
                .zip(items.iter())
                .all(|(pattern, item)| symbol_names_agree(pattern, item))
        }
        "over" => pattern
            .get("keys")
            .and_then(Json::as_object)
            .map(|keys| {
                keys.iter().all(|(name, child)| {
                    doc::get(message, name)
                        .map(|value| symbol_names_agree(child, &value))
                        .unwrap_or(true)
                })
            })
            .unwrap_or(true),
        "variable" => pattern
            .get("pattern")
            .map(|inner| symbol_names_agree(inner, message))
            .unwrap_or(true),
        "and" => pattern
            .get("patterns")
            .and_then(Json::as_array)
            .map(|items| items.iter().all(|item| symbol_names_agree(item, message)))
            .unwrap_or(true),
        _ => true,
    }
}

pub fn match_document(
    pattern: &Json,
    message: &Doc,
) -> Result<Option<HashMap<String, Doc>>, Error> {
    if !symbol_names_match(pattern, message) {
        return Ok(None);
    }
    let pattern = parse_pattern(&uffda_pattern(pattern))?;
    let iterable = match &pattern {
        Pattern::Then { patterns } => {
            let Some(values) = doc::items(message) else {
                return Ok(None);
            };
            if values.len() != patterns.len() {
                return Ok(None);
            }
            true
        }
        _ => false,
    };
    let names = binding_names(&pattern);
    let message = message.clone();
    let mut arena: Arena<Rootable![RuleExecution<'_>]> = Arena::new(|mc| {
        let input = if iterable {
            doc::items(&message)
                .unwrap_or_default()
                .iter()
                .map(|item| item.materialize(mc))
                .collect()
        } else {
            vec![message.materialize(mc)]
        };
        RuleExecution::for_pattern(mc, pattern, input)
    });
    loop {
        let done = arena
            .mutate_root(|mc, execution| execution.step(mc))
            .map_err(|error| Error::new(error.to_string()))?;
        if done {
            break;
        }
    }
    arena.mutate(|_, execution| {
        let matched = execution
            .result()
            .ok_or_else(|| Error::new("missing match"))?;
        if matched.kind != MatchKind::Ok {
            return Ok(None);
        }
        let mut bindings = HashMap::new();
        for name in names {
            if let Some(value) = binding_value(&matched, &name) {
                bindings.insert(
                    name,
                    DataDocument::from_value(value, DataLimits::default())
                        .map_err(|error| Error::new(error.to_string()))?,
                );
            }
        }
        Ok(Some(bindings))
    })
}

/// Match a stored field as one scalar value. Uffda Maybe (`number?`) that
/// skips a present non-null value is not a match: optional means null or T.
/// External edge: existing callers pass JSON.
pub fn match_field(pattern: &Json, value: &Json) -> Result<bool, Error> {
    match_field_document(pattern, &doc::from_json(value)?)
}

pub fn match_field_document(pattern: &Json, value: &Doc) -> Result<bool, Error> {
    if !symbol_names_match(pattern, value) {
        return Ok(false);
    }
    let pattern = parse_pattern(&uffda_pattern(pattern))?;
    let value = value.clone();
    let mut arena: Arena<Rootable![RuleExecution<'_>]> =
        Arena::new(|mc| RuleExecution::for_pattern(mc, pattern, vec![value.materialize(mc)]));
    loop {
        let done = arena
            .mutate_root(|mc, execution| execution.step(mc))
            .map_err(|error| Error::new(error.to_string()))?;
        if done {
            break;
        }
    }
    let is_null = doc::is_null(&value);
    arena.mutate(|_, execution| {
        let matched = execution
            .result()
            .ok_or_else(|| Error::new("missing match"))?;
        if matched.kind != MatchKind::Ok {
            return Ok(false);
        }
        let consumed = matched.end > matched.start;
        Ok(consumed || is_null)
    })
}

fn json_bindings(bindings: &HashMap<String, Json>) -> Result<HashMap<String, Doc>, Error> {
    bindings
        .iter()
        .map(|(name, value)| Ok((name.clone(), doc::from_json(value)?)))
        .collect()
}

fn json_map(bindings: HashMap<String, Doc>) -> Result<HashMap<String, Json>, Error> {
    bindings
        .into_iter()
        .map(|(name, value)| Ok((name, doc::to_json(&value)?)))
        .collect()
}

fn binding_value<'a>(
    matched: &PatternMatch<'a>,
    name: &str,
) -> Option<uffda_runtime::value::Value<'a>> {
    if let Some(value) = matched.environment.get(name) {
        return Some(value);
    }
    matched
        .children
        .as_slice()
        .iter()
        .find_map(|child| binding_value(child, name))
}

fn binding_names(pattern: &Pattern) -> Vec<String> {
    let mut names = Vec::new();
    collect_names(pattern, &mut names);
    names
}

fn collect_names(pattern: &Pattern, names: &mut Vec<String>) {
    match pattern {
        Pattern::Variable { name, pattern } => {
            names.push(name.clone());
            collect_names(pattern, names);
        }
        Pattern::And { patterns }
        | Pattern::Then { patterns }
        | Pattern::Or { patterns }
        | Pattern::Pipeline { steps: patterns } => {
            for pattern in patterns {
                collect_names(pattern, names);
            }
        }
        Pattern::Maybe { pattern }
        | Pattern::Not { pattern }
        | Pattern::Lookahead { pattern }
        | Pattern::Skip { pattern }
        | Pattern::Into { pattern }
        | Pattern::Except { pattern }
        | Pattern::Quantifier { pattern, .. }
        | Pattern::Projection { pattern, .. }
        | Pattern::Recover { pattern, .. } => collect_names(pattern, names),
        Pattern::Over { keys, rest } => {
            if let Some(keys) = keys {
                for (_, pattern) in keys {
                    collect_names(pattern, names);
                }
            }
            if let Some(rest) = rest {
                for clause in rest {
                    if let uffda_runtime::pattern::OverRestClause::Pattern { key, value, .. } =
                        clause
                    {
                        collect_names(key, names);
                        collect_names(value, names);
                    }
                }
            }
        }
        Pattern::Switch {
            default: Some(default),
            ..
        } => collect_names(default, names),
        Pattern::Resolve { args, .. } => {
            for pattern in args {
                collect_names(pattern, names);
            }
        }
        _ => {}
    }
}
