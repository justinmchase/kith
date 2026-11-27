//! Current documents by key. A save writes only when the named version is
//! the one still stored. The map is the state. Saves are not appended.

use crate::doc::{self, Doc};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
struct Record {
    version: u64,
    value: Doc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Store {
    documents: BTreeMap<String, Record>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            documents: BTreeMap::new(),
        }
    }
}

pub fn step(mut store: Store, message: &Doc) -> (Store, Vec<Doc>) {
    let reply = match command(message) {
        Command::Load(key) => match store.documents.get(&key) {
            Some(record) => document(&key, record),
            None => tagged("Missing", &key),
        },
        Command::Save {
            key,
            expected,
            value,
        } => store.save(key, expected, value),
        Command::Error(reason) => error(reason),
    };
    (store, vec![reply])
}

impl Store {
    fn save(&mut self, key: String, expected: Option<u64>, value: Doc) -> Doc {
        let current = self.documents.get(&key).map(|record| record.version);
        if current != expected {
            return conflict(&key, current);
        }
        let version = current.unwrap_or(0).saturating_add(1);
        self.documents
            .insert(key.clone(), Record { version, value });
        doc::array([
            doc::string("Saved"),
            doc::string(&key),
            doc::number(version as f64),
        ])
    }
}

enum Command {
    Load(String),
    Save {
        key: String,
        expected: Option<u64>,
        value: Doc,
    },
    Error(&'static str),
}

fn command(message: &Doc) -> Command {
    let (name, args) = match head(message) {
        Some(parts) => parts,
        None => return Command::Error("unknown"),
    };
    match name.as_str() {
        "Load" => {
            if args.len() != 1 {
                return Command::Error("shape");
            }
            match doc::as_str(&args[0]) {
                Some(key) => Command::Load(key),
                None => Command::Error("key"),
            }
        }
        "Save" => {
            if args.len() != 3 {
                return Command::Error("shape");
            }
            let Some(key) = doc::as_str(&args[0]) else {
                return Command::Error("key");
            };
            let expected = if doc::is_null(&args[1]) {
                None
            } else {
                match positive_version(&args[1]) {
                    Some(version) => Some(version),
                    None => return Command::Error("version"),
                }
            };
            Command::Save {
                key,
                expected,
                value: args[2].clone(),
            }
        }
        _ => Command::Error("unknown"),
    }
}

fn head(message: &Doc) -> Option<(String, Vec<Doc>)> {
    if let Some(name) = doc::as_str(message) {
        return Some((name, Vec::new()));
    }
    let items = doc::items(message)?;
    let name = doc::as_str(items.first()?)?;
    Some((name, items.into_iter().skip(1).collect()))
}

fn positive_version(value: &Doc) -> Option<u64> {
    let version = doc::as_u64(value)?;
    (version > 0).then_some(version)
}

fn document(key: &str, record: &Record) -> Doc {
    doc::array([
        doc::string("Document"),
        doc::string(key),
        doc::number(record.version as f64),
        record.value.clone(),
    ])
}

fn tagged(tag: &str, key: &str) -> Doc {
    doc::array([doc::string(tag), doc::string(key)])
}

fn conflict(key: &str, version: Option<u64>) -> Doc {
    doc::array([
        doc::string("Conflict"),
        doc::string(key),
        version
            .map(|version| doc::number(version as f64))
            .unwrap_or_else(doc::null),
    ])
}

fn error(reason: &str) -> Doc {
    doc::array([doc::string("Error"), doc::string(reason)])
}

pub fn to_document(store: &Store) -> Doc {
    let documents = store.documents.iter().map(|(key, record)| {
        doc::object([
            ("key", doc::string(key)),
            ("version", doc::number(record.version as f64)),
            ("value", record.value.clone()),
        ])
    });
    doc::object([
        ("kind", doc::string("store")),
        ("documents", doc::array(documents)),
    ])
}

pub fn from_document(value: &Doc) -> Option<Store> {
    if doc::as_str(&doc::get(value, "kind")?).as_deref() != Some("store") {
        return None;
    }
    let mut documents = BTreeMap::new();
    for entry in doc::items(&doc::get(value, "documents")?)? {
        let key = doc::as_str(&doc::get(&entry, "key")?)?;
        let version = doc::as_u64(&doc::get(&entry, "version")?)?;
        let stored = doc::get(&entry, "value")?;
        documents.insert(
            key,
            Record {
                version,
                value: stored,
            },
        );
    }
    Some(Store { documents })
}
