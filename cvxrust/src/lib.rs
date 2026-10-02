//! `cvxrust` — convex optimization modeling layer for `cvxx`.
//!
//! Provides the variable, expression, and problem types used by the Excel
//! add-in, plus a linear-programming solver backend built on top of the
//! [`clarabel`] conic interior-point solver. `clarabel` is the standing
//! solver framework for this crate: it natively supports free (unrestricted
//! sign) variables and quadratic objectives, so this module's translation
//! layer is intended as the foundation for future quadratic-objective
//! support rather than a throwaway linear-only integration.

use std::collections::HashMap;

use clarabel::algebra::CscMatrix;
use clarabel::solver::{
    DefaultSettings, DefaultSolver, IPSolver, NonnegativeConeT, SecondOrderConeT,
    SolverStatus as ClarabelStatus, SupportedConeT, ZeroConeT,
};
use nalgebra::{DMatrix, SymmetricEigen};

/// A decision variable of a given shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variable {
    /// Identity distinguishing this variable from any other, even one of
    /// the same shape.
    pub id: u64,
    /// `(rows, cols)`.
    pub shape: (usize, usize),
}

impl Variable {
    /// Creates a new variable with the requested id and shape.
    pub fn new(id: u64, shape: (usize, usize)) -> Self {
        Variable { id, shape }
    }
}

/// A lazy convex optimization expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A scalar numeric constant.
    Constant(f64),
    /// A reference to a variable.
    Variable(Variable),
    /// A reference to a parameter (dense numeric data).
    Parameter {
        /// Identity distinguishing this parameter reference from any
        /// other, even one of the same shape and data. Not used by the
        /// solver; read only for display purposes by `cvxx`.
        id: u64,
        shape: (usize, usize),
        data: Vec<f64>,
    },
    /// Addition of two expressions.
    Add(Box<Expression>, Box<Expression>),
    /// Subtraction of two expressions.
    Sub(Box<Expression>, Box<Expression>),
    /// Multiplication of two expressions.
    Mul(Box<Expression>, Box<Expression>),
    /// Division of two expressions.
    Div(Box<Expression>, Box<Expression>),
    /// Unary negation.
    Neg(Box<Expression>),
    /// Scaling by a scalar constant.
    Scale { scalar: f64, expr: Box<Expression> },
    /// The sum of every entry of a (possibly vector/matrix-shaped)
    /// expression, reduced to a `(1, 1)` value (SPEC-0014).
    Sum(Box<Expression>),
}

impl Expression {
    /// Creates a constant expression.
    pub fn constant(value: f64) -> Self {
        Expression::Constant(value)
    }

    /// Creates a variable expression.
    pub fn from_variable(variable: Variable) -> Self {
        Expression::Variable(variable)
    }

    /// Creates a parameter expression with the given opaque identity.
    pub fn from_parameter(id: u64, shape: (usize, usize), data: Vec<f64>) -> Self {
        Expression::Parameter { id, shape, data }
    }

    /// Creates an addition expression.
    #[allow(clippy::should_implement_trait)]
    pub fn add(left: Expression, right: Expression) -> Self {
        Expression::Add(Box::new(left), Box::new(right))
    }

    /// Creates a subtraction expression.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(left: Expression, right: Expression) -> Self {
        Expression::Sub(Box::new(left), Box::new(right))
    }

    /// Creates a multiplication expression.
    #[allow(clippy::should_implement_trait)]
    pub fn mul(left: Expression, right: Expression) -> Self {
        Expression::Mul(Box::new(left), Box::new(right))
    }

    /// Creates a division expression.
    #[allow(clippy::should_implement_trait)]
    pub fn div(left: Expression, right: Expression) -> Self {
        Expression::Div(Box::new(left), Box::new(right))
    }

    /// Creates a negation expression.
    #[allow(clippy::should_implement_trait)]
    pub fn neg(expr: Expression) -> Self {
        Expression::Neg(Box::new(expr))
    }

    /// Creates a scalar-scaling expression.
    pub fn scale(scalar: f64, expr: Expression) -> Self {
        Expression::Scale {
            scalar,
            expr: Box::new(expr),
        }
    }

    /// Creates a sum-reduction expression: every entry of `expr` (which may
    /// be vector/matrix-shaped) collapsed into a single `(1, 1)` value
    /// (SPEC-0014).
    pub fn sum(expr: Expression) -> Self {
        Expression::Sum(Box::new(expr))
    }
}

/// The optimization sense of a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sense {
    Minimize,
    Maximize,
}

/// A relational operator between two expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    LessEqual,
    GreaterEqual,
    Equal,
}

/// A single constraint: a relation between two expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub relation: Relation,
    pub lhs: Expression,
    pub rhs: Expression,
}

/// A convex optimization problem: an objective with a sense, a list of
/// constraints, and the ordered list of variables the caller wants solved
/// values for.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub sense: Sense,
    pub objective: Expression,
    pub constraints: Vec<Constraint>,
    /// Ordered, positionally aligned with `Solution::variable_values`.
    pub variables: Vec<Variable>,
}

/// The outcome of attempting to solve a [`Problem`].
#[derive(Debug, Clone, PartialEq)]
pub enum SolveStatus {
    Optimal,
    Infeasible,
    Unbounded,
    Error(String),
}

/// The result of a solve attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    pub status: SolveStatus,
    pub objective_value: Option<f64>,
    /// Aligned by index with `Problem::variables`; each inner `Vec<f64>` is
    /// row-major data matching that variable's shape. Empty when `status`
    /// is not `Optimal`.
    pub variable_values: Vec<Vec<f64>>,
}

/// Upper bound on the number of scalar decision variables and constraint
/// rows a problem may have. Larger problems return `SolveStatus::Error`
/// rather than building an unbounded-size conic program.
pub const MAX_VARIABLES: usize = 200;
pub const MAX_CONSTRAINTS: usize = 200;

/// Upper bound on `clarabel` solver iterations before giving up rather than
/// running indefinitely, set via `DefaultSettings::max_iter`.
pub const MAX_ITERATIONS: u32 = 200;

/// Eigenvalues of a quadratic constraint's coefficient matrix below this
/// (in absolute value, when negative) are treated as a convexity violation;
/// eigenvalues at or below this threshold in magnitude are treated as zero
/// (dropped from the second-order-cone factorization).
const PSD_TOLERANCE: f64 = 1e-8;

/// The reduction of an `Expression` into
/// `constant + linear . x + sum_{i<=j} quad[(i,j)] * x_i * x_j` form,
/// positionally aligned with a problem's variable list. `quad` is empty for
/// a purely affine expression, which is the common case and takes the same
/// code paths as a plain affine form throughout `solve`.
#[derive(Debug, Clone)]
struct QuadraticForm {
    constant: f64,
    linear: Vec<f64>,
    /// Sparse upper-triangular entries `(i, j, coeff)` with `i <= j`;
    /// `i == j` is the coefficient of `x_i^2`, `i < j` is the *total*
    /// coefficient of the `x_i * x_j` cross term (not halved).
    quad: Vec<(usize, usize, f64)>,
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
    fn is_affine(&self) -> bool {
        self.quad.is_empty()
    }

    fn negate(mut self) -> Self {
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

    fn sub(mut self, other: &QuadraticForm) -> Self {
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
/// collapses its operand to `(1, 1)`, but — like a non-scalar leaf — it
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
/// unless `shape == (1, 1)` (see [`reduce`]'s routing rule, SPEC-0014).
#[derive(Debug)]
struct ShapedForm {
    shape: (usize, usize),
    /// Row-major, length == shape.0 * shape.1.
    entries: Vec<QuadraticForm>,
}

/// Broadcasts two shapes per the existing `cvxx` diagnostic rule
/// (`src/analytics/shape.rs::broadcast_shape`, duplicated here since
/// `cvxrust` does not depend on `cvxx`): a `(1, 1)` operand broadcasts to
/// the other operand's shape; otherwise the two shapes must be equal.
fn broadcast_shape(a: (usize, usize), b: (usize, usize)) -> Result<(usize, usize), String> {
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
fn entry_at(form: &ShapedForm, k: usize) -> &QuadraticForm {
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
/// Recurses into itself (not [`reduce`]): once a non-`(1, 1)` leaf or a
/// `Sum` appears anywhere in an objective or constraint side, the entire
/// side is restricted to affine (degree <= 1) arithmetic, even for an
/// otherwise-scalar sub-expression nested inside it. This is a deliberate
/// scope boundary (SPEC-0014 Non-Objective), not an oversight.
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
/// (SPEC-0014).
fn reduce(
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

/// Step 2 — reduces `expr` to a [`QuadraticForm`] over `problem.variables`
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
            let inner = reduce(e, index, n)?;
            Ok(inner
                .entries
                .into_iter()
                .fold(QuadraticForm::zero(n, 0.0), |acc, x| acc.add(&x)))
        }
    }
}

fn dot(coeffs: &[f64], x: &[f64]) -> f64 {
    coeffs.iter().zip(x.iter()).map(|(c, xi)| c * xi).sum()
}

/// Builds a `clarabel` CSC matrix from row-major dense data (each entry of
/// `rows` is one constraint row of length `ncols`).
fn dense_rows_to_csc(rows: &[Vec<f64>], ncols: usize) -> CscMatrix<f64> {
    let m = rows.len();
    let mut colptr = Vec::with_capacity(ncols + 1);
    let mut rowval = Vec::new();
    let mut nzval = Vec::new();
    colptr.push(0);
    for j in 0..ncols {
        for (i, row) in rows.iter().enumerate() {
            let value = row[j];
            if value != 0.0 {
                rowval.push(i);
                nzval.push(value);
            }
        }
        colptr.push(rowval.len());
    }
    CscMatrix::new(m, ncols, colptr, rowval, nzval)
}

fn error_solution(message: impl Into<String>) -> Solution {
    Solution {
        status: SolveStatus::Error(message.into()),
        objective_value: None,
        variable_values: Vec::new(),
    }
}

/// Builds a dense, symmetric `n x n` matrix from sparse upper-triangular
/// `(i, j, coeff)` entries (`i <= j`), applying `diag_scale` to `i == j`
/// entries and mirroring `off_scale * coeff` into both `(i, j)` and `(j, i)`
/// otherwise. Used both for `clarabel`'s objective `P` matrix (`diag_scale =
/// 2.0, off_scale = 1.0`, since `clarabel` minimizes `(1/2) x^T P x + ...`)
/// and for a quadratic constraint's coefficient matrix `Q` (`diag_scale =
/// 1.0, off_scale = 0.5`, since a constraint is evaluated as `x^T Q x`
/// directly).
fn symmetric_dense(
    entries: &[(usize, usize, f64)],
    n: usize,
    diag_scale: f64,
    off_scale: f64,
) -> Vec<Vec<f64>> {
    let mut dense = vec![vec![0.0; n]; n];
    for &(i, j, coeff) in entries {
        if i == j {
            dense[i][i] += diag_scale * coeff;
        } else {
            dense[i][j] += off_scale * coeff;
            dense[j][i] += off_scale * coeff;
        }
    }
    dense
}

/// One second-order-cone row block: `a_rows`/`b_vals` (equal length, one
/// entry per cone dimension) to append to the constraint matrix/vector.
type SocBlock = (Vec<Vec<f64>>, Vec<f64>);

/// Reduces a quadratic constraint already normalized to `diff <= 0` form
/// (`diff` is `x^T Q x + q . x + c`, `Q` from `diff.quad`) into a
/// second-order-cone row block `(a_rows, b_vals)` such that pushing these
/// rows/values and a `SecondOrderConeT(a_rows.len())` cone reproduces the
/// original constraint exactly (SPEC-0011). Returns `Ok(None)` when `Q`'s
/// eigenvalues are all within `PSD_TOLERANCE` of zero (the quadratic part
/// numerically cancels out, so the constraint is really affine), and an
/// `Err` when `Q` is not positive semidefinite (the constraint is not
/// convex).
fn build_soc_block(diff: &QuadraticForm, n: usize) -> Result<Option<SocBlock>, String> {
    let dense = symmetric_dense(&diff.quad, n, 1.0, 0.5);
    let matrix = DMatrix::from_row_iterator(n, n, dense.iter().flatten().copied());
    let eigen = SymmetricEigen::new(matrix);

    if eigen
        .eigenvalues
        .iter()
        .any(|&lambda| lambda < -PSD_TOLERANCE)
    {
        return Err(
            "quadratic constraint is not convex (matrix is not positive semidefinite)".to_string(),
        );
    }

    let mut z_rows: Vec<Vec<f64>> = Vec::new();
    for k in 0..n {
        let lambda = eigen.eigenvalues[k];
        if lambda > PSD_TOLERANCE {
            let scale = lambda.sqrt();
            let column = eigen.eigenvectors.column(k);
            z_rows.push((0..n).map(|i| scale * column[i]).collect());
        }
    }
    if z_rows.is_empty() {
        return Ok(None);
    }

    // p(x) = (1 - t(x)) / 2, w(x) = (-1 - t(x)) / 2, where t(x) = diff.linear . x
    // + diff.constant; both share the coefficient vector t(x) / 2, embedded as
    // `s = b - A x` rows so that `s == p(x)` / `s == w(x)` / `s == z_k(x)`.
    let half_t: Vec<f64> = diff.linear.iter().map(|c| c / 2.0).collect();
    let mut a_rows = Vec::with_capacity(z_rows.len() + 2);
    let mut b_vals = Vec::with_capacity(z_rows.len() + 2);
    a_rows.push(half_t.clone());
    b_vals.push((1.0 - diff.constant) / 2.0);
    a_rows.push(half_t);
    b_vals.push((-1.0 - diff.constant) / 2.0);
    for row in z_rows {
        a_rows.push(row.iter().map(|c| -c).collect());
        b_vals.push(0.0);
    }
    Ok(Some((a_rows, b_vals)))
}

/// Attempts to solve `problem` by translating it into a conic program and
/// delegating to `clarabel`. See the crate-level docs and `SPEC-0010`/
/// `SPEC-0014` for the supported problem class.
pub fn solve(problem: &Problem) -> Solution {
    const SIZE_LIMIT_ERROR: &str =
        "problem exceeds solver size limit (200 scalar variables / 200 scalar constraint rows)";

    let mut offsets: HashMap<u64, usize> = HashMap::with_capacity(problem.variables.len());
    let mut n_total = 0usize;
    for v in &problem.variables {
        offsets.insert(v.id, n_total);
        n_total += v.shape.0 * v.shape.1;
    }

    if n_total > MAX_VARIABLES {
        return error_solution(SIZE_LIMIT_ERROR);
    }

    let obj_form = match reduce(&problem.objective, &offsets, n_total) {
        Ok(form) => form,
        Err(message) => return error_solution(message),
    };
    if obj_form.shape != (1, 1) {
        return error_solution(format!(
            "objective must evaluate to a single value (shape 1x1); got shape {}x{}",
            obj_form.shape.0, obj_form.shape.1
        ));
    }
    let obj = obj_form.entries.into_iter().next().unwrap();

    // Reduce each constraint to `diff <relation> 0` (`diff = lhs - rhs`),
    // broadcasting `lhs`/`rhs` to a common shape and emitting one row per
    // broadcast output entry (SPEC-0014): affine rows go into `rows`
    // (unchanged from SPEC-0010), convex quadratic rows are reduced to a
    // second-order-cone row block in `soc_blocks` (SPEC-0011).
    let mut rows: Vec<(Relation, Vec<f64>, f64)> = Vec::with_capacity(problem.constraints.len());
    let mut soc_blocks: Vec<SocBlock> = Vec::new();
    let mut total_constraint_rows = 0usize;
    for constraint in &problem.constraints {
        let lhs = match reduce(&constraint.lhs, &offsets, n_total) {
            Ok(form) => form,
            Err(message) => return error_solution(message),
        };
        let rhs = match reduce(&constraint.rhs, &offsets, n_total) {
            Ok(form) => form,
            Err(message) => return error_solution(message),
        };
        let shape = match broadcast_shape(lhs.shape, rhs.shape) {
            Ok(shape) => shape,
            Err(message) => return error_solution(message),
        };
        let count = shape.0 * shape.1;

        total_constraint_rows += count;
        if total_constraint_rows > MAX_CONSTRAINTS {
            return error_solution(SIZE_LIMIT_ERROR);
        }

        for k in 0..count {
            let mut diff = entry_at(&lhs, k).clone().sub(entry_at(&rhs, k));
            let mut relation = constraint.relation;

            if !diff.is_affine() {
                if relation == Relation::Equal {
                    return error_solution("quadratic equality constraints are not supported");
                }
                if relation == Relation::GreaterEqual {
                    diff = diff.negate();
                    relation = Relation::LessEqual;
                }
                match build_soc_block(&diff, n_total) {
                    Ok(Some(block)) => {
                        soc_blocks.push(block);
                        continue;
                    }
                    Ok(None) => {
                        // The quadratic part cancels out numerically; fall
                        // through to the ordinary affine row below.
                    }
                    Err(message) => return error_solution(message),
                }
            }

            rows.push((relation, diff.linear.clone(), -diff.constant));
        }
    }

    // Group affine rows by cone: equalities first (ZeroConeT), then <= and
    // negated >= rows together (NonnegativeConeT).
    let mut a_rows: Vec<Vec<f64>> = Vec::with_capacity(rows.len());
    let mut b: Vec<f64> = Vec::with_capacity(rows.len());
    let mut num_equal = 0usize;
    for (relation, coeffs, row_rhs) in &rows {
        if *relation == Relation::Equal {
            a_rows.push(coeffs.clone());
            b.push(*row_rhs);
            num_equal += 1;
        }
    }
    let mut num_ineq = 0usize;
    for (relation, coeffs, row_rhs) in &rows {
        match relation {
            Relation::LessEqual => {
                a_rows.push(coeffs.clone());
                b.push(*row_rhs);
                num_ineq += 1;
            }
            Relation::GreaterEqual => {
                a_rows.push(coeffs.iter().map(|c| -c).collect());
                b.push(-*row_rhs);
                num_ineq += 1;
            }
            Relation::Equal => {}
        }
    }

    // `clarabel` 0.9's KKT setup does not tolerate a zero-column problem
    // (no decision variables at all); evaluate that degenerate case
    // directly instead of invoking the solver. A quadratic (SOC)
    // constraint can never arise here, since it requires a variable-
    // dependent product, so `soc_blocks` is always empty when
    // `n_total == 0`.
    if n_total == 0 {
        for (relation, _, row_rhs) in &rows {
            let ok = match relation {
                Relation::Equal => row_rhs.abs() < 1e-9,
                Relation::LessEqual => *row_rhs >= -1e-9,
                Relation::GreaterEqual => *row_rhs <= 1e-9,
            };
            if !ok {
                return Solution {
                    status: SolveStatus::Infeasible,
                    objective_value: None,
                    variable_values: Vec::new(),
                };
            }
        }
        return Solution {
            status: SolveStatus::Optimal,
            objective_value: Some(obj.constant),
            variable_values: Vec::new(),
        };
    }

    let mut cones: Vec<SupportedConeT<f64>> = Vec::new();
    if num_equal > 0 {
        cones.push(ZeroConeT(num_equal));
    }
    if num_ineq > 0 {
        cones.push(NonnegativeConeT(num_ineq));
    }
    for (block_rows, block_b) in &soc_blocks {
        a_rows.extend(block_rows.iter().cloned());
        b.extend(block_b.iter().copied());
        cones.push(SecondOrderConeT(block_rows.len()));
    }

    // `clarabel` minimizes `(1/2) x^T P x + q^T x`; for `Maximize`, both `P`
    // and `q` are negated (`max f(x) = -min(-f(x))`).
    let p_matrix = if obj.is_affine() {
        CscMatrix::<f64>::zeros((n_total, n_total))
    } else {
        let sign = if problem.sense == Sense::Maximize {
            -1.0
        } else {
            1.0
        };
        let dense = symmetric_dense(&obj.quad, n_total, 2.0 * sign, sign);
        dense_rows_to_csc(&dense, n_total)
    };
    let q: Vec<f64> = match problem.sense {
        Sense::Minimize => obj.linear.clone(),
        Sense::Maximize => obj.linear.iter().map(|c| -c).collect(),
    };
    let a_matrix = dense_rows_to_csc(&a_rows, n_total);

    let settings = DefaultSettings {
        max_iter: MAX_ITERATIONS,
        verbose: false,
        ..Default::default()
    };

    let mut solver = DefaultSolver::new(&p_matrix, &q, &a_matrix, &b, &cones, settings);
    solver.solve();

    match solver.solution.status {
        ClarabelStatus::Solved | ClarabelStatus::AlmostSolved => {
            let x = &solver.solution.x;
            let mut variable_values = Vec::with_capacity(problem.variables.len());
            let mut cursor = 0usize;
            for v in &problem.variables {
                let count = v.shape.0 * v.shape.1;
                variable_values.push(x[cursor..cursor + count].to_vec());
                cursor += count;
            }
            let quadratic_value: f64 = obj
                .quad
                .iter()
                .map(|&(i, j, coeff)| coeff * x[i] * x[j])
                .sum();
            let objective_value = obj.constant + dot(&obj.linear, x) + quadratic_value;
            Solution {
                status: SolveStatus::Optimal,
                objective_value: Some(objective_value),
                variable_values,
            }
        }
        ClarabelStatus::PrimalInfeasible | ClarabelStatus::AlmostPrimalInfeasible => Solution {
            status: SolveStatus::Infeasible,
            objective_value: None,
            variable_values: Vec::new(),
        },
        ClarabelStatus::DualInfeasible | ClarabelStatus::AlmostDualInfeasible => Solution {
            status: SolveStatus::Unbounded,
            objective_value: None,
            variable_values: Vec::new(),
        },
        other => error_solution(format!("solver did not converge: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(id: u64) -> Variable {
        Variable::new(id, (1, 1))
    }

    fn index_of(vars: &[Variable]) -> HashMap<u64, usize> {
        vars.iter().enumerate().map(|(i, v)| (v.id, i)).collect()
    }

    // --- quadratization tests ---

    #[test]
    fn quadratizes_a_constant() {
        let form = quadratize(&Expression::constant(2.5), &HashMap::new(), 0).unwrap();
        assert_eq!(form.constant, 2.5);
        assert!(form.linear.is_empty());
        assert!(form.quad.is_empty());
    }

    #[test]
    fn quadratizes_a_single_variable() {
        let vars = vec![var(1), var(2)];
        let index = index_of(&vars);
        let form = quadratize(&Expression::from_variable(vars[1]), &index, vars.len()).unwrap();
        assert_eq!(form.constant, 0.0);
        assert_eq!(form.linear, vec![0.0, 1.0]);
        assert!(form.quad.is_empty());
    }

    #[test]
    fn quadratizes_a_parameter() {
        let form = quadratize(
            &Expression::from_parameter(1, (1, 1), vec![7.0]),
            &HashMap::new(),
            0,
        )
        .unwrap();
        assert_eq!(form.constant, 7.0);
    }

    #[test]
    fn quadratizes_add_and_sub() {
        let vars = vec![var(1)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);
        let add = quadratize(
            &Expression::add(x.clone(), Expression::constant(3.0)),
            &index,
            1,
        )
        .unwrap();
        assert_eq!(add.constant, 3.0);
        assert_eq!(add.linear, vec![1.0]);

        let sub = quadratize(&Expression::sub(Expression::constant(3.0), x), &index, 1).unwrap();
        assert_eq!(sub.constant, 3.0);
        assert_eq!(sub.linear, vec![-1.0]);
    }

    #[test]
    fn quadratizes_neg_and_scale() {
        let vars = vec![var(1)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);

        let neg = quadratize(&Expression::neg(x.clone()), &index, 1).unwrap();
        assert_eq!(neg.linear, vec![-1.0]);

        let scaled = quadratize(&Expression::scale(4.0, x), &index, 1).unwrap();
        assert_eq!(scaled.linear, vec![4.0]);
    }

    #[test]
    fn quadratizes_mul_by_constant_either_side() {
        let vars = vec![var(1)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);

        let left = quadratize(
            &Expression::mul(Expression::constant(2.0), x.clone()),
            &index,
            1,
        )
        .unwrap();
        assert_eq!(left.linear, vec![2.0]);

        let right = quadratize(&Expression::mul(x, Expression::constant(3.0)), &index, 1).unwrap();
        assert_eq!(right.linear, vec![3.0]);
    }

    #[test]
    fn quadratizes_div_by_constant() {
        let vars = vec![var(1)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);

        let form = quadratize(&Expression::div(x, Expression::constant(2.0)), &index, 1).unwrap();
        assert_eq!(form.linear, vec![0.5]);
    }

    #[test]
    fn nested_combination_quadratizes_correctly() {
        let vars = vec![var(1), var(2)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);
        let y = Expression::from_variable(vars[1]);

        // 2 * (x - y) + 3
        let expr = Expression::add(
            Expression::scale(2.0, Expression::sub(x, y)),
            Expression::constant(3.0),
        );
        let form = quadratize(&expr, &index, 2).unwrap();
        assert_eq!(form.constant, 3.0);
        assert_eq!(form.linear, vec![2.0, -2.0]);
        assert!(form.quad.is_empty());
    }

    #[test]
    fn mul_of_two_distinct_variables_is_quadratic() {
        let vars = vec![var(1), var(2)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);
        let y = Expression::from_variable(vars[1]);

        let form = quadratize(&Expression::mul(x, y), &index, 2).unwrap();
        assert_eq!(form.constant, 0.0);
        assert_eq!(form.linear, vec![0.0, 0.0]);
        assert_eq!(form.quad, vec![(0, 1, 1.0)]);
    }

    #[test]
    fn mul_of_a_variable_with_itself_is_quadratic() {
        let vars = vec![var(1)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);

        let form = quadratize(&Expression::mul(x.clone(), x), &index, 1).unwrap();
        assert_eq!(form.quad, vec![(0, 0, 1.0)]);
    }

    #[test]
    fn mul_scales_an_existing_quadratic_term() {
        let vars = vec![var(1), var(2)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);
        let y = Expression::from_variable(vars[1]);
        let xy = Expression::mul(x, y);

        let form = quadratize(&Expression::mul(Expression::constant(3.0), xy), &index, 2).unwrap();
        assert_eq!(form.quad, vec![(0, 1, 3.0)]);
    }

    #[test]
    fn mul_of_three_variable_dependent_terms_is_a_degree_error() {
        let vars = vec![var(1), var(2)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);
        let y = Expression::from_variable(vars[1]);
        let xy = Expression::mul(x.clone(), y);

        let err = quadratize(&Expression::mul(xy, x), &index, 2).unwrap_err();
        assert_eq!(
            err,
            "solver only supports linear and quadratic (degree <= 2) objectives and constraints; a product of three or more variable-dependent terms was found"
        );
    }

    #[test]
    fn div_by_variable_is_nonlinear_error() {
        let vars = vec![var(1), var(2)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);
        let y = Expression::from_variable(vars[1]);

        let err = quadratize(&Expression::div(x, y), &index, 2).unwrap_err();
        assert_eq!(
            err,
            "solver only supports linear or quadratic objectives and constraints; division by a variable-dependent term was found"
        );
    }

    #[test]
    fn div_by_quadratic_constant_scales_quad_term() {
        let vars = vec![var(1), var(2)];
        let index = index_of(&vars);
        let x = Expression::from_variable(vars[0]);
        let y = Expression::from_variable(vars[1]);
        let xy = Expression::mul(x, y);

        let form = quadratize(&Expression::div(xy, Expression::constant(2.0)), &index, 2).unwrap();
        assert_eq!(form.quad, vec![(0, 1, 0.5)]);
    }

    #[test]
    fn div_by_zero_is_an_error() {
        let form = quadratize(
            &Expression::div(Expression::constant(1.0), Expression::constant(0.0)),
            &HashMap::new(),
            0,
        );
        assert_eq!(
            form.unwrap_err(),
            "division by zero in objective or constraint"
        );
    }

    #[test]
    fn non_scalar_variable_is_a_shape_error() {
        // A bare (2, 1) variable used directly as the objective is rejected
        // because the objective must evaluate to a single value (SPEC-0014);
        // the variable itself is no longer rejected outright (it may still
        // be used, e.g., in constraints).
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::from_variable(Variable::new(1, (2, 1))),
            constraints: Vec::new(),
            variables: vec![Variable::new(1, (2, 1))],
        };
        let solution = solve(&problem);
        assert_eq!(
            solution.status,
            SolveStatus::Error(
                "objective must evaluate to a single value (shape 1x1); got shape 2x1".to_string()
            )
        );
    }

    #[test]
    fn non_scalar_parameter_is_a_shape_error() {
        // Same as above, for a bare (2, 1) parameter used directly as the
        // objective (SPEC-0014).
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::from_parameter(1, (2, 1), vec![1.0, 2.0]),
            constraints: Vec::new(),
            variables: Vec::new(),
        };
        let solution = solve(&problem);
        assert_eq!(
            solution.status,
            SolveStatus::Error(
                "objective must evaluate to a single value (shape 1x1); got shape 2x1".to_string()
            )
        );
    }

    // --- solver tests ---

    fn scalar_var(id: u64) -> Variable {
        Variable::new(id, (1, 1))
    }

    #[test]
    fn solves_a_simple_minimize_problem() {
        // minimize x + y subject to x >= 1, y >= 2
        let x = scalar_var(1);
        let y = scalar_var(2);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::add(Expression::from_variable(x), Expression::from_variable(y)),
            constraints: vec![
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: Expression::from_variable(x),
                    rhs: Expression::constant(1.0),
                },
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: Expression::from_variable(y),
                    rhs: Expression::constant(2.0),
                },
            ],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.objective_value.unwrap() - 3.0).abs() < 1e-6);
        assert!((solution.variable_values[0][0] - 1.0).abs() < 1e-6);
        assert!((solution.variable_values[1][0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn solves_the_same_problem_as_a_maximize() {
        // maximize x + y subject to x <= 1, y <= 2
        let x = scalar_var(1);
        let y = scalar_var(2);
        let problem = Problem {
            sense: Sense::Maximize,
            objective: Expression::add(Expression::from_variable(x), Expression::from_variable(y)),
            constraints: vec![
                Constraint {
                    relation: Relation::LessEqual,
                    lhs: Expression::from_variable(x),
                    rhs: Expression::constant(1.0),
                },
                Constraint {
                    relation: Relation::LessEqual,
                    lhs: Expression::from_variable(y),
                    rhs: Expression::constant(2.0),
                },
            ],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.objective_value.unwrap() - 3.0).abs() < 1e-6);
        assert!((solution.variable_values[0][0] - 1.0).abs() < 1e-6);
        assert!((solution.variable_values[1][0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn solves_a_problem_with_only_equal_constraints() {
        // minimize x subject to x == 5
        let x = scalar_var(1);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::from_variable(x),
            constraints: vec![Constraint {
                relation: Relation::Equal,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(5.0),
            }],
            variables: vec![x],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.variable_values[0][0] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn solves_a_problem_with_mixed_constraint_types() {
        // minimize x + y subject to x >= 1, y <= 10, x + y == 6
        let x = scalar_var(1);
        let y = scalar_var(2);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::add(Expression::from_variable(x), Expression::from_variable(y)),
            constraints: vec![
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: Expression::from_variable(x),
                    rhs: Expression::constant(1.0),
                },
                Constraint {
                    relation: Relation::LessEqual,
                    lhs: Expression::from_variable(y),
                    rhs: Expression::constant(10.0),
                },
                Constraint {
                    relation: Relation::Equal,
                    lhs: Expression::add(
                        Expression::from_variable(x),
                        Expression::from_variable(y),
                    ),
                    rhs: Expression::constant(6.0),
                },
            ],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.objective_value.unwrap() - 6.0).abs() < 1e-6);
    }

    #[test]
    fn reports_infeasible_problems() {
        // x <= 0 and x >= 1 cannot both hold
        let x = scalar_var(1);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::from_variable(x),
            constraints: vec![
                Constraint {
                    relation: Relation::LessEqual,
                    lhs: Expression::from_variable(x),
                    rhs: Expression::constant(0.0),
                },
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: Expression::from_variable(x),
                    rhs: Expression::constant(1.0),
                },
            ],
            variables: vec![x],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Infeasible);
    }

    #[test]
    fn reports_unbounded_problems() {
        // maximize x with no upper bound
        let x = scalar_var(1);
        let problem = Problem {
            sense: Sense::Maximize,
            objective: Expression::from_variable(x),
            constraints: Vec::new(),
            variables: vec![x],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Unbounded);
    }

    #[test]
    fn solves_a_problem_with_negative_optimal_x() {
        // minimize x subject to x >= -5 (free variable, no manual splitting needed)
        let x = scalar_var(1);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::from_variable(x),
            constraints: vec![Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(-5.0),
            }],
            variables: vec![x],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.variable_values[0][0] - (-5.0)).abs() < 1e-6);
    }

    #[test]
    fn solves_a_bare_objective_with_no_constraints() {
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(42.0),
            constraints: Vec::new(),
            variables: Vec::new(),
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert_eq!(solution.objective_value, Some(42.0));
        assert!(solution.variable_values.is_empty());
    }

    #[test]
    fn exceeding_max_variables_is_a_size_error() {
        let variables: Vec<Variable> = (0..(MAX_VARIABLES as u64 + 1)).map(scalar_var).collect();
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: Vec::new(),
            variables,
        };
        let solution = solve(&problem);
        assert_eq!(
            solution.status,
            SolveStatus::Error(
                "problem exceeds solver size limit (200 scalar variables / 200 scalar constraint rows)".to_string()
            )
        );
    }

    #[test]
    fn non_convergence_maps_to_error_status() {
        // A tiny max_iter is simulated by exceeding MAX_CONSTRAINTS instead,
        // since MAX_ITERATIONS is a crate-level constant; this test instead
        // forces a non-Solved/AlmostSolved clarabel status by building an
        // unbounded-below problem, which resolves to `DualInfeasible` and is
        // exercised by `reports_unbounded_problems` above. To reach the
        // generic `MaxIterations` mapping specifically, we rely on a
        // pathological but valid LP that clarabel reports as
        // `NumericalError` when given a single iteration via a deliberately
        // tiny settings override is not reachable through the public
        // `solve` API (which fixes `MAX_ITERATIONS`), so this test instead
        // exercises the mapping function directly.
        let status = map_status_for_test(ClarabelStatus::MaxIterations);
        assert_eq!(
            status,
            SolveStatus::Error("solver did not converge: MaxIterations".to_string())
        );
    }

    fn map_status_for_test(status: ClarabelStatus) -> SolveStatus {
        match status {
            ClarabelStatus::Solved | ClarabelStatus::AlmostSolved => SolveStatus::Optimal,
            ClarabelStatus::PrimalInfeasible | ClarabelStatus::AlmostPrimalInfeasible => {
                SolveStatus::Infeasible
            }
            ClarabelStatus::DualInfeasible | ClarabelStatus::AlmostDualInfeasible => {
                SolveStatus::Unbounded
            }
            other => SolveStatus::Error(format!("solver did not converge: {other:?}")),
        }
    }

    // --- quadratic solver tests (SPEC-0011) ---

    #[test]
    fn solves_a_quadratic_objective() {
        // minimize x^2 + y^2 subject to x + y >= 1
        let x = scalar_var(1);
        let y = scalar_var(2);
        let x_expr = Expression::from_variable(x);
        let y_expr = Expression::from_variable(y);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::add(
                Expression::mul(x_expr.clone(), x_expr.clone()),
                Expression::mul(y_expr.clone(), y_expr.clone()),
            ),
            constraints: vec![Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::add(x_expr, y_expr),
                rhs: Expression::constant(1.0),
            }],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.objective_value.unwrap() - 0.5).abs() < 1e-5);
        assert!((solution.variable_values[0][0] - 0.5).abs() < 1e-4);
        assert!((solution.variable_values[1][0] - 0.5).abs() < 1e-4);
    }

    #[test]
    fn maximizes_a_concave_quadratic_objective() {
        // maximize -(x^2 + y^2) subject to x + y == 2 (optimum at x = y = 1)
        let x = scalar_var(1);
        let y = scalar_var(2);
        let x_expr = Expression::from_variable(x);
        let y_expr = Expression::from_variable(y);
        let problem = Problem {
            sense: Sense::Maximize,
            objective: Expression::neg(Expression::add(
                Expression::mul(x_expr.clone(), x_expr.clone()),
                Expression::mul(y_expr.clone(), y_expr.clone()),
            )),
            constraints: vec![Constraint {
                relation: Relation::Equal,
                lhs: Expression::add(x_expr, y_expr),
                rhs: Expression::constant(2.0),
            }],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.objective_value.unwrap() - (-2.0)).abs() < 1e-4);
        assert!((solution.variable_values[0][0] - 1.0).abs() < 1e-4);
        assert!((solution.variable_values[1][0] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn solves_a_quadratic_less_equal_constraint() {
        // minimize -x subject to x^2 + y^2 <= 1 (optimum at x = 1, y = 0)
        let x = scalar_var(1);
        let y = scalar_var(2);
        let x_expr = Expression::from_variable(x);
        let y_expr = Expression::from_variable(y);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::neg(Expression::from_variable(x)),
            constraints: vec![Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::add(
                    Expression::mul(x_expr.clone(), x_expr),
                    Expression::mul(y_expr.clone(), y_expr),
                ),
                rhs: Expression::constant(1.0),
            }],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.variable_values[0][0] - 1.0).abs() < 1e-4);
        assert!(solution.variable_values[1][0].abs() < 1e-4);
    }

    #[test]
    fn solves_a_quadratic_greater_equal_constraint() {
        // minimize -x subject to 1 >= x^2 + y^2 (equivalent to the <= case)
        let x = scalar_var(1);
        let y = scalar_var(2);
        let x_expr = Expression::from_variable(x);
        let y_expr = Expression::from_variable(y);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::neg(Expression::from_variable(x)),
            constraints: vec![Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::constant(1.0),
                rhs: Expression::add(
                    Expression::mul(x_expr.clone(), x_expr),
                    Expression::mul(y_expr.clone(), y_expr),
                ),
            }],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.variable_values[0][0] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn quadratic_equality_constraint_is_unsupported() {
        // x^2 == 1
        let x = scalar_var(1);
        let x_expr = Expression::from_variable(x);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: vec![Constraint {
                relation: Relation::Equal,
                lhs: Expression::mul(x_expr.clone(), x_expr),
                rhs: Expression::constant(1.0),
            }],
            variables: vec![x],
        };
        let solution = solve(&problem);
        assert_eq!(
            solution.status,
            SolveStatus::Error("quadratic equality constraints are not supported".to_string())
        );
    }

    #[test]
    fn indefinite_quadratic_constraint_is_rejected_as_non_convex() {
        // x * y <= 1 (Q has eigenvalues +0.5 / -0.5, not PSD)
        let x = scalar_var(1);
        let y = scalar_var(2);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: vec![Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::mul(Expression::from_variable(x), Expression::from_variable(y)),
                rhs: Expression::constant(1.0),
            }],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(
            solution.status,
            SolveStatus::Error(
                "quadratic constraint is not convex (matrix is not positive semidefinite)"
                    .to_string()
            )
        );
    }

    #[test]
    fn cubic_term_in_a_constraint_is_a_degree_error() {
        // x * x * y <= 1
        let x = scalar_var(1);
        let y = scalar_var(2);
        let x_expr = Expression::from_variable(x);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: vec![Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::mul(
                    Expression::mul(x_expr.clone(), x_expr),
                    Expression::from_variable(y),
                ),
                rhs: Expression::constant(1.0),
            }],
            variables: vec![x, y],
        };
        let solution = solve(&problem);
        assert_eq!(
            solution.status,
            SolveStatus::Error(
                "solver only supports linear and quadratic (degree <= 2) objectives and constraints; a product of three or more variable-dependent terms was found"
                    .to_string()
            )
        );
    }

    #[test]
    fn builds_a_constraint() {
        let constraint = Constraint {
            relation: Relation::LessEqual,
            lhs: Expression::constant(1.0),
            rhs: Expression::constant(2.0),
        };
        assert_eq!(constraint.relation, Relation::LessEqual);
    }

    // --- vector/matrix affine solving + Sum tests (SPEC-0014) ---

    fn offsets_of(vars: &[Variable]) -> (HashMap<u64, usize>, usize) {
        let mut offsets = HashMap::with_capacity(vars.len());
        let mut n_total = 0usize;
        for v in vars {
            offsets.insert(v.id, n_total);
            n_total += v.shape.0 * v.shape.1;
        }
        (offsets, n_total)
    }

    #[test]
    fn linearize_shaped_parameter_matches_its_data() {
        let form = linearize_shaped(
            &Expression::from_parameter(1, (3, 1), vec![10.0, 20.0, 30.0]),
            &HashMap::new(),
            0,
        )
        .unwrap();
        assert_eq!(form.shape, (3, 1));
        assert_eq!(
            form.entries.iter().map(|e| e.constant).collect::<Vec<_>>(),
            vec![10.0, 20.0, 30.0]
        );
        assert!(form
            .entries
            .iter()
            .all(|e| e.linear.is_empty() || e.linear.iter().all(|c| *c == 0.0)));
    }

    #[test]
    fn linearize_shaped_variable_is_one_hot_per_entry() {
        let w = Variable::new(1, (2, 2));
        let (offsets, n_total) = offsets_of(&[w]);
        let form = linearize_shaped(&Expression::from_variable(w), &offsets, n_total).unwrap();
        assert_eq!(form.shape, (2, 2));
        assert_eq!(form.entries.len(), 4);
        for (k, entry) in form.entries.iter().enumerate() {
            assert_eq!(entry.constant, 0.0);
            assert_eq!(entry.linear[k], 1.0);
            assert_eq!(entry.linear.iter().filter(|c| **c != 0.0).count(), 1);
        }
    }

    #[test]
    fn linearize_shaped_add_broadcasts_scalar_across_vector() {
        let w = Variable::new(1, (3, 1));
        let (offsets, n_total) = offsets_of(&[w]);
        let expr = Expression::add(Expression::from_variable(w), Expression::constant(5.0));
        let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
        assert_eq!(form.shape, (3, 1));
        for (k, entry) in form.entries.iter().enumerate() {
            assert_eq!(entry.constant, 5.0);
            assert_eq!(entry.linear[k], 1.0);
        }
    }

    #[test]
    fn linearize_shaped_add_combines_equal_shapes_entrywise() {
        let w = Variable::new(1, (2, 1));
        let v = Variable::new(2, (2, 1));
        let (offsets, n_total) = offsets_of(&[w, v]);
        let expr = Expression::add(Expression::from_variable(w), Expression::from_variable(v));
        let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
        assert_eq!(form.shape, (2, 1));
        assert_eq!(form.entries[0].linear, vec![1.0, 0.0, 1.0, 0.0]);
        assert_eq!(form.entries[1].linear, vec![0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn linearize_shaped_mismatched_shapes_is_a_shape_mismatch_error() {
        let w = Variable::new(1, (2, 1));
        let v = Variable::new(2, (3, 1));
        let (offsets, n_total) = offsets_of(&[w, v]);
        let expr = Expression::add(Expression::from_variable(w), Expression::from_variable(v));
        let err = linearize_shaped(&expr, &offsets, n_total).unwrap_err();
        assert_eq!(err, "shape mismatch: 2x1 vs 3x1");
    }

    #[test]
    fn linearize_shaped_mul_by_constant_vector_is_affine() {
        let w = Variable::new(1, (3, 1));
        let (offsets, n_total) = offsets_of(&[w]);
        let weights = Expression::from_parameter(2, (3, 1), vec![2.0, 3.0, 4.0]);
        let expr = Expression::mul(weights, Expression::from_variable(w));
        let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
        assert_eq!(form.shape, (3, 1));
        assert_eq!(form.entries[0].linear, vec![2.0, 0.0, 0.0]);
        assert_eq!(form.entries[1].linear, vec![0.0, 3.0, 0.0]);
        assert_eq!(form.entries[2].linear, vec![0.0, 0.0, 4.0]);
    }

    #[test]
    fn linearize_shaped_mul_of_two_variable_vectors_is_an_error() {
        let w = Variable::new(1, (3, 1));
        let v = Variable::new(2, (3, 1));
        let (offsets, n_total) = offsets_of(&[w, v]);
        let expr = Expression::mul(Expression::from_variable(w), Expression::from_variable(v));
        let err = linearize_shaped(&expr, &offsets, n_total).unwrap_err();
        assert_eq!(err, VECTOR_MUL_ERROR);
    }

    #[test]
    fn linearize_shaped_div_by_constant_scales_every_entry() {
        let w = Variable::new(1, (2, 1));
        let (offsets, n_total) = offsets_of(&[w]);
        let expr = Expression::div(Expression::from_variable(w), Expression::constant(2.0));
        let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
        assert_eq!(form.entries[0].linear, vec![0.5, 0.0]);
        assert_eq!(form.entries[1].linear, vec![0.0, 0.5]);
    }

    #[test]
    fn linearize_shaped_div_by_variable_vector_is_an_error() {
        let w = Variable::new(1, (2, 1));
        let v = Variable::new(2, (2, 1));
        let (offsets, n_total) = offsets_of(&[w, v]);
        let expr = Expression::div(Expression::from_variable(w), Expression::from_variable(v));
        let err = linearize_shaped(&expr, &offsets, n_total).unwrap_err();
        assert_eq!(err, VECTOR_DIV_ERROR);
    }

    #[test]
    fn linearize_shaped_neg_and_scale_preserve_shape() {
        let w = Variable::new(1, (2, 1));
        let (offsets, n_total) = offsets_of(&[w]);
        let neg = linearize_shaped(
            &Expression::neg(Expression::from_variable(w)),
            &offsets,
            n_total,
        )
        .unwrap();
        assert_eq!(neg.shape, (2, 1));
        assert_eq!(neg.entries[0].linear, vec![-1.0, 0.0]);

        let scaled = linearize_shaped(
            &Expression::scale(4.0, Expression::from_variable(w)),
            &offsets,
            n_total,
        )
        .unwrap();
        assert_eq!(scaled.entries[1].linear, vec![0.0, 4.0]);
    }

    #[test]
    fn is_all_scalar_true_for_all_1x1_leaves_even_with_quadratic_term() {
        let x = scalar_var(1);
        let expr = Expression::mul(Expression::from_variable(x), Expression::from_variable(x));
        assert!(is_all_scalar(&expr));
    }

    #[test]
    fn is_all_scalar_false_once_any_leaf_is_non_scalar() {
        let w = Variable::new(1, (3, 1));
        assert!(!is_all_scalar(&Expression::from_variable(w)));
    }

    #[test]
    fn is_all_scalar_false_for_any_expression_containing_sum() {
        let x = scalar_var(1);
        let expr = Expression::sum(Expression::from_variable(x));
        assert!(!is_all_scalar(&expr));
    }

    #[test]
    fn scalar_quadratic_nested_in_a_vector_tree_is_rejected() {
        // Add(x * x, w) with w a (3, 1) variable: once routed through
        // linearize_shaped, the nested x * x product is restricted to
        // affine terms (SPEC-0014 Non-Objective), even though x * x alone
        // would be fine via quadratize.
        let x = scalar_var(1);
        let w = Variable::new(2, (3, 1));
        let (offsets, n_total) = offsets_of(&[x, w]);
        let x_expr = Expression::from_variable(x);
        let expr = Expression::add(
            Expression::mul(x_expr.clone(), x_expr),
            Expression::from_variable(w),
        );
        let err = reduce(&expr, &offsets, n_total).unwrap_err();
        assert_eq!(err, VECTOR_MUL_ERROR);
    }

    #[test]
    fn sum_of_a_vector_variable_sums_its_one_hot_entries() {
        let w = Variable::new(1, (3, 1));
        let (offsets, n_total) = offsets_of(&[w]);
        let form = reduce(
            &Expression::sum(Expression::from_variable(w)),
            &offsets,
            n_total,
        )
        .unwrap();
        assert_eq!(form.shape, (1, 1));
        assert_eq!(form.entries[0].linear, vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn sum_of_weighted_vector_gives_a_weighted_total() {
        let w = Variable::new(1, (3, 1));
        let (offsets, n_total) = offsets_of(&[w]);
        let weights = Expression::from_parameter(2, (3, 1), vec![2.0, 3.0, 4.0]);
        let expr = Expression::sum(Expression::mul(weights, Expression::from_variable(w)));
        let form = reduce(&expr, &offsets, n_total).unwrap();
        assert_eq!(form.shape, (1, 1));
        assert_eq!(form.entries[0].linear, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn sum_of_a_scalar_is_an_identity() {
        let form = reduce(
            &Expression::sum(Expression::constant(7.0)),
            &HashMap::new(),
            0,
        )
        .unwrap();
        assert_eq!(form.shape, (1, 1));
        assert_eq!(form.entries[0].constant, 7.0);
    }

    #[test]
    fn sum_of_scalar_quadratic_is_rejected() {
        // Sum(x * x): an all-(1,1)-leaf tree containing Sum is still routed
        // through linearize_shaped (is_all_scalar's Sum arm), so the nested
        // quadratic product is rejected.
        let x = scalar_var(1);
        let (offsets, n_total) = offsets_of(&[x]);
        let x_expr = Expression::from_variable(x);
        let expr = Expression::sum(Expression::mul(x_expr.clone(), x_expr));
        let err = reduce(&expr, &offsets, n_total).unwrap_err();
        assert_eq!(err, VECTOR_MUL_ERROR);
    }

    #[test]
    fn feasibility_only_problem_solves_a_boxed_matrix_variable() {
        // minimize 0 subject to M >= [[1,2],[3,4]] and M <= [[1,2],[3,4]]
        // (row-major), confirming genuine (rows, cols) matrix shapes (not
        // just column vectors) solve correctly end-to-end.
        let m = Variable::new(1, (2, 2));
        let m_expr = Expression::from_variable(m);
        let bound = Expression::from_parameter(2, (2, 2), vec![1.0, 2.0, 3.0, 4.0]);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: vec![
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: m_expr.clone(),
                    rhs: bound.clone(),
                },
                Constraint {
                    relation: Relation::LessEqual,
                    lhs: m_expr,
                    rhs: bound,
                },
            ],
            variables: vec![m],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert_eq!(solution.variable_values.len(), 1);
        for (got, want) in solution.variable_values[0].iter().zip([1.0, 2.0, 3.0, 4.0]) {
            assert!((got - want).abs() < 1e-6);
        }
    }

    #[test]
    fn feasibility_only_problem_solves_a_boxed_vector_variable() {
        // minimize 0 subject to w >= [1, 2, 3] and w <= [1, 2, 3]
        let w = Variable::new(1, (3, 1));
        let w_expr = Expression::from_variable(w);
        let bound = Expression::from_parameter(2, (3, 1), vec![1.0, 2.0, 3.0]);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: vec![
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: w_expr.clone(),
                    rhs: bound.clone(),
                },
                Constraint {
                    relation: Relation::LessEqual,
                    lhs: w_expr,
                    rhs: bound,
                },
            ],
            variables: vec![w],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert_eq!(solution.objective_value, Some(0.0));
        assert_eq!(solution.variable_values.len(), 1);
        for (got, want) in solution.variable_values[0].iter().zip([1.0, 2.0, 3.0]) {
            assert!((got - want).abs() < 1e-6);
        }
    }

    #[test]
    fn mixed_scalar_and_vector_problem_solves_both_independently() {
        // minimize x subject to x >= 3, with an unrelated (3, 1) variable w
        // tightly boxed to [2, 2, 2].
        let x = scalar_var(1);
        let w = Variable::new(2, (3, 1));
        let w_expr = Expression::from_variable(w);
        let bound = Expression::from_parameter(3, (3, 1), vec![2.0, 2.0, 2.0]);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::from_variable(x),
            constraints: vec![
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: Expression::from_variable(x),
                    rhs: Expression::constant(3.0),
                },
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: w_expr.clone(),
                    rhs: bound.clone(),
                },
                Constraint {
                    relation: Relation::LessEqual,
                    lhs: w_expr,
                    rhs: bound,
                },
            ],
            variables: vec![x, w],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.variable_values[0][0] - 3.0).abs() < 1e-6);
        for got in &solution.variable_values[1] {
            assert!((got - 2.0).abs() < 1e-6);
        }
    }

    #[test]
    fn elementwise_mul_equal_constraint_recovers_expected_values() {
        // 2 .* w == [4, 6, 8] => w == [2, 3, 4]
        let w = Variable::new(1, (3, 1));
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: vec![Constraint {
                relation: Relation::Equal,
                lhs: Expression::scale(2.0, Expression::from_variable(w)),
                rhs: Expression::from_parameter(2, (3, 1), vec![4.0, 6.0, 8.0]),
            }],
            variables: vec![w],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        for (got, want) in solution.variable_values[0].iter().zip([2.0, 3.0, 4.0]) {
            assert!((got - want).abs() < 1e-6);
        }
    }

    #[test]
    fn scalar_quadratic_constraint_broadcasts_against_vector_parameter() {
        // minimize -x subject to x^2 <= w, with w a (3, 1) parameter [4,4,4]
        // (so x^2 <= 4 for every broadcast row; optimum at x = 2).
        let x = scalar_var(1);
        let x_expr = Expression::from_variable(x);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::neg(x_expr.clone()),
            constraints: vec![Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::mul(x_expr.clone(), x_expr),
                rhs: Expression::from_parameter(2, (3, 1), vec![4.0, 4.0, 4.0]),
            }],
            variables: vec![x],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.variable_values[0][0] - 2.0).abs() < 1e-4);
    }

    #[test]
    fn budget_allocation_lp_with_sum_solves_the_least_cost_allocation() {
        // minimize sum(cost .* x) subject to x >= [0,0,0] and sum(x) >= 10,
        // with cost = [3, 1, 2] -> all budget should go to the cheapest
        // entry (index 1, cost 1), giving x = [0, 10, 0] and objective 10.
        let x = Variable::new(1, (3, 1));
        let x_expr = Expression::from_variable(x);
        let cost = Expression::from_parameter(2, (3, 1), vec![3.0, 1.0, 2.0]);
        let zero = Expression::from_parameter(3, (3, 1), vec![0.0, 0.0, 0.0]);
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::sum(Expression::mul(cost, x_expr.clone())),
            constraints: vec![
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: x_expr.clone(),
                    rhs: zero,
                },
                Constraint {
                    relation: Relation::GreaterEqual,
                    lhs: Expression::sum(x_expr),
                    rhs: Expression::constant(10.0),
                },
            ],
            variables: vec![x],
        };
        let solution = solve(&problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.objective_value.unwrap() - 10.0).abs() < 1e-4);
        assert!((solution.variable_values[0][1] - 10.0).abs() < 1e-4);
    }

    #[test]
    fn bare_vector_objective_is_a_shape_error_but_sum_wrapped_succeeds() {
        let w = Variable::new(1, (3, 1));
        let w_expr = Expression::from_variable(w);

        let bare_problem = Problem {
            sense: Sense::Minimize,
            objective: w_expr.clone(),
            constraints: Vec::new(),
            variables: vec![w],
        };
        assert_eq!(
            solve(&bare_problem).status,
            SolveStatus::Error(
                "objective must evaluate to a single value (shape 1x1); got shape 3x1".to_string()
            )
        );

        let summed_problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::sum(w_expr.clone()),
            constraints: vec![Constraint {
                relation: Relation::GreaterEqual,
                lhs: w_expr,
                rhs: Expression::from_parameter(2, (3, 1), vec![1.0, 1.0, 1.0]),
            }],
            variables: vec![w],
        };
        let solution = solve(&summed_problem);
        assert_eq!(solution.status, SolveStatus::Optimal);
        assert!((solution.objective_value.unwrap() - 3.0).abs() < 1e-4);
    }

    #[test]
    fn single_oversized_vector_variable_is_a_size_error() {
        let w = Variable::new(1, (MAX_VARIABLES + 1, 1));
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: Vec::new(),
            variables: vec![w],
        };
        let solution = solve(&problem);
        assert_eq!(
            solution.status,
            SolveStatus::Error(
                "problem exceeds solver size limit (200 scalar variables / 200 scalar constraint rows)".to_string()
            )
        );
    }
}
