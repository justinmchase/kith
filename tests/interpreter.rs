use kith::ast::{
    ping_program, Component, Export, Field, Handler, Instance, Port, Program, Scene, State,
    Statement,
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
            handlers,
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
            handlers: vec![Handler {
                kind: "handler".into(),
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

fn scene_program(components: Vec<Component>, root: Instance) -> Program {
    let mut exports: Vec<Export> = components
        .iter()
        .map(|component| export(&component.name))
        .collect();
    exports.push(export("Main"));
    Program {
        kind: "program".into(),
        imports: vec![],
        exports,
        components,
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
        vec![
            counting("Child"),
            counting("Other"),
            idle("Parent", vec![port("Child", "child")], handlers),
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
    let child = world.child(world.root_id(), "child").expect("child port");
    assert_eq!(world.kind(child), Some("Child"));
    assert_eq!(world.kind(world.root_id()), Some("Parent"));
    assert_eq!(world.state(child), Some("Idle"));
    world.inject(json!({"kind": "Ping", "n": 1})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.kind(child), Some("Child"));

    let program = mixed_parent_program(vec![]);
    let world = World::start(program).expect("mixed start");
    assert!(world.child(world.root_id(), "child").is_some());
    assert_eq!(world.children_of_kind(world.root_id(), "Other").len(), 1);
    assert!(world.child(world.root_id(), "other").is_none());
}

#[test]
// req:scenes-001 req:scenes-003
fn check_rejects_unknown_kind_duplicate_missing_and_wrong_port() {
    let mut unknown = ping_program();
    unknown.scenes[0].root.children[0].component = "Nope".into();
    assert!(World::check(&unknown).is_err());

    let mut duplicate = ping_program();
    duplicate.scenes[0]
        .root
        .children
        .push(instance("Child", Some("child"), vec![]));
    let err = World::check(&duplicate).expect_err("duplicate port");
    assert!(err.message.contains("duplicate port fill"));

    let mut missing = ping_program();
    missing.scenes[0].root.children.clear();
    let err = World::check(&missing).expect_err("missing port");
    assert!(err.message.contains("missing port fill"));

    let mut extra = ping_program();
    extra.scenes[0]
        .root
        .children
        .push(instance("Child", Some("other"), vec![]));
    let err = World::check(&extra).expect_err("unknown port");
    assert!(err.message.contains("unknown port"));

    let mut wrong = ping_program();
    wrong.components.push(idle("Other", vec![], vec![]));
    wrong.scenes[0].root.children[0].component = "Other".into();
    let err = World::check(&wrong).expect_err("wrong kind");
    assert!(err.message.contains("expects Child"));
    assert!(err.message.contains("Other"));
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
        root: instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    });
    two.exports.push(export("Other"));
    let err = World::check(&two).expect_err("two scenes");
    assert!(err.message.contains("more than one scene"));
}

#[test]
// req:components-002
fn check_requires_exactly_one_start_state() {
    World::check(&ping_program()).expect("valid program");
    let mut none = ping_program();
    none.components[0].states[0].start = false;
    assert!(World::check(&none).is_err());

    let mut two = ping_program();
    two.components[1].states.push(State {
        kind: "state".into(),
        name: "Other".into(),
        start: true,
        handlers: vec![],
    });
    assert!(World::check(&two).is_err());
    assert!(World::start(two).is_err());
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
    let child = world.child(world.root_id(), "child").unwrap();
    let other = world.children_of_kind(world.root_id(), "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(0.0)));
    assert_eq!(world.field(other, "seen"), Some(&json!(0.0)));
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
                pattern: kind_eq("To"),
                body: vec![Statement::SendTo {
                    port: "child".into(),
                    message: string_msg(),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: kind_eq("Kind"),
                body: vec![Statement::SendDown {
                    component: Some("Child".into()),
                    message: string_msg(),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: kind_eq("OtherKind"),
                body: vec![Statement::SendDown {
                    component: Some("Other".into()),
                    message: string_msg(),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: kind_eq("All"),
                body: vec![Statement::SendDown {
                    component: None,
                    message: string_msg(),
                }],
            },
        ],
    );
    let program = scene_program(
        vec![counting("Child"), counting("Other"), parent],
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
    let child = world.child(world.root_id(), "child").unwrap();
    let sibling = world.children_of_kind(world.root_id(), "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(0.0)));

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Kind"})).unwrap();
    world.run_until_idle().unwrap();
    let child = world.child(world.root_id(), "child").unwrap();
    let sibling = world.children_of_kind(world.root_id(), "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(0.0)));

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "OtherKind"})).unwrap();
    world.run_until_idle().unwrap();
    let child = world.child(world.root_id(), "child").unwrap();
    let sibling = world.children_of_kind(world.root_id(), "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(0.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(1.0)));

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "All"})).unwrap();
    world.run_until_idle().unwrap();
    let child = world.child(world.root_id(), "child").unwrap();
    let sibling = world.children_of_kind(world.root_id(), "Other")[0];
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
            pattern: kind_eq("Go"),
            body: vec![Statement::SendUp {
                message: object_kind("Ghost"),
            }],
        }],
    );
    let parent = idle(
        "Parent",
        vec![port("Child", "child")],
        vec![
            Handler {
                kind: "handler".into(),
                pattern: kind_eq("Go"),
                body: vec![Statement::SendTo {
                    port: "child".into(),
                    message: object_kind("Go"),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: kind_eq("Miss"),
                body: vec![Statement::SendTo {
                    port: "child".into(),
                    message: object_kind("Miss"),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: kind_eq("None"),
                body: vec![Statement::SendDown {
                    component: Some("Ghost".into()),
                    message: string_msg(),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: kind_eq("Missing"),
                body: vec![Statement::SendTo {
                    port: "missing".into(),
                    message: string_msg(),
                }],
            },
        ],
    );
    let program = scene_program(
        vec![child, parent],
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
            handlers: vec![
                Handler {
                    kind: "handler".into(),
                    pattern: json!({"kind": "any"}),
                    body: vec![Statement::Assign {
                        field: "picked".into(),
                        expression: json!({"kind": "number", "value": 1}),
                    }],
                },
                Handler {
                    kind: "handler".into(),
                    pattern: json!({"kind": "any"}),
                    body: vec![Statement::Assign {
                        field: "picked".into(),
                        expression: json!({"kind": "number", "value": 2}),
                    }],
                },
            ],
        }],
    };
    let program = scene_program(vec![one], instance("One", None, vec![]));
    let mut world = World::start(program).expect("start");
    world.inject(json!("go")).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "picked"), Some(&json!(1.0)));
}

#[test]
// req:components-006
fn ping_example_sets_heard_to_seven() {
    let mut world = World::start(ping_program()).expect("start");
    world.inject(json!({"kind": "Ping", "n": 7})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(7.0)));
}
