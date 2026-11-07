use kith::ast::{
    component_pattern, key_pattern, ping_program, Component, Export, Field, Handler, Instance,
    Program, Scene, State, Statement,
};
use kith::interpreter::World;
use serde_json::json;

fn export(name: &str) -> Export {
    Export {
        kind: "export".into(),
        name: name.into(),
    }
}

fn idle(name: &str, handlers: Vec<Handler>) -> Component {
    Component {
        kind: "component".into(),
        name: name.into(),
        fields: vec![],
        states: vec![State {
            kind: "state".into(),
            name: "Idle".into(),
            start: true,
            handlers,
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

#[test]
// req:components-001 req:scenes-001
fn parent_owns_scene_constructed_children() {
    let mut world = World::start(ping_program()).expect("start");
    let child = world.child(world.root_id(), "child").expect("child slot");
    assert_eq!(world.kind(child), Some("Child"));
    assert_eq!(world.kind(world.root_id()), Some("Parent"));
    assert_eq!(world.state(child), Some("Idle"));
    world.inject(json!({"kind": "Ping", "n": 1})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.kind(child), Some("Child"));
}

#[test]
// req:scenes-001
fn check_rejects_unknown_kind_and_duplicate_slot() {
    let mut unknown = ping_program();
    unknown.scenes[0].root.children[0].component = "Nope".into();
    assert!(World::check(&unknown).is_err());

    let mut duplicate = ping_program();
    duplicate.scenes[0]
        .root
        .children
        .push(instance("Child", Some("child"), vec![]));
    let err = World::check(&duplicate).expect_err("duplicate slot");
    assert!(err.message.contains("duplicate slot key"));
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
    let child = Component {
        kind: "component".into(),
        name: "Child".into(),
        fields: vec![Field {
            kind: "field".into(),
            name: "seen".into(),
            pattern: None,
            init: Some(json!({"kind": "number", "value": 0})),
        }],
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
    };
    let parent = idle("Parent", vec![]);
    let program = scene_program(
        vec![child, parent],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );
    let mut world = World::start(program).expect("start");
    let err = world
        .inject(json!({"kind": "Nope"}))
        .and_then(|_| world.run_until_idle())
        .expect_err("unhandled inject must not be offered to children");
    assert!(err.message.contains("unmatched"));
    let child = world.child(world.root_id(), "child").unwrap();
    assert_eq!(world.field(child, "seen"), Some(&json!(0.0)));
}

#[test]
// req:components-003
fn send_down_selects_by_key_kind_and_any() {
    let ping_child = Component {
        kind: "component".into(),
        name: "Child".into(),
        fields: vec![Field {
            kind: "field".into(),
            name: "seen".into(),
            pattern: None,
            init: Some(json!({"kind": "number", "value": 0})),
        }],
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
    };
    let other = Component {
        kind: "component".into(),
        name: "Other".into(),
        fields: vec![Field {
            kind: "field".into(),
            name: "seen".into(),
            pattern: None,
            init: Some(json!({"kind": "number", "value": 0})),
        }],
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
    };
    let parent = idle(
        "Parent",
        vec![
            Handler {
                kind: "handler".into(),
                pattern: json!({
                    "kind": "over",
                    "keys": {
                        "kind": {
                            "kind": "equal",
                            "value": {"kind": "value.literal", "value": "Key"}
                        }
                    }
                }),
                body: vec![Statement::SendDown {
                    children: key_pattern("child"),
                    message: json!({"kind": "string", "values": ["x"]}),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: json!({
                    "kind": "over",
                    "keys": {
                        "kind": {
                            "kind": "equal",
                            "value": {"kind": "value.literal", "value": "Kind"}
                        }
                    }
                }),
                body: vec![Statement::SendDown {
                    children: component_pattern("Child"),
                    message: json!({"kind": "string", "values": ["x"]}),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: json!({
                    "kind": "over",
                    "keys": {
                        "kind": {
                            "kind": "equal",
                            "value": {"kind": "value.literal", "value": "All"}
                        }
                    }
                }),
                body: vec![Statement::SendDown {
                    children: json!({"kind": "any"}),
                    message: json!({"kind": "string", "values": ["x"]}),
                }],
            },
        ],
    );

    let root = instance(
        "Parent",
        None,
        vec![
            instance("Child", Some("child"), vec![]),
            instance("Other", Some("other"), vec![]),
        ],
    );
    let program = scene_program(vec![ping_child, other, parent], root);

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Key"})).unwrap();
    world.run_until_idle().unwrap();
    let child = world.child(world.root_id(), "child").unwrap();
    let sibling = world.child(world.root_id(), "other").unwrap();
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(0.0)));

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Kind"})).unwrap();
    world.run_until_idle().unwrap();
    let child = world.child(world.root_id(), "child").unwrap();
    let sibling = world.child(world.root_id(), "other").unwrap();
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(0.0)));

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "All"})).unwrap();
    world.run_until_idle().unwrap();
    let child = world.child(world.root_id(), "child").unwrap();
    let sibling = world.child(world.root_id(), "other").unwrap();
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(1.0)));
}

#[test]
// req:components-004
fn unmatched_send_up_drops_and_unmatched_send_down_fails() {
    let child = idle(
        "Child",
        vec![Handler {
            kind: "handler".into(),
            pattern: json!({
                "kind": "over",
                "keys": {
                    "kind": {
                        "kind": "equal",
                        "value": {"kind": "value.literal", "value": "Go"}
                    }
                }
            }),
            body: vec![Statement::SendUp {
                message: json!({
                    "kind": "object",
                    "keys": [{
                        "kind": "objectKey",
                        "name": "kind",
                        "expression": {"kind": "string", "values": ["Ghost"]}
                    }]
                }),
            }],
        }],
    );
    let parent = idle(
        "Parent",
        vec![
            Handler {
                kind: "handler".into(),
                pattern: json!({
                    "kind": "over",
                    "keys": {
                        "kind": {
                            "kind": "equal",
                            "value": {"kind": "value.literal", "value": "Go"}
                        }
                    }
                }),
                body: vec![Statement::SendDown {
                    children: key_pattern("child"),
                    message: json!({
                        "kind": "object",
                        "keys": [{
                            "kind": "objectKey",
                            "name": "kind",
                            "expression": {"kind": "string", "values": ["Go"]}
                        }]
                    }),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: json!({
                    "kind": "over",
                    "keys": {
                        "kind": {
                            "kind": "equal",
                            "value": {"kind": "value.literal", "value": "Miss"}
                        }
                    }
                }),
                body: vec![Statement::SendDown {
                    children: key_pattern("child"),
                    message: json!({
                        "kind": "object",
                        "keys": [{
                            "kind": "objectKey",
                            "name": "kind",
                            "expression": {"kind": "string", "values": ["Miss"]}
                        }]
                    }),
                }],
            },
            Handler {
                kind: "handler".into(),
                pattern: json!({
                    "kind": "over",
                    "keys": {
                        "kind": {
                            "kind": "equal",
                            "value": {"kind": "value.literal", "value": "None"}
                        }
                    }
                }),
                body: vec![Statement::SendDown {
                    children: key_pattern("missing"),
                    message: json!({"kind": "string", "values": ["x"]}),
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

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "None"})).unwrap();
    let err = world.run_until_idle().expect_err("zero matches fail");
    assert!(err.message.contains("no child matched send down"));
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
