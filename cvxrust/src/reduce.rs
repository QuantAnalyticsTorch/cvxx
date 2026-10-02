//! Reduces (possibly vector/matrix-shaped) [`Expression`] trees into
//! numeric linear/quadratic forms ready for conic-program assembly.
//!
//! A single reduction function, `linearize_shaped`, handles every shape
//! uniformly (SPEC-0016): a scalar `(1, 1)` expression is simply the
//! `(1, 1)` special case of the same shape-broadcasting machinery used for
//! vector/matrix expressions. It supports vector/matrix-shaped variables
//! and parameters (`Sum`, `Index` sub-block selection, `MatMul`/
//! `Transpose`, SPEC-0014/SPEC-0015/SPEC-0018), and reduces a product or
//! matrix-product of two variable-dependent operands to a genuine
//! quadratic (degree-2) term via the shared `multiply_forms` helper,
//! rejecting only genuine degree-3-or-higher products (SPEC-0011/
//! SPEC-0016).
//!
//! [`crate::solver::solve`] calls only [`reduce_expression`] (a thin
//! wrapper around `linearize_shaped`); it never calls `linearize_shaped`
//! directly.

use std::collections::HashMap;

use crate::model::Expression;

/// The reduction of an `Expression` into
/// `constant + linear . x + sum_{i<=j} quad[(i,j)] * x_i * x_j` form,
/// positionally aligned with a problem's variable list. `quad` is empty for
/// a purely affine expression, which is the common case and takes the same
/// code paths as a plain affine form throughout `solve`.
#[derive(Debug, Clone)]
pub(crate) struct QuadraticForm {
    pub(crate) constant: f64,
    pub(crate) linear: Vec<f64>,
    /// Sparse upper-triangular entries `(i, j, coeff)` with `i <= j`;
    /// `i == j` is the coefficient of `x_i^2`, `i < j` is the *total*
    /// coefficient of the `x_i * x_j` cross term (not halved).
    pub(crate) quad: Vec<(usize, usize, f64)>,
}

impl QuadraticForm {
    fn zero(n: usize, constant: f64) -> Self {
        QuadraticForm {
            constant,
            linear: vec![0.0; n],
            quad: Vec::new(),
        }
    }

    /// A pure number: no linear or quadratic dependence on any variable.
    fn is_constant(&self) -> bool {
        self.quad.is_empty() && self.linear.iter().all(|c| *c == 0.0)
    }

    /// No quadratic term (degree <= 1).
    pub(crate) fn is_affine(&self) -> bool {
        self.quad.is_empty()
    }

    pub(crate) fn negate(mut self) -> Self {
        self.constant = -self.constant;
        for c in self.linear.iter_mut() {
            *c = -*c;
        }
        for (_, _, c) in self.quad.iter_mut() {
            *c = -*c;
        }
        self
    }

    fn scale(mut self, scalar: f64) -> Self {
        self.constant *= scalar;
        for c in self.linear.iter_mut() {
            *c *= scalar;
        }
        for (_, _, c) in self.quad.iter_mut() {
            *c *= scalar;
        }
        self
    }

    fn add(mut self, other: &QuadraticForm) -> Self {
        self.constant += other.constant;
        for (a, b) in self.linear.iter_mut().zip(other.linear.iter()) {
            *a += *b;
        }
        self.quad = combine_quad(self.quad, &other.quad, 1.0);
        self
    }

    pub(crate) fn sub(mut self, other: &QuadraticForm) -> Self {
        self.constant -= other.constant;
        for (a, b) in self.linear.iter_mut().zip(other.linear.iter()) {
            *a -= *b;
        }
        self.quad = combine_quad(self.quad, &other.quad, -1.0);
        self
    }
}

/// Merges `quad`'s entries with `sign * other`'s entries, keyed by `(i, j)`,
/// dropping any resulting zero coefficients to keep the list sparse.
fn combine_quad(
    quad: Vec<(usize, usize, f64)>,
    other: &[(usize, usize, f64)],
    sign: f64,
) -> Vec<(usize, usize, f64)> {
    let mut map: HashMap<(usize, usize), f64> = HashMap::new();
    for (i, j, c) in quad {
        *map.entry((i, j)).or_insert(0.0) += c;
    }
    for &(i, j, c) in other {
        *map.entry((i, j)).or_insert(0.0) += sign * c;
    }
    map.into_iter()
        .filter(|(_, c)| *c != 0.0)
        .map(|((i, j), c)| (i, j, c))
        .collect()
}

/// Builds the sparse, `i <= j`-canonicalized `(i, j, coeff)` list for
/// `sum_i sum_j a[i] * b[j] * x_i * x_j`, the quadratic term produced by
/// multiplying two affine forms with linear coefficients `a` and `b`.
fn outer_product_symmetrized(a: &[f64], b: &[f64]) -> Vec<(usize, usize, f64)> {
    let n = a.len();
    let mut result = Vec::new();
    for i in 0..n {
        for j in i..n {
            let coeff = if i == j {
                a[i] * b[i]
            } else {
                a[i] * b[j] + a[j] * b[i]
            };
            if coeff != 0.0 {
                result.push((i, j, coeff));
            }
        }
    }
    result
}

/// Row-major reduction of a (possibly vector/matrix-shaped) `Expression`:
/// one `QuadraticForm` per output entry, over the problem's full
/// `n_total`-long scalar-variable space. An entry's `quad` is non-empty
/// whenever that entry is a genuine degree-2 term — e.g. a product of two
/// variable-dependent operands (SPEC-0016) — regardless of the
/// expression's shape.
#[derive(Debug)]
pub(crate) struct ShapedForm {
    pub(crate) shape: (usize, usize),
    /// Row-major, length == shape.0 * shape.1.
    pub(crate) entries: Vec<QuadraticForm>,
}

/// Broadcasts two shapes per the existing `cvxx` diagnostic rule
/// (`src/analytics/shape.rs::broadcast_shape`, duplicated here since
/// `cvxrust` does not depend on `cvxx`): a `(1, 1)` operand broadcasts to
/// the other operand's shape; otherwise the two shapes must be equal.
pub(crate) fn broadcast_shape(
    a: (usize, usize),
    b: (usize, usize),
) -> Result<(usize, usize), String> {
    if a == (1, 1) {
        Ok(b)
    } else if b == (1, 1) || a == b {
        Ok(a)
    } else {
        Err(format!(
            "shape mismatch: {}x{} vs {}x{}",
            a.0, a.1, b.0, b.1
        ))
    }
}

/// Reads shaped-form entry `k` (row-major), broadcasting a `(1, 1)` form's
/// single entry across every `k` when `form.shape != (1, 1)`.
pub(crate) fn entry_at(form: &ShapedForm, k: usize) -> &QuadraticForm {
    if form.shape == (1, 1) {
        &form.entries[0]
    } else {
        &form.entries[k]
    }
}

/// Validates the standard 2-D matrix-multiplication shape rule: `a`'s
/// column count must equal `b`'s row count. Distinct from
/// `broadcast_shape`'s elementwise rule (SPEC-0018) — a `(1, 1)` operand
/// never broadcasts through this rule the way it does through
/// `broadcast_shape`. Duplicated from `src/analytics/shape.rs::matmul_shape`
/// (`cvxrust` does not depend on `cvxx`), same duplication rationale as
/// `broadcast_shape` above.
pub(crate) fn matmul_shape(a: (usize, usize), b: (usize, usize)) -> Result<(usize, usize), String> {
    if a.1 != b.0 {
        return Err(format!(
            "matrix multiplication requires the left operand's column \
             count to match the right operand's row count: {}x{} \
             (columns={}) vs {}x{} (rows={})",
            a.0, a.1, a.1, b.0, b.1, b.0
        ));
    }
    Ok((a.0, b.1))
}

/// A product of three or more variable-dependent terms (via any mix of
/// `Mul`/`MatMul`) exceeds the solver's degree-2 (quadratic) limit.
const DEGREE_ERROR: &str = "solver only supports linear and quadratic \
(degree <= 2) objectives and constraints; a product of three or more \
variable-dependent terms was found";

/// Division by a non-constant (variable-dependent) term is never legal,
/// regardless of shape or degree.
const DIVISION_ERROR: &str = "solver only supports linear or quadratic \
objectives and constraints; division by a variable-dependent term was found";

/// Reduces the product of two `QuadraticForm`s (already positioned over
/// the same `n_total`-long scalar-variable space) to a `QuadraticForm`.
///
/// - If either side is a pure constant, the product is the other side
///   scaled by that constant (degree unchanged).
/// - Else, if both sides are affine (`quad` empty) but neither is
///   constant, the product is a new degree-2 `QuadraticForm`.
/// - Else (at least one side already carries a quadratic term and the
///   other is non-constant) the product would be degree >= 3: rejected.
///
/// Used by both `linearize_shaped`'s `Expression::Mul` arm and
/// `matmul_entries`'s per-term accumulation (SPEC-0016) — a scalar product
/// is simply the `(1, 1)` special case of the same reduction a
/// vector/matrix-shaped product uses.
fn multiply_forms(l: QuadraticForm, r: QuadraticForm) -> Result<QuadraticForm, String> {
    if l.is_constant() {
        return Ok(r.scale(l.constant));
    }
    if r.is_constant() {
        return Ok(l.scale(r.constant));
    }
    if l.is_affine() && r.is_affine() {
        let linear: Vec<f64> = l
            .linear
            .iter()
            .zip(r.linear.iter())
            .map(|(&a, &b)| l.constant * b + r.constant * a)
            .collect();
        return Ok(QuadraticForm {
            constant: l.constant * r.constant,
            linear,
            quad: outer_product_symmetrized(&l.linear, &r.linear),
        });
    }
    Err(DEGREE_ERROR.to_string())
}

/// Shape-broadcasting reduction for every (objective, or one constraint
/// side) expression (SPEC-0014/SPEC-0016): a scalar `(1, 1)` expression is
/// simply the `(1, 1)` special case of the same machinery used for
/// vector/matrix expressions. Supports quadratic (degree-2) terms, via
/// `multiply_forms`, from a product or matrix-product of two
/// variable-dependent operands, rejecting only genuine degree-3-or-higher
/// products.
fn linearize_shaped(
    expr: &Expression,
    offsets: &HashMap<u64, usize>,
    n_total: usize,
) -> Result<ShapedForm, String> {
    match expr {
        Expression::Constant(c) => Ok(ShapedForm {
            shape: (1, 1),
            entries: vec![QuadraticForm::zero(n_total, *c)],
        }),
        Expression::Parameter { shape, data, .. } => Ok(ShapedForm {
            shape: *shape,
            entries: data
                .iter()
                .map(|&c| QuadraticForm::zero(n_total, c))
                .collect(),
        }),
        Expression::Variable(v) => match offsets.get(&v.id) {
            Some(&off) => {
                let count = v.shape.0 * v.shape.1;
                let entries = (0..count)
                    .map(|k| {
                        let mut form = QuadraticForm::zero(n_total, 0.0);
                        form.linear[off + k] = 1.0;
                        form
                    })
                    .collect();
                Ok(ShapedForm {
                    shape: v.shape,
                    entries,
                })
            }
            None => Err(
                "objective or constraint references a variable not included \
                 in the problem's variable list"
                    .to_string(),
            ),
        },
        Expression::Add(l, r) | Expression::Sub(l, r) => {
            let lf = linearize_shaped(l, offsets, n_total)?;
            let rf = linearize_shaped(r, offsets, n_total)?;
            let shape = broadcast_shape(lf.shape, rf.shape)?;
            let count = shape.0 * shape.1;
            let is_add = matches!(expr, Expression::Add(..));
            let entries = (0..count)
                .map(|k| {
                    let l_entry = entry_at(&lf, k).clone();
                    let r_entry = entry_at(&rf, k);
                    if is_add {
                        l_entry.add(r_entry)
                    } else {
                        l_entry.sub(r_entry)
                    }
                })
                .collect();
            Ok(ShapedForm { shape, entries })
        }
        Expression::Mul(l, r) => {
            let lf = linearize_shaped(l, offsets, n_total)?;
            let rf = linearize_shaped(r, offsets, n_total)?;
            let shape = broadcast_shape(lf.shape, rf.shape)?;
            let count = shape.0 * shape.1;
            let mut entries = Vec::with_capacity(count);
            for k in 0..count {
                let l_entry = entry_at(&lf, k).clone();
                let r_entry = entry_at(&rf, k).clone();
                entries.push(multiply_forms(l_entry, r_entry)?);
            }
            Ok(ShapedForm { shape, entries })
        }
        Expression::Div(l, r) => {
            let lf = linearize_shaped(l, offsets, n_total)?;
            let rf = linearize_shaped(r, offsets, n_total)?;
            let shape = broadcast_shape(lf.shape, rf.shape)?;
            let count = shape.0 * shape.1;
            let mut entries = Vec::with_capacity(count);
            for k in 0..count {
                let l_entry = entry_at(&lf, k);
                let r_entry = entry_at(&rf, k);
                if !r_entry.is_constant() {
                    return Err(DIVISION_ERROR.to_string());
                }
                if r_entry.constant.abs() < 1e-12 {
                    return Err("division by zero in objective or constraint".to_string());
                }
                entries.push(l_entry.clone().scale(1.0 / r_entry.constant));
            }
            Ok(ShapedForm { shape, entries })
        }
        Expression::Neg(e) => {
            let f = linearize_shaped(e, offsets, n_total)?;
            Ok(ShapedForm {
                shape: f.shape,
                entries: f.entries.into_iter().map(|e| e.negate()).collect(),
            })
        }
        Expression::Scale { scalar, expr } => {
            let f = linearize_shaped(expr, offsets, n_total)?;
            Ok(ShapedForm {
                shape: f.shape,
                entries: f.entries.into_iter().map(|e| e.scale(*scalar)).collect(),
            })
        }
        Expression::Sum(e) => {
            let inner = linearize_shaped(e, offsets, n_total)?;
            let summed = inner
                .entries
                .into_iter()
                .fold(QuadraticForm::zero(n_total, 0.0), |acc, entry| {
                    acc.add(&entry)
                });
            Ok(ShapedForm {
                shape: (1, 1),
                entries: vec![summed],
            })
        }
        Expression::Index {
            expr,
            row_start,
            col_start,
            rows,
            cols,
        } => {
            let inner = linearize_shaped(expr, offsets, n_total)?;
            select_sub_block(&inner, *row_start, *col_start, *rows, *cols)
        }
        Expression::Transpose(e) => {
            let inner = linearize_shaped(e, offsets, n_total)?;
            Ok(transpose_entries(&inner))
        }
        Expression::MatMul(l, r) => {
            let lf = linearize_shaped(l, offsets, n_total)?;
            let rf = linearize_shaped(r, offsets, n_total)?;
            matmul_entries(&lf, &rf, n_total)
        }
    }
}

/// Row-major transpose: a `rows x cols` form becomes `cols x rows`, entry
/// `(r, c)` moving to `(c, r)` (SPEC-0018).
fn transpose_entries(form: &ShapedForm) -> ShapedForm {
    let (rows, cols) = form.shape;
    let mut entries = Vec::with_capacity(rows * cols);
    for c in 0..cols {
        for r in 0..rows {
            entries.push(form.entries[r * cols + c].clone());
        }
    }
    ShapedForm {
        shape: (cols, rows),
        entries,
    }
}

/// Standard (2-D) matrix multiplication: entry `(i, j)` of the `m x n`
/// result is `sum_k l[i, k] * r[k, j]` over the shared `k` dimension. Each
/// individual product term is reduced via `multiply_forms` (SPEC-0016), so
/// a term with two variable-dependent sides becomes a quadratic term
/// rather than an error, as long as it stays degree <= 2; the running
/// per-output-entry accumulation is via repeated `QuadraticForm::add`
/// (SPEC-0018).
fn matmul_entries(l: &ShapedForm, r: &ShapedForm, n_total: usize) -> Result<ShapedForm, String> {
    let (m, k) = l.shape;
    let (k2, n) = r.shape;
    if k != k2 {
        return Err(matmul_shape(l.shape, r.shape).unwrap_err());
    }
    let mut entries = Vec::with_capacity(m * n);
    for i in 0..m {
        for j in 0..n {
            let mut acc = QuadraticForm::zero(n_total, 0.0);
            for t in 0..k {
                let l_entry = l.entries[i * k + t].clone();
                let r_entry = r.entries[t * n + j].clone();
                let term = multiply_forms(l_entry, r_entry)?;
                acc = acc.add(&term);
            }
            entries.push(acc);
        }
    }
    Ok(ShapedForm {
        shape: (m, n),
        entries,
    })
}

/// Row-major sub-block selection: entries at rows
/// `[row_start, row_start + rows)` and columns
/// `[col_start, col_start + cols)` of `form`, into a new `ShapedForm` of
/// shape `(rows, cols)`. Returns a descriptive `Err`, never panics, when
/// the requested block does not fit `form.shape` — defensive only; `cvxx`
/// already rejects this eagerly at `CVX.INDEX`/`index(...)` construction
/// time (see `src/analytics/shape.rs::check_index_bounds`), so this is
/// normally unreachable in practice, same reasoning as every other
/// "defensive, not `unreachable!()`" arm in this module.
fn select_sub_block(
    form: &ShapedForm,
    row_start: usize,
    col_start: usize,
    rows: usize,
    cols: usize,
) -> Result<ShapedForm, String> {
    let (form_rows, form_cols) = form.shape;
    if row_start + rows > form_rows || col_start + cols > form_cols {
        return Err(format!(
            "requested rows {}..{} and columns {}..{} are out of bounds for \
             a {form_rows}x{form_cols} operand",
            row_start + 1,
            row_start + rows,
            col_start + 1,
            col_start + cols
        ));
    }
    let mut entries = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let k = (row_start + r) * form_cols + (col_start + c);
            entries.push(form.entries[k].clone());
        }
    }
    Ok(ShapedForm {
        shape: (rows, cols),
        entries,
    })
}

/// Reduces any (objective, or one constraint side) expression to a
/// per-entry shaped form via the unified `linearize_shaped` (SPEC-0016).
/// The only entry point [`crate::solver::solve`] calls into this module.
pub(crate) fn reduce_expression(
    expr: &Expression,
    offsets: &HashMap<u64, usize>,
    n_total: usize,
) -> Result<ShapedForm, String> {
    linearize_shaped(expr, offsets, n_total)
}

#[cfg(test)]
#[path = "reduce_tests.rs"]
mod tests;
