//! Library surface for the `gen-examples` generator: exposes the builder
//! registry so both `main.rs` and the integration tests in `tests/` can
//! build every workbook without duplicating the (file name, builder)
//! table.

pub mod error;
pub mod scenario;
pub mod theme;
pub mod workbooks;

use std::path::Path;

pub use error::GenError;

/// One (file name, builder) pair per generated workbook.
pub type Builder = fn(&Path) -> Result<(), GenError>;

/// Maps each non-overview file name from [`workbooks::WORKBOOKS`] to its
/// builder function. Kept as a separate array (rather than folded into
/// `workbooks::WORKBOOKS` itself) so that table stays a plain data
/// constant usable from `overview.rs`.
const NAMED_BUILDERS: &[(&str, Builder)] = &[
    (
        "01-parameters-and-variables.xlsx",
        workbooks::parameters_and_variables::build,
    ),
    ("02-expressions.xlsx", workbooks::expressions::build),
    ("03-constraints.xlsx", workbooks::constraints::build),
    (
        "04-problems-and-solving.xlsx",
        workbooks::problems_and_solving::build,
    ),
    (
        "05-quadratic-problems.xlsx",
        workbooks::quadratic_problems::build,
    ),
    (
        "06-vector-matrix-indexing.xlsx",
        workbooks::vector_matrix_indexing::build,
    ),
    (
        "07-inspecting-results.xlsx",
        workbooks::inspecting_results::build,
    ),
    (
        "08-mixed-integer-programming.xlsx",
        workbooks::mixed_integer_programming::build,
    ),
];

/// Every workbook's (file name, builder) pair, in build order.
/// `00-overview.xlsx` is built first so its table of contents lists every
/// workbook that follows it.
pub fn all_builders() -> Vec<(&'static str, Builder)> {
    let mut all: Vec<(&'static str, Builder)> =
        vec![("00-overview.xlsx", workbooks::overview::build as Builder)];
    all.extend(NAMED_BUILDERS.iter().copied());
    all
}

/// Guards against a copy/paste mistake in [`NAMED_BUILDERS`]: two
/// workbooks writing to the same file name would silently overwrite one
/// another.
pub fn check_no_duplicate_names() -> Result<(), GenError> {
    let mut names: Vec<&str> = all_builders().into_iter().map(|(n, _)| n).collect();
    names.sort_unstable();
    let duplicate = names.windows(2).find(|pair| pair[0] == pair[1]);
    if let Some(pair) = duplicate {
        return Err(GenError::Invariant(format!(
            "duplicate workbook file name registered twice: {}",
            pair[0]
        )));
    }
    Ok(())
}

/// Builds every workbook into `out_dir`, collecting (rather than
/// short-circuiting on) failures. Returns the list of workbook file names
/// that failed to build, paired with their error.
pub fn generate_all(out_dir: &Path) -> Vec<(&'static str, GenError)> {
    let mut failures = Vec::new();
    for (name, build) in all_builders() {
        let path = out_dir.join(name);
        if let Err(err) = build(&path) {
            failures.push((name, err));
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_cover_every_expected_file_name() {
        let mut names: Vec<&str> = all_builders().into_iter().map(|(n, _)| n).collect();
        names.sort_unstable();
        let mut expected: Vec<&str> = std::iter::once("00-overview.xlsx")
            .chain(workbooks::WORKBOOKS.iter().map(|(n, _)| *n))
            .collect();
        expected.sort_unstable();
        assert_eq!(names, expected);
    }

    #[test]
    fn no_duplicate_file_names() {
        check_no_duplicate_names().expect("builder table should have no duplicate file names");
    }
}
