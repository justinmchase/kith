use crate::ast::{Component, Program, Scene, Workflow};
use crate::error::Error;
use crate::json_util::from_uffda_value;
use serde_json::Value as Json;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use uffda_runtime::module::CompiledModuleGraph;
use uffda_runtime::pattern::MatchKind;
use uffda_runtime::resolver::packages::{JsrPackages, Lockfile};
use uffda_runtime::resolver::Resolver;
use uffda_runtime::runtime::{InputMode, Runtime};
use url::Url;

include!(concat!(env!("OUT_DIR"), "/languages.rs"));

#[derive(Clone)]
struct Language {
    graph: Rc<CompiledModuleGraph>,
    entry: Url,
}

thread_local! {
    static LANGUAGE: RefCell<Option<Result<Language, String>>> = const { RefCell::new(None) };
}

fn language() -> Result<Language, Error> {
    LANGUAGE.with(|slot| {
        if slot.borrow().is_none() {
            *slot.borrow_mut() = Some(load_language());
        }
        match slot.borrow().as_ref() {
            Some(Ok(language)) => Ok(language.clone()),
            Some(Err(error)) => Err(Error::new(error.clone())),
            None => Err(Error::new("Kith language failed to load")),
        }
    })
}

fn load_language() -> Result<Language, String> {
    let mut resolver = Resolver::from_cwd().map_err(|error| error.to_string())?;
    let root = Url::parse("file:///kith/builtin/").map_err(|error| error.to_string())?;
    if DECLARATIONS.is_empty() {
        return Err("embedded Kith grammar is missing".into());
    }
    for (path, source) in DECLARATIONS {
        let url = root.join(path).map_err(|error| error.to_string())?;
        resolver.insert_declaration_codec_source(url, source);
    }
    let lockfile = lockfile();
    let packages = JsrPackages::standard(lockfile).map_err(|error| error.to_string())?;
    resolver.set_packages(packages);
    let mut imports = BTreeMap::new();
    imports.insert(
        "@justinmchase/uffda".into(),
        "jsr:@justinmchase/uffda@^0.10.8".into(),
    );
    resolver
        .set_import_map(imports)
        .map_err(|error| error.to_string())?;
    let path = DECLARATIONS
        .iter()
        .map(|(path, _)| *path)
        .find(|path| path.ends_with("kith.lang.uff"))
        .ok_or_else(|| "embedded Kith language module is missing".to_string())?;
    let entry = root.join(path).map_err(|error| error.to_string())?;
    let graph = resolver
        .load_graph(&entry)
        .map_err(|error| error.to_string())?;
    let graph = Rc::new(CompiledModuleGraph::compile(graph).map_err(|error| error.to_string())?);
    Ok(Language { graph, entry })
}

fn lockfile() -> Lockfile {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("uffda.lock");
    Lockfile::load(manifest).unwrap_or_else(|_| Lockfile::memory())
}

pub fn parse_source(source: &str) -> Result<Program, Error> {
    let language = language()?;
    let mut runtime = Runtime::with_mode(
        Rc::clone(&language.graph),
        &language.entry,
        Some("KithLang"),
        serde_json::json!(source),
        InputMode::Scalar,
    )
    .map_err(|error| Error::new(error.to_string()))?;
    runtime
        .run()
        .map_err(|error| Error::new(error.to_string()))?;
    let failed = runtime.read(|execution| {
        let Some(matched) = execution.result() else {
            return Some("Kith parse produced no result".to_string());
        };
        if matched.kind == MatchKind::Ok {
            None
        } else {
            let diagnostics = execution.diagnostics();
            Some(if diagnostics.is_empty() {
                "Kith source did not match the grammar".to_string()
            } else {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
    });
    if let Some(message) = failed {
        return Err(Error::new(message));
    }
    if let Ok(Some(serialized)) = runtime.serialize_result() {
        let tree: Json =
            serde_json::from_str(&serialized).map_err(|error| Error::new(error.to_string()))?;
        return serde_json::from_value(tree).map_err(|error| Error::new(error.to_string()));
    }
    let tree = runtime.read(|execution| {
        let matched = execution
            .result()
            .ok_or_else(|| Error::new("Kith parse produced no result"))?;
        from_uffda_value(matched.value)
    })?;
    serde_json::from_value(tree).map_err(|error| Error::new(error.to_string()))
}

pub fn parse_file(path: &Path) -> Result<Program, Error> {
    let source = std::fs::read_to_string(path).map_err(|error| Error::new(error.to_string()))?;
    parse_source(&source)
}

pub fn load_program(path: &Path) -> Result<Program, Error> {
    if looks_like_artifact(path) {
        load_artifact(path)
    } else {
        resolve_program(path)
    }
}

pub fn resolve_program(path: &Path) -> Result<Program, Error> {
    let mut visited = HashSet::new();
    load_module(path, &mut visited)
}

fn load_module(path: &Path, visited: &mut HashSet<PathBuf>) -> Result<Program, Error> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return Err(Error::new(format!("import cycle at {}", path.display())));
    }
    let mut program = parse_file(path)?;
    let imports = program.imports.clone();
    for import in imports {
        if import.module_url.contains(':') && !import.module_url.starts_with('.') {
            return Err(Error::new(format!(
                "unsupported import specifier {}",
                import.module_url
            )));
        }
        let target = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&import.module_url);
        let dep = load_module(&target, visited)?;
        let exported: HashSet<String> = dep.exported_names().map(str::to_string).collect();
        for name in &import.names {
            if !exported.contains(name) {
                return Err(Error::new(format!(
                    "{name} is not exported by {}",
                    import.module_url
                )));
            }
            if let Some(component) = dep.component(name).cloned() {
                merge_component(&mut program, component);
            } else if let Some(workflow) = dep.workflow(name).cloned() {
                merge_workflow(&mut program, workflow);
            } else if let Some(scene) = dep.scene(name).cloned() {
                merge_scene(&mut program, scene);
            } else {
                return Err(Error::new(format!(
                    "{name} is exported by {} but is not a component, workflow, or scene",
                    import.module_url
                )));
            }
        }
    }
    Ok(program)
}

fn merge_component(program: &mut Program, component: Component) {
    if program.component(&component.name).is_none() {
        program.components.push(component);
    }
}

fn merge_workflow(program: &mut Program, workflow: Workflow) {
    if program.workflow(&workflow.name).is_none() {
        program.workflows.push(workflow);
    }
}

fn merge_scene(program: &mut Program, scene: Scene) {
    if program.scene(&scene.name).is_none() {
        program.scenes.push(scene);
    }
}

pub fn looks_like_artifact(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "json")
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".kith.json"))
}

pub fn load_artifact(path: &Path) -> Result<Program, Error> {
    let text = std::fs::read_to_string(path).map_err(|error| Error::new(error.to_string()))?;
    let value: Json = serde_json::from_str(&text).map_err(|error| Error::new(error.to_string()))?;
    if let Some(program) = value.get("program") {
        serde_json::from_value(program.clone()).map_err(|error| Error::new(error.to_string()))
    } else {
        serde_json::from_value(value).map_err(|error| Error::new(error.to_string()))
    }
}

pub fn artifact_document(program: &Program) -> Json {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "program": program,
    })
}
