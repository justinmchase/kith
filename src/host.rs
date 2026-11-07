use crate::error::Error;
use crate::json_util::{from_uffda_value, to_uffda_json};
use serde_json::Value as Json;
use std::collections::HashMap;
use std::rc::Rc;
use uffda_runtime::environment::Environment;
use uffda_runtime::evaluate::Evaluation;
use uffda_runtime::expression::Expression;
use uffda_runtime::json;
use uffda_runtime::pattern::{MatchKind, Pattern};
use uffda_runtime::runtime::{InputMode, Runtime};
use uffda_runtime::value::Value;

pub fn parse_expression(ast: &Json) -> Result<Expression, Error> {
    json::from_str(&ast.to_string()).map_err(|error| Error::new(error.to_string()))
}

pub fn parse_pattern(ast: &Json) -> Result<Pattern, Error> {
    json::from_str(&ast.to_string()).map_err(|error| Error::new(error.to_string()))
}

pub fn evaluate(expression: &Json, bindings: &HashMap<String, Json>) -> Result<Json, Error> {
    let expression = parse_expression(expression)?;
    let captured = bindings.clone();
    let mut evaluation = Evaluation::with_environment(expression, |mc| {
        let values = captured
            .iter()
            .filter_map(|(name, value)| {
                json_to_value(mc, value)
                    .ok()
                    .map(|value| (Rc::<str>::from(name.as_str()), value))
            })
            .collect();
        Environment::new(mc, None, values)
    });
    let result = evaluation
        .finish()
        .map_err(|error| Error::new(error.to_string()))?;
    evaluation
        .read(&result, from_uffda_value)
        .map_err(|error| Error::new(error.to_string()))?
}

pub fn match_message(
    pattern: &Json,
    message: &Json,
) -> Result<Option<HashMap<String, Json>>, Error> {
    let pattern = parse_pattern(pattern)?;
    let names = binding_names(&pattern);
    let input = to_uffda_json(message)?;
    let mut runtime = Runtime::for_pattern(pattern, input, InputMode::Scalar)
        .map_err(|error| Error::new(error.to_string()))?;
    runtime
        .run()
        .map_err(|error| Error::new(error.to_string()))?;
    runtime.read(|execution| {
        let matched = execution
            .result()
            .ok_or_else(|| Error::new("missing match"))?;
        if matched.kind != MatchKind::Ok {
            return Ok(None);
        }
        let mut bindings = HashMap::new();
        for name in names {
            if let Some(value) = matched.environment.get(&name) {
                bindings.insert(name, from_uffda_value(value)?);
            }
        }
        Ok(Some(bindings))
    })
}

fn json_to_value<'gc>(mc: &uffda_heap::Mutation<'gc>, value: &Json) -> Result<Value<'gc>, Error> {
    Ok(match value {
        Json::Null => Value::Null,
        Json::Bool(value) => Value::Boolean(*value),
        Json::Number(value) => Value::Number(value.as_f64().ok_or_else(|| Error::new("number"))?),
        Json::String(value) => Value::string(mc, value.as_str()),
        Json::Array(values) => Value::array(
            mc,
            values
                .iter()
                .map(|value| json_to_value(mc, value))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Json::Object(values) => Value::object(
            mc,
            values
                .iter()
                .map(|(key, value)| Ok((key.as_str().into(), json_to_value(mc, value)?)))
                .collect::<Result<Vec<_>, Error>>()?,
        ),
    })
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
