use kith::parse::{parse_source, resolve_program};
use kith::World;
use serde_json::json;
use std::path::PathBuf;

const PING: &str = include_str!("../examples/ping.kith");

#[test]
// req:grammar-001 req:components-012 req:components-017
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
    assert!(component_names.contains(&"Ping"));
    let child = program.component("Child").expect("Child");
    assert_eq!(child.states[0].handlers[0].route, ["down"]);
    assert_eq!(child.states[0].handlers[0].pattern["kind"], "then");
    assert_eq!(
        child.states[0].handlers[0].pattern["patterns"]
            .as_array()
            .expect("then")
            .len(),
        2
    );
    let ping = program.component("Ping").expect("Ping");
    assert_eq!(ping.states[0].handlers[0].route, ["down"]);
    assert_eq!(ping.states[0].handlers[1].route, ["up"]);
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
    world.inject(json!(["Ping", 7])).unwrap();
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
  start {
    on any {
      complete { kind: "X" };
    }
  }
}

component Other {
  start {
    on (any) {
      complete { kind: "X" };
    }
  }
}

component Parent {
  Child child;

  start {
    on { kind: "To" } {
      send to child { kind: "X" };
    }
    on { kind: "Kind" } {
      send down Child { kind: "X" };
    }
    on { kind: "All" } {
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
    let parent = program.component("Parent").expect("Parent");
    match &parent.states[0].handlers[0].body[0] {
        kith::ast::Statement::SendTo { port, .. } => assert_eq!(port, "child"),
        other => panic!("expected sendTo, got {other:?}"),
    }
    match &parent.states[0].handlers[1].body[0] {
        kith::ast::Statement::SendDown { component, .. } => {
            assert_eq!(component.as_deref(), Some("Child"))
        }
        other => panic!("expected sendDown Child, got {other:?}"),
    }
    let children = &program.scene("Main").expect("Main").root.children;
    assert_eq!(children[0].key.as_deref(), Some("child"));
    assert_eq!(children[1].key, None);
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
    world.inject(json!(["Ping", 7])).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(7.0)));

    let dir = std::env::temp_dir().join(format!("kith-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("child.kith"),
        "export Other;\ncomponent Other {\n  start {}\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.kith"),
        "import \"./child.kith\" Child;\nexport Main;\nworkflow Box {\n  start succeed;\n}\nexport scene Main {\n  Box()\n}\n",
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
    let pattern = import.steps[0].pattern.as_ref().expect("Start pattern");
    assert_eq!(pattern["kind"], "over");
    assert!(pattern["keys"].get("items").is_some());
    assert!(matches!(
        import.steps[0].body,
        kith::ast::StepBody::Map { ref component, ref reduce, .. }
            if component == "Worker" && reduce == "Count"
    ));
    let count = program.component("Count").expect("Count");
    assert!(count.fields.iter().any(|field| field.name == "total"));
    let fold = &count.states[0].handlers[0];
    assert_eq!(fold.pattern["kind"], "variable");
    assert_eq!(fold.pattern["name"], "item");
    assert!(fold.item.is_none());
    let last = count.states[0].handlers.last().expect("else");
    assert_eq!(last.pattern["kind"], "any");
    World::check(&program).expect("import checks");
}

#[test]
// req:grammar-001 req:workflows-005 req:workflows-008 req:components-008 req:components-009 req:components-010 req:components-011 req:dispatch-003 req:runtime-001 req:components-012
fn order_source_parses_tasks_picks_and_map() {
    let program = parse_source(include_str!("../examples/order.kith")).expect("parse order.kith");
    let order = program.workflow("Order").expect("Order");
    assert!(order.steps.iter().any(|step| step.start
        && matches!(
            step.body,
            kith::ast::StepBody::Task { ref component } if component == "ValidateOrder"
        )));
    assert!(order
        .steps
        .iter()
        .any(|step| matches!(step.body, kith::ast::StepBody::Choice { .. })));
    let fulfill = order
        .steps
        .iter()
        .find(|step| step.name == "Fulfill")
        .expect("Fulfill");
    let pattern = fulfill.pattern.as_ref().expect("Fulfill pattern");
    assert_eq!(pattern["kind"], "over");
    assert!(pattern["keys"].get("items").is_some());
    assert!(pattern["keys"].get("orderId").is_some());
    assert!(matches!(
        &fulfill.body,
        kith::ast::StepBody::Map { component, reduce, init, .. }
            if component == "Carrier"
                && reduce == "Summarize"
                && init.is_some()
    ));
    let summarize = program.component("Summarize").expect("Summarize");
    assert!(summarize.fields.iter().any(|field| field.name == "total"));
    assert!(summarize.fields.iter().any(|field| field.name == "orderId"));
    assert_eq!(summarize.states[0].name, "Start");
    assert!(!summarize.states[0].named);
    let fold = &summarize.states[0].handlers[0];
    assert_eq!(fold.pattern["kind"], "variable");
    assert_eq!(fold.pattern["name"], "item");
    assert_eq!(fold.route, ["up", "down", "self"]);
    assert!(fold.item.is_none());
    let done = summarize.states[0].handlers.last().expect("else");
    assert_eq!(done.pattern["kind"], "any");
    assert_eq!(done.route, ["up", "down", "self"]);
    assert!(matches!(
        done.body.last(),
        Some(kith::ast::Statement::Complete { .. })
    ));
    let validate = program.component("ValidateOrder").expect("ValidateOrder");
    let last = validate.states[0].handlers.last().expect("else");
    assert_eq!(last.pattern["kind"], "any");
    let carrier = program.component("Carrier").expect("Carrier");
    assert!(carrier
        .ports
        .iter()
        .any(|port| port.name == "ship" && port.component == "MockShip"));
    assert!(carrier.states.iter().any(|state| state.name == "Shipping"));
    assert!(matches!(
        carrier.states[0].handlers[0].body.last(),
        Some(kith::ast::Statement::Goto { state }) if state == "Shipping"
    ));
    let mock = program.component("MockShip").expect("MockShip");
    assert_eq!(mock.states[0].handlers[0].route, ["down"]);
    assert!(matches!(
        &mock.states[0].handlers[0].body.last(),
        Some(kith::ast::Statement::SendUp { after: Some(delay), .. })
            if delay["kind"] == "invocation"
                && delay["expression"]["name"] == "rand"
    ));
    assert_eq!(carrier.states[0].handlers[0].route, ["down"]);
    let shipping = carrier
        .states
        .iter()
        .find(|state| state.name == "Shipping")
        .expect("Shipping");
    assert_eq!(shipping.handlers[0].route, ["up"]);
    let scene = &program.scene("Main").expect("Main").root;
    assert!(scene.children.iter().any(|child| {
        child.component == "Carrier"
            && child.children.iter().any(|nested| {
                nested.key.as_deref() == Some("ship") && nested.component == "MockShip"
            })
    }));
    World::check(&program).expect("order checks");
}

#[test]
// req:grammar-001 req:runtime-002 req:components-012 req:runtime-003 req:components-014 req:components-015 req:components-016 req:components-017
fn http_source_parses_client_and_mock() {
    let program = parse_source(include_str!("../examples/http.kith")).expect("parse http.kith");
    let client = program.component("Client").expect("Client");
    assert_eq!(client.states[0].name, "Idle");
    assert!(client.states[0].named);
    assert_eq!(client.states[0].handlers[0].route, ["down"]);
    let sending = client
        .states
        .iter()
        .find(|state| state.name == "Sending")
        .expect("Sending");
    assert_eq!(sending.handlers[0].route, ["up"]);
    assert!(matches!(
        sending.handlers[0].body.last(),
        Some(kith::ast::Statement::Goto { state }) if state == "Idle"
    ));
    let http = program.component("Http").expect("Http");
    assert!(!http.states[0].named);
    assert_eq!(http.states[0].name, "Start");
    assert_eq!(http.states[0].handlers[0].route, ["down"]);
    let health = program.component("HealthCheck").expect("HealthCheck");
    assert_eq!(health.states[0].name, "Checking");
    assert!(health.states[0].named);
    assert_eq!(health.states[0].handlers[0].route, ["init"]);
    assert_eq!(health.states[0].handlers[1].route, ["self"]);
    assert!(matches!(
        &health.states[0].handlers[1].body[0],
        kith::ast::Statement::SendTo { .. }
    ));
    let validating = health
        .states
        .iter()
        .find(|state| state.name == "Validating")
        .expect("Validating");
    assert!(matches!(
        &validating.handlers[0].body[1],
        kith::ast::Statement::Send { after: Some(delay), .. }
            if delay["kind"] == "number" && delay["value"] == 5.0
    ));
    assert!(matches!(
        validating.handlers[0].body.last(),
        Some(kith::ast::Statement::Goto { state }) if state == "Checking"
    ));
    World::check(&program).expect("http checks");
}

#[test]
// req:components-015
fn send_without_a_route_fails_parse() {
    let err = parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send { kind: "Tick" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect_err("send requires a route");
    assert!(!err.message.is_empty());
    let delayed = parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send after 5 { kind: "Tick" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect_err("send after requires a route");
    assert!(!delayed.message.is_empty());
}

#[test]
// req:components-012
fn on_route_or_list_parses() {
    let program = parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on up or down { kind: "Tick" } {}
    on { kind: "Check" } {}
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse or list");
    let box_kind = program.component("Box").expect("Box");
    assert_eq!(box_kind.states[0].handlers[0].route, ["up", "down"]);
    assert_eq!(box_kind.states[0].handlers[1].route, ["up", "down", "self"]);
    World::check(&program).expect("or list checks");
}

#[test]
// req:components-017 req:components-019 req:grammar-001
fn message_sequences_and_object_patterns_parse() {
    let program = parse_source(
        r#"
export Box;
export Main;
component Box {
  field s1: any = 200;
  field s2: any = 200;
  field s3: any = 200;
  start {
    on self "Check" a:200 b:200 c:200 {
      send self "Check" s1 s2 s3;
    }
    on down { kind: "GET" url: any } {
      send up { kind: "Done" status: 200 };
    }
    on up v:{ kind: "Done" status: number } {
      (writeLine stdout v);
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse sequences");
    let box_kind = program.component("Box").expect("Box");
    let handlers = &box_kind.states[0].handlers;
    assert_eq!(handlers[0].pattern["kind"], "then");
    assert_eq!(
        handlers[0].pattern["patterns"]
            .as_array()
            .expect("then")
            .len(),
        4
    );
    match &handlers[0].body[0] {
        kith::ast::Statement::Send { message, .. } => {
            assert_eq!(message["kind"], "sequence");
            assert_eq!(message["items"].as_array().expect("items").len(), 4);
        }
        other => panic!("expected send, got {other:?}"),
    }
    assert_eq!(handlers[1].pattern["kind"], "over");
    assert!(handlers[1].pattern["keys"].get("kind").is_some());
    assert!(handlers[1].pattern["keys"].get("url").is_some());
    assert_eq!(handlers[2].pattern["kind"], "variable");
    assert_eq!(handlers[2].pattern["name"], "v");
    assert_eq!(handlers[2].pattern["pattern"]["kind"], "over");
    match &handlers[2].body[0] {
        kith::ast::Statement::Expr { expression } => {
            assert_eq!(expression["kind"], "invocation");
            assert_eq!(expression["expression"]["name"], "writeLine");
        }
        other => panic!("expected expr, got {other:?}"),
    }
    World::check(&program).expect("sequence program checks");
}

#[test]
// req:components-019 req:grammar-001
fn expression_statements_parse_and_print_is_not_a_statement() {
    let program = parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      (writeLine stdout "hi");
      (print "hi");
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse expr stmts");
    let body = &program.component("Box").expect("Box").states[0].handlers[0].body;
    assert!(matches!(body[0], kith::ast::Statement::Expr { .. }));
    assert!(matches!(body[1], kith::ast::Statement::Expr { .. }));
    World::check(&program).expect("expr stmts check");

    let err = parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      print "hi";
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect_err("print statement is gone");
    assert!(!err.to_string().is_empty());
}
