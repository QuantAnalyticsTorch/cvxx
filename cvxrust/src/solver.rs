//! Translates a [`Problem`] into a conic program and delegates to
//! [`clarabel`] to solve it. See the crate-level docs and `SPEC-0010`/
//! `SPEC-0011`/`SPEC-0014` for the supported problem class.

use std::collections::HashMap;

use clarabel::algebra::CscMatrix;
use clarabel::solver::{
    DefaultSettings, DefaultSolver, IPSolver, NonnegativeConeT, SecondOrderConeT,
    SolverStatus as ClarabelStatus, SupportedConeT, ZeroConeT,
};
use nalgebra::{DMatrix, SymmetricEigen};

use crate::model::{Problem, Relation, Sense, Solution, SolveStatus, Variable};
use crate::reduce::{broadcast_shape, entry_at, reduce_expression, QuadraticForm};

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

/// Attempts to solve `problem`, routing to the `clarabel`-backed
/// continuous path when it has no domain restrictions, or to the
/// `microlp`-backed mixed-integer path (SPEC-0019,
/// `crate::solver_milp::solve_mixed_integer`) otherwise. See the
/// crate-level docs for the rationale.
pub fn solve(problem: &Problem) -> Solution {
    if problem.domains.is_empty() {
        solve_continuous(problem)
    } else {
        crate::solver_milp::solve_mixed_integer(problem)
    }
}

/// Size-limit error shared by both the `clarabel` and `microlp` solve
/// paths (SPEC-0010/SPEC-0019).
pub(crate) const SIZE_LIMIT_ERROR: &str =
    "problem exceeds solver size limit (200 scalar variables / 200 scalar constraint rows)";

/// Builds the `cvxrust::Variable::id` -> global scalar offset map, and the
/// total scalar-variable count `n_total`, shared by both the `clarabel`
/// and `microlp` solve paths (SPEC-0010/SPEC-0019).
pub(crate) fn variable_offsets(variables: &[Variable]) -> (HashMap<u64, usize>, usize) {
    let mut offsets: HashMap<u64, usize> = HashMap::with_capacity(variables.len());
    let mut n_total = 0usize;
    for v in variables {
        offsets.insert(v.id, n_total);
        n_total += v.shape.0 * v.shape.1;
    }
    (offsets, n_total)
}

/// Attempts to solve `problem` by translating it into a conic program and
/// delegating to `clarabel`. See the crate-level docs and `SPEC-0010`/
/// `SPEC-0014` for the supported problem class. Only called when
/// `problem.domains` is empty; see `solve` above.
fn solve_continuous(problem: &Problem) -> Solution {
    let (offsets, n_total) = variable_offsets(&problem.variables);

    if n_total > MAX_VARIABLES {
        return error_solution(SIZE_LIMIT_ERROR);
    }

    let obj_form = match reduce_expression(&problem.objective, &offsets, n_total) {
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
        let lhs = match reduce_expression(&constraint.lhs, &offsets, n_total) {
            Ok(form) => form,
            Err(message) => return error_solution(message),
        };
        let rhs = match reduce_expression(&constraint.rhs, &offsets, n_total) {
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
#[path = "solver_tests.rs"]
mod tests;
