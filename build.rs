//! Writes `version.txt` at the repository root from the crate version in
//! `Cargo.toml`, so non-Rust release tooling (docs build, packaging script)
//! has a single file to read without invoking `cargo`.
//!
//! See `specifications/0001-release-pipeline-and-versioning.md`.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION not set by Cargo");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set by Cargo");
    let version_file = Path::new(&manifest_dir).join("version.txt");
    let contents = format!("{version}\n");

    // Avoid dirtying the working tree with a no-op write when nothing changed.
    let up_to_date = fs::read_to_string(&version_file)
        .map(|existing| existing == contents)
        .unwrap_or(false);
    if !up_to_date {
        fs::write(&version_file, contents).expect("failed to write version.txt");
    }

    println!("cargo:rerun-if-changed=Cargo.toml");
}
