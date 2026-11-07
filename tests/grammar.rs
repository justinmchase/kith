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
    assert!(names.contains(&"Ping"));
    assert!(names.contains(&"Main"));
    let component_names: Vec<_> = program
        .components
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    assert!(component_names.contains(&"Child"));
    assert!(program.workflow("Ping").is_some());
    let ping = program.workflow("Ping").expect("Ping");
    assert!(ping.fields.iter().any(|field| field.name == "heard"));
    assert!(ping
        .ports
        .iter()
        .any(|port| port.name == "child" && port.component == "Child"));
    let scene = program.scene("Main").expect("Main");
    assert_eq!(scene.root.component, "Ping");
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
// req:grammar-001 req:components-003
fn mixed_unnamed_child_and_send_down_kind_parse() {
    let source = r#"
export Child;
export Other;
export Parent;
export Main;

component Child {
  start Idle {
    on (any) {}
  }
}

component Other {
  start Idle {
    on (any) {}
  }
}

workflow Parent {
  Child child;

  start Idle {
    on ({ kind: "To" }) {
      send to child { kind: "X" };
    }
    on ({ kind: "Kind" }) {
      send down Child { kind: "X" };
    }
    on ({ kind: "All" }) {
      send down any { kind: "X" };
    }
  }
}

export scene Main {
  Parent() {
    child = Child();
    Other();
  }
}
"#;
    let program = parse_source(source).expect("parse mixed");
    let parent = program.workflow("Parent").expect("Parent");
    let ping = parent.steps[0].handlers[0].body[0].clone();
    let down = parent.steps[0].handlers[1].body[0].clone();
    let any = parent.steps[0].handlers[2].body[0].clone();
    match ping {
        kith::ast::Statement::SendTo { ref port, .. } => assert_eq!(port, "child"),
        other => panic!("expected sendTo, got {other:?}"),
    }
    match down {
        kith::ast::Statement::SendDown { ref component, .. } => {
            assert_eq!(component.as_deref(), Some("Child"))
        }
        other => panic!("expected sendDown Child, got {other:?}"),
    }
    match any {
        kith::ast::Statement::SendDown { ref component, .. } => assert!(component.is_none()),
        other => panic!("expected sendDown any, got {other:?}"),
    }
    let children = &program.scene("Main").expect("Main").root.children;
    assert_eq!(children[0].key.as_deref(), Some("child"));
    assert_eq!(children[1].key, None);
    assert_eq!(children[1].component, "Other");
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
        "import \"./child.kith\" Child;\nexport Main;\nworkflow Box {\n  start Idle {}\n}\nexport scene Main {\n  Box()\n}\n",
    )
    .unwrap();
    let err = resolve_program(&dir.join("main.kith")).expect_err("missing export");
    assert!(err.message.contains("not exported"));
}

#[test]
// req:grammar-001 req:workflows-003
fn import_source_parses_fanout_and_join() {
    let source = include_str!("../examples/import.kith");
    let program = parse_source(source).expect("parse import.kith");
    let import = program.workflow("Import").expect("Import");
    assert!(matches!(
        import.steps[0].handlers[0].body[0],
        kith::ast::Statement::Fanout { .. }
    ));
    assert_eq!(
        import.steps[0].join.as_ref().map(|join| join.kind.as_str()),
        Some("all")
    );
    assert!(!import.steps[1].start);
    World::check(&program).expect("import checks");
}
