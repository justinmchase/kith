//! Built-in `kith:` modules. Rules and functions are an immutable Uffda
//! declaration registered with the resolver. Components are Kith host drivers
//! on top of that declaration.

mod data;
mod rules;
mod stream;
mod time;
mod value;

use crate::ast::{Component, Program, State};
use crate::error::Error;
use serde_json::{json, Value as Json};
use std::collections::HashMap;
use uffda_runtime::codec::data::DataDocument;
use uffda_runtime::resolver::{decode_declaration, Resolver};
use url::Url;

pub use rules::{match_with_rules, uses_rule};
pub use stream::Pipe;
pub use time::{instant_text, parse_instant, process_zone, wall_millis, zone_value};
pub use value::{
    bytes_json, bytes_of, encode_base64url, instant_json, instant_millis, is_bytes, is_instant,
};

const DATA: &str = r#"{
  "imports": [],
  "exports": [
    {"kind":"rule","name":"Bytes"},
    {"kind":"rule","name":"Blob"},
    {"kind":"rule","name":"Buffer"},
    {"kind":"func","name":"bytes"},
    {"kind":"func","name":"byte_at"},
    {"kind":"func","name":"byte_length"},
    {"kind":"func","name":"bytes_concat"},
    {"kind":"func","name":"bytes_slice"},
    {"kind":"func","name":"blob"},
    {"kind":"func","name":"blob_bytes"},
    {"kind":"func","name":"blob_type"},
    {"kind":"func","name":"buffer"},
    {"kind":"func","name":"buffer_slice"},
    {"kind":"func","name":"buffer_bytes"}
  ],
  "rules": [
    {"name":"Bytes","parameters":[],"pattern":{"kind":"any"}},
    {"name":"Blob","parameters":[],"pattern":{"kind":"over","keys":{"type":{"kind":"type","type":"String"},"bytes":{"kind":"resolve","targetKind":"reference","name":"Bytes","args":[]}}}},
    {"name":"Buffer","parameters":[],"pattern":{"kind":"over","keys":{"bytes":{"kind":"resolve","targetKind":"reference","name":"Bytes","args":[]},"start":{"kind":"type","type":"Number"},"end":{"kind":"type","type":"Number"}}}}
  ],
  "funcs": [
    {"name":"bytes","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"byte_at","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"byte_length","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"bytes_concat","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"bytes_slice","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"blob","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"blob_bytes","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"blob_type","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"buffer","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"buffer_slice","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"buffer_bytes","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}}
  ],
  "decorators": []
}"#;

const STREAM: &str = r#"{
  "imports": [],
  "exports": [
    {"kind":"rule","name":"Chunk"},
    {"kind":"rule","name":"End"}
  ],
  "rules": [
    {"name":"Chunk","parameters":[],"pattern":{"kind":"equal","value":{"kind":"value.literal","value":"Chunk"}}},
    {"name":"End","parameters":[],"pattern":{"kind":"equal","value":{"kind":"value.literal","value":"End"}}}
  ],
  "funcs": [],
  "decorators": []
}"#;

const TIME: &str = r#"{
  "imports": [],
  "exports": [
    {"kind":"rule","name":"Instant"},
    {"kind":"rule","name":"DateTime"},
    {"kind":"rule","name":"Zone"},
    {"kind":"func","name":"instant"},
    {"kind":"func","name":"instant_text"},
    {"kind":"func","name":"zone"},
    {"kind":"func","name":"date_time"},
    {"kind":"func","name":"instant_at"},
    {"kind":"func","name":"add_seconds"},
    {"kind":"func","name":"diff_seconds"},
    {"kind":"func","name":"offset_seconds"}
  ],
  "rules": [
    {"name":"Instant","parameters":[],"pattern":{"kind":"type","type":"Date"}},
    {"name":"DateTime","parameters":[],"pattern":{"kind":"over","keys":{"year":{"kind":"type","type":"Number"},"month":{"kind":"type","type":"Number"},"day":{"kind":"type","type":"Number"},"hour":{"kind":"type","type":"Number"},"minute":{"kind":"type","type":"Number"},"second":{"kind":"type","type":"Number"},"millisecond":{"kind":"type","type":"Number"}}}},
    {"name":"Zone","parameters":[],"pattern":{"kind":"or","patterns":[{"kind":"over","keys":{"kind":{"kind":"equal","value":{"kind":"value.literal","value":"utc"}}}},{"kind":"over","keys":{"kind":{"kind":"equal","value":{"kind":"value.literal","value":"fixed"}},"seconds":{"kind":"type","type":"Number"}}},{"kind":"over","keys":{"kind":{"kind":"equal","value":{"kind":"value.literal","value":"iana"}},"id":{"kind":"type","type":"String"}}}]}}
  ],
  "funcs": [
    {"name":"instant","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"instant_text","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"zone","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"date_time","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"instant_at","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"add_seconds","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"diff_seconds","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}},
    {"name":"offset_seconds","pattern":{"kind":"end"},"expression":{"kind":"value","value":null}}
  ],
  "decorators": []
}"#;

#[derive(Clone, Debug, PartialEq)]
pub enum Driver {
    Pipe(Pipe),
    Clock,
}

pub fn classify(specifier: &str) -> Result<bool, Error> {
    let Some(name) = specifier.strip_prefix("kith:") else {
        return Ok(false);
    };
    if !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_lowercase()) {
        Ok(true)
    } else {
        Err(Error::new(format!(
            "invalid kith import specifier {specifier}"
        )))
    }
}

pub fn bind(program: &mut Program, specifier: &str, names: &[String]) -> Result<(), Error> {
    let mut resolver = open_resolver()?;
    let url = Url::parse(specifier).map_err(|error| Error::new(error.to_string()))?;
    let declaration = resolver
        .load_declaration(&url)
        .map_err(|_| Error::new(format!("unknown built-in module {specifier}")))?;
    let exports = export_map(&declaration)?;
    for name in names {
        if let Some(kind) = occupied(program, name) {
            return Err(Error::new(format!("{name} collides with a {kind}")));
        }
        if let Some(kind) = exports.get(name) {
            match kind.as_str() {
                "rule" => program.rules.push(name.clone()),
                "func" => program.functions.push(name.clone()),
                other => {
                    return Err(Error::new(format!(
                        "{name} is not a function or rule export ({other})"
                    )))
                }
            }
        } else if component_names(specifier).contains(&name.as_str()) {
            program.components.push(host_component(name));
            program.host_kinds.push(name.clone());
        } else {
            return Err(Error::new(format!("{name} is not exported by {specifier}")));
        }
    }
    Ok(())
}

pub fn call(name: &str, args: &[Json]) -> Result<Json, Error> {
    if let Some(value) = data::call(name, args)? {
        return Ok(value);
    }
    if let Some(value) = time::call(name, args)? {
        return Ok(value);
    }
    Err(Error::new(format!("unknown built-in function {name}")))
}

pub fn initial_driver(kind: &str) -> Option<Driver> {
    match kind {
        "Pipe" => Some(Driver::Pipe(Pipe::new())),
        "Clock" => Some(Driver::Clock),
        _ => None,
    }
}

pub fn drive_pipe(pipe: Pipe, message: &Json) -> (Pipe, Vec<Json>) {
    let (pipe, replies) = stream::step(pipe, message);
    (pipe, replies)
}

pub fn driver_json(driver: &Driver) -> Json {
    match driver {
        Driver::Pipe(pipe) => stream::to_json(pipe),
        Driver::Clock => json!({"kind": "clock"}),
    }
}

pub fn driver_from(value: &Json) -> Option<Driver> {
    match value.get("kind").and_then(Json::as_str) {
        Some("pipe") => stream::from_json(value).map(Driver::Pipe),
        Some("clock") => Some(Driver::Clock),
        _ => None,
    }
}

pub fn reexport(
    program: &mut Program,
    name: &str,
    functions: &[String],
    rules: &[String],
    host: bool,
) {
    if host && !program.host_kinds.iter().any(|kind| kind == name) {
        program.host_kinds.push(name.to_string());
    }
    if functions.iter().any(|item| item == name)
        && !program.functions.iter().any(|item| item == name)
    {
        program.functions.push(name.to_string());
    }
    if rules.iter().any(|item| item == name) && !program.rules.iter().any(|item| item == name) {
        program.rules.push(name.to_string());
    }
}

fn open_resolver() -> Result<Resolver, Error> {
    let mut resolver = Resolver::from_cwd().map_err(|error| Error::new(error.to_string()))?;
    install(&mut resolver)?;
    Ok(resolver)
}

fn install(resolver: &mut Resolver) -> Result<(), Error> {
    for (specifier, source) in [
        ("kith:data", DATA),
        ("kith:stream", STREAM),
        ("kith:time", TIME),
    ] {
        let url = Url::parse(specifier).map_err(|error| Error::new(error.to_string()))?;
        let declaration =
            decode_declaration(&url, source).map_err(|error| Error::new(error.to_string()))?;
        resolver.insert_declaration(url, declaration);
    }
    Ok(())
}

fn component_names(specifier: &str) -> &'static [&'static str] {
    match specifier {
        "kith:stream" => &["Pipe"],
        "kith:time" => &["Clock"],
        _ => &[],
    }
}

fn host_component(name: &str) -> Component {
    Component {
        kind: "component".into(),
        name: name.to_string(),
        fields: Vec::new(),
        ports: Vec::new(),
        states: vec![State {
            kind: "state".into(),
            name: "Start".into(),
            start: true,
            named: false,
            handlers: Vec::new(),
        }],
    }
}

fn occupied(program: &Program, name: &str) -> Option<&'static str> {
    if program.component(name).is_some() {
        Some("component")
    } else if program.workflow(name).is_some() {
        Some("workflow")
    } else if program.scene(name).is_some() {
        Some("scene")
    } else if program.functions.iter().any(|item| item == name) {
        Some("function")
    } else if program.rules.iter().any(|item| item == name) {
        Some("rule")
    } else {
        None
    }
}

fn export_map(
    declaration: &uffda_runtime::resolver::ModuleDeclaration,
) -> Result<HashMap<String, String>, Error> {
    let mut exports = HashMap::new();
    for export in &declaration.exports {
        let name = document_string(export, "name")?;
        let kind = document_string(export, "kind")?;
        exports.insert(name, kind);
    }
    Ok(exports)
}

fn document_string(document: &DataDocument, key: &str) -> Result<String, Error> {
    document
        .field(key)
        .map_err(|error| Error::new(error.to_string()))?
        .and_then(|value| value.string_value())
        .ok_or_else(|| Error::new(format!("built-in declaration missing {key}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::rc::Rc;

    #[test]
    // req:builtins-001
    fn kith_urls_are_registered_cached_and_not_fetched_or_locked() {
        let root =
            std::env::temp_dir().join(format!("kith-builtin-resolver-{}", std::process::id()));
        let out = root.join("bin");
        std::fs::create_dir_all(&out).unwrap();
        let mut resolver = Resolver::new(root.clone(), out).unwrap();
        install(&mut resolver).unwrap();
        let url = Url::parse("kith:data").unwrap();
        let first = resolver.load_declaration(&url).unwrap();
        let second = resolver.load_declaration(&url).unwrap();
        assert!(Rc::ptr_eq(&first, &second));
        assert!(resolver.load_path(&url).is_err());
        assert!(resolver
            .load_declaration(&Url::parse("kith:net").unwrap())
            .is_err());
        let mut rewritten = BTreeMap::new();
        rewritten.insert("@scope/name".into(), "kith:data".into());
        assert!(resolver.set_import_map(rewritten).is_err());
        let names = export_map(&first).unwrap();
        assert_eq!(names.get("Bytes").map(String::as_str), Some("rule"));
        assert_eq!(names.get("bytes").map(String::as_str), Some("func"));
        assert!(!names.contains_key("Pipe"));
        let mut imports = BTreeMap::new();
        imports.insert(
            "@justinmchase/uffda".into(),
            "jsr:@justinmchase/uffda@^0.10.8".into(),
        );
        resolver.set_import_map(imports).unwrap();
        let mapped = resolver.load_declaration(&url).unwrap();
        assert_eq!(
            export_map(&mapped)
                .unwrap()
                .get("Bytes")
                .map(String::as_str),
            Some("rule")
        );
        assert!(
            !directory_has_lock(&root),
            "built-in resolution must not write a lockfile"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    fn directory_has_lock(root: &PathBuf) -> bool {
        let mut pending = vec![root.clone()];
        while let Some(dir) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == "kith.lock" || name == "uffda.lock")
                {
                    return true;
                }
            }
        }
        false
    }
}
