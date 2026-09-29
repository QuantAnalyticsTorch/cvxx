use thiserror::Error;

/// Errors that can occur while building or looking up `cvxx` registry
/// objects. Every variant is translated to an Excel-visible `#VALUE!` by the
/// `excel` module; diagnostics are logged, not written to the worksheet.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CvxError {
    #[error("range is empty or missing")]
    EmptyRange,
    #[error("cell contains a non-numeric value")]
    NonNumericCell,
    #[error("invalid dimension: {0}")]
    InvalidDimension(String),
    #[error("name '{0}' is already registered")]
    DuplicateName(String),
    #[error("no entry found for name '{0}'")]
    NameNotFound(String),
    #[error("invalid handle '{0}'")]
    InvalidHandle(String),
    #[error("invalid expression: {0}")]
    InvalidExpression(String),
    #[error("unknown identifier '{0}'")]
    UnknownIdentifier(String),
    #[error("registry error: {0}")]
    Registry(String),
}
