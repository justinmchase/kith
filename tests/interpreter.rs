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
            named: true,
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
            rules: None,
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

fn over_equal(key: &str, value: serde_json::Value) -> serde_json::Value {
    json!({
        "kind": "over",
        "keys": {
            key: {
                "kind": "equal",
                "value": {"kind": "value.literal", "value": value}
            }
        }
    })
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
        functions: vec![],
        rules: vec![],
        host_kinds: vec![],
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
    world
        .inject_document(kith::doc::array([
            kith::doc::symbol("Ping"),
            kith::doc::number(1.0),
        ]))
        .unwrap();
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
// req:components-002
fn check_requires_exactly_one_start_state() {
    World::check(&ping_program()).expect("valid program");
    let mut none = ping_program();
    none.components[0].states[0].start = false;
    assert!(World::check(&none).is_err());
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
// req:scenes-002
fn check_accepts_a_component_scene_root() {
    let program = scene_program(
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
                pattern: kind_eq("Down"),
                body: vec![Statement::SendDown {
                    component: None,
                    message: string_msg(),
                    after: None,
                }],
            },
        ],
    );
    let program = scene_program(
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

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Down"})).unwrap();
    world.run_until_idle().unwrap();
    let parent_id = world.root_id();
    let child = world.child(parent_id, "child").unwrap();
    let sibling = world.children_of_kind(parent_id, "Other")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(sibling, "seen"), Some(&json!(1.0)));
}

#[test]
// req:components-022
fn unreachable_children_are_destroyed_between_turns() {
    let child = idle(
        "Child",
        vec![],
        vec![Handler {
            kind: "handler".into(),
            route: vec![],
            item: None,
            pattern: kind_eq("Ping"),
            body: vec![Statement::SendUp {
                message: object_kind("Heard"),
                after: None,
            }],
        }],
    );
    let item = idle(
        "Item",
        vec![],
        vec![Handler {
            kind: "handler".into(),
            route: vec![],
            item: None,
            pattern: kind_eq("Tick"),
            body: vec![Statement::SendUp {
                message: object_kind("Pong"),
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
                pattern: kind_eq("Ping"),
                body: vec![Statement::SendTo {
                    port: "child".into(),
                    message: object_kind("Ping"),
                    after: None,
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec!["up".into()],
                item: None,
                pattern: kind_eq("Heard"),
                body: vec![Statement::Assign {
                    field: "heard".into(),
                    expression: json!({"kind": "number", "value": 1}),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec!["up".into()],
                item: None,
                pattern: kind_eq("Pong"),
                body: vec![Statement::Assign {
                    field: "reply".into(),
                    expression: json!({"kind": "number", "value": 1}),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Later"),
                body: vec![Statement::SendTo {
                    port: "item".into(),
                    message: object_kind("Tick"),
                    after: Some(json!({"kind": "number", "value": 1})),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Drop"),
                body: vec![
                    Statement::SendTo {
                        port: "item".into(),
                        message: object_kind("Tick"),
                        after: None,
                    },
                    Statement::Release {
                        filter: over_equal("port", json!("item")),
                    },
                ],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Scene"),
                body: vec![Statement::Release {
                    filter: over_equal("port", json!("child")),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Kind"),
                body: vec![Statement::Release {
                    filter: over_equal("kind", json!("Item")),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Field"),
                body: vec![Statement::Release {
                    filter: over_equal("seen", json!(0)),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Missing"),
                body: vec![Statement::Release {
                    filter: over_equal("port", json!("missing")),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Done"),
                body: vec![Statement::Complete {
                    message: json!({"kind": "number", "value": 1}),
                }],
            },
        ],
    );
    let parent = Component {
        kind: parent.kind,
        name: parent.name,
        fields: vec![
            Field {
                kind: "field".into(),
                name: "heard".into(),
                pattern: None,
                init: Some(json!({"kind": "number", "value": 0})),
                rules: None,
            },
            Field {
                kind: "field".into(),
                name: "reply".into(),
                pattern: None,
                init: Some(json!({"kind": "number", "value": 0})),
                rules: None,
            },
        ],
        ports: parent.ports,
        states: parent.states,
    };
    let program = scene_program(
        vec![parent, child, item, idle("Grand", vec![], vec![])],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );
    let mut world = World::start(program).expect("start");
    let root = world.root_id();
    let item_id = world
        .hold_dynamic_child(
            root,
            &instance(
                "Item",
                Some("item"),
                vec![instance("Grand", Some("inner"), vec![])],
            ),
        )
        .expect("hold");
    let grand_id = world.children_of_kind(item_id, "Grand")[0];

    world.inject(json!({"kind": "Ping"})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(root, "heard"), Some(&json!(1.0)));

    for kind in ["Missing", "Kind", "Field"] {
        world.inject(json!({"kind": kind})).unwrap();
        let err = world.run_until_idle().expect_err(kind);
        assert!(
            err.message.contains("no child matched"),
            "{kind}: {}",
            err.message
        );
        assert!(world.kind(item_id).is_some());
        assert!(world.child(root, "child").is_some());
    }

    world.inject(json!({"kind": "Scene"})).unwrap();
    let err = world.run_until_idle().expect_err("scene child stays");
    assert!(err.message.contains("scene-constructed"), "{}", err.message);
    assert!(world.kind(item_id).is_some());
    assert!(world.kind(grand_id).is_some());
    assert!(world.child(root, "child").is_some());

    world.inject(json!({"kind": "Later"})).unwrap();
    world.run_until_idle().unwrap();
    world.inject(json!({"kind": "Drop"})).unwrap();
    world.run_until_idle().unwrap();
    assert!(world.kind(item_id).is_none());
    assert!(world.kind(grand_id).is_none());
    assert!(world.child(root, "child").is_some());
    assert_eq!(world.field(root, "heard"), Some(&json!(1.0)));
    assert_eq!(world.field(root, "reply"), Some(&json!(0.0)));
    assert!(!world.fire_next_timer().unwrap());
    assert_eq!(world.field(root, "reply"), Some(&json!(0.0)));

    world.inject(json!({"kind": "Done"})).unwrap();
    world.run_until_idle().unwrap();
    assert!(world.child(root, "child").is_some());
    assert_eq!(world.output(root), Some(&json!(1.0)));
}

#[test]
// req:components-023
fn a_transition_constructs_an_unnamed_child_that_inits_once() {
    let number = |value: i64| json!({"kind": "number", "value": value});
    let text = |value: &str| json!({"kind": "string", "values": [value]});
    let field = |name: &str| Field {
        kind: "field".into(),
        name: name.into(),
        pattern: None,
        init: Some(number(0)),
        rules: None,
    };
    let phase = json!({"kind": "reference", "name": "phase"});
    let item = idle(
        "Item",
        vec![port("Leaf", "inner")],
        vec![
            Handler {
                kind: "handler".into(),
                route: vec!["init".into()],
                item: None,
                pattern: json!({"kind": "any"}),
                body: vec![
                    Statement::Assign {
                        field: "phase".into(),
                        expression: number(1),
                    },
                    Statement::SendUp {
                        message: object_kind("Hi"),
                        after: None,
                    },
                ],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Tick"),
                body: vec![Statement::Assign {
                    field: "saw".into(),
                    expression: phase,
                }],
            },
        ],
    );
    let item = Component {
        kind: item.kind,
        name: item.name,
        fields: vec![field("phase"), field("saw")],
        ports: item.ports,
        states: item.states,
    };
    let child = idle(
        "Child",
        vec![],
        vec![Handler {
            kind: "handler".into(),
            route: vec![],
            item: None,
            pattern: kind_eq("Tick"),
            body: vec![],
        }],
    );
    let parent = idle(
        "Parent",
        vec![port("Child", "child")],
        vec![
            Handler {
                kind: "handler".into(),
                route: vec!["up".into()],
                item: None,
                pattern: kind_eq("Hi"),
                body: vec![Statement::Assign {
                    field: "heard".into(),
                    expression: number(1),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("Make"),
                body: vec![
                    Statement::Construct {
                        component: "Item".into(),
                        key: Some(text("one")),
                        children: vec![instance("Leaf", Some("inner"), vec![])],
                    },
                    Statement::Construct {
                        component: "Item".into(),
                        key: Some(text("two")),
                        children: vec![instance("Leaf", Some("inner"), vec![])],
                    },
                    Statement::SendDown {
                        component: None,
                        message: object_kind("Tick"),
                        after: None,
                    },
                ],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("DropOne"),
                body: vec![Statement::Release {
                    filter: over_equal("key", json!("one")),
                }],
            },
            Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("DropEarly"),
                body: vec![
                    Statement::Construct {
                        component: "Item".into(),
                        key: None,
                        children: vec![instance("Leaf", Some("inner"), vec![])],
                    },
                    Statement::Release {
                        filter: over_equal("port", json!(null)),
                    },
                ],
            },
        ],
    );
    let parent = Component {
        kind: parent.kind,
        name: parent.name,
        fields: vec![field("heard")],
        ports: parent.ports,
        states: parent.states,
    };
    let program = scene_program(
        vec![
            parent.clone(),
            child,
            item.clone(),
            idle("Leaf", vec![], vec![]),
        ],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );
    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!({"kind": "Make"})).unwrap();
    world.run_until_idle().unwrap();
    let root = world.root_id();
    let items = world.children_of_kind(root, "Item");
    assert_eq!(items.len(), 2);
    let port_child = world.child(root, "child").unwrap();
    assert!(items.iter().all(|id| *id != port_child));
    assert!(world.child(root, "item").is_none());
    assert!(items
        .iter()
        .all(|id| world.field(*id, "phase") == Some(&json!(1.0))));
    assert!(items
        .iter()
        .all(|id| world.field(*id, "saw") == Some(&json!(1.0))));
    assert_eq!(world.field(root, "heard"), Some(&json!(1.0)));
    let keys: Vec<_> = items
        .iter()
        .map(|id| world.held_key(*id).unwrap())
        .collect();
    assert!(keys.contains(&json!("one")));
    assert!(keys.contains(&json!("two")));
    assert!(items.iter().all(|id| world.child(*id, "inner").is_some()));

    world.inject(json!({"kind": "DropOne"})).unwrap();
    world.run_until_idle().unwrap();
    let left = world.children_of_kind(root, "Item");
    assert_eq!(left.len(), 1);
    assert_eq!(world.held_key(left[0]), Some(json!("two")));
    assert!(world.child(root, "child").is_some());

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "DropEarly"})).unwrap();
    world.run_until_idle().unwrap();
    assert!(world.children_of_kind(world.root_id(), "Item").is_empty());
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(0.0)));
    assert!(world.child(world.root_id(), "child").is_some());

    let mut bad = scene_program(
        vec![
            parent,
            idle("Child", vec![], vec![]),
            item,
            idle("Leaf", vec![], vec![]),
            idle("Stranger", vec![], vec![]),
        ],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );
    if let Statement::Construct { children, .. } =
        &mut bad.components[0].states[0].handlers[1].body[0]
    {
        children[0].component = "Stranger".into();
    }
    let err = World::check(&bad).expect_err("stranger is not Leaf");
    assert!(err.message.contains("expects Leaf"), "{}", err.message);
}

#[test]
// req:components-024
fn a_post_condition_must_match_before_the_turn_commits() {
    let gate = r#"
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}

component Gate {
  field n: number = 0 {
    in Closed
      post 0
    in Open
      post 1
  }
  tick: Counter;

  start Closed {
    on down @Open {
      send to tick @Tick;
      goto Open;
    }
    on down @Stuck {
      n -> 1;
      send to tick @Tick;
      goto Open;
    }
    on down @Stay {
      n -> 1;
      send to tick @Tick;
    }
    on down @Finish {
      n -> 1;
      complete 7;
    }
    on down @FinishOk {
      complete 7;
    }
  }

  state Open {}
}

export scene Main {
  Gate() {
    tick = Counter();
  }
}
"#;
    let deliver = |source: &str, name: &str| {
        let mut world =
            World::start(kith::parse::parse_source(source).expect("parse")).expect("start");
        world
            .send_document(world.root_id(), kith::doc::symbol(name))
            .unwrap();
        world
    };
    let seen = |world: &World| {
        let tick = world.child(world.root_id(), "tick").unwrap();
        world.field(tick, "seen").cloned()
    };

    let mut world = deliver(gate, "Open");
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Open"));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(0.0)));
    assert_eq!(seen(&world), Some(json!(1.0)));

    let mut world = deliver(gate, "Stuck");
    let err = world.run_until_idle().expect_err("closed post misses");
    assert!(err.message.contains("post"), "{}", err.message);
    assert!(err.message.contains("Closed"), "{}", err.message);
    assert_eq!(world.state(world.root_id()), Some("Closed"));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(0.0)));
    assert_eq!(seen(&world), Some(json!(0.0)));

    let mut world = deliver(gate, "Stay");
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Closed"));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(1.0)));
    assert_eq!(seen(&world), Some(json!(1.0)));

    let mut world = deliver(gate, "Finish");
    world.run_until_idle().unwrap();
    assert!(world.completed(world.root_id()));
    assert_eq!(world.state(world.root_id()), Some("Closed"));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(1.0)));

    let mut world = deliver(gate, "FinishOk");
    world.run_until_idle().unwrap();
    assert!(world.completed(world.root_id()));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(0.0)));

    let created = r#"
component Closed {
  field n: number = 0 {
    in Idle
      post 1
  }
  field heard: number = 0;
  start Idle {
    on init {
      heard -> 1;
    }
  }
}
export scene Main { Closed(); }
"#;
    let mut world = World::start(kith::parse::parse_source(created).expect("parse created"))
        .expect("creation skips post");
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(0.0)));
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(1.0)));
}

#[test]
// req:components-024
fn the_gate_post_example_commits_a_match_and_drops_a_miss() {
    let source = r#"
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}

component Gate {
  field n: number = 0 {
    in Closed
      post 0
    in Open
      post 1
  }
  tick: Counter;

  start Closed {
    on down @Open {
      send to tick @Tick;
      goto Open;
    }
    on down @Stuck {
      n -> 1;
      send to tick @Tick;
      goto Open;
    }
  }

  state Open {}
}

export scene Main {
  Gate() {
    tick = Counter();
  }
}
"#;
    let mut world =
        World::start(kith::parse::parse_source(source).expect("parse gate")).expect("start");
    let root = world.root_id();
    world
        .send_document(root, kith::doc::symbol("Open"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(root), Some("Open"));
    assert_eq!(world.field(root, "n"), Some(&json!(0.0)));
    let tick = world.child(root, "tick").unwrap();
    assert_eq!(world.field(tick, "seen"), Some(&json!(1.0)));

    let mut world =
        World::start(kith::parse::parse_source(source).expect("parse gate")).expect("start");
    let root = world.root_id();
    world
        .send_document(root, kith::doc::symbol("Stuck"))
        .unwrap();
    let err = world.run_until_idle().expect_err("post misses");
    assert!(err.message.contains("post"), "{}", err.message);
    assert!(err.message.contains("Closed"), "{}", err.message);
    assert_eq!(world.state(root), Some("Closed"));
    assert_eq!(world.field(root, "n"), Some(&json!(0.0)));
    let tick = world.child(root, "tick").unwrap();
    assert_eq!(world.field(tick, "seen"), Some(&json!(0.0)));
}

#[test]
// req:components-025
fn the_latch_entry_example_commits_a_match_and_drops_a_miss() {
    let source = r#"
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}

component Latch {
  field n: number = 0 {
    in Shut
    in Open
      pre 1
  }
  tick: Counter;

  start Shut {
    on down @Open {
      n -> 1;
      send to tick @Tick;
      goto Open;
    }
    on down @Stuck {
      send to tick @Tick;
      goto Open;
    }
  }

  state Open {}
}

export scene Main {
  Latch() {
    tick = Counter();
  }
}
"#;
    let mut world =
        World::start(kith::parse::parse_source(source).expect("parse latch")).expect("start");
    let root = world.root_id();
    world
        .send_document(root, kith::doc::symbol("Open"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(root), Some("Open"));
    assert_eq!(world.field(root, "n"), Some(&json!(1.0)));
    let tick = world.child(root, "tick").unwrap();
    assert_eq!(world.field(tick, "seen"), Some(&json!(1.0)));

    let mut world =
        World::start(kith::parse::parse_source(source).expect("parse latch")).expect("start");
    let root = world.root_id();
    world
        .send_document(root, kith::doc::symbol("Stuck"))
        .unwrap();
    let err = world.run_until_idle().expect_err("pre misses");
    assert!(err.message.contains("pre"), "{}", err.message);
    assert!(err.message.contains("Open"), "{}", err.message);
    assert!(!err.message.contains("post"), "{}", err.message);
    assert_eq!(world.state(root), Some("Shut"));
    assert_eq!(world.field(root, "n"), Some(&json!(0.0)));
    let tick = world.child(root, "tick").unwrap();
    assert_eq!(world.field(tick, "seen"), Some(&json!(0.0)));
}

#[test]
// req:components-025
fn an_entry_pattern_is_matched_before_the_post_condition() {
    let counter = r#"
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}
"#;
    let scene = r#"
export scene Main {
  Box() {
    tick = Counter();
  }
}
"#;
    let order = format!(
        r#"
{counter}
component Box {{
  field n: number = 0 {{
    in Shut
      post 9
    in Open
      pre 1
  }}
  tick: Counter;
  start Shut {{
    on down @LeaveBad {{
      send to tick @Tick;
      goto Open;
    }}
  }}
  state Open {{}}
}}
{scene}
"#
    );
    let mut world =
        World::start(kith::parse::parse_source(&order).expect("parse order")).expect("start");
    world
        .send_document(world.root_id(), kith::doc::symbol("LeaveBad"))
        .unwrap();
    let err = world.run_until_idle().expect_err("pre before post");
    assert!(err.message.contains("pre"), "{}", err.message);
    assert!(err.message.contains("Open"), "{}", err.message);
    assert!(!err.message.contains("post"), "{}", err.message);
    assert_eq!(world.state(world.root_id()), Some("Shut"));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(0.0)));
    let tick = world.child(world.root_id(), "tick").unwrap();
    assert_eq!(world.field(tick, "seen"), Some(&json!(0.0)));

    let post_miss = format!(
        r#"
{counter}
component Box {{
  field n: number = 1 {{
    in Idle
      post 1
    in Wide
      pre 0
  }}
  tick: Counter;
  start Idle {{
    on down @PostMiss {{
      n -> 0;
      send to tick @Tick;
      goto Wide;
    }}
  }}
  state Wide {{}}
}}
{scene}
"#
    );
    let mut world =
        World::start(kith::parse::parse_source(&post_miss).expect("parse post")).expect("start");
    world
        .send_document(world.root_id(), kith::doc::symbol("PostMiss"))
        .unwrap();
    let err = world.run_until_idle().expect_err("post after pre");
    assert!(err.message.contains("post"), "{}", err.message);
    assert!(err.message.contains("Idle"), "{}", err.message);
    assert!(!err.message.contains("Wide"), "{}", err.message);
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(1.0)));
    let tick = world.child(world.root_id(), "tick").unwrap();
    assert_eq!(world.field(tick, "seen"), Some(&json!(0.0)));

    let moves = format!(
        r#"
{counter}
component Box {{
  field n: number = 9 {{
    in Idle
      pre 9
    in Open
      pre 1
  }}
  field heard: number = 1;
  tick: Counter;
  start Idle {{
    on down @Break {{
      n -> 0;
      send to tick @Tick;
    }}
    on down @Finish {{
      n -> 0;
      complete 7;
    }}
    on down @Bad {{
      n -> "no";
      goto Open;
    }}
    on down @Pick {{
      n -> 0;
      goto Open;
    }}
    on down @Pick {{
      heard -> 4;
    }}
  }}
  state Open {{}}
}}
{scene}
"#
    );
    let mut world =
        World::start(kith::parse::parse_source(&moves).expect("parse moves")).expect("start");
    world
        .send_document(world.root_id(), kith::doc::symbol("Break"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(0.0)));
    let tick = world.child(world.root_id(), "tick").unwrap();
    assert_eq!(world.field(tick, "seen"), Some(&json!(1.0)));

    let mut world =
        World::start(kith::parse::parse_source(&moves).expect("parse moves")).expect("start");
    world
        .send_document(world.root_id(), kith::doc::symbol("Finish"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert!(world.completed(world.root_id()));
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(0.0)));

    let mut world =
        World::start(kith::parse::parse_source(&moves).expect("parse moves")).expect("start");
    world
        .send_document(world.root_id(), kith::doc::symbol("Bad"))
        .unwrap();
    let err = world.run_until_idle().expect_err("field pattern first");
    assert!(
        err.message.contains("field n did not match"),
        "{}",
        err.message
    );
    assert!(!err.message.contains("pre"), "{}", err.message);
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(9.0)));

    let mut world =
        World::start(kith::parse::parse_source(&moves).expect("parse moves")).expect("start");
    world
        .send_document(world.root_id(), kith::doc::symbol("Pick"))
        .unwrap();
    let err = world.run_until_idle().expect_err("first handler");
    assert!(err.message.contains("pre"), "{}", err.message);
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(1.0)));
    assert_eq!(world.field(world.root_id(), "n"), Some(&json!(9.0)));

    let closed = r#"
component Closed {
  field n: number = 0 {
    in Idle
      pre 1
  }
  start Idle {
    on init {}
  }
}
export scene Main { Closed(); }
"#;
    let err = World::start(kith::parse::parse_source(closed).expect("parse closed"))
        .err()
        .expect("creation pre misses");
    assert!(err.message.contains("pre"), "{}", err.message);
    assert!(err.message.contains("Idle"), "{}", err.message);
}

#[test]
// req:components-026
fn a_field_stays_in_scope_only_in_the_states_it_names() {
    let source = r#"
component Box {
  field total: number = 0;
  field note: number? = null {
    in Open|Hold write
      post number
  }
  field phase: number = 1 {
    in Open write
  }

  start Idle {
    on down @Go {
      total -> 2;
      goto Open;
    }
    on down @Again {
      goto Open;
    }
    on down @Done {
      complete 7;
    }
  }

  state Open {
    on down @Bump {
      note -> 5;
      goto Hold;
    }
    on down @Wipe {
      note -> 9;
      goto Idle;
    }
    on down @Finish {
      complete 3;
    }
  }

  state Hold {
    on down @Drop {
      goto Idle;
    }
  }
}

export scene Main { Box(); }
"#;
    let program = kith::parse::parse_source(source).expect("parse box");
    let note = program
        .component("Box")
        .expect("Box")
        .fields
        .iter()
        .find(|field| field.name == "note")
        .expect("note");
    let rules = note.rules.as_ref().expect("note rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(
        rules[0].states,
        vec!["Open".to_string(), "Hold".to_string()]
    );
    assert!(rules[0].write);
    assert!(rules[0].pre.is_none());
    assert_eq!(rules[0].post.as_ref().expect("post")["kind"], "type");
    assert!(program
        .component("Box")
        .unwrap()
        .fields
        .iter()
        .find(|field| field.name == "total")
        .unwrap()
        .rules
        .is_none());

    let root_fields = |world: &World| {
        let root = world.root_id();
        (
            world.state(root).unwrap().to_string(),
            world.field(root, "total").cloned(),
            world.field(root, "note").cloned(),
            world.field(root, "phase").cloned(),
        )
    };
    let deliver = |world: &mut World, name: &str| {
        world
            .send_document(world.root_id(), kith::doc::symbol(name))
            .unwrap();
        world.run_until_idle().unwrap();
    };

    let mut world = World::start(program.clone()).expect("start");
    let root = world.root_id();
    assert_eq!(world.field(root, "total"), Some(&json!(0.0)));
    assert!(world.field(root, "note").is_none());
    assert!(world.field(root, "phase").is_none());
    deliver(&mut world, "Go");
    assert_eq!(
        root_fields(&world),
        (
            "Open".into(),
            Some(json!(2.0)),
            Some(json!(null)),
            Some(json!(1.0))
        )
    );
    deliver(&mut world, "Bump");
    assert_eq!(
        root_fields(&world),
        ("Hold".into(), Some(json!(2.0)), Some(json!(5.0)), None)
    );
    deliver(&mut world, "Drop");
    assert_eq!(
        root_fields(&world),
        ("Idle".into(), Some(json!(2.0)), None, None)
    );
    deliver(&mut world, "Again");
    assert_eq!(
        root_fields(&world),
        (
            "Open".into(),
            Some(json!(2.0)),
            Some(json!(null)),
            Some(json!(1.0))
        )
    );

    let mut world = World::start(program.clone()).expect("start");
    deliver(&mut world, "Go");
    deliver(&mut world, "Wipe");
    assert_eq!(
        root_fields(&world),
        ("Idle".into(), Some(json!(2.0)), None, None)
    );

    let mut world = World::start(program.clone()).expect("start");
    deliver(&mut world, "Go");
    deliver(&mut world, "Finish");
    assert!(world.completed(world.root_id()));
    assert_eq!(world.state(world.root_id()), Some("Open"));
    assert_eq!(world.field(world.root_id(), "note"), Some(&json!(null)));

    let mut world = World::start(program.clone()).expect("start");
    deliver(&mut world, "Done");
    assert!(world.completed(world.root_id()));
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert!(world.field(world.root_id(), "note").is_none());
    assert_eq!(world.field(world.root_id(), "total"), Some(&json!(0.0)));

    let strict_source = r#"
component Box {
  field strict: number = "no";
  start Idle {}
}
export scene Main { Box(); }
"#;
    let err = World::start(kith::parse::parse_source(strict_source).expect("parse strict"))
        .err()
        .expect("strict misses at create");
    assert!(
        err.message.contains("field strict did not match"),
        "{}",
        err.message
    );

    let enter_source = r#"
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}
component Box {
  field total: number = 0;
  field broken: number = "no" {
    in Tight
  }
  tick: Counter;
  start Idle {
    on down @Bad {
      send to tick @Tick;
      goto Tight;
    }
  }
  state Tight {}
}
export scene Main {
  Box() {
    tick = Counter();
  }
}
"#;
    let mut world = World::start(kith::parse::parse_source(enter_source).expect("parse enter"))
        .expect("broken is absent at create");
    assert_eq!(world.field(world.root_id(), "total"), Some(&json!(0.0)));
    assert!(world.field(world.root_id(), "broken").is_none());
    world
        .send_document(world.root_id(), kith::doc::symbol("Bad"))
        .unwrap();
    let err = world.run_until_idle().expect_err("entering init misses");
    assert!(
        err.message.contains("field broken did not match"),
        "{}",
        err.message
    );
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert_eq!(world.field(world.root_id(), "total"), Some(&json!(0.0)));
    let tick = world.child(world.root_id(), "tick").unwrap();
    assert_eq!(world.field(tick, "seen"), Some(&json!(0.0)));

    let write_source = r#"
component Box {
  field total: number = 0;
  field note: number = 0 {
    in Open write
  }
  start Idle {
    on down @Write {
      note -> 1;
    }
  }
  state Open {}
}
export scene Main { Box(); }
"#;
    let write = kith::parse::parse_source(write_source).expect("parse write");
    let err = World::check(&write).expect_err("write outside the set");
    assert!(err.message.contains("not in scope"), "{}", err.message);
    assert!(err.message.contains("Idle"), "{}", err.message);

    let read_source = r#"
component Box {
  field total: number = 0;
  field note: number = 0 {
    in Open
  }
  start Idle {
    on down @Read {
      total -> note;
    }
  }
  state Open {}
}
export scene Main { Box(); }
"#;
    let read = kith::parse::parse_source(read_source).expect("parse read");
    let err = World::check(&read).expect_err("read outside the set");
    assert!(err.message.contains("not in scope"), "{}", err.message);

    let locked = r#"
component Box {
  field shown: number = 1 {
    in Idle
    in Open write
  }
  start Idle {
    on down @Set {
      shown -> 2;
    }
  }
  state Open {}
}
export scene Main { Box(); }
"#;
    let err = World::check(&kith::parse::parse_source(locked).expect("parse locked"))
        .expect_err("not writable");
    assert!(err.message.contains("not writable"), "{}", err.message);
    assert!(err.message.contains("Idle"), "{}", err.message);

    let leave = r#"
component Box {
  field note: number = 3 {
    in Open
    in Hold write
  }
  start Idle {
    on down @Go {
      goto Open;
    }
  }
  state Open {
    on down @Next {
      goto Hold;
    }
  }
  state Hold {}
}
export scene Main { Box(); }
"#;
    let mut world =
        World::start(kith::parse::parse_source(leave).expect("parse leave")).expect("start");
    world
        .send_document(world.root_id(), kith::doc::symbol("Go"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "note"), Some(&json!(3.0)));
    world
        .send_document(world.root_id(), kith::doc::symbol("Next"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Hold"));
    assert_eq!(world.field(world.root_id(), "note"), Some(&json!(3.0)));

    let dup_source = r#"
component Box {
  field total: number = 0;
  field total: number = 1;
  start Idle {}
}
export scene Main { Box(); }
"#;
    let err = World::check(&kith::parse::parse_source(dup_source).expect("parse duplicate"))
        .expect_err("one name");
    assert!(
        err.message.contains("duplicate field total"),
        "{}",
        err.message
    );

    let twice = r#"
component Box {
  field note: number = 0 {
    in Open
    in Open|Hold
  }
  start Idle {}
  state Open {}
  state Hold {}
}
export scene Main { Box(); }
"#;
    let err = World::check(&kith::parse::parse_source(twice).expect("parse twice"))
        .expect_err("named twice");
    assert!(err.message.contains("names Open twice"), "{}", err.message);

    let unknown = r#"
component Box {
  field note: number = 0 {
    in Nope
  }
  start Idle {}
}
export scene Main { Box(); }
"#;
    let err = World::check(&kith::parse::parse_source(unknown).expect("parse unknown"))
        .expect_err("unknown state");
    assert!(
        err.message.contains("unknown state Nope"),
        "{}",
        err.message
    );

    let mut empty = program.clone();
    empty
        .components
        .iter_mut()
        .find(|component| component.name == "Box")
        .unwrap()
        .fields
        .push(Field {
            kind: "field".into(),
            name: "ghost".into(),
            pattern: None,
            init: None,
            rules: Some(vec![]),
        });
    let err = World::check(&empty).expect_err("empty state list");
    assert!(err.message.contains("names no states"), "{}", err.message);
}

#[test]
// req:components-004
fn unmatched_send_up_drops_and_unmatched_send_to_fails() {
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
                    component: None,
                    message: object_kind("Nope"),
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
        vec![parent, child.clone()],
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
    world
        .run_until_idle()
        .expect("unmatched send down is ignored");

    let empty = scene_program(
        vec![idle(
            "Parent",
            vec![],
            vec![Handler {
                kind: "handler".into(),
                route: vec![],
                item: None,
                pattern: kind_eq("None"),
                body: vec![Statement::SendDown {
                    component: None,
                    message: string_msg(),
                    after: None,
                }],
            }],
        )],
        instance("Parent", None, vec![]),
    );
    let mut world = World::start(empty).expect("start");
    world.inject(json!({"kind": "None"})).unwrap();
    world
        .run_until_idle()
        .expect("send down with no children is not an error");

    let named = scene_program(
        vec![
            idle(
                "Parent",
                vec![port("Child", "child")],
                vec![Handler {
                    kind: "handler".into(),
                    route: vec![],
                    item: None,
                    pattern: kind_eq("Down"),
                    body: vec![Statement::SendDown {
                        component: Some("Child".into()),
                        message: string_msg(),
                        after: None,
                    }],
                }],
            ),
            child.clone(),
        ],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );
    let err = World::check(&named).expect_err("selector is rejected");
    assert!(
        err.message.contains("must not name a component"),
        "{}",
        err.message
    );

    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Missing"})).unwrap();
    let err = world.run_until_idle().expect_err("missing port fails");
    assert!(err.message.contains("unknown port missing"));
}

#[test]
// req:components-003 req:components-004
fn a_send_down_is_ignored_when_a_child_does_not_handle_it() {
    let mixed = kith::parse::parse_source(
        r#"
export Forest;
export Main;
component Deciduous {
  field kept: number = 0;
  start {
    on down @Season @Winter {
      kept -> 1;
    }
  }
}
component Evergreen {
  field kept: number = 0;
  start {
    on down @Drought {
      kept -> 1;
    }
  }
}
component Forest {
  start {
    on down @Go {
      send down @Season @Winter;
    }
  }
}
export scene Main {
  Forest() {
    Deciduous();
    Evergreen();
  }
}
"#,
    )
    .expect("parse mixed forest");
    let mut world = World::start(mixed).expect("start mixed");
    world.inject_document(kith::doc::symbol("Go")).unwrap();
    world.run_until_idle().expect("mixed forest succeeds");
    let root = world.root_id();
    let deciduous = world.children_of_kind(root, "Deciduous")[0];
    let evergreen = world.children_of_kind(root, "Evergreen")[0];
    assert_eq!(world.field(deciduous, "kept"), Some(&json!(1.0)));
    assert_eq!(world.field(evergreen, "kept"), Some(&json!(0.0)));

    let evergreens = kith::parse::parse_source(
        r#"
export Forest;
export Main;
component Evergreen {
  field kept: number = 0;
  start {
    on down @Drought {
      kept -> 1;
    }
  }
}
component Forest {
  start {
    on down @Go {
      send down @Season @Winter;
    }
  }
}
export scene Main {
  Forest() {
    Evergreen();
    Evergreen();
  }
}
"#,
    )
    .expect("parse evergreen forest");
    let mut world = World::start(evergreens).expect("start evergreens");
    world.inject_document(kith::doc::symbol("Go")).unwrap();
    world
        .run_until_idle()
        .expect("a forest of only evergreens succeeds");
    let root = world.root_id();
    let trees = world.children_of_kind(root, "Evergreen");
    assert_eq!(trees.len(), 2);
    assert!(trees
        .iter()
        .all(|id| world.field(*id, "kept") == Some(&json!(0.0))));
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
            rules: None,
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
    let program = scene_program(vec![one], instance("One", None, vec![]));
    let mut world = World::start(program).expect("start");
    world.inject(json!("go")).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "picked"), Some(&json!(1.0)));
}

#[test]
// req:components-006 req:components-017
fn ping_example_sets_heard_to_seven() {
    let mut missed = World::start(ping_program()).expect("start");
    let err = missed
        .inject(json!(["Ping", 7.0]))
        .and_then(|_| missed.run_until_idle())
        .expect_err("string Ping does not match @Ping");
    assert!(err.message.contains("unmatched"), "{}", err.message);

    let mut world = World::start(ping_program()).expect("start");
    world
        .inject_document(kith::doc::array([
            kith::doc::symbol("Ping"),
            kith::doc::number(7.0),
        ]))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(7.0)));
}

#[test]
// req:components-008
fn else_matches_a_message_the_typed_handler_missed() {
    let program = kith::parse::parse_source(
        r#"
export ValidateOrder;
export Main;
component ValidateOrder {
  start {
    on { kind: "Order" id: any } {
      complete { kind: "Accepted" };
    }
    else {
      complete { kind: "Rejected" };
    }
  }
}
export scene Main { ValidateOrder(); }
"#,
    )
    .expect("parse else");
    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Nope"})).unwrap();
    world.run_until_idle().unwrap();
    let output = world.output(world.root_id()).expect("rejected");
    assert_eq!(output["kind"], json!("Rejected"));
}

#[test]
// req:components-010
fn else_complete_uses_field_shorthand() {
    let program = kith::parse::parse_source(
        r#"
export Summarize;
export Main;
component Summarize {
  field orderId: any = "o-1";
  field total: number = 0;
  start {
    on down { kind: "Item" } {
      total -> (add total 1);
    }
    else complete { kind: "Summarize" orderId total };
  }
}
export scene Main { Summarize(); }
"#,
    )
    .expect("parse else complete");
    let mut world = World::start(program).expect("start");
    world.inject(json!({"kind": "Item"})).unwrap();
    world.run_until_idle().unwrap();
    assert!(!world.completed(world.root_id()));
    assert_eq!(world.field(world.root_id(), "total"), Some(&json!(1.0)));
    world.inject(json!({"kind": "Done"})).unwrap();
    world.run_until_idle().unwrap();
    let output = world.output(world.root_id()).expect("summary");
    assert_eq!(output["kind"], json!("Summarize"));
    assert_eq!(output["orderId"], json!("o-1"));
    assert_eq!(output["total"], json!(1.0));
}

#[test]
// req:components-009 req:components-011 req:dispatch-003 req:runtime-001
fn carrier_waits_for_mock_ship_then_completes() {
    let program = kith::parse::parse_source(
        r#"
export MockShip;
export Carrier;
export Main;
component MockShip {
  start {
    on down { kind: "Dispatch" item: any index: number } {
      (writeLine stdout { kind: "InTransit", item: item, index: index });
      send after (rand 1 3) up { kind: "Delivered" };
    }
  }
}
component Carrier {
  field item: string = "";
  field index: number = 0;
  ship: MockShip;
  start Waiting {
    on down { kind: "Item" item: any index: number } {
      item -> item;
      index -> index;
      (writeLine stdout { kind: "PickupRequested", item: item, index: index });
      send to ship { kind: "Dispatch" item index };
      goto Shipping;
    }
  }
  state Shipping {
    on up { kind: "Delivered" } {
      complete { kind: "Shipped" item index };
    }
  }
}
export scene Main {
  Carrier() {
    ship = MockShip();
  }
}
"#,
    )
    .expect("parse carrier");
    let mut world = World::start(program).expect("start");
    world
        .inject(json!({"kind": "Item", "item": "book", "index": 0}))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Shipping"));
    assert!(!world.completed(world.root_id()));
    assert_eq!(
        world.kind(world.child(world.root_id(), "ship").unwrap()),
        Some("MockShip")
    );
    assert!(world
        .prints()
        .iter()
        .any(|value| value["kind"] == json!("PickupRequested")));
    assert!(world
        .prints()
        .iter()
        .any(|value| value["kind"] == json!("InTransit")));
    world.run().unwrap();
    let output = world.output(world.root_id()).expect("shipped");
    assert_eq!(output["kind"], json!("Shipped"));
    assert_eq!(output["item"], json!("book"));
}

fn http_program() -> kith::ast::Program {
    kith::parse::parse_source(include_str!("../examples/health.kith")).expect("parse http.kith")
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
        .send_document(
            client,
            kith::doc::array([
                kith::doc::symbol("Response"),
                kith::doc::number(200.0),
                kith::doc::string("ok"),
            ]),
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
  http: Http;
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
  client: Client;
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
      (writeLine stdout "Healthy");
      send after 5 self "Tick";
      goto Checking;
    }
    on self "Check" a:(number & not 200) b:(number & not 200) c:(number & not 200) {
      (writeLine stdout "Unhealthy");
      send after 5 self "Tick";
      goto Checking;
    }
    else {
      (writeLine stdout "Indeterminate");
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
      (writeLine stdout { kind: "Home" });
    }
  }
  state Away {
    on self { kind: "Ping" } {
      (writeLine stdout { kind: "Away" });
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
      (writeLine stdout { kind: "Down" });
    }
    on { kind: "Tick" } {
      (writeLine stdout { kind: "Implied" });
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
      (writeLine stdout { kind: "Down" });
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
      (writeLine stdout { kind: "Hit" });
    }
    on down { kind: "Tick" } {
      (writeLine stdout { kind: "Down" });
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
      (writeLine stdout { kind: "Hit" });
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
      (writeLine stdout "Healthy");
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
      (writeLine stdout v.status);
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse object capture");
    let mut world = World::start(object).expect("start object");
    world.run_until_idle().unwrap();
    assert!(world.prints().iter().any(|value| value == &json!(200.0)));

    let broken = r#"
export Child;
export Parent;
export Main;
component Child {
  start {
    on down @Listen host:string port:number {
      (writeLine stdout host);
      (writeLine stdout port);
    }
  }
}
component Parent {
  child: Child;
  start {
    on init {
      send to child @Listen
        (coalesce "127.0.0.1")
        (int (coalesce "3000"));
    }
  }
}
export scene Main {
  Parent() { child = Child(); }
}
"#;
    let program = kith::parse::parse_source(broken).expect("parse multiline listen");
    let parent = program.component("Parent").expect("Parent");
    let init = parent.states[0].handlers.iter().next().expect("init");
    match &init.body[0] {
        kith::ast::Statement::SendTo { message, .. } => {
            assert_eq!(message["kind"], "sequence");
            let items = message["items"].as_array().expect("items");
            assert_eq!(items.len(), 3);
            assert_eq!(items[0]["kind"], "symbol");
            assert_eq!(items[0]["name"], "Listen");
        }
        other => panic!("expected send, got {other:?}"),
    }
    let mut world = World::start(program).expect("start multiline listen");
    world.run_until_idle().unwrap();
    assert_eq!(world.prints()[0], json!("127.0.0.1"));
    assert_eq!(world.prints()[1], json!(3000.0));

    let string_name = broken.replace("on down @Listen", "on down \"Listen\"");
    let missed =
        World::start(kith::parse::parse_source(&string_name).expect("parse string listen"))
            .and_then(|mut world| world.run_until_idle())
            .expect_err("a string Listen does not match @Listen");
    assert!(missed.message.contains("unmatched"), "{}", missed.message);
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

#[test]
// req:components-007 req:components-019 req:runtime-004
fn write_line_and_print_record_diagnostics() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      (writeLine stdout "hi");
      (print "Requested" "https://example.com");
      (write stdout "no newline");
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse writeLine");
    let mut world = World::start(program).expect("start");
    world.run_until_idle().unwrap();
    assert_eq!(
        world.prints(),
        &[
            json!("hi"),
            json!(["Requested", "https://example.com"]),
            json!("no newline"),
        ]
    );
    assert!(!world.completed(world.root_id()));
}

#[test]
// req:components-019
fn check_rejects_io_and_async_in_assign_and_expr_stmt() {
    let assign_io = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  field heard: any;
  start {
    on init {
      heard -> (writeLine stdout "n");
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse assign io");
    let err = World::check(&assign_io).expect_err("assign writeLine");
    assert!(err.message.contains("same-turn I/O"));

    let init_io = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  field heard: any = (writeLine stdout "n");
  start {
    on init {}
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse field init io");
    let err = World::check(&init_io).expect_err("field init writeLine");
    assert!(err.message.contains("same-turn I/O"));

    let assign_async = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  field heard: any;
  start {
    on init {
      heard -> (http_get "https://example.com");
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse assign async");
    let err = World::check(&assign_async).expect_err("assign http_get");
    assert!(err.message.contains("async callee"));

    let expr_async = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      (http_get "https://example.com");
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse expr async");
    let err = World::check(&expr_async).expect_err("expr http_get");
    assert!(err.message.contains("async callee"));
}

#[test]
// req:runtime-004
fn unknown_callee_fails_evaluation() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      (nope 1);
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse unknown callee");
    let err = World::start(program)
        .and_then(|mut world| world.run_until_idle())
        .expect_err("unknown callee");
    assert!(!err.message.is_empty());
}

#[test]
// req:runtime-005
fn http_get_result_matches_on_in_the_goto_state() {
    let program = kith::parse::parse_source(
        r#"
export Client;
export Main;
component Client {
  start Idle {
    on down "Go" url:any {
      send self (http_get url);
      goto Fetching;
    }
  }
  state Fetching {
    on { status: 200 url: any } {
      (writeLine stdout "ok");
      goto Idle;
    }
    else {
      (writeLine stdout "unknown");
      goto Idle;
    }
  }
}
export scene Main { Client(); }
"#,
    )
    .expect("parse http_get");
    let mut world = World::start(program).expect("start");
    world.inject(json!(["Go", "https://example.com"])).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Fetching"));
    assert!(world.fire_next_timer().unwrap());
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert_eq!(world.prints(), &[json!("ok")]);
}

#[test]
// req:runtime-005 req:components-019
fn http_get_send_joins_and_goto_is_this_turn() {
    let program = kith::parse::parse_source(
        r#"
export Client;
export Main;
component Client {
  start Idle {
    on down "Go" url1:any url2:any {
      send self (http_get url1) (http_get url2);
      goto Fetching;
    }
  }
  state Fetching {
    on a:any b:any {
      (writeLine stdout a.status);
      (writeLine stdout b.url);
      goto Idle;
    }
  }
}
export scene Main { Client(); }
"#,
    )
    .expect("parse join");
    let mut world = World::start(program).expect("start");
    world
        .inject(json!(["Go", "https://a.example", "https://b.example"]))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Fetching"));
    assert!(world.fire_next_timer().unwrap(), "http_get join");
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert_eq!(world.prints(), &[json!(200.0), json!("https://b.example")]);
}

#[test]
// req:runtime-005
fn send_after_cannot_take_http_get() {
    let program = kith::parse::parse_source(
        r#"
export Box;
export Main;
component Box {
  start {
    on init {
      send after 1 self (http_get "https://example.com");
    }
  }
}
export scene Main { Box(); }
"#,
    )
    .expect("parse send after async");
    let err = World::check(&program).expect_err("send after http_get");
    assert!(err.message.contains("send after"));
}

#[test]
// req:runtime-005
fn http_get_snapshot_restore_still_delivers() {
    let source = r#"
export Client;
export Main;
component Client {
  start Idle {
    on down "Go" url:any {
      send self (http_get url);
      goto Fetching;
    }
  }
  state Fetching {
    on { status: 200 url: any } {
      (writeLine stdout "ok");
      goto Idle;
    }
  }
}
export scene Main { Client(); }
"#;
    let program = kith::parse::parse_source(source).expect("parse http_get snapshot");
    let mut world = World::start(program.clone()).expect("start");
    world.inject(json!(["Go", "https://example.com"])).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Fetching"));
    let snap = world.snapshot().expect("snapshot");
    let mut restored = World::restore(program, &snap).expect("restore");
    assert_eq!(restored.state(restored.root_id()), Some("Fetching"));
    assert!(restored.fire_next_timer().unwrap());
    restored.run_until_idle().unwrap();
    assert_eq!(restored.state(restored.root_id()), Some("Idle"));
    assert_eq!(restored.prints(), &[json!("ok")]);
}
