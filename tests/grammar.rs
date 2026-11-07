use kith::parse::parse_source;
use kith::World;
use serde_json::json;

const PING: &str = include_str!("../examples/ping.kith");

#[test]
// req:grammar-001
fn ping_source_parses_to_the_component_tree() {
    let program = parse_source(PING).expect("parse ping.kith");
    assert_eq!(program.kind, "program");
    assert_eq!(program.root, "Parent");
    let names: Vec<_> = program
        .components
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    assert!(names.contains(&"Child"));
    assert!(names.contains(&"Parent"));
    let parent = program.component("Parent").expect("Parent");
    assert!(parent.fields.iter().any(|field| field.name == "heard"));
    assert!(parent
        .children
        .iter()
        .any(|child| child.name == "child" && child.component == "Child"));

    let mut world = World::start(program).expect("interpreter accepts parsed tree");
    world.inject(json!({"kind": "Ping", "n": 7})).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(world.field(world.root_id(), "heard"), Some(&json!(7.0)));
}
