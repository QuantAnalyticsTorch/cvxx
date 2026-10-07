//! Translates a [`Problem`] containing at least one integer/binary domain
//! restriction into a [`microlp::Problem`] and delegates to its
//! branch-and-bound solver (SPEC-0019). See the crate-level docs and
//! `docs/architecture.md`'s 2026-10-07 decision for why `microlp` is a
//! second, dedicated solver path rather than an extension of the
//! `clarabel`/conic translation layer in `crate::solver`.
//!
//! Only reachable via `crate::solver::solve` when `Problem::domains` is
//! non-empty; a purely continuous problem never enters this module.

use std::collections::HashMap;
use std::time::Duration;

use crate::model::{Domain, Problem, Relation, Sense, Solution, SolveStatus};
use crate::reduce::{broadcast_shape, entry_at, reduce_expression};
use crate::solver::{variable_offsets, MAX_CONSTRAINTS, MAX_VARIABLES, SIZE_LIMIT_ERROR};

/// Fixed wall-clock budget for a `microlp` mixed-integer solve, analogous
/// in spirit to `clarabel`'s `MAX_ITERATIONS` on the continuous path. Not
/// user-configurable (SPEC-0019 Non-Objective).
pub(crate) const MILP_TIME_LIMIT_SECS: f64 = 10.0;

/// Fixed branch-and-bound node budget for a `microlp` mixed-integer solve.
/// Not user-configurable (SPEC-0019 Non-Objective).
pub(crate) const MILP_NODE_LIMIT: u64 = 100_000;

/// Plain-language explanation returned when a domain-restricted problem's
/// objective or any constraint carries a quadratic term (SPEC-0019 Data
/// Model step 2; MIQP is an explicit non-objective).
const QUADRATIC_ERROR: &str = "mixed-integer solving (CVX.INTEGER/CVX.BINARY) does not yet \
support quadratic objectives or constraints; remove the quadratic term(s) or remove the \
integer/binary declaration(s)";

/// Returned when the search stops at the time/node limit without ever
/// finding a feasible incumbent: neither `Infeasible` nor `StoppedAtLimit`
/// would be an honest description of that outcome (SPEC-0019 Data Model
/// step 6 / Error Handling).
const NO_INCUMBENT_ERROR: &str = "mixed-integer solve did not find a feasible solution within \
the time/node limit; feasibility is undetermined (not proven infeasible)";

fn error_solution(message: impl Into<String>) -> Solution {
    Solution {
        status: SolveStatus::Error(message.into()),
        objective_value: None,
        variable_values: Vec::new(),
    }
}

/// Attempts to solve a `problem` with at least one domain restriction.
/// Reduces the objective and every constraint row exactly as
/// `crate::solver::solve_continuous` does (reusing all of its existing
/// validation), rejects any quadratic term, builds a flattened
/// per-scalar-entry domain map (`Domain::Binary` wins over
/// `Domain::Integer` on overlap), and delegates to `microlp`.
pub(crate) fn solve_mixed_integer(problem: &Problem) -> Solution {
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
    if !obj.is_affine() {
        return error_solution(QUADRATIC_ERROR);
    }

    let mut rows: Vec<(Relation, Vec<f64>, f64)> = Vec::with_capacity(problem.constraints.len());
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
            let diff = entry_at(&lhs, k).clone().sub(entry_at(&rhs, k));
            if !diff.is_affine() {
                return error_solution(QUADRATIC_ERROR);
            }
            rows.push((constraint.relation, diff.linear.clone(), -diff.constant));
        }
    }

    let domain_map = build_domain_map(&problem.domains, &offsets);

    let direction = match problem.sense {
        Sense::Minimize => microlp::OptimizationDirection::Minimize,
        Sense::Maximize => microlp::OptimizationDirection::Maximize,
    };
    let mut mp = microlp::Problem::new(direction);
    let mut vars = Vec::with_capacity(n_total);
    for i in 0..n_total {
        let coeff = obj.linear.get(i).copied().unwrap_or(0.0);
        // Continuous entries use microlp's native `f64::NEG_INFINITY`/
        // `INFINITY` bounds, exactly like a `clarabel`-path free variable.
        // The real `microlp` 0.6.0 API takes `(i32, i32)` native bounds
        // for an integer variable (not `(i64, i64)` as this
        // specification originally assumed); `(i32::MIN, i32::MAX)` is
        // the closest available equivalent to "unbounded at the solver
        // level". Because that is still a finite box bound, an
        // objective driven purely by an unbounded *integer* entry will
        // report a large finite `Optimal` rather than `Unbounded` — an
        // unavoidable consequence of `microlp`'s integer bound type, not
        // a bug in this translation; any numeric bound the user actually
        // wants is still expressed as an ordinary affine constraint row
        // either way.
        let var = match domain_map.get(&i) {
            Some(Domain::Binary) => mp.add_binary_var(coeff),
            Some(Domain::Integer) => mp.add_integer_var(coeff, (i32::MIN, i32::MAX)),
            None => mp.add_var(coeff, (f64::NEG_INFINITY, f64::INFINITY)),
        };
        vars.push(var);
    }

    for (relation, coeffs, rhs) in &rows {
        let terms: Vec<(microlp::Variable, f64)> = coeffs
            .iter()
            .enumerate()
            .filter(|(_, c)| **c != 0.0)
            .map(|(i, &c)| (vars[i], c))
            .collect();
        let op = match relation {
            Relation::LessEqual => microlp::ComparisonOp::Le,
            Relation::GreaterEqual => microlp::ComparisonOp::Ge,
            Relation::Equal => microlp::ComparisonOp::Eq,
        };
        mp.add_constraint(terms, op, *rhs);
    }

    let mut options = microlp::SolveOptions::default();
    options.time_limit = Some(Duration::from_secs_f64(MILP_TIME_LIMIT_SECS));
    options.node_limit = Some(MILP_NODE_LIMIT);

    translate_outcome(problem, &vars, mp.solve_with(options))
}

/// Expands every `DomainConstraint`'s `(row_start, col_start, rows, cols)`
/// sub-block into the global scalar indices it covers (the same numbering
/// `offsets`/`quadratize` already assigns), tagged with its `domain`.
/// Where two or more restrictions cover the same scalar index, the more
/// restrictive one wins: `Domain::Binary` over `Domain::Integer`
/// (`Domain`'s `Ord`), so tightening an already-integer sub-block to
/// binary with a second, narrower declaration is not a conflict.
fn build_domain_map(
    domains: &[crate::model::DomainConstraint],
    offsets: &HashMap<u64, usize>,
) -> HashMap<usize, Domain> {
    let mut domain_map: HashMap<usize, Domain> = HashMap::new();
    for dc in domains {
        let Some(&base) = offsets.get(&dc.variable.id) else {
            // The variable is not part of this problem's variable list;
            // cvxx always includes a domain's variable in
            // `ProblemEntry::variables`, so this should not normally
            // occur. Skipped defensively rather than panicking.
            continue;
        };
        let var_cols = dc.variable.shape.1;
        for r in 0..dc.rows {
            for c in 0..dc.cols {
                let local = (dc.row_start + r) * var_cols + (dc.col_start + c);
                let global = base + local;
                domain_map
                    .entry(global)
                    .and_modify(|existing| {
                        if dc.domain > *existing {
                            *existing = dc.domain;
                        }
                    })
                    .or_insert(dc.domain);
            }
        }
    }
    domain_map
}

/// Translates a `microlp` solve outcome into a `cvxrust::Solution`
/// (SPEC-0019 Data Model step 6/7).
fn translate_outcome(
    problem: &Problem,
    vars: &[microlp::Variable],
    outcome: Result<microlp::SolveOutcome, microlp::Error>,
) -> Solution {
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(microlp::Error::Infeasible) => {
            return Solution {
                status: SolveStatus::Infeasible,
                objective_value: None,
                variable_values: Vec::new(),
            }
        }
        Err(microlp::Error::Unbounded) => {
            return Solution {
                status: SolveStatus::Unbounded,
                objective_value: None,
                variable_values: Vec::new(),
            }
        }
        Err(other) => return error_solution(format!("mixed-integer solve failed: {other}")),
    };

    match outcome {
        microlp::SolveOutcome::Solution(solution) => {
            let status = match solution.status() {
                microlp::SolutionStatus::Optimal => SolveStatus::Optimal,
                microlp::SolutionStatus::Feasible => SolveStatus::StoppedAtLimit,
            };
            let mut variable_values = Vec::with_capacity(problem.variables.len());
            let mut cursor = 0usize;
            for v in &problem.variables {
                let count = v.shape.0 * v.shape.1;
                let values: Vec<f64> = (cursor..cursor + count)
                    .map(|i| solution.var_value(vars[i]))
                    .collect();
                variable_values.push(values);
                cursor += count;
            }
            Solution {
                status,
                objective_value: Some(solution.objective()),
                variable_values,
            }
        }
        microlp::SolveOutcome::Interrupted(_) => error_solution(NO_INCUMBENT_ERROR),
    }
}

#[cfg(test)]
#[path = "solver_milp_tests.rs"]
mod tests;
