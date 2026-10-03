//! Error type shared by every workbook builder and the CLI entry point.

use std::fmt;
use std::io;

use rust_xlsxwriter::XlsxError;

/// A single generator failure, tagged with the workbook that produced it so
/// `main.rs` can report every failing workbook instead of stopping at the
/// first one (SPEC-0017's "Error Handling" section).
#[derive(Debug)]
pub enum GenError {
    /// The output directory could not be created or is not writable.
    Io(io::Error),
    /// `rust_xlsxwriter` reported an error while building or saving a
    /// workbook.
    Xlsx(XlsxError),
    /// A generator-level invariant was violated (e.g. a formula referenced a
    /// `CVX.*` function name that isn't registered in `src/excel/mod.rs`).
    Invariant(String),
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenError::Io(err) => write!(f, "I/O error: {err}"),
            GenError::Xlsx(err) => write!(f, "xlsx error: {err}"),
            GenError::Invariant(msg) => write!(f, "invariant violated: {msg}"),
        }
    }
}

impl std::error::Error for GenError {}

impl From<io::Error> for GenError {
    fn from(err: io::Error) -> Self {
        GenError::Io(err)
    }
}

impl From<XlsxError> for GenError {
    fn from(err: XlsxError) -> Self {
        GenError::Xlsx(err)
    }
}
