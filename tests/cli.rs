use serde_json::json;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kith")
}

fn ping() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/ping.kith")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().expect("run kith")
}

#[test]
// req:cli-001 req:components-006 req:scenes-002
fn parse_check_compile_and_run_ping() {
    let ping = ping();
    let ping = ping.to_str().unwrap();

    let parsed = run(&["parse", ping]);
    assert!(parsed.status.success(), "{}", stderr(&parsed));
    let tree: serde_json::Value = serde_json::from_slice(&parsed.stdout).expect("parse json");
    assert_eq!(tree["kind"], "program");
    assert!(tree.get("root").is_none());
    assert_eq!(tree["scenes"][0]["name"], "Main");

    let checked = run(&["check", ping]);
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let dir = std::env::temp_dir().join(format!("kith-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.kith");
    std::fs::write(
        &bad,
        "export Broken;\nexport Main;\ncomponent Broken {\n  state Idle {}\n}\nexport scene Main {\n  Broken()\n}\n",
    )
    .unwrap();
    let rejected = run(&["check", bad.to_str().unwrap()]);
    assert!(!rejected.status.success(), "missing start state must fail");

    let no_scene = dir.join("noscene.kith");
    std::fs::write(
        &no_scene,
        "export Broken;\ncomponent Broken {\n  start Idle {}\n}\n",
    )
    .unwrap();
    let rejected_scene = run(&["check", no_scene.to_str().unwrap()]);
    assert!(
        !rejected_scene.status.success(),
        "missing exported scene must fail"
    );

    let copied = dir.join("ping.kith");
    std::fs::copy(ping, &copied).unwrap();
    let compiled = run(&["compile", copied.to_str().unwrap()]);
    assert!(compiled.status.success(), "{}", stderr(&compiled));
    let artifact = String::from_utf8(compiled.stdout).unwrap();
    let artifact = artifact.trim();
    assert!(PathBuf::from(artifact).exists(), "{artifact}");

    let ran = run(&["run", ping, "--message", r#"{"kind":"Ping","n":7}"#]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    let fields: serde_json::Value = serde_json::from_slice(&ran.stdout).expect("run json");
    assert_eq!(fields["heard"], json!(7.0));
}

#[test]
// req:grammar-002
fn parse_does_not_resolve_imports_but_check_does() {
    let uses = fixture("uses_child.kith");
    let uses = uses.to_str().unwrap();
    let parsed = run(&["parse", uses]);
    assert!(parsed.status.success(), "{}", stderr(&parsed));
    let tree: serde_json::Value = serde_json::from_slice(&parsed.stdout).expect("parse json");
    assert_eq!(tree["imports"][0]["moduleUrl"], "./child.kith");
    assert!(tree["components"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|component| component["name"] != "Child"));

    let checked = run(&["check", uses]);
    assert!(checked.status.success(), "{}", stderr(&checked));

    let ran = run(&["run", uses, "--message", r#"{"kind":"Ping","n":7}"#]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    let fields: serde_json::Value = serde_json::from_slice(&ran.stdout).expect("run json");
    assert_eq!(fields["heard"], json!(7.0));
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
