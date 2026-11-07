use kith::ast::{Child, Component, Field, Handler, Program, State, Statement};
use kith::interpreter::World;
use kith::ping_program;
use serde_json::json;

#[test]
// req:components-001
fn parent_owns_declared_children() {
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
    let program = Program {
        kind: "program".into(),
        root: "Parent".into(),
        components: vec![
            Component {
                kind: "component".into(),
                name: "Child".into(),
                fields: vec![Field {
                    kind: "field".into(),
                    name: "seen".into(),
                    pattern: None,
                    init: Some(json!({"kind": "number", "value": 0})),
                }],
                children: vec![],
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
            },
            Component {
                kind: "component".into(),
                name: "Parent".into(),
                fields: vec![],
                children: vec![Child {
                    kind: "child".into(),
                    component: "Child".into(),
                    name: "child".into(),
                }],
                states: vec![State {
                    kind: "state".into(),
                    name: "Idle".into(),
                    start: true,
                    handlers: vec![],
                }],
            },
        ],
    };
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
// req:components-004
fn unmatched_bubble_drops_and_unmatched_down_fails() {
    let program = Program {
        kind: "program".into(),
        root: "Parent".into(),
        components: vec![
            Component {
                kind: "component".into(),
                name: "Child".into(),
                fields: vec![],
                children: vec![],
                states: vec![State {
                    kind: "state".into(),
                    name: "Idle".into(),
                    start: true,
                    handlers: vec![Handler {
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
                        body: vec![Statement::Bubble {
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
                }],
            },
            Component {
                kind: "component".into(),
                name: "Parent".into(),
                fields: vec![],
                children: vec![Child {
                    kind: "child".into(),
                    component: "Child".into(),
                    name: "child".into(),
                }],
                states: vec![State {
                    kind: "state".into(),
                    name: "Idle".into(),
                    start: true,
                    handlers: vec![
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
                            body: vec![Statement::Down {
                                child: "child".into(),
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
                            body: vec![Statement::Down {
                                child: "child".into(),
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
                    ],
                }],
            },
        ],
    };

    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Go"})).unwrap();
    world.run_until_idle().expect("unmatched bubble drops");

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Miss"})).unwrap();
    let err = world.run_until_idle().expect_err("unmatched down fails");
    assert!(err.message.contains("unmatched down"));
    assert!(err.message.contains("Child"));
}

#[test]
// req:components-005
fn first_matching_handler_consumes_the_message() {
    let program = Program {
        kind: "program".into(),
        root: "One".into(),
        components: vec![Component {
            kind: "component".into(),
            name: "One".into(),
            fields: vec![Field {
                kind: "field".into(),
                name: "picked".into(),
                pattern: None,
                init: Some(json!({"kind": "number", "value": 0})),
            }],
            children: vec![],
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
        }],
    };
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
