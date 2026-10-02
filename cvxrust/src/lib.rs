//! `cvxrust` — convex optimization modeling layer for `cvxx`.
//!
//! Provides the variable, expression, and problem types used by the Excel
//! add-in, plus a linear-programming solver backend built on top of the
//! [`clarabel`] conic interior-point solver. `clarabel` is the standing
//! solver framework for this crate: it natively supports free (unrestricted
//! sign) variables and quadratic objectives, so this crate's translation
//! layer is intended as the foundation for future quadratic-objective
//! support rather than a throwaway linear-only integration.
//!
//! Organized into three modules, each small enough to read and reason about
//! on its own:
//!
//! - `model` (private; re-exported) — the data model: `Variable`,
//!   `Expression`, `Constraint`, `Problem`, `Solution`. Pure data
//!   definitions, no reduction or solving logic.
//! - `reduce` (private, crate-internal) — reduces (possibly vector/matrix-
//!   shaped) `Expression` trees into numeric linear/quadratic forms
//!   (SPEC-0010/SPEC-0011/SPEC-0014). `solver` is its only consumer.
//! - `solver` (private; re-exported) — translates a reduced `Problem` into
//!   a conic program and delegates to `clarabel`. Exposes the crate's one
//!   real entry point, [`solve`].

mod model;
mod reduce;
mod solver;

pub use model::{
    Constraint, Expression, Problem, Relation, Sense, Solution, SolveStatus, Variable,
};
pub use solver::{solve, MAX_CONSTRAINTS, MAX_ITERATIONS, MAX_VARIABLES};
