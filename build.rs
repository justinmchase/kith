use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let grammar = manifest.join("src/grammar/kith.lang.uff");
    println!("cargo:rerun-if-changed={}", grammar.display());
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("uffda.jsonc").display()
    );

    let artifacts = manifest.join("bin/ast");
    if artifacts.exists() {
        std::fs::remove_dir_all(&artifacts).expect("clear previous Kith grammar artifacts");
    }

    let status = Command::new("uffda")
        .current_dir(&manifest)
        .args([
            "compile",
            "--config",
            "uffda.jsonc",
            "src/grammar/kith.lang.uff",
        ])
        .status()
        .unwrap_or_else(|error| {
            panic!("kith build requires a released uffda CLI on PATH ({error})")
        });
    if !status.success() {
        panic!("uffda compile failed while embedding the Kith grammar");
    }

    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("languages.rs");
    uffda_build::embed(uffda_build::Embed {
        artifact_root: manifest.join("bin/ast"),
        authored_root: None,
        logical_root: String::new(),
        out_file: out,
    })
    .expect("embed compiled Kith grammar artifacts");
}
