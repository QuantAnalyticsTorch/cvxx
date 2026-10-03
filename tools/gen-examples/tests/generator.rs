//! Integration tests for the `gen-examples` generator (SPEC-0017's "Test
//! Approach"): every workbook builds to a valid, non-empty `.xlsx`; the
//! generated file-name set matches the Interface table exactly; every
//! `CVX.*` formula referenced in the generator's own source still exists
//! in `src/excel/mod.rs`'s registration list; and no input value is left
//! orphaned (declared but never referenced by a formula).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

/// Directory this crate lives in (`tools/gen-examples`).
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Repository root, two levels above `tools/gen-examples`.
fn repo_root() -> PathBuf {
    manifest_dir()
        .parent()
        .and_then(Path::parent)
        .expect("tools/gen-examples should be two levels below the repo root")
        .to_path_buf()
}

/// Every `workbooks/*.rs` source file, as (display name, contents) pairs,
/// used by the source-scanning tests below.
fn workbook_sources() -> Vec<(String, String)> {
    let dir = manifest_dir().join("src").join("workbooks");
    let mut sources = Vec::new();
    for entry in fs::read_dir(&dir).expect("src/workbooks should exist") {
        let entry = entry.expect("readable directory entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let contents = fs::read_to_string(&path).expect("readable source file");
            sources.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                contents,
            ));
        }
    }
    sources
}

#[test]
fn every_workbook_builds_a_valid_nonempty_xlsx() {
    let dir = std::env::temp_dir().join(format!("gen-examples-test-valid-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");

    for (name, build) in gen_examples::all_builders() {
        let path = dir.join(name);
        build(&path).unwrap_or_else(|err| panic!("{name} failed to build: {err}"));

        let bytes = fs::read(&path).unwrap_or_else(|err| panic!("{name} was not written: {err}"));
        assert!(!bytes.is_empty(), "{name} was written but is empty");

        // An .xlsx is a zip archive; assert it unzips and contains the
        // mandatory xl/workbook.xml entry.
        let reader = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader)
            .unwrap_or_else(|err| panic!("{name} is not a valid zip archive: {err}"));
        archive
            .by_name("xl/workbook.xml")
            .unwrap_or_else(|err| panic!("{name} has no xl/workbook.xml entry: {err}"));
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn generated_file_names_match_the_expected_set_exactly() {
    let dir = std::env::temp_dir().join(format!("gen-examples-test-names-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");

    for (name, build) in gen_examples::all_builders() {
        build(&dir.join(name)).unwrap_or_else(|err| panic!("{name} failed to build: {err}"));
    }

    let actual: BTreeSet<String> = fs::read_dir(&dir)
        .expect("readable temp dir")
        .map(|e| {
            e.expect("readable entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let expected: BTreeSet<String> = gen_examples::all_builders()
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect();

    assert_eq!(
        actual.len(),
        8,
        "expected exactly eight generated workbooks"
    );
    assert_eq!(
        actual, expected,
        "generated file names must match the Interface table exactly"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn every_cvx_formula_name_is_still_registered_in_src_excel_mod_rs() {
    let registry_path = repo_root().join("src").join("excel").join("mod.rs");
    let registry_source = fs::read_to_string(&registry_path)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", registry_path.display()));

    let name_pattern = Regex::new(r#""(CVX\.[A-Z_]+)""#).unwrap();
    let registered: BTreeSet<&str> = name_pattern
        .captures_iter(&registry_source)
        .map(|c| c.get(1).unwrap().as_str())
        .collect();
    assert!(
        !registered.is_empty(),
        "expected at least one CVX.* registration in src/excel/mod.rs"
    );

    for (file, contents) in workbook_sources() {
        for found in name_pattern.captures_iter(&contents) {
            let name = found.get(1).unwrap().as_str();
            assert!(
                registered.contains(name),
                "{file} references {name}, which is not registered in src/excel/mod.rs \
                 (it may have been renamed or removed)"
            );
        }
    }
}

/// Given the text immediately following a call's opening `(`, returns the
/// call's argument text up to (not including) the matching closing `)`,
/// correctly skipping parentheses that appear inside Rust string literals.
fn extract_balanced_call_args(rest: &str) -> &str {
    let mut depth: i32 = 1;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in rest.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &rest[..idx];
                }
            }
            _ => {}
        }
    }
    rest
}

/// Splits a balanced call-argument string (as returned by
/// [`extract_balanced_call_args`]) into its top-level comma-separated
/// arguments, respecting nested `(...)`/`[...]` and string literals so
/// that labels/notes containing commas or numbers are not mistaken for
/// separate arguments.
fn split_top_level_args(args: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0usize;
    for (idx, ch) in args.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(args[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    let tail = args[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

#[test]
fn every_input_value_is_referenced_somewhere_later_in_its_workbook_source() {
    // Matches `let binding = sheet.input_scalar(...)` / `input_vector` /
    // `input_matrix` (bound) and bare `sheet.input_scalar(...)` (discarded,
    // used only when the literal itself is re-typed directly into a later
    // formula string, e.g. 04-problems-and-solving.xlsx).
    let call_pattern =
        Regex::new(r"(?:let\s+(\w+)\s*=\s*)?sheet\.(input_scalar|input_vector|input_matrix)\(")
            .unwrap();
    let number_pattern = Regex::new(r"-?\d+(?:\.\d+)?").unwrap();

    for (file, contents) in workbook_sources() {
        for call in call_pattern.captures_iter(&contents) {
            let call_end = call.get(0).unwrap().end();
            let rest = &contents[call_end..];
            let call_args = extract_balanced_call_args(rest);

            if let Some(binding) = call.get(1) {
                // Bound: the address is used later via the variable name.
                let later_usage = contents[call_end..].matches(binding.as_str()).count();
                assert!(
                    later_usage > 0,
                    "{file}: input bound to `{}` is never referenced again",
                    binding.as_str()
                );
            } else {
                // Discarded: the literal number(s) in the value argument
                // (the second top-level argument: a scalar, or a `&[...]`
                // vector/matrix literal) must reappear later in the file,
                // embedded directly in a formula string.
                let args = split_top_level_args(call_args);
                let value_arg = args.get(1).copied().unwrap_or_default();
                let literals: Vec<&str> = number_pattern
                    .find_iter(value_arg)
                    .map(|m| m.as_str())
                    .collect();
                assert!(
                    !literals.is_empty(),
                    "{file}: discarded input call's value argument `{value_arg}` has no detectable numeric literal"
                );
                let later = &contents[call_end + call_args.len()..];
                let found = literals.iter().any(|lit| {
                    // A Rust literal like `24.0` is often typed as a bare
                    // `24` inside a formula string, so accept either
                    // textual form.
                    let value: f64 = lit.parse().unwrap_or(f64::NAN);
                    let trimmed = format!("{value}");
                    later.contains(lit) || later.contains(&trimmed)
                });
                assert!(
                    found,
                    "{file}: discarded input literal(s) {literals:?} never reappear in a later formula"
                );
            }
        }
    }
}
