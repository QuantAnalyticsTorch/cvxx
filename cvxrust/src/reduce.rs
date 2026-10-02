//! Reduces (possibly vector/matrix-shaped) [`Expression`] trees into
//! numeric linear/quadratic forms ready for conic-program assembly.
//!
//! Two parallel reduction paths exist, selected by [`reduce_expression`]'s
//! routing rule (SPEC-0014):
//!
//! - `quadratize` â€” the original scalar-only reduction (SPEC-0010/
//!   SPEC-0011), which supports quadratic (degree-2, "variable Ã—
//!   variable") terms, but only when every `Variable`/`Parameter` leaf in
//!   the expression has shape `(1, 1)` and the expression contains no
//!   `Sum`.
//! - `linearize_shaped` â€” the affine-only, shape-broadcasting reduction
//!   (SPEC-0014) used otherwise, which supports vector/matrix-shaped
//!   variables and parameters (and `Sum`), but rejects any product or
//!   quotient of two variable-dependent operands.
//!
//! [`crate::solver::solve`] calls only [`reduce_expression`]; it never
//! calls `quadratize`/`linearize_shaped` directly.

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

/// `true` when every `Variable`/`Parameter` leaf reachable from `expr` has
/// shape `(1, 1)` **and** `expr` contains no `Sum` node. (`Sum` always
/// collapses its operand to `(1, 1)`, but â€” like a non-scalar leaf â€” it
/// forces the affine-only `linearize_shaped` path; see below.) Preserves
/// SPEC-0011's quadratic support exactly for any expression where this is
/// `true`.
fn is_all_scalar(expr: &Expression) -> bool {
    check_expr_shapes(expr).is_ok()
}

fn check_expr_shapes(expr: &Expression) -> Result<(), String> {
    const SHAPE_ERROR: &str = "solver only supports scalar (1x1) variables and parameters";

    match expr {
        Expression::Constant(_) => Ok(()),
        Expression::Parameter { shape, .. } => {
            if *shape != (1, 1) {
                Err(SHAPE_ERROR.to_string())
            } else {
                Ok(())
            }
        }
        Expression::Variable(v) => {
            if v.shape != (1, 1) {
                Err(SHAPE_ERROR.to_string())
            } else {
                Ok(())
            }
        }
        Expression::Add(l, r)
        | Expression::Sub(l, r)
        | Expression::Mul(l, r)
        | Expression::Div(l, r) => {
            check_expr_shapes(l)?;
            check_expr_shapes(r)
        }
        Expression::Neg(e) => check_expr_shapes(e),
        Expression::Scale { expr, .. } => check_expr_shapes(expr),
        Expression::Sum(_) => Err(SHAPE_ERROR.to_string()),
    }
}

/// Row-major reduction of a (possibly vector/matrix-shaped) `Expression`:
/// one `QuadraticForm` per output entry, over the problem's full
/// `n_total`-long scalar-variable space. Every entry's `quad` is empty
/// unless `shape == (1, 1)` (see [`reduce_expression`]'s routing rule,
/// SPEC-0014).
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

const VECTOR_MUL_ERROR: &str = "solver only supports linear (affine) \
objectives and constraints once a vector or matrix (non-1x1) variable or \
parameter is involved; a product of two variable-dependent terms was found";

const VECTOR_DIV_ERROR: &str = "solver only supports linear (affine) \
objectives and constraints once a vector or matrix (non-1x1) variable or \
parameter is involved; division by a variable-dependent term was found";

/// Affine, shape-broadcasting reduction used whenever `is_all_scalar`
/// is `false` for the top-level expression being reduced (SPEC-0014).
/// Recurses into itself (not [`reduce_expression`]): once a non-`(1, 1)`
/// leaf or a `Sum` appears anywhere in an objective or constraint side, the
/// entire side is restricted to affine (degree <= 1) arithmetic, even for
/// an otherwise-scalar sub-expression nested inside it. This is a
/// deliberate scope boundary (SPEC-0014 Non-Objective), not an oversight.
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
                let l_entry = entry_at(&lf, k);
                let r_entry = entry_at(&rf, k);
                entries.push(if l_entry.is_constant() {
                    r_entry.clone().scale(l_entry.constant)
                } else if r_entry.is_constant() {
                    l_entry.clone().scale(r_entry.constant)
                } else {
                    return Err(VECTOR_MUL_ERROR.to_string());
                });
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
                    return Err(VECTOR_DIV_ERROR.to_string());
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
    }
}

/// Reduces any (objective, or one constraint side) expression to a
/// per-entry shaped form, routing through the quadratic-capable
/// `quadratize` when `is_all_scalar` (preserving SPEC-0011 exactly) or the
/// affine-only, shape-broadcasting `linearize_shaped` otherwise
/// (SPEC-0014). The only entry point [`crate::solver::solve`] calls into
/// this module.
pub(crate) fn reduce_expression(
    expr: &Expression,
    offsets: &HashMap<u64, usize>,
    n_total: usize,
) -> Result<ShapedForm, String> {
    if is_all_scalar(expr) {
        let form = quadratize(expr, offsets, n_total)?;
        Ok(ShapedForm {
            shape: (1, 1),
            entries: vec![form],
        })
    } else {
        linearize_shaped(expr, offsets, n_total)
    }
}

/// Step 2 â€” reduces `expr` to a [`QuadraticForm`] over `problem.variables`
/// (via `index`, mapping variable id to position), or returns a descriptive
/// error for any construct of degree higher than 2 (or an illegal
/// division).
fn quadratize(
    expr: &Expression,
    index: &HashMap<u64, usize>,
    n: usize,
) -> Result<QuadraticForm, String> {
    match expr {
        Expression::Constant(c) => Ok(QuadraticForm::zero(n, *c)),
        Expression::Parameter { data, .. } => Ok(QuadraticForm::zero(n, data[0])),
        Expression::Variable(v) => match index.get(&v.id) {
            Some(&i) => {
                let mut form = QuadraticForm::zero(n, 0.0);
                form.linear[i] = 1.0;
                Ok(form)
            }
            None => Err(
                "objective or constraint references a variable not included in the problem's variable list"
                    .to_string(),
            ),
        },
        Expression::Add(l, r) => {
            let lf = quadratize(l, index, n)?;
            let rf = quadratize(r, index, n)?;
            Ok(lf.add(&rf))
        }
        Expression::Sub(l, r) => {
            let lf = quadratize(l, index, n)?;
            let rf = quadratize(r, index, n)?;
            Ok(lf.sub(&rf))
        }
        Expression::Neg(e) => Ok(quadratize(e, index, n)?.negate()),
        Expression::Scale { scalar, expr } => Ok(quadratize(expr, index, n)?.scale(*scalar)),
        Expression::Mul(l, r) => {
            let lf = quadratize(l, index, n)?;
            let rf = quadratize(r, index, n)?;
            if lf.is_constant() {
                Ok(rf.scale(lf.constant))
            } else if rf.is_constant() {
                Ok(lf.scale(rf.constant))
            } else if lf.is_affine() && rf.is_affine() {
                let linear: Vec<f64> = (0..n)
                    .map(|i| lf.constant * rf.linear[i] + rf.constant * lf.linear[i])
                    .collect();
                Ok(QuadraticForm {
                    constant: lf.constant * rf.constant,
                    linear,
                    quad: outer_product_symmetrized(&lf.linear, &rf.linear),
                })
            } else {
                Err(
                    "solver only supports linear and quadratic (degree <= 2) objectives and constraints; a product of three or more variable-dependent terms was found"
                        .to_string(),
                )
            }
        }
        Expression::Div(l, r) => {
            let lf = quadratize(l, index, n)?;
            let rf = quadratize(r, index, n)?;
            if rf.is_constant() {
                if rf.constant.abs() < 1e-12 {
                    Err("division by zero in objective or constraint".to_string())
                } else {
                    Ok(lf.scale(1.0 / rf.constant))
                }
            } else {
                Err(
                    "solver only supports linear or quadratic objectives and constraints; division by a variable-dependent term was found"
                        .to_string(),
                )
            }
        }
        Expression::Sum(e) => {
            // By construction, `quadratize` is only ever invoked when
            // `is_all_scalar` is `true`, and `is_all_scalar` returns `false`
            // for any expression containing `Sum` anywhere (SPEC-0014), so
            // this arm is unreachable in practice. It is still implemented
            // defensively (not `unreachable!()`) to keep this match
            // exhaustive without ever panicking on malformed-but-well-typed
            // input, consistent with the rest of this module.
            let inner = reduce_expression(e, index, n)?;
            Ok(inner
                .entries
                .into_iter()
                .fold(QuadraticForm::zero(n, 0.0), |acc, x| acc.add(&x)))
        }
    }
}

#[cfg(test)]
#[path = "reduce_tests.rs"]
mod tests;
