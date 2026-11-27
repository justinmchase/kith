use kith::codec::{decode, encode};
use kith::doc::{self, Doc};
use kith::parse::resolve_program;
use kith::World;
use serde_json::{json, Value as Json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn scratch(source: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "kith-stdlib-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.kith"), source).unwrap();
    dir
}

fn load(source: &str) -> Result<kith::ast::Program, kith::Error> {
    resolve_program(&scratch(source).join("main.kith"))
}

fn start(source: &str) -> World {
    World::start(load(source).expect("resolve")).expect("start")
}

fn root(world: &World) -> kith::ActorId {
    world.root_id()
}

fn send(world: &mut World, message: &str) {
    world.send(root(world), json!(message)).unwrap();
    world.run_until_idle().unwrap();
}

fn fail(world: &mut World, message: &str) -> String {
    world.send(root(world), json!(message)).unwrap();
    world.run_until_idle().expect_err("turn").message
}

fn number(world: &World) -> f64 {
    world
        .field(root(world), "n")
        .and_then(Json::as_f64)
        .unwrap_or_else(|| panic!("n {:?}", world.field(root(world), "n")))
}

fn text(world: &World) -> String {
    doc::as_str(&doc_field(world, "got")).unwrap()
}

fn doc_field(world: &World, name: &str) -> Doc {
    world
        .field_document(root(world), name)
        .unwrap_or_else(doc::null)
}

fn snapshot_json(world: &World) -> Json {
    decode(&world.snapshot().unwrap()).unwrap()
}

fn counter(world: &World) -> String {
    snapshot_json(world)["idCounter"]
        .as_str()
        .expect("idCounter text")
        .to_string()
}

fn restore_counter(source: &str, counter: Option<&str>) -> World {
    let program = load(source).expect("resolve");
    let mut world = World::start(program.clone()).unwrap();
    world.run_until_idle().unwrap();
    let mut snap = snapshot_json(&world);
    match counter {
        Some(value) => snap["idCounter"] = json!(value),
        None => {
            snap.as_object_mut().unwrap().remove("idCounter");
        }
    }
    World::restore(program, &encode(&snap).unwrap()).unwrap()
}

const ID: &str = r#"
export Probe;
export Main;
component Probe {
  field got: any;
  field n: number = 0;
  start {
    on down "Id" { got -> (id); }
    on down "Bad" { got -> (id 1); }
    on down "Rand" { n -> (rand 0 1000000); }
  }
}
export scene Main { Probe() {} }
"#;

#[test]
// req:runtime-007
fn id_is_an_explicit_sortable_object_id() {
    let mut fresh = start(ID);
    fresh.run_until_idle().unwrap();
    assert_eq!(counter(&fresh), "0", "construction must not mint an id");
    send(&mut fresh, "Id");
    assert_eq!(text(&fresh), "000000000000000000000001");
    assert_eq!(counter(&fresh), "1");
    send(&mut fresh, "Id");
    let second = text(&fresh);
    assert_eq!(second, "000000000000000000000002");
    assert!("000000000000000000000001" < second.as_str());

    let mut boundary = restore_counter(ID, Some("16777214"));
    send(&mut boundary, "Id");
    let low = text(&boundary);
    assert_eq!(low, "000000000000000000ffffff");
    let mut carried = restore_counter(ID, Some("16777215"));
    send(&mut carried, "Id");
    let high = text(&carried);
    assert_eq!(high, "000000010000000000000000");
    assert!(low < high);

    let mut omitted = restore_counter(ID, None);
    send(&mut omitted, "Id");
    assert_eq!(text(&omitted), "000000000000000000000001");

    let program = load(ID).unwrap();
    let mut world = World::start(program.clone()).unwrap();
    send(&mut world, "Id");
    let snap = world.snapshot().unwrap();
    let mut restored = World::restore(program, &snap).unwrap();
    send(&mut restored, "Id");
    assert_eq!(text(&restored), "000000000000000000000002");

    let mut exhausted = restore_counter(ID, Some("72057594037927935"));
    let error = fail(&mut exhausted, "Id");
    assert!(error.contains("exhausted"), "{error}");
    assert_eq!(counter(&exhausted), "72057594037927935");

    let mut bad = start(ID);
    let error = fail(&mut bad, "Bad");
    assert!(error.contains("argument"), "{error}");

    let mut plain = start(ID);
    send(&mut plain, "Rand");
    let mut after_id = start(ID);
    send(&mut after_id, "Id");
    send(&mut after_id, "Rand");
    assert_eq!(number(&plain), number(&after_id));
}

const DRAWS: &str = r#"
import "kith:crypto" Crypto;
import "kith:encoding" hex_encode;
export Probe;
export Main;
component Probe {
  field got: any;
  field n: number = 0;
  crypto: Crypto;
  start {
    on down "Unit" { n -> (random); }
    on down "Add" { n -> (add (random) 0); }
    on down "Wide" { n -> (uniform 10 20); }
    on down "Back" { n -> (uniform 5 1); }
    on down "Same" { n -> (uniform 1 1); }
    on down "Extra" { n -> (random 1); }
    on down "Zero" { n -> (bernoulli 0); }
    on down "One" { n -> (bernoulli 1); }
    on down "P" { n -> (bernoulli 2); }
    on down "Norm" { n -> (normal 0 1); }
    on down "Flat" { n -> (normal 0 0); }
    on down "Rand" { n -> (rand 0 1000000); }
    on down "Bytes" { send to crypto "Random" 4; }
    on up "Bytes" value:any { got -> (hex_encode value); }
  }
}
export scene Main { Probe() { crypto = Crypto(); } }
"#;

fn last_number(messages: &[&str]) -> f64 {
    let mut world = start(DRAWS);
    for message in messages {
        send(&mut world, message);
    }
    number(&world)
}

fn after_failure(bad: &str, then: &str) -> f64 {
    let mut world = start(DRAWS);
    let _ = fail(&mut world, bad);
    send(&mut world, then);
    number(&world)
}

fn bytes_of(prefix: &[&str]) -> String {
    let mut world = start(DRAWS);
    for message in prefix {
        send(&mut world, message);
    }
    send(&mut world, "Bytes");
    text(&world)
}

#[test]
// req:random-001
fn numeric_draws_share_the_rand_stream() {
    let mut world = start(DRAWS);
    send(&mut world, "Unit");
    let first = number(&world);
    assert!((0.0..1.0).contains(&first), "{first}");
    send(&mut world, "Unit");
    let second = number(&world);
    assert!((0.0..1.0).contains(&second), "{second}");
    assert_ne!(first, second);
    assert_eq!(last_number(&["Add"]), last_number(&["Unit"]));

    let mut wide = start(DRAWS);
    send(&mut wide, "Wide");
    let uniform = number(&wide);
    assert!((10.0..20.0).contains(&uniform), "{uniform}");

    let mut zero = start(DRAWS);
    send(&mut zero, "Zero");
    assert_eq!(number(&zero), 0.0);
    let mut one = start(DRAWS);
    send(&mut one, "One");
    assert_eq!(number(&one), 1.0);

    let mut normal = start(DRAWS);
    send(&mut normal, "Norm");
    assert!(number(&normal).is_finite());
    assert_eq!(last_number(&["Norm"]), last_number(&["Norm"]));
    assert_eq!(
        last_number(&["Norm", "Rand"]),
        last_number(&["Rand", "Rand", "Rand"])
    );
    assert_eq!(
        last_number(&["Unit", "Rand"]),
        last_number(&["Rand", "Rand"])
    );
    assert_eq!(after_failure("Back", "Rand"), last_number(&["Rand"]));
    assert_eq!(after_failure("Same", "Rand"), last_number(&["Rand"]));
    assert_eq!(after_failure("P", "Rand"), last_number(&["Rand"]));
    assert_eq!(after_failure("Flat", "Rand"), last_number(&["Rand"]));
    assert_eq!(after_failure("Extra", "Rand"), last_number(&["Rand"]));

    let mut failed = start(DRAWS);
    let error = fail(&mut failed, "Extra");
    assert!(error.contains("argument"), "{error}");
    let back = fail(&mut failed, "Back");
    assert!(back.contains("less than"), "{back}");

    let program = load(DRAWS).unwrap();
    let mut world = World::start(program.clone()).unwrap();
    send(&mut world, "Unit");
    let snap = world.snapshot().unwrap();
    let mut restored = World::restore(program, &snap).unwrap();
    send(&mut restored, "Unit");
    assert_eq!(number(&restored), second);

    assert_eq!(bytes_of(&[]), bytes_of(&["Unit"]));
    assert_eq!(last_number(&["Bytes", "Rand"]), last_number(&["Rand"]));
}

const SEEDED: &str = r#"
import "kith:random" Random;
import "kith:crypto" Crypto;
import "kith:encoding" hex_encode;
export Probe;
export Main;
component Probe {
  field got: any;
  field n: number = 0;
  rng: Random;
  crypto: Crypto;
  start {
    on down "Unit" { send to rng "Unit"; }
    on down "Range" { send to rng "Range" 0 1000000; }
    on down "Wide" { send to rng "Uniform" 10 20; }
    on down "Back" { send to rng "Uniform" 5 1; }
    on down "Zero" { send to rng "Bernoulli" 0; }
    on down "One" { send to rng "Bernoulli" 1; }
    on down "P" { send to rng "Bernoulli" 2; }
    on down "Norm" { send to rng "Normal" 0 1; }
    on down "Flat" { send to rng "Normal" 0 0; }
    on down "Nope" { send to rng "Nope"; }
    on down "Rand" { n -> (rand 0 1000000); }
    on down "Bytes" { send to crypto "Random" 4; }
    on up "Number" value:number { n -> value; }
    on up "Error" reason:string { got -> reason; }
    on up "Bytes" value:any { got -> (hex_encode value); }
  }
}
export scene Main {
  Probe() {
    rng = Random(seed: 1);
    crypto = Crypto();
  }
}
"#;

fn seeded(seed: &str) -> String {
    SEEDED.replace("Random(seed: 1)", &format!("Random(seed: {seed})"))
}

fn seeded_number(seed: &str, messages: &[&str]) -> f64 {
    let mut world = start(&seeded(seed));
    for message in messages {
        send(&mut world, message);
    }
    number(&world)
}

fn construction(source: &str) -> String {
    match load(source) {
        Err(error) => error.message,
        Ok(program) => match World::start(program) {
            Err(error) => error.message,
            Ok(_) => panic!("construction was accepted"),
        },
    }
}

fn random_program(args: &str) -> String {
    format!(
        r#"
import "kith:random" Random;
export Main;
component Probe {{
  rng: Random;
  start {{}}
}}
export scene Main {{
  Probe() {{ rng = {args}; }}
}}
"#
    )
}

#[test]
// req:random-002
fn random_component_fixes_a_sequence_from_its_seed() {
    let first = seeded_number("1", &["Unit"]);
    assert!((0.0..1.0).contains(&first), "{first}");
    assert_eq!(first, seeded_number("1", &["Unit"]));
    assert_ne!(first, seeded_number("2", &["Unit"]));
    assert_eq!(seeded_number("1", &["Range"]), last_number(&["Rand"]));

    let wide = seeded_number("1", &["Wide"]);
    assert!((10.0..20.0).contains(&wide), "{wide}");
    assert_eq!(seeded_number("1", &["Zero"]), 0.0);
    assert_eq!(seeded_number("1", &["One"]), 1.0);
    let normal = seeded_number("1", &["Norm"]);
    assert!(normal.is_finite(), "{normal}");
    assert_eq!(normal, seeded_number("1", &["Norm"]));

    let mut missed = start(SEEDED);
    send(&mut missed, "Nope");
    assert_eq!(text(&missed), "unknown");
    send(&mut missed, "Unit");
    assert_eq!(number(&missed), first);
    for message in ["Back", "P", "Flat"] {
        let mut world = start(SEEDED);
        send(&mut world, message);
        send(&mut world, "Unit");
        assert_eq!(number(&world), first, "{message}");
    }
    let mut bounds = start(SEEDED);
    send(&mut bounds, "Back");
    assert_eq!(text(&bounds), "bounds");
    let mut probability = start(SEEDED);
    send(&mut probability, "P");
    assert_eq!(text(&probability), "probability");
    let mut deviation = start(SEEDED);
    send(&mut deviation, "Flat");
    assert_eq!(text(&deviation), "deviation");

    assert_eq!(
        seeded_number("1", &["Unit", "Rand"]),
        last_number(&["Rand"])
    );
    let mut bytes = start(SEEDED);
    send(&mut bytes, "Bytes");
    let alone = text(&bytes);
    send(&mut bytes, "Unit");
    let mut with_draw = start(SEEDED);
    send(&mut with_draw, "Unit");
    send(&mut with_draw, "Bytes");
    assert_eq!(text(&with_draw), alone);

    let program = load(SEEDED).unwrap();
    let mut world = World::start(program.clone()).unwrap();
    send(&mut world, "Unit");
    let snap = snapshot_json(&world);
    let driver = snap["instances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["kind"] == "Random")
        .unwrap()["driver"]
        .clone();
    assert!(driver["draws"].is_null());
    assert!(driver["state"].as_str().is_some());
    let mut restored = World::restore(program, &world.snapshot().unwrap()).unwrap();
    send(&mut restored, "Unit");
    assert_eq!(number(&restored), seeded_number("1", &["Unit", "Unit"]));

    for (args, needle) in [
        ("Random()", "seed"),
        ("Random(seed: \"no\")", "seed"),
        ("Random(seed: (sub 0 1))", "seed"),
        ("Random(seed: 1, seed: 1)", "duplicate"),
        ("Random(seed: 1 extra: 1)", "unknown field"),
    ] {
        let error = construction(&random_program(args));
        assert!(error.contains(needle), "{args}: {error}");
    }
    let fraction = construction(
        r#"
        import "kith:random" Random;
        import "kith:time" instant diff_seconds;
        export Main;
        component Probe {
          rng: Random;
          start {}
        }
        export scene Main {
          Probe() {
            rng = Random(seed: (diff_seconds (instant "2024-01-01T00:00:01.500Z") (instant "2024-01-01T00:00:00.000Z")));
          }
        }
        "#,
    );
    assert!(fraction.contains("seed"), "{fraction}");
    let crypto = construction(
        r#"
        import "kith:crypto" Random;
        export Main;
        component Probe { start {} }
        export scene Main { Probe() {} }
        "#,
    );
    assert!(crypto.contains("not exported"), "{crypto}");
    let random = construction(
        r#"
        import "kith:random" Crypto;
        export Main;
        component Probe { start {} }
        export scene Main { Probe() {} }
        "#,
    );
    assert!(random.contains("not exported"), "{random}");
}

const STORE: &str = r#"
import "kith:store" Store;
export Probe;
export Main;
component Probe {
  field got: any;
  store: Store;
  other: Store;
  start {
    on down "Create" { send to store "Save" "a" null 1; }
    on down "Update" { send to store "Save" "a" 1 { n: 2 }; }
    on down "Stale" { send to store "Save" "a" 1 { n: 9 }; }
    on down "Read" { send to store "Load" "a"; }
    on down "Miss" { send to store "Load" "b"; }
    on down "Ghost" { send to store "Save" "b" 1 5; }
    on down "Key" { send to store "Load" 1; }
    on down "Shape" { send to store "Save" "a"; }
    on down "Version" { send to store "Save" "a" "no" 1; }
    on down "Nope" { send to store "Nope"; }
    on down "Other" { send to other "Load" "a"; }
    on up message:any { got -> message; }
  }
}
export scene Main {
  Probe() {
    store = Store();
    other = Store();
  }
}
"#;

fn reply(world: &mut World, message: &str) -> Doc {
    send(world, message);
    doc_field(world, "got")
}

fn tag(message: &Doc) -> String {
    doc::as_str(&doc::items(message).unwrap()[0]).unwrap()
}

#[test]
// req:persistence-001
fn store_saves_the_current_document_or_replies_conflict() {
    let mut world = start(STORE);
    let saved = reply(&mut world, "Create");
    assert_eq!(
        saved,
        doc::array([doc::string("Saved"), doc::string("a"), doc::number(1.0)])
    );
    let loaded = reply(&mut world, "Read");
    assert_eq!(
        loaded,
        doc::array([
            doc::string("Document"),
            doc::string("a"),
            doc::number(1.0),
            doc::number(1.0),
        ])
    );
    let missing = reply(&mut world, "Miss");
    assert_eq!(
        missing,
        doc::array([doc::string("Missing"), doc::string("b")])
    );
    let updated = reply(&mut world, "Update");
    assert_eq!(tag(&updated), "Saved");
    assert_eq!(doc::as_number(&doc::items(&updated).unwrap()[2]), Some(2.0));
    let current = reply(&mut world, "Read");
    let items = doc::items(&current).unwrap();
    assert_eq!(doc::as_str(&items[0]).as_deref(), Some("Document"));
    assert_eq!(doc::as_number(&items[2]), Some(2.0));
    assert_eq!(
        doc::as_number(&doc::get(&items[3], "n").unwrap()),
        Some(2.0)
    );

    let conflict = reply(&mut world, "Stale");
    assert_eq!(
        conflict,
        doc::array([doc::string("Conflict"), doc::string("a"), doc::number(2.0),])
    );
    let still = reply(&mut world, "Read");
    assert_eq!(
        doc::as_number(&doc::get(&doc::items(&still).unwrap()[3], "n").unwrap()),
        Some(2.0)
    );

    let ghost = reply(&mut world, "Ghost");
    assert_eq!(
        ghost,
        doc::array([doc::string("Conflict"), doc::string("b"), doc::null()])
    );
    assert_eq!(tag(&reply(&mut world, "Key")), "Error");
    assert_eq!(
        doc::as_str(&doc::items(&doc_field(&world, "got")).unwrap()[1]).as_deref(),
        Some("key")
    );
    assert_eq!(
        doc::as_str(&doc::items(&reply(&mut world, "Shape")).unwrap()[1]).as_deref(),
        Some("shape")
    );
    assert_eq!(
        doc::as_str(&doc::items(&reply(&mut world, "Version")).unwrap()[1]).as_deref(),
        Some("version")
    );
    assert_eq!(
        reply(&mut world, "Nope"),
        doc::array([doc::string("Error"), doc::string("unknown")])
    );
    let unchanged = reply(&mut world, "Read");
    assert_eq!(
        doc::as_number(&doc::items(&unchanged).unwrap()[2]),
        Some(2.0)
    );

    let other = reply(&mut world, "Other");
    assert_eq!(tag(&other), "Missing");

    let snap = snapshot_json(&world);
    let driver = snap["instances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            item["kind"] == "Store"
                && item["driver"]["documents"]
                    .as_array()
                    .is_some_and(|documents| documents.len() == 1)
        })
        .expect("the store that holds a")["driver"]
        .clone();
    assert!(driver["events"].is_null());
    let documents = driver["documents"].as_array().unwrap();
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0]["key"], "a");
    assert_eq!(documents[0]["version"], 2.0);
    assert_eq!(documents[0]["value"]["n"], 2.0);

    let program = load(STORE).unwrap();
    let mut restored = World::restore(program, &world.snapshot().unwrap()).unwrap();
    let again = reply(&mut restored, "Read");
    assert_eq!(
        doc::as_number(&doc::get(&doc::items(&again).unwrap()[3], "n").unwrap()),
        Some(2.0)
    );

    let error = construction(
        r#"
        import "kith:store" Store;
        export Main;
        component Probe {
          store: Store;
          start {}
        }
        export scene Main { Probe() { store = Store(seed: 1); } }
        "#,
    );
    assert!(
        error.contains("does not take construction arguments"),
        "{error}"
    );
}
