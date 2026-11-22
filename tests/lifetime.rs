use kith::ast::{
    Component, Export, Field, Handler, Instance, Port, Program, Scene, State, Statement,
};
use kith::interpreter::World;
use serde_json::{json, Value as Json};

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

fn field(name: &str, init: i64, states: Option<Vec<&str>>) -> Field {
    Field {
        kind: "field".into(),
        name: name.into(),
        pattern: None,
        init: Some(json!({"kind": "number", "value": init})),
        states: states.map(|names| names.into_iter().map(str::to_string).collect()),
    }
}

fn handler(pattern: Json, route: &[&str], body: Vec<Statement>) -> Handler {
    Handler {
        kind: "handler".into(),
        pattern,
        item: None,
        route: route.iter().map(|name| (*name).to_string()).collect(),
        body,
    }
}

fn state(
    name: &str,
    start: bool,
    handlers: Vec<Handler>,
    entry: Option<Json>,
    post: Option<Json>,
) -> State {
    State {
        kind: "state".into(),
        name: name.into(),
        start,
        named: true,
        handlers,
        entry,
        post,
    }
}

fn component(name: &str, fields: Vec<Field>, ports: Vec<Port>, states: Vec<State>) -> Component {
    Component {
        kind: "component".into(),
        name: name.into(),
        fields,
        ports,
        states,
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

fn program(components: Vec<Component>, root: Instance) -> Program {
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

fn kind_eq(kind: &str) -> Json {
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

fn any() -> Json {
    json!({"kind": "any"})
}

fn number(value: i64) -> Json {
    json!({"kind": "number", "value": value})
}

fn object_kind(kind: &str) -> Json {
    json!({
        "kind": "object",
        "keys": [{
            "kind": "objectKey",
            "name": "kind",
            "expression": {"kind": "string", "values": [kind]}
        }]
    })
}

fn over_equal(key: &str, value: Json) -> Json {
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

fn assign(field: &str, expression: Json) -> Statement {
    Statement::Assign {
        field: field.into(),
        expression,
    }
}

fn reference(name: &str) -> Json {
    json!({"kind": "reference", "name": name})
}

fn counting(name: &str, field_name: &str) -> Component {
    component(
        name,
        vec![field(field_name, 0, None)],
        vec![],
        vec![state(
            "Idle",
            true,
            vec![handler(any(), &[], vec![assign(field_name, number(1))])],
            None,
            None,
        )],
    )
}

fn stranger(name: &str) -> Component {
    component(
        name,
        vec![field("seen", 0, None)],
        vec![],
        vec![state(
            "Idle",
            true,
            vec![handler(
                kind_eq("Tick"),
                &[],
                vec![assign("seen", number(1))],
            )],
            None,
            None,
        )],
    )
}

fn run(program: Program, message: Json) -> World {
    let mut world = World::start(program).expect("start");
    world.inject(message).unwrap();
    world.run_until_idle().unwrap();
    world
}

#[test]
// req:components-020
fn unnamed_send_down_is_looks_like_and_send_up_is_one_route() {
    let parent = component(
        "Parent",
        vec![field("heard", 0, None)],
        vec![port("Child", "child")],
        vec![state(
            "Idle",
            true,
            vec![
                handler(
                    kind_eq("To"),
                    &[],
                    vec![Statement::SendTo {
                        port: "child".into(),
                        message: object_kind("Tick"),
                        after: None,
                    }],
                ),
                handler(
                    kind_eq("Down"),
                    &[],
                    vec![Statement::SendDown {
                        component: Some("Child".into()),
                        message: object_kind("Tick"),
                        after: None,
                        filter: None,
                    }],
                ),
                handler(
                    kind_eq("All"),
                    &[],
                    vec![Statement::SendDown {
                        component: None,
                        message: object_kind("Tick"),
                        after: None,
                        filter: None,
                    }],
                ),
                handler(
                    kind_eq("Ping"),
                    &[],
                    vec![Statement::SendDown {
                        component: Some("Reply".into()),
                        message: object_kind("Ping"),
                        after: None,
                        filter: None,
                    }],
                ),
                handler(kind_eq("Up"), &["up"], vec![assign("heard", number(1))]),
            ],
            None,
            None,
        )],
    );
    let mock = counting("Mock", "n");
    let named = counting("Child", "seen");
    let other = stranger("Stranger");
    let built = program(
        vec![parent.clone(), named.clone(), mock.clone(), other],
        instance(
            "Parent",
            None,
            vec![
                instance("Child", Some("child"), vec![]),
                instance("Mock", None, vec![]),
                instance("Stranger", None, vec![]),
            ],
        ),
    );

    let world = run(built.clone(), json!({"kind": "To"}));
    let root = world.root_id();
    let child = world.child(root, "child").unwrap();
    let mock_id = world.children_of_kind(root, "Mock")[0];
    let stranger_id = world.children_of_kind(root, "Stranger")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(mock_id, "n"), Some(&json!(0.0)));
    assert_eq!(world.field(stranger_id, "seen"), Some(&json!(0.0)));

    let world = run(built.clone(), json!({"kind": "Down"}));
    let root = world.root_id();
    let child = world.child(root, "child").unwrap();
    let mock_id = world.children_of_kind(root, "Mock")[0];
    let stranger_id = world.children_of_kind(root, "Stranger")[0];
    assert_eq!(world.field(child, "seen"), Some(&json!(1.0)));
    assert_eq!(world.field(mock_id, "n"), Some(&json!(1.0)));
    assert_eq!(world.field(stranger_id, "seen"), Some(&json!(0.0)));
    assert!(world.glossary_key(child).unwrap().is_null());
    assert!(world.glossary_key(mock_id).unwrap().is_null());

    let world = run(built.clone(), json!({"kind": "All"}));
    let root = world.root_id();
    assert_eq!(
        world.field(world.child(root, "child").unwrap(), "seen"),
        Some(&json!(1.0))
    );
    assert_eq!(
        world.field(world.children_of_kind(root, "Mock")[0], "n"),
        Some(&json!(1.0))
    );
    assert_eq!(
        world.field(world.children_of_kind(root, "Stranger")[0], "seen"),
        Some(&json!(1.0))
    );

    let reply = component(
        "Reply",
        vec![],
        vec![],
        vec![state(
            "Idle",
            true,
            vec![handler(
                kind_eq("Ping"),
                &[],
                vec![Statement::SendUp {
                    message: object_kind("Up"),
                    after: None,
                }],
            )],
            None,
            None,
        )],
    );
    let world = run(
        program(
            vec![parent, named.clone(), reply.clone()],
            instance(
                "Parent",
                None,
                vec![
                    instance("Child", Some("child"), vec![]),
                    instance("Reply", None, vec![]),
                ],
            ),
        ),
        json!({"kind": "Ping"}),
    );
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(1.0)));
    assert_eq!(
        world.field(world.child(world.root_id(), "child").unwrap(), "seen"),
        Some(&json!(0.0))
    );

    let mut filled = program(
        vec![
            component(
                "Parent",
                vec![],
                vec![port("Child", "child")],
                vec![state("Idle", true, vec![], None, None)],
            ),
            named,
            mock,
        ],
        instance(
            "Parent",
            None,
            vec![instance("Mock", Some("child"), vec![])],
        ),
    );
    World::check(&filled).expect("a mock may fill the port");
    filled.scenes[0].root.children[0].component = "Stranger".into();
    filled.components.push(stranger("Stranger"));
    let err = World::check(&filled).expect_err("stranger does not satisfy Child");
    assert!(err.message.contains("expects Child"), "{}", err.message);
}

#[test]
// req:components-021
fn filtered_send_down_matches_the_parent_record_and_fails_when_empty() {
    let child = counting("Child", "seen");
    let other = counting("Other", "seen");
    let parent = component(
        "Parent",
        vec![field("secret", 1, None)],
        vec![port("Child", "child")],
        vec![state(
            "Idle",
            true,
            vec![
                handler(
                    kind_eq("Port"),
                    &[],
                    vec![Statement::SendDown {
                        component: None,
                        message: object_kind("Tick"),
                        after: None,
                        filter: Some(over_equal("port", json!("child"))),
                    }],
                ),
                handler(
                    kind_eq("Kind"),
                    &[],
                    vec![Statement::SendDown {
                        component: None,
                        message: object_kind("Tick"),
                        after: None,
                        filter: Some(over_equal("component", json!("Child"))),
                    }],
                ),
                handler(
                    kind_eq("Secret"),
                    &[],
                    vec![Statement::SendDown {
                        component: None,
                        message: object_kind("Tick"),
                        after: None,
                        filter: Some(over_equal("secret", json!(1))),
                    }],
                ),
                handler(
                    kind_eq("None"),
                    &[],
                    vec![Statement::SendDown {
                        component: None,
                        message: object_kind("Tick"),
                        after: None,
                        filter: Some(over_equal("port", json!("missing"))),
                    }],
                ),
            ],
            None,
            None,
        )],
    );
    let built = program(
        vec![parent.clone(), child.clone(), other.clone()],
        instance(
            "Parent",
            None,
            vec![
                instance("Child", Some("child"), vec![]),
                instance("Other", None, vec![]),
            ],
        ),
    );
    let world = run(built.clone(), json!({"kind": "Port"}));
    let root = world.root_id();
    assert_eq!(
        world.field(world.child(root, "child").unwrap(), "seen"),
        Some(&json!(1.0))
    );
    assert_eq!(
        world.field(world.children_of_kind(root, "Other")[0], "seen"),
        Some(&json!(0.0))
    );

    let world = run(built.clone(), json!({"kind": "Kind"}));
    let root = world.root_id();
    assert_eq!(
        world.field(world.child(root, "child").unwrap(), "seen"),
        Some(&json!(1.0))
    );
    assert_eq!(
        world.field(world.children_of_kind(root, "Other")[0], "seen"),
        Some(&json!(1.0))
    );

    let mut world = World::start(built.clone()).unwrap();
    world.inject(json!({"kind": "Secret"})).unwrap();
    let err = world
        .run_until_idle()
        .expect_err("filter cannot see fields");
    assert!(err.message.contains("no child matched"), "{}", err.message);

    let mut world = World::start(built).unwrap();
    world.inject(json!({"kind": "None"})).unwrap();
    let err = world.run_until_idle().expect_err("zero matches");
    assert!(err.message.contains("no child matched"), "{}", err.message);

    let grand = counting("Grand", "seen");
    let quiet = component(
        "Quiet",
        vec![],
        vec![port("Grand", "inner")],
        vec![state(
            "Idle",
            true,
            vec![handler(kind_eq("Yes"), &[], vec![])],
            None,
            None,
        )],
    );
    let selector = component(
        "Parent",
        vec![],
        vec![port("Quiet", "child")],
        vec![state(
            "Idle",
            true,
            vec![handler(
                kind_eq("Miss"),
                &[],
                vec![Statement::SendDown {
                    component: None,
                    message: object_kind("Nope"),
                    after: None,
                    filter: Some(over_equal("port", json!("child"))),
                }],
            )],
            None,
            None,
        )],
    );
    let mut world = World::start(program(
        vec![selector, quiet, grand],
        instance(
            "Parent",
            None,
            vec![instance(
                "Quiet",
                Some("child"),
                vec![instance("Grand", Some("inner"), vec![])],
            )],
        ),
    ))
    .unwrap();
    world.inject(json!({"kind": "Miss"})).unwrap();
    let err = world.run_until_idle().expect_err("unmatched down");
    assert!(err.message.contains("unmatched down"), "{}", err.message);
    let quiet_id = world.child(world.root_id(), "child").unwrap();
    let grand_id = world.child(quiet_id, "inner").unwrap();
    assert_eq!(world.field(grand_id, "seen"), Some(&json!(0.0)));
}

#[test]
// req:components-022 req:components-023
fn unreachable_children_are_destroyed_between_turns() {
    let item = component(
        "Item",
        vec![field("seen", 0, None)],
        vec![port("Grand", "inner")],
        vec![state(
            "Idle",
            true,
            vec![
                handler(
                    any(),
                    &["init"],
                    vec![Statement::SendUp {
                        message: object_kind("Hi"),
                        after: None,
                    }],
                ),
                handler(
                    kind_eq("Ping"),
                    &[],
                    vec![Statement::SendUp {
                        message: object_kind("Pong"),
                        after: None,
                    }],
                ),
            ],
            None,
            None,
        )],
    );
    let grand = component(
        "Grand",
        vec![],
        vec![],
        vec![state("Idle", true, vec![], None, None)],
    );
    let parent = component(
        "Parent",
        vec![field("heard", 0, None), field("reply", 0, None)],
        vec![port("Child", "child")],
        vec![state(
            "Idle",
            true,
            vec![
                handler(kind_eq("Hi"), &["up"], vec![assign("heard", number(1))]),
                handler(kind_eq("Pong"), &["up"], vec![assign("reply", number(1))]),
                handler(
                    kind_eq("Make"),
                    &[],
                    vec![Statement::Construct {
                        component: "Item".into(),
                        key: Some(number(7)),
                        children: vec![instance("Grand", Some("inner"), vec![])],
                    }],
                ),
                handler(
                    kind_eq("Later"),
                    &[],
                    vec![Statement::SendDown {
                        component: Some("Item".into()),
                        message: object_kind("Ping"),
                        after: Some(number(1)),
                        filter: None,
                    }],
                ),
                handler(
                    kind_eq("Drop"),
                    &[],
                    vec![Statement::Release {
                        filter: over_equal("key", json!(7)),
                    }],
                ),
                handler(
                    kind_eq("Scene"),
                    &[],
                    vec![Statement::Release {
                        filter: over_equal("port", json!("child")),
                    }],
                ),
                handler(
                    kind_eq("Done"),
                    &[],
                    vec![Statement::Complete { message: number(1) }],
                ),
            ],
            None,
            None,
        )],
    );
    let child = component(
        "Child",
        vec![field("seen", 0, None)],
        vec![],
        vec![state("Idle", true, vec![], None, None)],
    );
    let built = program(
        vec![parent, child, item, grand],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );

    let mut world = World::start(built.clone()).unwrap();
    world.inject(json!({"kind": "Make"})).unwrap();
    world.run_until_idle().unwrap();
    let root = world.root_id();
    assert_eq!(world.field(root, "heard"), Some(&json!(1.0)));
    let item_id = world.children_of_kind(root, "Item")[0];
    let grand_id = world.children_of_kind(item_id, "Grand")[0];
    assert_eq!(world.glossary_key(item_id), Some(json!(7.0)));
    assert!(world.child(root, "child").is_some());
    assert_ne!(world.child(root, "child"), Some(item_id));

    world.inject(json!({"kind": "Scene"})).unwrap();
    let err = world.run_until_idle().expect_err("scene child stays");
    assert!(err.message.contains("scene-constructed"), "{}", err.message);
    assert!(world.child(root, "child").is_some());
    assert!(world.kind(item_id).is_some());

    world.inject(json!({"kind": "Later"})).unwrap();
    world.run_until_idle().unwrap();
    world.inject(json!({"kind": "Drop"})).unwrap();
    world.run_until_idle().unwrap();
    assert!(world.kind(item_id).is_none());
    assert!(world.kind(grand_id).is_none());
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
    let item = component(
        "Item",
        vec![field("phase", 0, None), field("saw", 0, None)],
        vec![port("Leaf", "inner")],
        vec![state(
            "Idle",
            true,
            vec![
                handler(
                    any(),
                    &["init"],
                    vec![
                        assign("phase", number(1)),
                        Statement::SendUp {
                            message: object_kind("Hi"),
                            after: None,
                        },
                    ],
                ),
                handler(any(), &["down"], vec![assign("saw", reference("phase"))]),
            ],
            None,
            None,
        )],
    );
    let leaf = counting("Leaf", "seen");
    let mock = counting("MockLeaf", "n");
    let stranger = stranger("Stranger");
    let parent = component(
        "Parent",
        vec![field("heard", 0, None)],
        vec![port("Child", "child")],
        vec![state(
            "Idle",
            true,
            vec![
                handler(
                    kind_eq("Make"),
                    &[],
                    vec![
                        Statement::Construct {
                            component: "Item".into(),
                            key: Some(number(1)),
                            children: vec![instance("MockLeaf", Some("inner"), vec![])],
                        },
                        Statement::Construct {
                            component: "Item".into(),
                            key: Some(number(2)),
                            children: vec![instance("Leaf", Some("inner"), vec![])],
                        },
                        Statement::SendDown {
                            component: None,
                            message: object_kind("Tick"),
                            after: None,
                            filter: Some(over_equal("key", json!(1))),
                        },
                    ],
                ),
                handler(
                    kind_eq("DropEarly"),
                    &[],
                    vec![
                        Statement::Construct {
                            component: "Item".into(),
                            key: None,
                            children: vec![instance("Leaf", Some("inner"), vec![])],
                        },
                        Statement::Release {
                            filter: over_equal("component", json!("Item")),
                        },
                    ],
                ),
                handler(kind_eq("Hi"), &["up"], vec![assign("heard", number(1))]),
            ],
            None,
            None,
        )],
    );
    let kept = component(
        "Child",
        vec![field("seen", 0, None)],
        vec![],
        vec![state("Idle", true, vec![], None, None)],
    );
    let built = program(
        vec![
            parent.clone(),
            kept.clone(),
            item.clone(),
            leaf.clone(),
            mock.clone(),
        ],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );
    let world = run(built, json!({"kind": "Make"}));
    let root = world.root_id();
    let items = world.children_of_kind(root, "Item");
    assert_eq!(items.len(), 2);
    assert!(world.child(root, "child").is_some());
    assert!(items
        .iter()
        .all(|id| world.child(root, "child") != Some(*id)));
    let first = items
        .iter()
        .copied()
        .find(|id| world.glossary_key(*id) == Some(json!(1.0)))
        .unwrap();
    let second = items
        .iter()
        .copied()
        .find(|id| world.glossary_key(*id) == Some(json!(2.0)))
        .unwrap();
    assert_eq!(world.field(first, "phase"), Some(&json!(1.0)));
    assert_eq!(world.field(first, "saw"), Some(&json!(1.0)));
    assert_eq!(world.field(second, "phase"), Some(&json!(1.0)));
    assert_eq!(world.field(second, "saw"), Some(&json!(0.0)));
    assert!(world.child(first, "inner").is_some());
    assert_eq!(
        world.kind(world.child(first, "inner").unwrap()),
        Some("MockLeaf")
    );

    let early = program(
        vec![parent, kept, item, leaf, mock, stranger],
        instance(
            "Parent",
            None,
            vec![instance("Child", Some("child"), vec![])],
        ),
    );
    let world = run(early.clone(), json!({"kind": "DropEarly"}));
    assert!(world.children_of_kind(world.root_id(), "Item").is_empty());
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(0.0)));

    let mut bad = early;
    bad.components[0].states[0].handlers[0].body[0] = Statement::Construct {
        component: "Item".into(),
        key: None,
        children: vec![instance("Stranger", Some("inner"), vec![])],
    };
    let err = World::check(&bad).expect_err("port fill must satisfy Leaf");
    assert!(err.message.contains("expects Leaf"), "{}", err.message);
}

#[test]
// req:components-024 req:components-025
fn entry_then_post_must_match_before_commit() {
    let seen_eq = |value: i64| over_equal("seen", json!(value));
    let machine = |post: Option<Json>, entry_hot: Option<Json>, body: Vec<Handler>| {
        component(
            "Machine",
            vec![field("seen", 0, None)],
            vec![],
            vec![
                state("Idle", true, body, None, post),
                state("Hot", false, vec![], entry_hot, None),
            ],
        )
    };
    let miss = machine(
        Some(seen_eq(0)),
        None,
        vec![handler(
            kind_eq("Go"),
            &[],
            vec![
                assign("seen", number(1)),
                Statement::Send {
                    message: object_kind("Tick"),
                    after: None,
                },
                Statement::Goto {
                    state: "Idle".into(),
                },
            ],
        )],
    );
    // Goto of the same named state is legal. The post misses after the assign.
    let mut world = World::start(program(vec![miss], instance("Machine", None, vec![]))).unwrap();
    world.inject(json!({"kind": "Go"})).unwrap();
    let err = world.run_until_idle().expect_err("post miss");
    assert!(err.message.contains("post-condition"), "{}", err.message);
    assert_eq!(world.field(world.root_id(), "seen"), Some(&json!(0.0)));
    assert_eq!(world.state(world.root_id()), Some("Idle"));

    let complete = component(
        "Machine",
        vec![field("seen", 0, None)],
        vec![],
        vec![state(
            "Idle",
            true,
            vec![handler(
                kind_eq("Go"),
                &[],
                vec![
                    assign("seen", number(1)),
                    Statement::Complete { message: number(1) },
                ],
            )],
            None,
            Some(seen_eq(0)),
        )],
    );
    let mut world =
        World::start(program(vec![complete], instance("Machine", None, vec![]))).unwrap();
    world.inject(json!({"kind": "Go"})).unwrap();
    let err = world
        .run_until_idle()
        .expect_err("complete still checks post");
    assert!(err.message.contains("post-condition"), "{}", err.message);
    assert!(world.output(world.root_id()).is_none());
    assert_eq!(world.field(world.root_id(), "seen"), Some(&json!(0.0)));

    let created = component(
        "Machine",
        vec![field("seen", 0, None)],
        vec![],
        vec![state("Idle", true, vec![], None, Some(seen_eq(1)))],
    );
    let err = match World::start(program(vec![created], instance("Machine", None, vec![]))) {
        Err(err) => err,
        Ok(_) => panic!("creation post"),
    };
    assert!(err.message.contains("post-condition"), "{}", err.message);

    let entry_miss = component(
        "Machine",
        vec![field("seen", 0, None)],
        vec![],
        vec![state(
            "Idle",
            true,
            vec![],
            Some(seen_eq(1)),
            Some(seen_eq(0)),
        )],
    );
    let err = match World::start(program(vec![entry_miss], instance("Machine", None, vec![]))) {
        Err(err) => err,
        Ok(_) => panic!("entry before post"),
    };
    assert!(err.message.contains("entry pattern"), "{}", err.message);

    let goto = machine(
        None,
        Some(seen_eq(1)),
        vec![
            handler(
                kind_eq("Go"),
                &[],
                vec![Statement::Goto {
                    state: "Hot".into(),
                }],
            ),
            handler(any(), &[], vec![assign("seen", number(7))]),
        ],
    );
    let mut world = World::start(program(vec![goto], instance("Machine", None, vec![]))).unwrap();
    world.inject(json!({"kind": "Go"})).unwrap();
    let err = world.run_until_idle().expect_err("entry miss rolls back");
    assert!(err.message.contains("entry pattern"), "{}", err.message);
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert_eq!(world.field(world.root_id(), "seen"), Some(&json!(0.0)));
}

#[test]
// req:components-026
fn state_scoped_fields_persist_inside_the_set_and_initialize_on_entry() {
    let token_states = Some(vec!["Hot", "Warm"]);
    let machine = component(
        "Machine",
        vec![field("base", 0, None), field("token", 0, token_states)],
        vec![],
        vec![
            state(
                "Idle",
                true,
                vec![handler(
                    kind_eq("Enter"),
                    &[],
                    vec![
                        assign("base", number(1)),
                        Statement::Goto {
                            state: "Hot".into(),
                        },
                    ],
                )],
                None,
                None,
            ),
            state(
                "Hot",
                false,
                vec![handler(
                    kind_eq("Next"),
                    &[],
                    vec![
                        assign("token", number(5)),
                        Statement::Goto {
                            state: "Warm".into(),
                        },
                    ],
                )],
                Some(json!({
                    "kind": "over",
                    "keys": {
                        "base": {"kind": "equal", "value": {"kind": "value.literal", "value": 1}},
                        "token": {"kind": "equal", "value": {"kind": "value.literal", "value": 0}}
                    }
                })),
                None,
            ),
            state(
                "Warm",
                false,
                vec![handler(
                    kind_eq("Leave"),
                    &[],
                    vec![Statement::Goto {
                        state: "Idle".into(),
                    }],
                )],
                Some(json!({
                    "kind": "over",
                    "keys": {
                        "base": {"kind": "equal", "value": {"kind": "value.literal", "value": 1}},
                        "token": {"kind": "equal", "value": {"kind": "value.literal", "value": 5}}
                    }
                })),
                None,
            ),
        ],
    );
    let built = program(vec![machine], instance("Machine", None, vec![]));
    let mut world = World::start(built).unwrap();
    assert_eq!(world.field(world.root_id(), "base"), Some(&json!(0.0)));
    assert!(world.field(world.root_id(), "token").is_none());
    world.inject(json!({"kind": "Enter"})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Hot"));
    assert_eq!(world.field(world.root_id(), "base"), Some(&json!(1.0)));
    assert_eq!(world.field(world.root_id(), "token"), Some(&json!(0.0)));
    world.inject(json!({"kind": "Next"})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Warm"));
    assert_eq!(world.field(world.root_id(), "token"), Some(&json!(5.0)));
    world.inject(json!({"kind": "Leave"})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.state(world.root_id()), Some("Idle"));
    assert!(world.field(world.root_id(), "token").is_none());
    assert_eq!(world.field(world.root_id(), "base"), Some(&json!(1.0)));

    let outside = component(
        "Machine",
        vec![field("token", 0, Some(vec!["Hot"]))],
        vec![],
        vec![
            state(
                "Idle",
                true,
                vec![handler(
                    kind_eq("Go"),
                    &[],
                    vec![assign("token", number(1))],
                )],
                None,
                None,
            ),
            state("Hot", false, vec![], None, None),
        ],
    );
    let err = World::check(&program(vec![outside], instance("Machine", None, vec![]))).unwrap_err();
    assert!(err.message.contains("not in scope"), "{}", err.message);

    let shared = component(
        "Machine",
        vec![
            field("token", 0, None),
            field("token", 0, Some(vec!["Hot"])),
        ],
        vec![],
        vec![
            state("Idle", true, vec![], None, None),
            state("Hot", false, vec![], None, None),
        ],
    );
    let err = World::check(&program(vec![shared], instance("Machine", None, vec![]))).unwrap_err();
    assert!(err.message.contains("duplicate field"), "{}", err.message);

    let named = component(
        "Machine",
        vec![field("token", 0, Some(vec!["Missing"]))],
        vec![],
        vec![state("Idle", true, vec![], None, None)],
    );
    let err = World::check(&program(vec![named], instance("Machine", None, vec![]))).unwrap_err();
    assert!(err.message.contains("unknown state"), "{}", err.message);

    let entry = component(
        "Machine",
        vec![field("token", 0, Some(vec!["Hot"]))],
        vec![],
        vec![
            state(
                "Idle",
                true,
                vec![],
                Some(over_equal("token", json!(0))),
                None,
            ),
            state("Hot", false, vec![], None, None),
        ],
    );
    let err = World::check(&program(vec![entry], instance("Machine", None, vec![]))).unwrap_err();
    assert!(err.message.contains("not in scope"), "{}", err.message);
}
