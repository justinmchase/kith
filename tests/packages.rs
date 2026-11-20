use std::fs;
use std::path::PathBuf;

fn manifest(relative: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

#[test]
// req:runtime-006
fn cli_package_stays_off_crates_io() {
    let cargo = manifest("Cargo.toml");
    assert!(
        cargo.contains("name = \"kith\""),
        "CLI package must be named kith"
    );
    assert!(
        cargo.contains("publish = false"),
        "CLI package must not publish to crates.io"
    );
}

#[test]
// req:runtime-006
fn runtime_package_is_publishable() {
    let cargo = manifest("crates/kith-runtime/Cargo.toml");
    assert!(
        cargo.contains("name = \"kith-runtime\""),
        "library package must be named kith-runtime"
    );
    assert!(
        cargo.contains("publish = true"),
        "kith-runtime must be publishable to crates.io"
    );
}
