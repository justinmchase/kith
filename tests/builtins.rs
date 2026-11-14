use kith::codec::decode;
use kith::doc;
use kith::parse::resolve_program;
use kith::World;
use serde_json::{json, Value as Json};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn scratch(files: &[(&str, &str)]) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "kith-builtins-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, source) in files {
        std::fs::write(dir.join(name), source).unwrap();
    }
    dir
}

fn load(source: &str) -> Result<kith::ast::Program, kith::Error> {
    let dir = scratch(&[("main.kith", source)]);
    resolve_program(&dir.join("main.kith"))
}

fn start(source: &str) -> World {
    World::start(load(source).expect("resolve")).expect("start")
}

fn root(world: &World) -> kith::ActorId {
    world.root_id()
}

fn field<'a>(world: &'a World, name: &str) -> &'a Json {
    world.field(root(world), name).unwrap_or(&Json::Null)
}

fn send(world: &mut World, message: &str) {
    world.send(root(world), json!(message)).unwrap();
    world.run_until_idle().unwrap();
}

fn fail(world: &mut World, message: &str) -> String {
    world.send(root(world), json!(message)).unwrap();
    world.run_until_idle().expect_err("turn").message
}

const PROBE: &str = r#"
import "kith:data" Bytes Blob Buffer bytes byte_at byte_length bytes_concat bytes_slice blob blob_bytes blob_type buffer buffer_slice buffer_bytes;
import "kith:stream" Chunk End Pipe;
import "kith:time" Instant DateTime Zone instant instant_text zone date_time instant_at add_seconds diff_seconds offset_seconds Clock;

export Probe;
export Main;

component Probe {
  field raw: any;
  field got: any;
  field saw: any;
  field err: any;
  field n: number = 0;
  Pipe pipe;
  Clock clock;

  start {
    on down "Len" { n -> (byte_length (bytes [1 2 3])); }
    on down "Empty" { n -> (byte_length (bytes [])); }
    on down "At" { n -> (byte_at (bytes [1 2 3]) 1); }
    on down "Cat" { n -> (byte_length (bytes_concat (bytes [1]) (bytes [2 3]) (bytes []))); }
    on down "Cut" { n -> (byte_at (bytes_slice (bytes [1 2 3 4]) 1 3) 0); }
    on down "AddLen" { n -> (add (byte_length (bytes [1 2])) 1); }
    on down "Codec" { got -> (to_codec (bytes [1 2 3])); }
    on down "Round" { n -> (byte_at (from_codec (to_codec (bytes [1 2 3]))) 0); }
    on down "Digest" { n -> (byte_length (sha256 "hi")); }
    on down "Blob" { send self (blob "not/a/real type" (bytes [65])); }
    on self value:(Blob) { got -> (blob_type value); n -> (byte_length (blob_bytes value)); }
    on down "Buf" { send self (buffer_slice (buffer (bytes [9 8 7 6])) 1 3); }
    on self value:(Buffer) { got -> (byte_at (buffer_bytes value) 0); }
    on down "Rule" { n -> 1; }
    on down value:(Bytes) { n -> 7; }
    on down "Wait" { send to pipe "Pull"; }
    on down "Give" { send to pipe "Push" 9; }
    on down "Again" { send to pipe "Pull"; }
    on down "Queue" { send to pipe "Push" "x"; }
    on down "Take" { send to pipe "Pull"; }
    on down "Shut" { send to pipe "Close"; }
    on down "Nope" { send to pipe "Nope"; }
    on down "Now" { send to clock "Now"; }
    on down "Ask" { send to clock "Zone"; }
    on down "Later" {
      send after (diff_seconds (instant "2024-01-01T00:00:05.000Z") (instant "2024-01-01T00:00:00.000Z")) self "Tick";
    }
    on self "Tick" { n -> 1; }
    on down "Plus" n:number {
      got -> (instant_text (add_seconds (instant "2024-01-01T00:00:00.000Z") n));
    }
    on down "Diff" {
      n -> (diff_seconds (instant "2024-01-01T00:00:01.500Z") (instant "2024-01-01T00:00:00.000Z"));
    }
    on down "Line" {
      got -> (date_time (instant "2024-01-01T00:00:00.000Z") (zone "America/New_York"));
    }
    on down "Tokyo" {
      got -> (date_time (instant "2024-01-01T00:00:00.000Z") (zone "Asia/Tokyo"));
    }
    on down "Back" {
      got -> (instant_text (instant_at (date_time (instant "2024-01-02T00:00:00.000Z") (zone "America/New_York")) (zone "America/New_York")));
    }
    on down "Off" {
      n -> (offset_seconds (zone "America/New_York") (instant "2024-07-01T12:00:00.000Z"));
    }
    on down "Winter" {
      n -> (offset_seconds (zone "America/New_York") (instant "2024-01-01T00:00:00.000Z"));
    }
    on down "Fixed" { saw -> (zone "+05:30"); got -> (zone "+00:00"); }
    on down "Utc" { saw -> (zone "UTC"); }
    on up (Chunk) value:any { got -> value; }
    on up "Pushed" { saw -> got; }
    on up (End) { err -> "End"; }
    on up "Closed" { saw -> err; }
    on up "Error" reason:string { err -> reason; }
    on up "Instant" value:(Instant) {
      n -> (add n 1);
      got -> (instant_text value);
    }
    on up "Zone" value:(Zone) { saw -> value; }
  }
}

export scene Main {
  Probe() {
    pipe = Pipe();
    clock = Clock();
  }
}
"#;

#[test]
// req:builtins-001
fn kith_prefix_binds_exports_and_rejects_the_other_shapes() {
    let program = load(PROBE).expect("probe");
    assert!(program.functions.iter().any(|name| name == "bytes"));
    assert!(program.rules.iter().any(|name| name == "Bytes"));
    assert!(program.host_kinds.iter().any(|name| name == "Pipe"));
    assert!(program.host_kinds.iter().any(|name| name == "Clock"));
    assert!(program.component("Pipe").is_some());

    let invalid = load(
        r#"import "kith:Data" Bytes; component Root { start {} } export scene Main { Root() {} }"#,
    );
    assert!(invalid
        .unwrap_err()
        .message
        .contains("invalid kith import specifier"));
    let empty = load(
        r#"import "kith:" Bytes; component Root { start {} } export scene Main { Root() {} }"#,
    );
    assert!(empty
        .unwrap_err()
        .message
        .contains("invalid kith import specifier"));
    let slash = load(
        r#"import "kith:fs/promises" readFile; component Root { start {} } export scene Main { Root() {} }"#,
    );
    assert!(slash
        .unwrap_err()
        .message
        .contains("invalid kith import specifier"));
    let net = load(
        r#"import "kith:net" Socket; component Root { start {} } export scene Main { Root() {} }"#,
    );
    assert!(net
        .unwrap_err()
        .message
        .contains("unknown built-in module kith:net"));
    let jsr = load(
        r#"import "jsr:@scope/name" Name; component Root { start {} } export scene Main { Root() {} }"#,
    );
    assert!(jsr
        .unwrap_err()
        .message
        .contains("unsupported import specifier"));
    let missing = load(
        r#"import "kith:data" Nope; component Root { start {} } export scene Main { Root() {} }"#,
    );
    assert!(missing
        .unwrap_err()
        .message
        .contains("Nope is not exported by kith:data"));
    let collision = load(
        r#"
        import "kith:data" bytes;
        component bytes { start {} }
        export scene Main { bytes() {} }
    "#,
    );
    assert!(collision
        .unwrap_err()
        .message
        .contains("bytes collides with a component"));

    let dir = scratch(&[
        (
            "lib.kith",
            r#"
            import "kith:stream" Pipe;
            import "kith:data" bytes Bytes;
            export Pipe;
            export bytes;
            export Bytes;
            "#,
        ),
        (
            "main.kith",
            r#"
            import "./lib.kith" Pipe bytes Bytes;
            export Probe;
            export Main;
            component Probe {
              field raw: any = (bytes [1]);
              Pipe pipe;
              start { on down { } }
            }
            export scene Main { Probe() { pipe = Pipe(); } }
            "#,
        ),
    ]);
    let program = resolve_program(&dir.join("main.kith")).expect("re-export");
    assert!(program.host_kinds.iter().any(|name| name == "Pipe"));
    assert!(program.functions.iter().any(|name| name == "bytes"));
    assert!(program.rules.iter().any(|name| name == "Bytes"));
    World::start(program).expect("re-exported pipe is a driver");

    let dir = scratch(&[(
        "main.kith",
        r#"import "kith:data" bytes; component Root { start {} } export scene Main { Root() {} }"#,
    )]);
    let parsed = Command::new(env!("CARGO_BIN_EXE_kith"))
        .args(["parse", dir.join("main.kith").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        parsed.status.success(),
        "{}",
        String::from_utf8_lossy(&parsed.stderr)
    );
    let tree = decode(&String::from_utf8_lossy(&parsed.stdout)).unwrap();
    assert_eq!(tree["imports"][0]["moduleUrl"], "kith:data");
    assert!(tree["functions"].as_array().unwrap().is_empty());
    let checked = Command::new(env!("CARGO_BIN_EXE_kith"))
        .args(["check", dir.join("main.kith").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
}

#[test]
// req:builtins-002
fn data_functions_build_bytes_blobs_and_buffers() {
    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    send(&mut world, "Len");
    assert_eq!(field(&world, "n"), &json!(3.0));
    send(&mut world, "Empty");
    assert_eq!(field(&world, "n"), &json!(0.0));
    send(&mut world, "At");
    assert_eq!(field(&world, "n"), &json!(2.0));
    send(&mut world, "Cat");
    assert_eq!(field(&world, "n"), &json!(3.0));
    send(&mut world, "Cut");
    assert_eq!(field(&world, "n"), &json!(2.0));
    send(&mut world, "AddLen");
    assert_eq!(field(&world, "n"), &json!(3.0));
    send(&mut world, "Codec");
    let text = field(&world, "got").as_str().unwrap();
    assert!(text.contains("@bytes"), "{text}");
    assert!(text.contains("AQID"), "{text}");
    send(&mut world, "Round");
    assert_eq!(field(&world, "n"), &json!(1.0));
    send(&mut world, "Digest");
    assert_eq!(field(&world, "n"), &json!(32.0));
    send(&mut world, "Blob");
    assert_eq!(field(&world, "got"), &json!("not/a/real type"));
    assert_eq!(field(&world, "n"), &json!(1.0));
    send(&mut world, "Buf");
    assert_eq!(field(&world, "got"), &json!(8.0));
    world
        .send_document(root(&world), doc::bytes(vec![4, 5]))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(field(&world, "n"), &json!(7.0));

    assert!(fresh_fails(
        r#"
        import "kith:data" bytes;
        component Root {
          field n: any;
          start { on down "x" { n -> (bytes [256]); } }
        }
        export scene Main { Root() {} }
    "#
    ));
    let mut world = start(
        r#"
        import "kith:data" bytes;
        component Root {
          field n: any;
          start { on down n:number { n -> (bytes [n]); } }
        }
        export scene Main { Root() {} }
    "#,
    );
    world.run_until_idle().unwrap();
    world.send(root(&world), json!(1.5)).unwrap();
    assert!(world.run_until_idle().is_err());
    assert!(fresh_fails(
        r#"
        import "kith:data" bytes byte_at;
        component Root {
          field n: any;
          start { on down "x" { n -> (byte_at (bytes [1]) 1); } }
        }
        export scene Main { Root() {} }
    "#
    ));
    assert!(fresh_fails(
        r#"
        import "kith:data" bytes bytes_slice;
        component Root {
          field n: any;
          start { on down "x" { n -> (bytes_slice (bytes [1 2]) 1 0); } }
        }
        export scene Main { Root() {} }
    "#
    ));
    assert!(fresh_fails(
        r#"
        import "kith:data" bytes buffer buffer_slice buffer_bytes;
        component Root {
          field n: any;
          start { on down "x" { n -> (buffer_bytes (buffer_slice (buffer (bytes [1 2])) 0 3)); } }
        }
        export scene Main { Root() {} }
    "#
    ));
    assert!(fresh_fails(
        r#"
        component Root {
          field n: any;
          start { on down "x" { n -> (bytes [1]); } }
        }
        export scene Main { Root() {} }
    "#
    ));
}

fn fresh_fails(source: &str) -> bool {
    let mut world = start(source);
    world.run_until_idle().unwrap();
    world.send(root(&world), json!("x")).unwrap();
    world.run_until_idle().is_err()
}

#[test]
// req:builtins-003
fn pipe_waits_answers_and_restores() {
    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    send(&mut world, "Wait");
    assert!(field(&world, "got").is_null());
    send(&mut world, "Again");
    assert_eq!(field(&world, "err"), &json!("busy"));
    send(&mut world, "Give");
    assert_eq!(field(&world, "got"), &json!(9.0));
    assert_eq!(field(&world, "saw"), &json!(9.0));

    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    send(&mut world, "Wait");
    send(&mut world, "Shut");
    assert_eq!(field(&world, "err"), &json!("End"));
    assert_eq!(field(&world, "saw"), &json!("End"));

    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    send(&mut world, "Queue");
    send(&mut world, "Queue");
    assert_eq!(field(&world, "err"), &json!("full"));

    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    send(&mut world, "Queue");
    send(&mut world, "Shut");
    let program = load(PROBE).unwrap();
    let snap = world.snapshot().unwrap();
    let mut world = World::restore(program, &snap).unwrap();
    send(&mut world, "Take");
    assert_eq!(field(&world, "got"), &json!("x"));
    send(&mut world, "Take");
    assert_eq!(field(&world, "err"), &json!("End"));

    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    send(&mut world, "Shut");
    send(&mut world, "Give");
    assert_eq!(field(&world, "err"), &json!("closed"));
    send(&mut world, "Nope");
    assert_eq!(field(&world, "err"), &json!("unknown"));

    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    send(&mut world, "Wait");
    send(&mut world, "Nope");
    assert_eq!(field(&world, "err"), &json!("unknown"));
    send(&mut world, "Give");
    assert_eq!(field(&world, "got"), &json!(9.0));
}

#[test]
// req:builtins-004
fn time_values_stay_pure_and_clock_is_the_wall_read() {
    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    assert_eq!(world.virtual_now(), 0);
    world.send(root(&world), json!(["Plus", 1.5])).unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(field(&world, "got"), &json!("2024-01-01T00:00:01.5Z"));
    send(&mut world, "Diff");
    assert_eq!(field(&world, "n"), &json!(1.5));
    send(&mut world, "Line");
    assert_eq!(field(&world, "got")["year"].as_f64(), Some(2023.0));
    assert_eq!(field(&world, "got")["month"].as_f64(), Some(12.0));
    assert_eq!(field(&world, "got")["day"].as_f64(), Some(31.0));
    assert_eq!(field(&world, "got")["hour"].as_f64(), Some(19.0));
    send(&mut world, "Tokyo");
    assert_eq!(field(&world, "got")["hour"].as_f64(), Some(9.0));
    assert_eq!(field(&world, "got")["day"].as_f64(), Some(1.0));
    send(&mut world, "Back");
    assert_eq!(field(&world, "got"), &json!("2024-01-02T00:00:00Z"));
    send(&mut world, "Off");
    assert_eq!(field(&world, "n"), &json!(-14400.0));
    send(&mut world, "Winter");
    assert_eq!(field(&world, "n"), &json!(-18000.0));
    send(&mut world, "Fixed");
    assert_eq!(field(&world, "saw")["seconds"].as_f64(), Some(19800.0));
    assert_eq!(field(&world, "got")["kind"], json!("fixed"));
    assert_eq!(field(&world, "got")["seconds"].as_f64(), Some(0.0));
    send(&mut world, "Utc");
    assert_eq!(field(&world, "saw"), &json!({"kind": "utc"}));

    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    world
        .replace_clock(
            "America/New_York",
            &["2024-01-01T00:00:00.000Z", "2024-01-01T00:00:01.000Z"],
        )
        .unwrap();
    send(&mut world, "Now");
    assert_eq!(field(&world, "got"), &json!("2024-01-01T00:00:00Z"));
    assert_eq!(field(&world, "n"), &json!(1.0));
    assert_eq!(world.virtual_now(), 0);
    send(&mut world, "Ask");
    assert_eq!(field(&world, "saw")["id"], json!("America/New_York"));
    let program = load(PROBE).unwrap();
    let snap = world.snapshot().unwrap();
    let decoded = decode(&snap).unwrap();
    assert_eq!(decoded["clock"]["replies"].as_array().unwrap().len(), 1);
    let mut world = World::restore(program, &snap).unwrap();
    send(&mut world, "Now");
    assert_eq!(field(&world, "got"), &json!("2024-01-01T00:00:01Z"));
    send(&mut world, "Now");
    assert_eq!(field(&world, "err"), &json!("clock"));
    let snap = world.snapshot().unwrap();
    let decoded = decode(&snap).unwrap();
    assert_eq!(decoded["clock"]["replies"].as_array().unwrap().len(), 2);

    let mut world = start(PROBE);
    world.run_until_idle().unwrap();
    send(&mut world, "Later");
    assert_eq!(field(&world, "n"), &json!(0.0));
    assert_eq!(world.virtual_now(), 0);
    assert!(world.fire_next_timer().unwrap());
    assert!(world.virtual_now() > 0);
    world.run_until_idle().unwrap();
    assert_eq!(field(&world, "n"), &json!(1.0));
}

#[test]
// req:builtins-004
fn a_gap_and_an_overlap_fail_the_expression() {
    let source = r#"
        import "kith:time" instant_at zone;
        component Root {
          field got: any;
          start {
            on down "Gap" {
              got -> (instant_at { year: 2024, month: 3, day: 10, hour: 2, minute: 30, second: 0, millisecond: 0 } (zone "America/New_York"));
            }
            on down "Overlap" {
              got -> (instant_at { year: 2024, month: 11, day: 3, hour: 1, minute: 30, second: 0, millisecond: 0 } (zone "America/New_York"));
            }
            on down "Leap" {
              got -> (instant_at { year: 2023, month: 2, day: 29, hour: 0, minute: 0, second: 0, millisecond: 0 } (zone "UTC"));
            }
          }
        }
        export scene Main { Root() {} }
    "#;
    let mut world = start(source);
    world.run_until_idle().unwrap();
    let gap = fail(&mut world, "Gap");
    assert!(gap.contains("gap"), "{gap}");
    let overlap = fail(&mut world, "Overlap");
    assert!(overlap.contains("overlap"), "{overlap}");
    let leap = fail(&mut world, "Leap");
    assert!(
        leap.contains("invalid date") || leap.contains("date"),
        "{leap}"
    );
}
