---
id: SPEC-0010
title: Linear programming solver backend for CVX.SOLVE
issue: ISSUE-0010
status: implemented
created: 2026-09-30
---

## Objective

Replace `cvxrust::solve`'s permanent `SolveStatus::Error("not implemented")`
stub with a real solver that correctly solves linear programs built from
scalar (`1x1`) variables and parameters: affine objectives subject to affine
`<=`, `>=`, and `=` constraints, by translating the problem into a conic
program and delegating to the [`clarabel`](https://crates.io/crates/clarabel)
crate. `clarabel` is adopted as the solver engine for `cvxrust` going
forward: it natively supports free (unrestricted-sign) variables, linear
programs, and quadratic programs, so this same translation layer is the
intended foundation for quadratic-objective support in a future
specification rather than a second, unrelated solver integration. This
specification also fixes a variable identity gap in `cvxrust::Variable` that
would otherwise make it impossible to tell two same-shape variables apart
inside an expression tree, which is required for any correct solver.

## Non-Objective

- Quadratic (or any other non-affine) objectives or constraints. A problem
  containing a product or quotient of two variable-dependent sub-expressions
  is detected and reported as an unsupported problem type (see Error
  Handling); it is not solved approximately or rejected silently. Quadratic
  programming support is left to a future specification (tentatively
  `SPEC-0011`), which is expected to populate `clarabel`'s quadratic `P`
  matrix rather than introduce a different solver.
- Variables or parameters with a shape other than `(1, 1)` (vectors,
  matrices). These are also detected and reported as unsupported (see Error
  Handling), left to a future specification once elementwise/matrix
  semantics for `Expression::Mul`/`Div` are defined. `CVX.VARIABLE` and
  `CVX.PARAMETER` themselves are unaffected and may still create non-scalar
  entries; only solving a problem that references one is out of scope here.
- Dual values, sensitivity analysis, warm starts, and solver tuning options
  beyond the fixed iteration/time bounds defined below (out of scope per
  `ISSUE-0010`; dual values/sensitivity remain covered by a future issue as
  noted in `SPEC-0006`, though `clarabel` computes duals internally and a
  future spec can expose them cheaply).
- Result inspection functions (`ISSUE-0007`) and convenience solvers
  (`ISSUE-0008`).
- Any solver engine other than `clarabel`. `clarabel` is the standing
  framework for convex solving in this repository; future solver work
  (quadratic, conic, etc.) extends this integration instead of adding
  alternatives.
- Changes to `CVX.PROBLEM`, `CVX.SOLVE`, or any other Excel-facing function
  signature. This specification only changes behavior inside `cvxrust` plus
  the one `cvxrust::Variable` construction call site in `cvxx`'s registry.

## Interface

### `cvxrust::Variable` identity (breaking change from SPEC-0003)

`Variable` gains an `id` field so that two variables of the same shape can be
told apart inside an `Expression` tree:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variable {
    pub id: u64,
    /// `(rows, cols)`.
    pub shape: (usize, usize),
}

impl Variable {
    pub fn new(id: u64, shape: (usize, usize)) -> Self {
        Variable { id, shape }
    }
}
```

- `cvxx`'s `Registry::insert_variable` (`src/core/registry.rs`, SPEC-0003) is
  updated to pass an id derived from the entry's own freshly generated
  `Uuid`: `Variable::new(uuid.as_u64_pair().0, shape)`. This keeps variable
  identity 1:1 with the registry's `VariableEntry::uuid` without adding a
  second counter or changing `VariableEntry`'s public fields.
- All other existing call sites that construct a bare `Variable` only for
  structural-equality assertions in tests (`src/analytics/resolve.rs`,
  `src/excel/expression.rs`) are updated to pass a matching id (reading it
  back off the inserted `VariableEntry` rather than hard-coding one), since
  `Variable` equality now includes `id`.

### `cvxrust` solver additions

```rust
/// Upper bound on the number of scalar decision variables and constraint
/// rows a problem may have. Larger problems return `SolveStatus::Error`
/// rather than building an unbounded-size conic program.
pub const MAX_VARIABLES: usize = 200;
pub const MAX_CONSTRAINTS: usize = 200;

/// Upper bound on `clarabel` solver iterations before giving up rather than
/// running indefinitely, set via `DefaultSettings::max_iter`.
pub const MAX_ITERATIONS: u32 = 200;

pub fn solve(problem: &Problem) -> Solution;
```

`solve` is reimplemented as described in Data Model below. Its signature is
unchanged from SPEC-0006. No other public `cvxrust` items are added or
changed.

### New `cvxrust` dependency

```toml
[dependencies]
clarabel = "0.9"
```

Default features only — the LP problems built here only use `clarabel`'s
zero cone and nonnegative cone, which do not require the optional
BLAS/LAPACK-backed feature flags `clarabel` offers for SDP support.

## Data Model

### Step 1 — size and shape validation

- If `problem.variables.len() > MAX_VARIABLES` or
  `problem.constraints.len() > MAX_CONSTRAINTS`, return
  `SolveStatus::Error("problem exceeds solver size limit (200 variables / 200 constraints)")`
  immediately.
- If any `Variable` in `problem.variables`, or any `Variable`/`Parameter`
  reachable while walking `problem.objective` or any constraint's `lhs`/`rhs`,
  has a shape other than `(1, 1)`, return
  `SolveStatus::Error("solver only supports scalar (1x1) variables and parameters")`.

### Step 2 — linearization

Build a variable index map `id -> usize` from `problem.variables` (position
= index). Define:

```rust
struct AffineForm {
    constant: f64,
    /// Length == problem.variables.len(); coeffs[i] is the coefficient of
    /// problem.variables[i].
    coeffs: Vec<f64>,
}
```

Recursively reduce an `Expression` to an `AffineForm` (returning an error
that maps to `SolveStatus::Error` on failure):

- `Constant(c)` → `{ constant: c, coeffs: all zero }`.
- `Parameter { shape, data }` → shape must be `(1,1)` (checked in Step 1);
  `{ constant: data[0], coeffs: all zero }`.
- `Variable(v)` → shape must be `(1,1)` (checked in Step 1); look up `v.id`
  in the index map — if absent, return
  `SolveStatus::Error("objective or constraint references a variable not included in the problem's variable list")`;
  otherwise `{ constant: 0.0, coeffs: one-hot 1.0 at that index }`.
- `Add(l, r)` / `Sub(l, r)` → reduce both sides, add/subtract constants and
  coefficient vectors elementwise.
- `Neg(e)` → reduce `e`, negate constant and all coefficients.
- `Scale { scalar, expr }` → reduce `expr`, multiply constant and all
  coefficients by `scalar`.
- `Mul(l, r)` → reduce both sides. If exactly one side's `coeffs` are all
  zero (a pure constant value), multiply the other side's `AffineForm` by
  that constant value. If **both** sides have at least one nonzero
  coefficient, return
  `SolveStatus::Error("solver only supports linear (affine) objectives and constraints; a product of two variable-dependent terms was found")`.
- `Div(l, r)` → reduce both sides. If `r.coeffs` are all zero and
  `r.constant != 0.0` (within `1e-12`), divide `l`'s constant and
  coefficients by `r.constant`. If `r.constant == 0.0`, return
  `SolveStatus::Error("division by zero in objective or constraint")`. If
  `r.coeffs` has a nonzero entry, return
  `SolveStatus::Error("solver only supports linear (affine) objectives and constraints; division by a variable-dependent term was found")`.

Apply this to `problem.objective` (call the result `obj`), and to each
constraint's `lhs`/`rhs`, producing `diff = lhs_form - lhs_form.constant... `
— concretely: `row.coeffs = lhs.coeffs - rhs.coeffs`,
`row.rhs = rhs.constant - lhs.constant`, giving the standard row
`sum(row.coeffs[i] * x[i]) <relation> row.rhs`.

### Step 3 — conic-program translation and solve (`clarabel`)

`clarabel` solves problems of the form
`minimize (1/2) x'Px + q'x subject to Ax + s = b, s in K` for a product of
cones `K`. Variables are native to `clarabel` as unrestricted reals, so no
free-variable splitting is needed (unlike a hand-rolled simplex tableau).

1. **Objective vector**: `P` is the all-zero `n x n` sparse matrix (no
   quadratic term is supported here). `q` is `obj.coeffs` when
   `problem.sense == Sense::Minimize`, or the elementwise negation of
   `obj.coeffs` when `Sense::Maximize` (`clarabel` only minimizes; negating
   `q` turns a maximization into an equivalent minimization). `obj.constant`
   is not passed to `clarabel` (it has no constant term); it is added back
   into the reported `objective_value` in Recovery below.
2. **Constraint rows, grouped by cone**: for each linearized constraint row
   (`row.coeffs`, relation, `row.rhs`, from Step 2, *no* RHS-sign
   normalization needed — `clarabel` accepts any sign of `b`):
   - `Equal` rows are grouped first, contributing one row each to `A`/`b`
     under a single `ZeroConeT(num_equal)` cone.
   - `LessEqual` rows follow, contributing `A` row `= row.coeffs`,
     `b` entry `= row.rhs`, under a shared `NonnegativeConeT(num_ineq)` cone.
   - `GreaterEqual` rows are rewritten as `LessEqual` by negating the row
     (`A` row `= -row.coeffs`, `b` entry `= -row.rhs`) and included in the
     same `NonnegativeConeT` block as the `LessEqual` rows.
   - The final `cones` vector is `vec![ZeroConeT(num_equal), NonnegativeConeT(num_ineq)]`,
     omitting either entry when its count is zero (`clarabel` requires
     non-empty cones).
   - A problem with zero constraints passes an empty `A`/`b`/`cones`.
3. **Settings**: `DefaultSettings { max_iter: MAX_ITERATIONS, verbose: false, ..Default::default() }`.
4. **Solve**: construct `DefaultSolver::new(&P, &q, &A, &b, &cones, settings)`
   (all matrices as `clarabel::algebra::CscMatrix`) and call `solver.solve()`,
   then read `solver.solution`.
5. **Status mapping** (`solver.solution.status`, a `clarabel::solver::SolverStatus`):
   - `Solved` or `AlmostSolved` → proceed to Recovery, `SolveStatus::Optimal`.
   - `PrimalInfeasible` or `AlmostPrimalInfeasible` → `SolveStatus::Infeasible`.
   - `DualInfeasible` or `AlmostDualInfeasible` → `SolveStatus::Unbounded`
     (dual infeasibility of this minimization primal corresponds to primal
     unboundedness).
   - Any other status (`MaxIterations`, `MaxTime`, `NumericalError`, etc.) →
     `SolveStatus::Error("solver did not converge: <status debug string>")`.

### Step 4 — recovery

For each original variable `x_j`, read `solver.solution.x[j]` directly (no
splitting to undo) and set `variable_values[j] = vec![x[j]]` (one element,
since only `(1,1)` variables are supported), positionally aligned with
`problem.variables`. Compute
`objective_value = obj.constant + dot(obj.coeffs, x)` directly from the
recovered `x` (rather than trusting `solver.solution.obj_val`'s sign
convention), which is correct for both `Minimize` and `Maximize` since it
re-evaluates the original (unnegated) objective. Return
`Solution { status: SolveStatus::Optimal, objective_value: Some(value), variable_values }`.

## Error Handling

- All failure paths identified above (size limit, non-scalar shape,
  nonlinear term, division by zero or by a variable-dependent term, dangling
  variable reference, unrecognized `clarabel` solver status) return
  `Solution { status: SolveStatus::Error(message), objective_value: None, variable_values: vec![] }`
  with a distinct, descriptive `message` per case (verbatim strings given
  above, except the `clarabel` status passthrough message). No panics: all
  array/index accesses during linearization and conic-program construction
  are bounds-checked and return an `Error` status instead of panicking on
  malformed-but-well-typed `Problem` values (e.g., an `id` not present in
  `problem.variables`).
- `CVX.SOLVE` (`src/excel/problem.rs`, SPEC-0006) requires no changes: it
  already maps any `SolveStatus::Error(message)` to `#VALUE!` (logging
  `message` via `tracing::error!`) and stores a `ResultEntry` for `Optimal`,
  `Infeasible`, and `Unbounded`, so the new, more specific error messages and
  the new `Infeasible`/`Unbounded`/`Optimal` outcomes surface automatically
  through the existing plumbing.
- `docs/problems.md` is updated to remove the "`cvxrust` does not yet
  implement a solver" caveat and instead document the supported problem
  class (scalar affine objectives/constraints) and the unsupported-problem
  error messages.

## Test Approach

- `cvxrust` unit tests for linearization: constants, single variables,
  parameters, and each operator (`Add`, `Sub`, `Mul` by a constant, `Div` by
  a constant, `Neg`, `Scale`), including nested combinations; `Mul`/`Div`
  between two variable-dependent terms returns the documented nonlinear
  error; a non-`(1,1)` variable or parameter returns the documented shape
  error.
- `cvxrust` unit tests for the `clarabel`-backed solver covering:
  - A simple two-variable minimize problem with a known optimal
    objective/variable values, checked against a hand-computed answer.
  - The same problem as a maximize problem, confirming the sign handling.
  - A problem with only `<=` constraints, only `>=` constraints, only `=`
    constraints, and a mix of all three.
  - An infeasible problem (e.g., `x <= 0` and `x >= 1`) → `Infeasible`.
  - An unbounded problem (e.g., maximize `x` with no upper-bounding
    constraint) → `Unbounded`.
  - A problem whose optimal `x` is negative, confirming free variables are
    handled correctly without any manual splitting.
  - A problem with no constraints (bare objective) still resolves correctly.
  - A problem exceeding `MAX_VARIABLES`/`MAX_CONSTRAINTS` → the size-limit
    `Error`.
  - A `clarabel` status other than `Solved`/`AlmostSolved` (forced via a
    tiny `max_iter` on a nontrivial problem to trigger `MaxIterations`) maps
    to the documented `Error` message.
- `cvxx` unit test (regression for the identity fix) in
  `src/excel/problem.rs`: build a problem referencing two distinct scalar
  variables of identical shape via `CVX.SOLVE`'s existing test harness,
  confirm each variable's `ResultEntry.variable_values` entry matches its
  own coefficient, not the other variable's.
- `cvxx` unit tests updated at every existing `Variable::new(shape)` call
  site (`src/analytics/resolve.rs`, `src/excel/expression.rs`,
  `src/core/registry.rs`) to pass the matching id and continue to pass.
- Integration test: a sample workbook builds a small linear program (e.g., a
  simple resource-allocation problem with two variables and a mix of `<=`
  and `>=` constraints) through `CVX.MINIMIZE`/`CVX.PROBLEM`/`CVX.SOLVE` and
  checks the resulting handle is a `cvx:result:` handle whose stored status
  is `Optimal` with the expected objective value (result *inspection* via a
  worksheet formula is out of scope per `ISSUE-0007`, so the test reads the
  registry directly).

## Dependencies

- SPEC-0002 for registry/handle conventions.
- SPEC-0003 for the existing `Variable`/`VariableEntry` this specification
  extends with an `id` field.
- SPEC-0004 for the `Expression` AST shapes (`Constant`, `Variable`,
  `Parameter`, `Add`, `Sub`, `Mul`, `Div`, `Neg`, `Scale`) that linearization
  walks.
- SPEC-0006 for `Sense`, `Relation`, `Constraint`, `Problem`, `SolveStatus`,
  `Solution`, and the `CVX.SOLVE` plumbing that calls `cvxrust::solve`
  unchanged.
- New external crate dependency: `clarabel` (added to `cvxrust/Cargo.toml`
  only; `cvxx`'s own `Cargo.toml` is unchanged since it only depends on
  `cvxrust`'s existing public API). `clarabel` is adopted as the standing
  solver framework for this repository.
- A future `SPEC-0011` is expected to extend solver support to quadratic
  objectives and/or non-scalar variables by populating `clarabel`'s `P`
  matrix and expanding the linearization step introduced here, rather than
  integrating a different solver.

## Status

Implemented as specified.

- `Variable` gained the `id` field exactly as described; `Registry::insert_variable`
  and every other `Variable::new` call site were updated to pass a matching id.
- `cvxrust::solve` was reimplemented on top of `clarabel` following Steps 1-4
  (shape validation, linearization, conic-program translation, and recovery)
  exactly as specified, including the `MAX_VARIABLES`/`MAX_CONSTRAINTS`/`MAX_ITERATIONS`
  limits and the documented error messages.
- The zero-variable (bare-objective, no-constraint) case is short-circuited
  before invoking `clarabel`, since `clarabel` 0.9's KKT setup does not
  tolerate a zero-column problem; this evaluates the trivial affine optimum
  directly (checking any zero-variable constraints for feasibility) instead
  of building a degenerate conic program, per the "pick a sound resolution"
  guidance for this edge case.
- `docs/problems.md` was updated to document the supported problem class and
  error messages, removing the "no solver implemented" caveat.
- All `cvxrust` and `cvxx` unit tests described in Test Approach pass,
  including the negative-optimal-`x` and bare-objective cases;
  `cargo fmt` and `cargo clippy --all-targets -- -D warnings` are clean.
