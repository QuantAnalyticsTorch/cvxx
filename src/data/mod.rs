//! Excel range parsing and normalization into dense Rust matrices.

use xladd::variant::Variant;

use crate::core::error::CvxError;

/// A dense numeric range read from Excel, in row-major order.
#[derive(Debug)]
pub struct ParsedRange {
    /// `(rows, cols)`.
    pub shape: (usize, usize),
    pub data: Vec<f64>,
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
}
