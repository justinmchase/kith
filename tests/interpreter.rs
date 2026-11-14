use kith::ast::{
    ping_program, ChoiceClause, Component, Export, Field, FlowStep, Handler, Instance, Next, Port,
    Program, Scene, State, Statement, StepBody, Workflow,
};
use kith::interpreter::World;
use serde_json::json;

fn export(name: &str) -> Export {
    Export {
        kind: "export".into(),
        name: name.into(),
    }
}

fn port(component: &str, name: &str) -> Port {
    Port {
        kind: "port".into(),
        component: component.into(),
        name: name.into(),
    }
}

fn idle(name: &str, ports: Vec<Port>, handlers: Vec<Handler>) -> Component {
    Component {
        kind: "component".into(),
        name: name.into(),
        fields: vec![],
        ports,
        states: vec![State {
            kind: "state".into(),
            name: "Idle".into(),
            start: true,
            named: true,
            handlers,
        }],
    }
}

fn task_flow(name: &str, component: &str) -> Workflow {
    Workflow {
        kind: "workflow".into(),
        name: name.into(),
        start: "Start".into(),
        steps: vec![FlowStep {
            name: "Start".into(),
            start: true,
            pattern: None,
            body: StepBody::Task {
                component: component.into(),
            },
            next: Some(Next::Succeed {}),
        }],
    }
}

fn map_flow(name: &str, items: &str, component: &str, reduce: &str) -> Workflow {
    Workflow {
        kind: "workflow".into(),
        name: name.into(),
        start: "Start".into(),
        steps: vec![FlowStep {
            name: "Start".into(),
            start: true,
            pattern: None,
            body: StepBody::Map {
                items: json!({"kind": "reference", "name": items}),
                component: component.into(),
                reduce: reduce.into(),
                init: None,
            },
            next: Some(Next::Succeed {}),
        }],
    }
}

fn counting(name: &str) -> Component {
    Component {
        kind: "component".into(),
        name: name.into(),
        fields: vec![Field {
            kind: "field".into(),
            name: "seen".into(),
            pattern: None,
            init: Some(json!({"kind": "number", "value": 0})),
        }],
        ports: vec![],
        states: vec![State {
            kind: "state".into(),
            name: "Idle".into(),
            start: true,
            named: true,
            handlers: vec![Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: json!({"kind": "any"}),
                body: vec![Statement::Assign {
                    field: "seen".into(),
                    expression: json!({"kind": "number", "value": 1}),
                }],
            }],
        }],
    }
}

fn instance(component: &str, key: Option<&str>, children: Vec<Instance>) -> Instance {
    Instance {
        kind: "instance".into(),
        component: component.into(),
        key: key.map(str::to_string),
        children,
    }
}

fn scene_program(workflows: Vec<Workflow>, components: Vec<Component>, root: Instance) -> Program {
    let mut exports: Vec<Export> = workflows
        .iter()
        .map(|workflow| export(&workflow.name))
        .collect();
    exports.extend(components.iter().map(|component| export(&component.name)));
    exports.push(export("Main"));
    Program {
        kind: "program".into(),
        imports: vec![],
        exports,
        components,
        workflows,
        scenes: vec![Scene {
            kind: "scene".into(),
            name: "Main".into(),
            root,
        }],
    }
}

fn kind_eq(kind: &str) -> serde_json::Value {
    json!({
        "kind": "over",
        "keys": {
            "kind": {
                "kind": "equal",
                "value": {"kind": "value.literal", "value": kind}
            }
        }
    })
}

fn string_msg() -> serde_json::Value {
    json!({"kind": "string", "values": ["x"]})
}

fn object_kind(kind: &str) -> serde_json::Value {
    json!({
        "kind": "object",
        "keys": [{
            "kind": "objectKey",
            "name": "kind",
            "expression": {"kind": "string", "values": [kind]}
        }]
    })
}

fn mixed_parent_program(handlers: Vec<Handler>) -> Program {
    scene_program(
        vec![],
        vec![
            idle("Parent", vec![port("Child", "child")], handlers),
            counting("Child"),
            counting("Other"),
        ],
        instance(
            "Parent",
            None,
            vec![
                instance("Child", Some("child"), vec![]),
                instance("Other", None, vec![]),
            ],
        ),
    )
}

#[test]
// req:components-001 req:scenes-001
fn parent_owns_scene_constructed_children() {
    let mut world = World::start(ping_program()).expect("start");
    world.inject(json!(["Ping", 1])).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.kind(world.root_id()), Some("Ping"));
    assert_eq!(world.children_of_kind(world.root_id(), "Child").len(), 1);

    let program = mixed_parent_program(vec![]);
    let mut world = World::start(program).expect("mixed start");
    world.inject(json!({"kind": "Nope"})).ok();
    world.run_until_idle().ok();
    let parent = world.root_id();
    assert!(world.child(parent, "child").is_some());
    assert_eq!(world.children_of_kind(parent, "Other").len(), 1);
}

#[test]
// req:scenes-001 req:scenes-003
fn check_rejects_unknown_kind_duplicate_missing_and_wrong_port() {
    let mut unknown = mixed_parent_program(vec![]);
    unknown.scenes[0].root.children[0].component = "Nope".into();
    assert!(World::check(&unknown).is_err());

    let mut duplicate = mixed_parent_program(vec![]);
    duplicate.scenes[0]
        .root
        .children
        .push(instance("Child", Some("child"), vec![]));
    let err = World::check(&duplicate).expect_err("duplicate port");
    assert!(err.message.contains("duplicate port fill"));

    let mut missing = mixed_parent_program(vec![]);
    missing.scenes[0].root.children.clear();
    let err = World::check(&missing).expect_err("missing port");
    assert!(err.message.contains("missing port fill"));

    let mut extra = mixed_parent_program(vec![]);
    extra.scenes[0]
        .root
        .children
        .push(instance("Child", Some("other"), vec![]));
    let err = World::check(&extra).expect_err("unknown port");
    assert!(err.message.contains("unknown port"));

    let mut wrong = mixed_parent_program(vec![]);
    wrong.components.push(idle("OtherKind", vec![], vec![]));
    wrong.scenes[0].root.children[0].component = "OtherKind".into();
    let err = World::check(&wrong).expect_err("wrong kind");
    assert!(err.message.contains("expects Child"));
}

#[test]
// req:scenes-002
fn start_requires_a_unique_exported_scene() {
    World::start(ping_program()).expect("ping exports Main");
    let mut none = ping_program();
    none.exports.retain(|export| export.name != "Main");
    assert!(World::check(&none).is_err());

    let mut two = ping_program();
    two.scenes.push(Scene {
        kind: "scene".into(),
        name: "Other".into(),
        root: instance("Ping", None, vec![]),
    });
    two.exports.push(export("Other"));
    let err = World::check(&two).expect_err("two scenes");
    assert!(err.message.contains("more than one scene"));
}

#[test]
// req:components-002 req:workflows-001
fn check_requires_exactly_one_start_state() {
    World::check(&ping_program()).expect("valid program");
    let mut none = ping_program();
    none.components[0].states[0].start = false;
    assert!(World::check(&none).is_err());

    let mut two = ping_program();
    two.workflows.push(Workflow {
        kind: "workflow".into(),
        name: "Flow".into(),
        start: "Start".into(),
        steps: vec![
            FlowStep {
                name: "Start".into(),
                start: true,
                pattern: None,
                body: StepBody::Succeed {},
                next: None,
            },
            FlowStep {
                name: "Other".into(),
                start: true,
                pattern: None,
                body: StepBody::Succeed {},
                next: None,
            },
        ],
    });
    assert!(World::check(&two).is_err());
}

#[test]
// req:components-009
fn check_rejects_goto_of_an_unknown_state() {
    let mut program = ping_program();
    program.components[0].states[0].handlers[0]
        .body
        .push(Statement::Goto {
            state: "Nope".into(),
        });
    let err = World::check(&program).expect_err("unknown state");
    assert!(err.message.contains("unknown state Nope"));
}

#[test]
// req:components-009
fn check_rejects_goto_of_unnamed_start() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on down any {
      goto Start;
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse unnamed start goto");
    let err = World::check(&program).expect_err("unnamed start is not a goto target");
    assert!(err.message.contains("unknown state Start"));
}

#[test]
// req:components-009
fn delivered_while_waiting_is_unmatched() {
    let program = kith::parse::parse_source(
        r#"
export Carrier;
export Main;
component Carrier {
  start Waiting {
    on { kind: "Item", item: any, index: number } {
      goto Shipping;
    }
  }
  state Shipping {
    on { kind: "Delivered" } {
      complete { kind: "Shipped" };
    }
  }
}
export scene Main { Carrier(); }
"#,
    )
    .expect("parse carrier");
    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Delivered"})).unwrap();
    let err = world.run_until_idle().expect_err("still waiting");
    assert!(err.message.contains("unmatched"));
}

#[test]
// req:workflows-001
fn check_accepts_a_component_scene_root() {
    let program = scene_program(
        vec![],
        vec![idle("Box", vec![], vec![])],
        instance("Box", None, vec![]),
    );
    World::check(&program).expect("component root");
}

#[test]
// req:components-003
fn routing_is_explicit_and_does_not_capture() {
    let program = mixed_parent_program(vec![]);
    let mut world = World::start(program).expect("start");
    let err = world
        .inject(json!({"kind": "Nope"}))
        .and_then(|_| world.run_until_idle())
        .expect_err("unhandled inject must not be offered to children");
    assert!(err.message.contains("unmatched"));
}

#[test]
// req:components-003
fn send_to_and_send_down_kind_select_named_and_unnamed() {
    let parent = idle(
        "Parent",
        vec![port("Child", "child")],
        vec![
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("To"),
                body: vec![Statement::SendTo {
                    port: "child".into(),
                    message: string_msg(),
                    after: None,
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Kind"),
                body: vec![Statement::SendDown {
                    component: Some("Child".into()),
                    message: string_msg(),
                    after: None,
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("OtherKind"),
                body: vec![Statement::SendDown {
                    component: Some("Other".into()),
                    message: string_msg(),
                    after: None,
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("All"),
                body: vec![Statement::SendDown {
                    component: None,
                    message: string_msg(),
                    after: None,
                }],
            },
        ],
    );
    let program = scene_program(
        vec![],
        vec![parent, counting("Child"), counting("Other")],
        instance(
            "Parent",
            None,
            vec![
                instance("Child", Some("child"), vec![]),
                instance("Other", None, vec![]),
            ],
        ),
    );

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "To"})).unwrap();
    world.run_until_idle().unwrap();
    let parent_id = world.root_id();
    let child = world.child(parent_id, "child").unwrap();
    let sibling = world.children_of_kind(parent_id, "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(0.0)));

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Kind"})).unwrap();
    world.run_until_idle().unwrap();
    let parent_id = world.root_id();
    let child = world.child(parent_id, "child").unwrap();
    let sibling = world.children_of_kind(parent_id, "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(0.0)));

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "OtherKind"})).unwrap();
    world.run_until_idle().unwrap();
    let parent_id = world.root_id();
    let child = world.child(parent_id, "child").unwrap();
    let sibling = world.children_of_kind(parent_id, "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(0.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(1.0)));

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "All"})).unwrap();
    world.run_until_idle().unwrap();
    let parent_id = world.root_id();
    let child = world.child(parent_id, "child").unwrap();
    let sibling = world.children_of_kind(parent_id, "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(1.0)));
}

#[test]
// req:components-004
fn unmatched_send_up_drops_and_unmatched_to_or_down_fails() {
    let child = idle(
        "Child",
        vec![],
        vec![Handler {
            kind: "handler".into(),
            route: vec![],
            item: None,
            pattern: kind_eq("Go"),
            body: vec![Statement::SendUp {
                message: object_kind("Ghost"),
                after: None,
            }],
        }],
    );
    let parent = idle(
        "Parent",
        vec![port("Child", "child")],
        vec![
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Go"),
                body: vec![Statement::SendTo {
                    port: "child".into(),
                    message: object_kind("Go"),
                    after: None,
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Miss"),
                body: vec![Statement::SendTo {
                    port: "child".into(),
                    message: object_kind("Miss"),
                    after: None,
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("None"),
                body: vec![Statement::SendDown {
                    component: Some("Ghost".into()),
                    message: string_msg(),
                    after: None,
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Missing"),
                body: vec![Statement::SendTo {
                    port: "missing".into(),
                    message: string_msg(),
                    after: None,
                }],
            },
        ],
    );
    let program = scene_program(
        vec![],
        vec![parent, child],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Go"})).unwrap();
    world.run_until_idle().expect("unmatched send up drops");

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Miss"})).unwrap();
    let err = world.run_until_idle().expect_err("unmatched down fails");
    assert!(err.message.contains("unmatched down"));
    assert!(err.message.contains("Child"));

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "None"})).unwrap();
    let err = world.run_until_idle().expect_err("zero matches fail");
    assert!(err.message.contains("no child matched send down"));

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Missing"})).unwrap();
    let err = world.run_until_idle().expect_err("missing port fails");
    assert!(err.message.contains("unknown port missing"));
}

#[test]
// req:components-005
fn first_matching_handler_consumes_the_message() {
    let one = Component {
        kind: "component".into(),
        name: "One".into(),
        fields: vec![Field {
            kind: "field".into(),
            name: "picked".into(),
            pattern: None,
            init: Some(json!({"kind": "number", "value": 0})),
        }],
        ports: vec![],
        states: vec![State {
            kind: "state".into(),
            name: "Idle".into(),
            start: true,
            named: true,
            handlers: vec![
                Handler {
                    kind: "handler".into(),
                    route: vec![],
                    item: None,
                    pattern: json!({"kind": "any"}),
                    body: vec![Statement::Assign {
                        field: "picked".into(),
                        expression: json!({"kind": "number", "value": 1}),
                    }],
                },
                Handler {
                    kind: "handler".into(),
                    route: vec![],
                    item: None,
                    pattern: json!({"kind": "any"}),
                    body: vec![Statement::Assign {
                        field: "picked".into(),
                        expression: json!({"kind": "number", "value": 2}),
                    }],
                },
            ],
        }],
    };
    let program = scene_program(vec![], vec![one], instance("One", None, vec![]));
    let mut world = World::start(program).expect("start");
    world.inject(json!("go")).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "picked"), Some(&json!(1.0)));
}

#[test]
// req:components-006 req:components-017
fn ping_example_sets_heard_to_seven() {
    let mut world = World::start(ping_program()).expect("start");
    world.inject(json!(["Ping", 7])).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(7.0)));
}

#[test]
// req:workflows-002
fn then_and_pick_transition_with_working_value() {
    let workflow = Workflow {
        kind: "workflow".into(),
        name: "Flow".into(),
        start: "Start".into(),
        steps: vec![
            FlowStep {
                name: "Start".into(),
                start: true,
                pattern: None,
                body: StepBody::Pass {
                    value: json!({
                        "kind": "object",
                        "keys": [{
                            "kind": "objectKey",
                            "name": "ok",
                            "expression": {"kind": "boolean", "value": true}
                        }]
                    }),
                },
                next: Some(Next::Goto {
                    step: "Decide".into(),
                }),
            },
            FlowStep {
                name: "Decide".into(),
                start: false,
                pattern: None,
                body: StepBody::Choice {
                    clauses: vec![
                        ChoiceClause {
                            pattern: kind_eq("unused"),
                            step: "No".into(),
                        },
                        ChoiceClause {
                            pattern: json!({"kind": "any"}),
                            step: "Yes".into(),
                        },
                    ],
                },
                next: None,
            },
            FlowStep {
                name: "Yes".into(),
                start: false,
                pattern: None,
                body: StepBody::Pass {
                    value: json!({"kind": "number", "value": 2}),
                },
                next: Some(Next::Succeed {}),
            },
            FlowStep {
                name: "No".into(),
                start: false,
                pattern: None,
                body: StepBody::Fail {
                    error: json!({"kind": "string", "values": ["no"]}),
                },
                next: None,
            },
        ],
    };
    let program = scene_program(vec![workflow], vec![], instance("Flow", None, vec![]));
    let mut world = World::start(program).expect("start");
    world.inject(json!({})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.output(world.root_id()), Some(&json!(2.0)));
}

fn worker() -> Component {
    Component {
        kind: "component".into(),
        name: "Worker".into(),
        fields: vec![],
        ports: vec![],
        states: vec![State {
            kind: "state".into(),
            name: "Idle".into(),
            start: true,
            named: true,
            handlers: vec![Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: json!({
                    "kind": "over",
                    "keys": {
                        "kind": {
                            "kind": "equal",
                            "value": {"kind": "value.literal", "value": "Item"}
                        },
                        "item": {
                            "kind": "variable",
                            "name": "item",
                            "pattern": {"kind": "any"}
                        }
                    },
                    "rest": null
                }),
                body: vec![Statement::Complete {
                    message: json!({
                        "kind": "object",
                        "keys": [
                            {
                                "kind": "objectKey",
                                "name": "kind",
                                "expression": {"kind": "string", "values": ["Done"]}
                            },
                            {
                                "kind": "objectKey",
                                "name": "item",
                                "expression": {"kind": "reference", "name": "item"}
                            }
                        ]
                    }),
                }],
            }],
        }],
    }
}

fn count_reducer() -> Component {
    Component {
        kind: "component".into(),
        name: "Count".into(),
        fields: vec![Field {
            kind: "field".into(),
            name: "total".into(),
            pattern: Some(json!({"kind": "type", "type": "number"})),
            init: Some(json!({"kind": "number", "value": 0})),
        }],
        ports: vec![],
        states: vec![State {
            kind: "state".into(),
            name: "Start".into(),
            start: true,
            named: true,
            handlers: vec![
                Handler {
                    kind: "handler".into(),
                    route: vec![],
                    item: None,
                    pattern: json!({
                        "kind": "variable",
                        "name": "item",
                        "pattern": {"kind": "any"}
                    }),
                    body: vec![Statement::Assign {
                        field: "total".into(),
                        expression: json!({
                            "kind": "invocation",
                            "expression": {"kind": "reference", "name": "add"},
                            "args": [
                                {"kind": "reference", "name": "total"},
                                {"kind": "number", "value": 1}
                            ]
                        }),
                    }],
                },
                Handler {
                    kind: "handler".into(),
                    route: vec![],
                    item: None,
                    pattern: json!({"kind": "any"}),
                    body: vec![Statement::Complete {
                        message: json!({"kind": "reference", "name": "total"}),
                    }],
                },
            ],
        }],
    }
}

#[test]
// req:workflows-003
fn fanout_zero_one_and_many_reduce() {
    let program = scene_program(
        vec![map_flow("Import", "items", "Worker", "Count")],
        vec![worker(), count_reducer()],
        instance("Import", None, vec![]),
    );

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Start", "items": []})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.output(world.root_id()), Some(&json!(0.0)));

    let mut world = World::start(program.clone()).expect("start");
    world
        .inject(json!({"kind": "Start", "items": ["a"]}))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.output(world.root_id()), Some(&json!(1.0)));

    let mut world = World::start(program).expect("start");
    world
        .inject(json!({"kind": "Start", "items": ["a", "b", "c"]}))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.output(world.root_id()), Some(&json!(3.0)));
}

#[test]
// req:workflows-003
fn fanout_of_kind_with_ports_fails_check() {
    let mut boxed = worker();
    boxed.name = "Boxed".into();
    boxed.ports.push(port("Worker", "inner"));
    let program = scene_program(
        vec![map_flow("Import", "items", "Boxed", "Count")],
        vec![worker(), boxed, count_reducer()],
        instance("Import", None, vec![]),
    );
    let err = World::check(&program).expect_err("ports");
    assert!(err.message.contains("must not declare ports"));
}

#[test]
// req:workflows-004
fn child_workflow_completes_to_parent() {
    let child = task_flow("ChildFlow", "Unit");
    let unit = idle(
        "Unit",
        vec![],
        vec![Handler {
            kind: "handler".into(),
            route: vec![],
            item: None,
            pattern: json!({"kind": "any"}),
            body: vec![Statement::Complete {
                message: object_kind("Done"),
            }],
        }],
    );
    let parent = task_flow("ParentFlow", "ChildFlow");
    let program = scene_program(
        vec![parent, child],
        vec![unit],
        instance("ParentFlow", None, vec![]),
    );
    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Go"})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(
        world.output(world.root_id()).unwrap()["kind"],
        json!("Done")
    );
}

#[test]
// req:workflows-006
fn snapshot_restore_continues_a_run() {
    let child = idle(
        "Child",
        vec![],
        vec![Handler {
            kind: "handler".into(),
            route: vec![],
            item: None,
            pattern: json!({"kind": "any"}),
            body: vec![Statement::Complete {
                message: object_kind("Pong"),
            }],
        }],
    );
    let program = scene_program(
        vec![task_flow("Flow", "Child")],
        vec![child],
        instance("Flow", None, vec![]),
    );
    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Ping", "n": 7})).unwrap();
    let snap = world.snapshot().expect("snapshot");
    assert!(
        serde_json::from_str::<serde_json::Value>(&snap).is_err(),
        "snapshot must be a data codec record, not JSON"
    );
    let mut restored = World::restore(program, &snap).expect("restore");
    restored.run_until_idle().unwrap();
    assert_eq!(
        restored.output(restored.root_id()).unwrap()["kind"],
        json!("Pong")
    );
}

#[test]
// req:workflows-007
fn run_path_executes_dynamic_ir() {
    let host = Workflow {
        kind: "workflow".into(),
        name: "Host".into(),
        start: "Start".into(),
        steps: vec![FlowStep {
            name: "Start".into(),
            start: true,
            pattern: None,
            body: StepBody::Run {
                path: "plan".into(),
            },
            next: Some(Next::Succeed {}),
        }],
    };
    let program = scene_program(
        vec![host],
        vec![idle(
            "Child",
            vec![],
            vec![Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: json!({"kind": "any"}),
                body: vec![Statement::Complete {
                    message: object_kind("Dyn"),
                }],
            }],
        )],
        instance("Host", None, vec![]),
    );
    let mut world = World::start(program).expect("start");
    world
        .inject(json!({
            "plan": { "kind": "task", "component": "Child" }
        }))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.output(world.root_id()).unwrap()["kind"], json!("Dyn"));
}

fn order_program() -> kith::ast::Program {
    kith::parse::parse_source(include_str!("../examples/order.kith")).expect("parse order.kith")
}

fn paid_order() -> serde_json::Value {
    json!({
        "kind": "Order",
        "orderId": "o-1",
        "items": ["sku-a", "sku-b"],
        "card": "ok"
    })
}

#[test]
// req:workflows-005 req:workflows-008 req:components-007 req:components-009 req:components-010 req:components-011 req:dispatch-003 req:runtime-001
fn order_workflow_waits_for_delivery_then_summarizes() {
    let mut world = World::start(order_program()).expect("start order");
    world.inject(paid_order()).unwrap();
    world.run_until_idle().unwrap();
    assert!(!world.completed(world.root_id()));
    assert_eq!(world.progress(world.root_id()).unwrap()["total"], json!(2));
    assert_eq!(world.prints().len(), 4);
    assert_eq!(world.prints()[0]["kind"], json!("PickupRequested"));
    assert!(world
        .prints()
        .iter()
        .any(|value| value["kind"] == json!("InTransit")));
    for carrier in world.children_of_kind(world.root_id(), "Carrier") {
        assert_eq!(world.state(carrier), Some("Shipping"));
        assert_eq!(
            world.kind(world.child(carrier, "ship").unwrap()),
            Some("MockShip")
        );
    }

    world.run().unwrap();
    let output = world.output(world.root_id()).unwrap();
    assert_eq!(output["kind"], json!("Summarize"));
    assert_eq!(output["orderId"], json!("o-1"));
    assert_eq!(output["total"], json!(2.0));
    assert!(output.get("shipments").is_none());
    assert!(world
        .prints()
        .iter()
        .any(|value| value["kind"] == json!("Tally") && value["total"] == json!(2.0)));
}

#[test]
// req:workflows-005 req:workflows-006 req:dispatch-003 req:runtime-001
fn order_snapshot_restore_resumes_delivery() {
    let program = order_program();
    let mut world = World::start(program.clone()).expect("start order");
    world.inject(paid_order()).unwrap();
    world.run_until_idle().unwrap();
    let snap = world.snapshot().expect("snapshot");
    let mut restored = World::restore(program, &snap).expect("restore");
    assert_eq!(restored.prints().len(), 4);
    for carrier in restored.children_of_kind(restored.root_id(), "Carrier") {
        assert_eq!(restored.state(carrier), Some("Shipping"));
    }
    restored.run().unwrap();
    let output = restored.output(restored.root_id()).unwrap();
    assert_eq!(output["kind"], json!("Summarize"));
    assert_eq!(output["orderId"], json!("o-1"));
}

#[test]
// req:workflows-005 req:components-008
fn order_workflow_rejects_an_invalid_order() {
    let mut world = World::start(order_program()).expect("start order");
    world.inject(json!({"kind": "Nope"})).unwrap();
    let err = world.run_until_idle().expect_err("invalid order");
    assert!(err.message.contains("invalid order"));
}

fn http_program() -> kith::ast::Program {
    kith::parse::parse_source(include_str!("../examples/http.kith")).expect("parse http.kith")
}

fn settle_http_request(world: &mut World) {
    world.run_until_idle().unwrap();
    assert!(world.fire_next_timer().unwrap(), "http timer");
    world.run_until_idle().unwrap();
}

fn health_cycle(world: &mut World) {
    assert!(world.fire_next_timer().unwrap(), "tick timer");
    settle_http_request(world);
}

#[test]
// req:runtime-002 req:components-012 req:dispatch-003 req:runtime-001 req:runtime-003 req:components-014 req:components-015 req:components-017
fn http_mock_waits_then_client_sends_up() {
    let mut world = World::start(http_program()).expect("start http");
    world.run_until_idle().unwrap();
    assert!(!world.completed(world.root_id()));
    assert_eq!(world.state(world.root_id()), Some("Waiting"));
    let client = world.child(world.root_id(), "client").expect("client");
    assert_eq!(world.state(client), Some("Sending"));
    assert!(!world.completed(client));
    assert!(world
        .prints()
        .iter()
        .any(|value| value == &json!(["Requested", "https://example.com"])));

    settle_http_request(&mut world);
    assert_eq!(world.state(world.root_id()), Some("Checking"));
    assert_eq!(world.state(client), Some("Idle"));
    assert!(!world.completed(world.root_id()));
    assert!(world
        .prints()
        .iter()
        .any(|value| value == &json!("Indeterminate")));
}

#[test]
// req:components-013
fn complete_marks_done_without_changing_state() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on down any {
      complete { kind: "Done" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse box");
    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Go"})).unwrap();
    world.run_until_idle().unwrap();
    assert!(world.completed(world.root_id()));
    assert_eq!(world.state(world.root_id()), Some("Start"));
    assert_eq!(
        world.output(world.root_id()),
        Some(&json!({"kind": "Done"}))
    );
    let err = world
        .inject(json!({"kind": "Again"}))
        .and_then(|_| world.run_until_idle())
        .expect_err("complete takes no further turns");
    assert!(err.message.contains("unmatched"));
}

#[test]
// req:components-013 req:runtime-002
fn http_stays_in_start_and_does_not_complete() {
    let mut world = World::start(http_program()).expect("start http");
    world.run_until_idle().unwrap();
    let client = world.child(world.root_id(), "client").expect("client");
    let http = world.child(client, "http").expect("http port");
    assert_eq!(world.state(http), Some("Start"));
    assert!(!world.completed(http));
    assert_eq!(world.state(client), Some("Sending"));
    assert_eq!(world.state(world.root_id()), Some("Waiting"));
}

#[test]
// req:components-012
fn http_parent_response_is_unmatched_down() {
    let mut world = World::start(http_program()).expect("start http");
    world.run_until_idle().unwrap();
    let client = world.child(world.root_id(), "client").expect("client");
    let err = world
        .send(
            client,
            json!(["Response", 200.0, "ok"]),
        )
        .and_then(|_| world.run_until_idle())
        .expect_err("parent Response must not match on up");
    assert!(err.message.contains("unmatched"));
}

#[test]
// req:runtime-003 req:components-015 req:components-018
fn health_check_three_ok_statuses_are_not_unhealthy() {
    let mut world = World::start(http_program()).expect("start http");
    settle_http_request(&mut world);
    assert!(world
        .prints()
        .iter()
        .any(|value| value == &json!("Indeterminate")));
    health_cycle(&mut world);
    health_cycle(&mut world);
    assert!(world
        .prints()
        .iter()
        .any(|value| value == &json!("Healthy")));
    assert!(world
        .prints()
        .iter()
        .all(|value| value != &json!("Unhealthy")));
    assert_eq!(world.field(world.root_id(), "s3"), Some(&json!(200.0)));
}

#[test]
// req:runtime-003
fn health_check_prints_unhealthy_when_last_three_are_not_200() {
    let program = kith::parse::parse_source(
        r#"
export Http;
export Client;
export HealthCheck;
export Main;

component Http {
  start {
    on down "GET" url:any {
      send after 1 up "Response" 500 "no";
    }
  }
}

component Client {
  Http http;
  start Idle {
    on down "Go" url:any {
      send to http "GET" url;
      goto Sending;
    }
  }
  state Sending {
    on up "Response" status:number body:any {
      send up "Done" status body;
      goto Idle;
    }
  }
}

component HealthCheck {
  field s1: number?;
  field s2: number?;
  field s3: number?;
  Client client;
  start Checking {
    on init {
      send self "Tick";
    }
    on self "Tick" {
      send to client "Go" "https://example.com";
      goto Waiting;
    }
  }
  state Waiting {
    on up "Done" status:number body:any {
      s1 -> s2;
      s2 -> s3;
      s3 -> status;
      send self "Check" s1 s2 s3;
      goto Validating;
    }
  }
  state Validating {
    on self "Check" a:200 b:200 c:200 {
      print "Healthy";
      send after 5 self "Tick";
      goto Checking;
    }
    on self "Check" a:(number & not 200) b:(number & not 200) c:(number & not 200) {
      print "Unhealthy";
      send after 5 self "Tick";
      goto Checking;
    }
    else {
      print "Indeterminate";
      send after 5 self "Tick";
      goto Checking;
    }
  }
}

export scene Main {
  HealthCheck() {
    client = Client() {
      http = Http();
    }
  }
}
"#,
    )
    .expect("parse unhealthy");
    let mut world = World::start(program).expect("start");
    settle_http_request(&mut world);
    assert!(world
        .prints()
        .iter()
        .any(|value| value == &json!("Indeterminate")));
    health_cycle(&mut world);
    health_cycle(&mut world);
    assert!(world
        .prints()
        .iter()
        .any(|value| value == &json!("Unhealthy")));
}

#[test]
// req:components-014 req:components-015 req:components-016
fn send_then_goto_matches_the_final_state() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start Home {
    on init {
      send self { kind: "Ping" };
      goto Away;
    }
    on self { kind: "Ping" } {
      print { kind: "Home" };
    }
  }
  state Away {
    on self { kind: "Ping" } {
      print { kind: "Away" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse send then goto");
    let mut world = World::start(program).expect("start");
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Away"));
    assert!(world
        .prints()
        .iter()
        .any(|value| value["kind"] == json!("Away")));
    assert!(world
        .prints()
        .iter()
        .all(|value| value["kind"] != json!("Home")));
}

#[test]
// req:components-016
fn check_rejects_goto_then_send() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start Home {
    on down any {
      goto Home;
      send self { kind: "Tick" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse goto then send");
    let err = World::check(&program).expect_err("goto must be last");
    assert!(err.message.contains("must be last"));
}

#[test]
// req:components-016
fn check_rejects_two_gotos_or_goto_and_complete() {
    let two_gotos = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start Home {
    on down any {
      goto Home;
      goto Home;
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse two gotos");
    let err = World::check(&two_gotos).expect_err("two gotos");
    assert!(err.message.contains("must be last"));

    let both = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start Home {
    on down any {
      goto Home;
      complete { kind: "Done" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse goto and complete");
    let err = World::check(&both).expect_err("goto and complete");
    assert!(err.message.contains("must be last"));
}

#[test]
// req:components-015
fn send_self_matches_unqualified_on_not_on_down() {
    let implied = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send self { kind: "Tick" };
    }
    on down { kind: "Tick" } {
      print { kind: "Down" };
    }
    on { kind: "Tick" } {
      print { kind: "Implied" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse implied self");
    let mut world = World::start(implied).expect("start");
    world.run_until_idle().unwrap();
    assert!(world
        .prints()
        .iter()
        .any(|value| value["kind"] == json!("Implied")));
    assert!(world
        .prints()
        .iter()
        .all(|value| value["kind"] != json!("Down")));

    let down_only = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send self { kind: "Tick" };
    }
    on down { kind: "Tick" } {
      print { kind: "Down" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse down only");
    let mut world = World::start(down_only).expect("start");
    let err = world
        .run_until_idle()
        .expect_err("on down must not take send self");
    assert!(err.message.contains("unmatched self"));
}

#[test]
// req:components-012 req:components-015
fn on_up_or_self_matches_self_not_down() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send self { kind: "Tick" };
    }
    on up or self { kind: "Tick" } {
      print { kind: "Hit" };
    }
    on down { kind: "Tick" } {
      print { kind: "Down" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse up or self");
    let mut world = World::start(program).expect("start");
    world.run_until_idle().unwrap();
    assert!(world
        .prints()
        .iter()
        .any(|value| value["kind"] == json!("Hit")));
    assert!(world
        .prints()
        .iter()
        .all(|value| value["kind"] != json!("Down")));
    world.inject(json!({"kind": "Tick"})).unwrap();
    world.run_until_idle().unwrap();
    assert!(world
        .prints()
        .iter()
        .any(|value| value["kind"] == json!("Down")));
}

#[test]
// req:components-012
fn check_rejects_duplicate_handler_route() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on down or down { kind: "Tick" } {}
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse duplicate route");
    let err = World::check(&program).expect_err("duplicate route");
    assert!(err.message.contains("duplicate route down"));
}

#[test]
// req:components-012
fn on_up_or_down_does_not_match_send_self() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send self { kind: "Tick" };
    }
    on up or down { kind: "Tick" } {
      print { kind: "Hit" };
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse up or down");
    let mut world = World::start(program).expect("start");
    let err = world
        .run_until_idle()
        .expect_err("up or down must not take send self");
    assert!(err.message.contains("unmatched self"));
}

#[test]
// req:components-017
fn send_sequence_matches_then_and_object_still_captures() {
    let sequence = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send self "Check" 200 200 200;
    }
    on self "Check" a:200 b:200 c:200 {
      print "Healthy";
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse sequence");
    let mut world = World::start(sequence).expect("start sequence");
    world.run_until_idle().unwrap();
    assert!(world
        .prints()
        .iter()
        .any(|value| value == &json!("Healthy")));

    let object = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send self { kind: "Done" status: 200 };
    }
    on self v:{ kind: "Done" status: number } {
      print v.status;
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse object capture");
    let mut world = World::start(object).expect("start object");
    world.run_until_idle().unwrap();
    assert!(world
        .prints()
        .iter()
        .any(|value| value == &json!(200.0)));
}

#[test]
// req:components-018
fn assign_must_match_the_field_pattern() {
    let ok = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  field s1: number?;
  start {
    on init {
      s1 -> 200;
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse number?");
    let mut world = World::start(ok).expect("start");
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "s1"), Some(&json!(200.0)));

    let bad = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  field s1: number?;
  start {
    on init {
      s1 -> "no";
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse string assign");
    let err = World::start(bad)
        .and_then(|mut world| world.run_until_idle())
        .expect_err("string is not number?");
    assert!(err.message.contains("field s1 did not match"));

    let required = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  field n: number;
  start {}
}
export scene Main { Box(); }
"#,
    )
    .expect("parse required number");
    let err = World::start(required)
        .map(|_| ())
        .expect_err("null is not number");
    assert!(err.message.contains("field n did not match"));

    let unknown = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      ghost -> 1;
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse unknown field");
    let err = World::start(unknown)
        .and_then(|mut world| world.run_until_idle())
        .expect_err("unknown field");
    assert!(err.message.contains("unknown field ghost"));
}
