//! Imported rule names are Uffda resolve rules. The handler pattern is the
//! grammar's JSON syntax tree; it is parsed into a `Pattern` and matched by
//! the Uffda runtime against a module graph that imports the built-in rules.

use crate::doc::{self, Doc};
use crate::error::Error;
use crate::host::parse_pattern;
use std::collections::HashMap;
use std::rc::Rc;
use uffda_heap::{Arena, Rootable};
use uffda_runtime::codec::data::{DataDocument, DataLimits};
use uffda_runtime::module::CompiledModuleGraph;
use uffda_runtime::pattern::{MatchKind, Pattern, PatternError, PatternMatch};
use uffda_runtime::resolver::{decode_declaration, Resolver};
use uffda_runtime::runtime::{RuleExecution, RuntimeError};
use url::Url;

pub fn uses_rule(pattern: &serde_json::Value, rules: &[String]) -> bool {
    // The Kith grammar embeds the Uffda pattern syntax tree as JSON.
    let Ok(pattern) = parse_pattern(pattern) else {
        return false;
    };
    pattern_uses(&pattern, rules)
}

pub fn match_with_rules(
    pattern: &serde_json::Value,
    message: &Doc,
    rules: &[String],
) -> Result<Option<HashMap<String, Doc>>, Error> {
    if !uses_rule(pattern, rules) {
        return crate::host::match_document(pattern, message);
    }
    // The Kith grammar embeds the Uffda pattern syntax tree as JSON.
    let parsed = parse_pattern(pattern)?;
    let input = match &parsed {
        Pattern::Then { patterns } => {
            let Some(items) = doc::items(message) else {
                return Ok(None);
            };
            if items.len() != patterns.len() {
                return Ok(None);
            }
            items
        }
        _ => vec![message.clone()],
    };
    let graph = harness(pattern, &parsed, rules)?;
    let names = binding_names(&parsed);
    let mut arena: Arena<Rootable![RuleExecution<'_>]> = Arena::try_new(|mc| {
        let values = input.iter().map(|item| item.materialize(mc)).collect();
        RuleExecution::new(
            mc,
            Rc::clone(&graph),
            &Url::parse("kith:match").expect("match url"),
            Some("Match"),
            values,
        )
    })
    .map_err(|error| Error::new(error.to_string()))?;
    loop {
        let step = arena.mutate_root(|mc, execution| execution.step(mc));
        match step {
            Ok(true) => break,
            Ok(false) => {}
            Err(RuntimeError::Pattern(PatternError::TypeError(_))) => return Ok(None),
            Err(error) => return Err(Error::new(error.to_string())),
        }
    }
    arena.mutate(|_, execution| {
        let Some(matched) = execution.result() else {
            return Err(Error::new("missing match"));
        };
        if matched.kind != MatchKind::Ok || !accepted(&graph, &*matched) {
            return Ok(None);
        }
        let mut bindings = HashMap::new();
        for name in names {
            if let Some(value) = binding_value(&matched, &name) {
                let document = DataDocument::from_value(value, DataLimits::default())
                    .map_err(|error| Error::new(error.to_string()))?;
                bindings.insert(name, document);
            }
        }
        Ok(Some(bindings))
    })
}

fn pattern_uses(pattern: &Pattern, rules: &[String]) -> bool {
    match pattern {
        Pattern::Resolve { name, args, .. } => {
            name.as_ref()
                .is_some_and(|name| rules.iter().any(|rule| rule == name))
                || args.iter().any(|pattern| pattern_uses(pattern, rules))
        }
        Pattern::Variable { pattern, .. }
        | Pattern::Maybe { pattern }
        | Pattern::Not { pattern }
        | Pattern::Lookahead { pattern }
        | Pattern::Skip { pattern }
        | Pattern::Into { pattern }
        | Pattern::Except { pattern }
        | Pattern::Quantifier { pattern, .. }
        | Pattern::Projection { pattern, .. }
        | Pattern::Recover { pattern, .. } => pattern_uses(pattern, rules),
        Pattern::And { patterns }
        | Pattern::Then { patterns }
        | Pattern::Or { patterns }
        | Pattern::Pipeline { steps: patterns } => {
            patterns.iter().any(|pattern| pattern_uses(pattern, rules))
        }
        Pattern::Over { keys, rest } => {
            keys.iter()
                .flatten()
                .any(|(_, pattern)| pattern_uses(pattern, rules))
                || rest.iter().flatten().any(|clause| match clause {
                    uffda_runtime::pattern::OverRestClause::Pattern { key, value, .. } => {
                        pattern_uses(key, rules) || pattern_uses(value, rules)
                    }
                    uffda_runtime::pattern::OverRestClause::Any { .. } => false,
                })
        }
        Pattern::Switch { default, .. } => default
            .as_ref()
            .is_some_and(|pattern| pattern_uses(pattern, rules)),
        _ => false,
    }
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
        .find_map(|child| binding_value(&**child, name))
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

/// `Value::Bytes` reports runtime type Object, so no Uffda type pattern
/// matches only bytes. The Bytes rule is still resolved by the matcher; a
/// successful resolve is kept only when the value is bytes. Buffer's window
/// is the same kind of predicate on top of the object pattern.
fn accepted(graph: &CompiledModuleGraph, matched: &PatternMatch<'_>) -> bool {
    if let Some(rule) = matched.rule.as_ref() {
        if let Some(declaration) = graph.declaration(&rule.member) {
            if !rule_value_ok(&declaration.name, &matched.value) {
                return false;
            }
        }
    }
    matched
        .children
        .as_slice()
        .iter()
        .all(|child| child.kind == MatchKind::Fail || accepted(graph, &**child))
}

fn rule_value_ok(name: &str, value: &uffda_runtime::value::Value<'_>) -> bool {
    match name {
        "Bytes" => matches!(value.raw(), uffda_runtime::value::Value::Bytes(_)),
        "Buffer" => {
            let Ok(document) = DataDocument::from_value(*value, DataLimits::default()) else {
                return false;
            };
            super::data::buffer_in_window(&document)
        }
        _ => true,
    }
}

fn harness(
    pattern: &serde_json::Value,
    parsed: &Pattern,
    rules: &[String],
) -> Result<Rc<CompiledModuleGraph>, Error> {
    let mut names = Vec::new();
    collect_resolves(parsed, rules, &mut names);
    names.sort();
    names.dedup();
    let mut imports = Vec::new();
    for (module, module_names) in [
        ("kith:data", &["Bytes", "Blob", "Buffer"][..]),
        ("kith:stream", &["Chunk", "End"][..]),
        ("kith:time", &["Instant", "DateTime", "Zone"][..]),
    ] {
        let used: Vec<_> = module_names
            .iter()
            .filter(|name| names.iter().any(|used| used == *name))
            .copied()
            .collect();
        if used.is_empty() {
            continue;
        }
        imports.push(serde_json::json!({
            "kind": "module",
            "moduleUrl": module,
            "names": used,
            "module": serde_json::Value::Null
        }));
    }
    // External edge: `decode_declaration` reads the module declaration, and
    // the handler pattern is the grammar's JSON syntax tree.
    let source = serde_json::json!({
        "imports": imports,
        "exports": [{"kind": "rule", "name": "Match"}],
        "rules": [{
            "name": "Match",
            "parameters": [],
            "pattern": pattern
        }],
        "funcs": [],
        "decorators": []
    })
    .to_string();
    let mut resolver = Resolver::from_cwd().map_err(|error| Error::new(error.to_string()))?;
    super::install(&mut resolver)?;
    let url = Url::parse("kith:match").map_err(|error| Error::new(error.to_string()))?;
    let declaration =
        decode_declaration(&url, &source).map_err(|error| Error::new(error.to_string()))?;
    resolver.insert_declaration(url.clone(), declaration);
    let graph = resolver
        .load_graph(&url)
        .map_err(|error| Error::new(error.to_string()))?;
    CompiledModuleGraph::compile(graph)
        .map(Rc::new)
        .map_err(|error| Error::new(error.to_string()))
}

fn collect_resolves(pattern: &Pattern, rules: &[String], names: &mut Vec<String>) {
    if pattern_uses(pattern, rules) {
        match pattern {
            Pattern::Resolve { name, args, .. } => {
                if let Some(name) = name {
                    if rules.iter().any(|rule| rule == name) {
                        names.push(name.clone());
                    }
                }
                for pattern in args {
                    collect_resolves(pattern, rules, names);
                }
            }
            Pattern::Variable { pattern, .. }
            | Pattern::Maybe { pattern }
            | Pattern::Not { pattern }
            | Pattern::Lookahead { pattern }
            | Pattern::Skip { pattern }
            | Pattern::Into { pattern }
            | Pattern::Except { pattern }
            | Pattern::Quantifier { pattern, .. }
            | Pattern::Projection { pattern, .. }
            | Pattern::Recover { pattern, .. } => collect_resolves(pattern, rules, names),
            Pattern::And { patterns }
            | Pattern::Then { patterns }
            | Pattern::Or { patterns }
            | Pattern::Pipeline { steps: patterns } => {
                for pattern in patterns {
                    collect_resolves(pattern, rules, names);
                }
            }
            Pattern::Over { keys, rest } => {
                if let Some(keys) = keys {
                    for (_, pattern) in keys {
                        collect_resolves(pattern, rules, names);
                    }
                }
                if let Some(rest) = rest {
                    for clause in rest {
                        if let uffda_runtime::pattern::OverRestClause::Pattern {
                            key, value, ..
                        } = clause
                        {
                            collect_resolves(key, rules, names);
                            collect_resolves(value, rules, names);
                        }
                    }
                }
            }
            Pattern::Switch {
                default: Some(default),
                ..
            } => collect_resolves(default, rules, names),
            _ => {}
        }
    }
}
