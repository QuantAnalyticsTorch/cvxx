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
//! A problem containing at least one integer/binary domain restriction
//! (SPEC-0019) instead routes through a second, dedicated translation
//! layer built on [`microlp`], a pure-Rust LP/MILP solver with built-in
//! branch-and-bound. Purely continuous problems are completely unaffected
//! by this second path; see `docs/architecture.md`'s 2026-10-07 decision
//! for the rationale.
//!
//! Organized into four modules, each small enough to read and reason about
//! on its own:
//!
//! - `model` (private; re-exported) — the data model: `Variable`,
//!   `Expression`, `Constraint`, `Domain`, `DomainConstraint`, `Problem`,
//!   `Solution`. Pure data definitions, no reduction or solving logic.
//! - `reduce` (private, crate-internal) — reduces (possibly vector/matrix-
//!   shaped) `Expression` trees into numeric linear/quadratic forms
//!   (SPEC-0010/SPEC-0011/SPEC-0014). `solver`/`solver_milp` are its only
//!   consumers.
//! - `solver` (private; re-exported) — translates a reduced `Problem` with
//!   no domain restrictions into a conic program and delegates to
//!   `clarabel`. Exposes the crate's one real entry point, [`solve`], which
//!   routes to `solver_milp` instead when `Problem::domains` is non-empty.
//! - `solver_milp` (private, crate-internal) — translates a reduced
//!   `Problem` with at least one domain restriction into a `microlp`
//!   problem and delegates to its branch-and-bound solver (SPEC-0019).

mod model;
mod reduce;
mod solver;
mod solver_milp;

pub use model::{
    Constraint, Domain, DomainConstraint, Expression, Problem, Relation, Sense, Solution,
    SolveStatus, Variable,
};
pub use solver::{solve, MAX_CONSTRAINTS, MAX_ITERATIONS, MAX_VARIABLES};
