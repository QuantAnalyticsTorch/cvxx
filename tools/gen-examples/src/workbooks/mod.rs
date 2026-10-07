//! One module per generated workbook (SPEC-0017's "Module layout"). Each
//! module exposes a single `pub fn build(path: &Path) -> Result<(), GenError>`
//! that constructs one `.xlsx` file end-to-end and saves it to `path`.

pub mod constraints;
pub mod expressions;
pub mod inspecting_results;
pub mod mixed_integer_programming;
pub mod overview;
pub mod parameters_and_variables;
pub mod problems_and_solving;
pub mod quadratic_problems;
pub mod vector_matrix_indexing;

/// File name, scenario summary pairs for every non-overview workbook, in
/// generation/table-of-contents order. Shared by `overview::build` (for the
/// table of contents) and `main.rs` (for the "exactly these files" check).
pub const WORKBOOKS: &[(&str, &str)] = &[
    (
        "01-parameters-and-variables.xlsx",
        "A household's known monthly expenses (parameter) alongside an unknown savings target (variable).",
    ),
    (
        "02-expressions.xlsx",
        "A markup-pricing calculator that builds a profit expression from price, cost, and quantity.",
    ),
    (
        "03-constraints.xlsx",
        "A factory's production limits expressed as a checklist of capacity and demand constraints.",
    ),
    (
        "04-problems-and-solving.xlsx",
        "The classic diet problem: minimize weekly grocery cost subject to nutrition requirements.",
    ),
    (
        "05-quadratic-problems.xlsx",
        "Portfolio risk minimization: choose asset weights that minimize variance subject to a target return.",
    ),
    (
        "06-vector-matrix-indexing.xlsx",
        "Selecting individual line items or sub-ranges out of a multi-period budget matrix.",
    ),
    (
        "07-inspecting-results.xlsx",
        "Re-examining a solved diet problem: reading back status, cost, quantities, and handle descriptions.",
    ),
    (
        "08-mixed-integer-programming.xlsx",
        "Project selection: choosing which proposals to fund (yes/no decisions) within a fixed budget.",
    ),
];
