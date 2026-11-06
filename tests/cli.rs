use serde_json::json;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kith")
}

fn ping() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/ping.kith")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().expect("run kith")
}

#[test]
// req:cli-001 req:components-006
fn parse_check_compile_and_run_ping() {
    let ping = ping();
    let ping = ping.to_str().unwrap();

    let parsed = run(&["parse", ping]);
    assert!(parsed.status.success(), "{}", stderr(&parsed));
    let tree: serde_json::Value = serde_json::from_slice(&parsed.stdout).expect("parse json");
    assert_eq!(tree["kind"], "program");
    assert_eq!(tree["root"], "Parent");

    let checked = run(&["check", ping]);
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let dir = std::env::temp_dir().join(format!("kith-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.kith");
    std::fs::write(
        &bad,
        "component Broken {\n  state Idle {}\n}\nroot Broken;\n",
    )
    .unwrap();
    let rejected = run(&["check", bad.to_str().unwrap()]);
    assert!(!rejected.status.success(), "missing start state must fail");

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

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
