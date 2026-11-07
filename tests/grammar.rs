use kith::parse::{parse_source, resolve_program};
use kith::World;
use serde_json::json;
use std::path::PathBuf;

const PING: &str = include_str!("../examples/ping.kith");

#[test]
// req:grammar-001
fn ping_source_parses_to_the_component_tree() {
    let program = parse_source(PING).expect("parse ping.kith");
    assert_eq!(program.kind, "program");
    let names: Vec<_> = program.exported_names().collect();
    assert!(names.contains(&"Child"));
    assert!(names.contains(&"Parent"));
    assert!(names.contains(&"Main"));
    let component_names: Vec<_> = program
        .components
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    assert!(component_names.contains(&"Child"));
    assert!(component_names.contains(&"Parent"));
    let parent = program.component("Parent").expect("Parent");
    assert!(parent.fields.iter().any(|field| field.name == "heard"));
    let scene = program.scene("Main").expect("Main");
    assert_eq!(scene.root.component, "Parent");
    assert!(scene.root.key.is_none());
    assert!(scene
        .root
        .children
        .iter()
        .any(|child| child.key.as_deref() == Some("child") && child.component == "Child"));

    let mut world = World::start(program).expect("interpreter accepts parsed tree");
    world.inject(json!({"kind": "Ping", "n": 7})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(7.0)));
}

#[test]
// req:grammar-002
fn import_list_parses_and_relative_files_resolve() {
    let source = include_str!("fixtures/uses_child.kith");
    let parsed = parse_source(source).expect("parse uses_child");
    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].module_url, "./child.kith");
    assert_eq!(parsed.imports[0].names, vec!["Child"]);
    assert!(parsed.component("Child").is_none());

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/uses_child.kith");
    let program = resolve_program(&path).expect("resolve import");
    assert!(program.component("Child").is_some());
    let mut world = World::start(program).expect("start imported child");
    world.inject(json!({"kind": "Ping", "n": 7})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(7.0)));

    let dir = std::env::temp_dir().join(format!("kith-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("child.kith"),
        "export Other;\ncomponent Other {\n  start Idle {}\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.kith"),
        "import \"./child.kith\" Child;\nexport Main;\ncomponent Box {\n  start Idle {}\n}\nexport scene Main {\n  Box()\n}\n",
    )
    .unwrap();
    let err = resolve_program(&dir.join("main.kith")).expect_err("missing export");
    assert!(err.message.contains("not exported"));
}
