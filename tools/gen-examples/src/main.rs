//! CLI entry point: generates every branded `cvxx` example workbook into an
//! output directory (`--out`, defaulting to `docs/examples`).
//!
//! `cargo run -p gen-examples --release -- --out docs/examples`

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use gen_examples::{all_builders, check_no_duplicate_names, generate_all};

fn parse_out_dir(args: &[String]) -> PathBuf {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--out" {
            if let Some(value) = iter.next() {
                return PathBuf::from(value);
            }
        } else if let Some(value) = arg.strip_prefix("--out=") {
            return PathBuf::from(value);
        }
    }
    PathBuf::from("docs/examples")
}

fn main() -> ExitCode {
    if let Err(err) = check_no_duplicate_names() {
        eprintln!("gen-examples: {err}");
        return ExitCode::FAILURE;
    }

    let args: Vec<String> = env::args().skip(1).collect();
    let out_dir = parse_out_dir(&args);

    if let Err(err) = fs::create_dir_all(&out_dir) {
        eprintln!(
            "gen-examples: could not create output directory {}: {err}",
            out_dir.display()
        );
        return ExitCode::FAILURE;
    }

    let failures = generate_all(&out_dir);
    let total = all_builders().len();
    if failures.is_empty() {
        println!(
            "gen-examples: wrote {total} workbook(s) to {}",
            out_dir.display()
        );
        ExitCode::SUCCESS
    } else {
        for (name, err) in &failures {
            eprintln!("gen-examples: failed to build {name}: {err}");
        }
        eprintln!(
            "gen-examples: {} of {total} workbook(s) failed",
            failures.len()
        );
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_out_dir_space_form() {
        let args: Vec<String> = vec!["--out".into(), "some/dir".into()];
        assert_eq!(parse_out_dir(&args), PathBuf::from("some/dir"));
    }

    #[test]
    fn parses_out_dir_equals_form() {
        let args: Vec<String> = vec!["--out=some/dir".into()];
        assert_eq!(parse_out_dir(&args), PathBuf::from("some/dir"));
    }

    #[test]
    fn defaults_out_dir_when_absent() {
        let args: Vec<String> = vec![];
        assert_eq!(parse_out_dir(&args), PathBuf::from("docs/examples"));
    }
}
