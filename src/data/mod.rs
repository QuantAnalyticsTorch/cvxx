//! Excel range parsing and normalization into dense Rust matrices.

use xladd::entrypoint::excel12;
use xladd::variant::Variant;
use xladd::xlcall::{xlCoerce, xltypeNum, xltypeStr};

use crate::core::error::CvxError;

/// Maximum supported row or column count for variables and parameters.
pub const MAX_DIMENSION: usize = 1_000_000;

/// A dense numeric range read from Excel, in row-major order.
#[derive(Debug)]
pub struct ParsedRange {
    /// `(rows, cols)`.
    pub shape: (usize, usize),
    pub data: Vec<f64>,
}

/// Parses an Excel scalar into a positive integer dimension (rows or cols).
/// Rejects non-numeric, non-integer, zero, negative, and out-of-range values.
pub fn parse_dimension(value: &Variant) -> Result<usize, CvxError> {
    let n = coerce_to_f64(value).ok_or_else(|| {
        CvxError::InvalidDimension("dimension must be a positive integer".to_string())
    })?;

    if !n.is_finite() || n.fract() != 0.0 || n <= 0.0 {
        return Err(CvxError::InvalidDimension(
            "dimension must be a positive integer".to_string(),
        ));
    }

    let limit = MAX_DIMENSION as f64;
    if n > limit {
        return Err(CvxError::InvalidDimension(format!(
            "dimension exceeds maximum of {MAX_DIMENSION}"
        )));
    }

    Ok(n as usize)
}

/// Parses an Excel scalar into a numeric constant.
pub fn parse_scalar(value: &Variant) -> Result<f64, CvxError> {
    coerce_to_f64(value)
        .ok_or_else(|| CvxError::InvalidExpression("scalar argument must be numeric".to_string()))
}

/// Parses a required string argument, coercing single-cell references to
/// their string content.
pub fn parse_string(value: &Variant) -> Result<String, CvxError> {
    if is_blank(value) {
        return Err(CvxError::InvalidExpression(
            "expected a string argument".to_string(),
        ));
    }

    if let Some(s) = value.as_string() {
        return Ok(s);
    }

    let coerced = excel12(
        xlCoerce,
        &mut [value.clone(), Variant::from_int(xltypeStr as i32)],
    );
    if let Some(s) = coerced.as_string() {
        return Ok(s);
    }

    Err(CvxError::InvalidExpression(
        "expected a string argument".to_string(),
    ))
}

/// Parses an optional name argument. Missing/blank/empty strings become
/// `None`. Single-cell references containing a string are resolved.
pub fn parse_optional_name(value: &Variant) -> Result<Option<String>, CvxError> {
    if is_blank(value) {
        return Ok(None);
    }

    let name = parse_string(value)?;
    let trimmed = name.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn coerce_to_f64(value: &Variant) -> Option<f64> {
    if let Some(f) = value.as_f64() {
        return Some(f);
    }
    if let Some(i) = value.as_i32() {
        return Some(f64::from(i));
    }
    if is_blank(value) {
        return None;
    }

    let coerced = excel12(
        xlCoerce,
        &mut [value.clone(), Variant::from_int(xltypeNum as i32)],
    );
    coerced.as_f64()
}

fn is_blank(value: &Variant) -> bool {
    value.is_missing() || value.to_string() == "#NIL"
}

/// Parses an Excel range into a dense row-major `f64` matrix. Empty cells
/// are treated as zero; a missing or empty argument is an error.
pub fn parse_range(range: &Variant) -> Result<ParsedRange, CvxError> {
    let (cols, rows) = range.dim();
    if cols == 0 || rows == 0 {
        return Err(CvxError::EmptyRange);
    }

    let mut data = Vec::with_capacity(rows * cols);
    for row in 0..rows {
        for col in 0..cols {
            data.push(parse_cell(&range.at(col, row))?);
        }
    }

    Ok(ParsedRange {
        shape: (rows, cols),
        data,
    })
}

fn parse_cell(cell: &Variant) -> Result<f64, CvxError> {
    if let Some(n) = cell.as_f64() {
        return Ok(n);
    }
    if let Some(i) = cell.as_i32() {
        return Ok(i as f64);
    }
    if let Some(s) = cell.as_string() {
        return if s.trim().is_empty() {
            Ok(0.0)
        } else {
            Err(CvxError::NonNumericCell)
        };
    }
    // Blank cells within a range surface as a nil XLOPER, which has no
    // dedicated accessor; fall back to the Display representation.
    if cell.to_string() == "#NIL" {
        return Ok(0.0);
    }
    Err(CvxError::NonNumericCell)
}

/// Parses a range of cells as optional strings, flattening it row-major.
/// Blank cells and empty/whitespace-only strings become `None`. A non-blank
/// cell that does not contain a string is an error. A missing/empty range
/// yields an empty vector rather than an error, since it is a valid input to
/// `CVX.CONSTRAINTS` when combined with other arguments.
pub fn parse_optional_string_range(range: &Variant) -> Result<Vec<Option<String>>, CvxError> {
    let (cols, rows) = range.dim();
    if cols == 0 || rows == 0 {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity(rows * cols);
    for row in 0..rows {
        for col in 0..cols {
            out.push(parse_optional_string_cell(&range.at(col, row))?);
        }
    }
    Ok(out)
}

fn parse_optional_string_cell(cell: &Variant) -> Result<Option<String>, CvxError> {
    if is_blank(cell) {
        return Ok(None);
    }
    if let Some(s) = cell.as_string() {
        let trimmed = s.trim();
        return Ok(if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        });
    }
    Err(CvxError::InvalidExpression(
        "expected a constraint handle or name".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_scalar() {
        let v = Variant::from_float(4.0);
        let parsed = parse_range(&v).unwrap();
        assert_eq!(parsed.shape, (1, 1));
        assert_eq!(parsed.data, vec![4.0]);
    }

    #[test]
    fn parses_a_matrix_row_major() {
        // 2 columns x 3 rows.
        let cells: Vec<Variant> = (1..=6).map(|n| Variant::from_float(n as f64)).collect();
        let v = Variant::from_array(2, 3, &cells);
        let parsed = parse_range(&v).unwrap();
        assert_eq!(parsed.shape, (3, 2));
        assert_eq!(parsed.data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn treats_blank_cells_as_zero() {
        let cells = vec![Variant::from_float(1.0), Variant::new()];
        let v = Variant::from_array(2, 1, &cells);
        let parsed = parse_range(&v).unwrap();
        assert_eq!(parsed.data, vec![1.0, 0.0]);
    }

    #[test]
    fn rejects_text_cells() {
        let cells = vec![Variant::from_float(1.0), Variant::from_str("abc")];
        let v = Variant::from_array(2, 1, &cells);
        assert_eq!(parse_range(&v).unwrap_err(), CvxError::NonNumericCell);
    }

    #[test]
    fn rejects_missing_range() {
        assert_eq!(
            parse_range(&Variant::missing()).unwrap_err(),
            CvxError::EmptyRange
        );
    }

    #[test]
    fn parses_integer_dimension() {
        assert_eq!(parse_dimension(&Variant::from_float(3.0)).unwrap(), 3);
    }

    #[test]
    fn rejects_non_numeric_dimension() {
        assert_eq!(
            parse_dimension(&Variant::from_str("abc")).unwrap_err(),
            CvxError::InvalidDimension("dimension must be a positive integer".to_string())
        );
    }

    #[test]
    fn rejects_zero_dimension() {
        assert_eq!(
            parse_dimension(&Variant::from_float(0.0)).unwrap_err(),
            CvxError::InvalidDimension("dimension must be a positive integer".to_string())
        );
    }

    #[test]
    fn rejects_negative_dimension() {
        assert_eq!(
            parse_dimension(&Variant::from_float(-2.0)).unwrap_err(),
            CvxError::InvalidDimension("dimension must be a positive integer".to_string())
        );
    }

    #[test]
    fn rejects_fractional_dimension() {
        assert_eq!(
            parse_dimension(&Variant::from_float(2.5)).unwrap_err(),
            CvxError::InvalidDimension("dimension must be a positive integer".to_string())
        );
    }

    #[test]
    fn rejects_oversized_dimension() {
        assert!(matches!(
            parse_dimension(&Variant::from_float((MAX_DIMENSION + 1) as f64)).unwrap_err(),
            CvxError::InvalidDimension(_)
        ));
    }

    #[test]
    fn parses_string_range_flattened_row_major() {
        let cells = vec![
            Variant::from_str("a"),
            Variant::from_str("b"),
            Variant::from_str("c"),
            Variant::from_str("d"),
        ];
        let v = Variant::from_array(2, 2, &cells);
        let parsed = parse_optional_string_range(&v).unwrap();
        assert_eq!(
            parsed,
            vec![
                Some("a".to_string()),
                Some("b".to_string()),
                Some("c".to_string()),
                Some("d".to_string()),
            ]
        );
    }

    #[test]
    fn treats_blank_cells_as_none_in_string_range() {
        let cells = vec![Variant::from_str("a"), Variant::new()];
        let v = Variant::from_array(2, 1, &cells);
        let parsed = parse_optional_string_range(&v).unwrap();
        assert_eq!(parsed, vec![Some("a".to_string()), None]);
    }

    #[test]
    fn rejects_non_string_cells_in_string_range() {
        let cells = vec![Variant::from_str("a"), Variant::from_float(1.0)];
        let v = Variant::from_array(2, 1, &cells);
        assert!(matches!(
            parse_optional_string_range(&v).unwrap_err(),
            CvxError::InvalidExpression(_)
        ));
    }

    #[test]
    fn treats_missing_range_as_empty() {
        let parsed = parse_optional_string_range(&Variant::missing()).unwrap();
        assert!(parsed.is_empty());
    }
}
