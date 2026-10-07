---
id: SPEC-0019
title: Mixed-integer and binary decision variables for linear problems
issue: ISSUE-0019
status: draft
created: 2026-10-07
---

## Objective

Let a user restrict some or all elements of a variable to integer values, or
to binary (0/1) values, and solve a problem containing a mix of continuous,
integer, and binary variables — provided the objective and every constraint
are affine (no quadratic terms; SPEC-0011's quadratic support is unchanged
but not combinable with this specification's domain restrictions). Two new
Excel functions, `CVX.INTEGER` and `CVX.BINARY`, declare a domain
restriction over a variable or a rectangular sub-block of one (reusing
`CVX.INDEX`, SPEC-0015); the restriction is supplied to `CVX.PROBLEM`
exactly like an ordinary constraint, and `CVX.SOLVE` reports a correct
optimal answer, an honest `Infeasible`/`Unbounded` status, or a new
`stopped_at_limit` status carrying the best feasible answer found before a
time/node limit was reached without proving optimality.

This specification adopts
[`microlp`](https://crates.io/crates/microlp) as a second, dedicated solver
dependency inside `cvxrust`, used only for problems containing at least one
integer/binary domain restriction; purely continuous problems are
unaffected and keep routing through the existing `clarabel` translation
layer (SPEC-0010/SPEC-0011). See the "Mixed-integer linear programming
needs a second, dedicated solver crate" entry (dated 2026-10-07) in
`docs/architecture.md` for the full rationale for rejecting `HiGHS` (no
pure-Rust binding; its only Rust crate requires a C++ compiler and CMake,
or a pre-installed system library, neither of which the Windows XLL build
currently needs) and rejecting hand-rolled branch-and-bound on top of
`clarabel` (an interior-point conic solver with no integrality or
warm-start support suited to repeated node re-solves).

## Non-Objective

- Mixed-integer **quadratic** programming (MIQP): a problem combining a
  domain restriction with any quadratic objective/constraint term is
  rejected with a descriptive error (see Error Handling), not solved
  approximately or by silently dropping the quadratic term. Deferred to a
  future issue/specification, per the architecture decision.
- Dual values, sensitivity analysis, or solver tuning beyond the fixed
  time/node limit introduced here (consistent with SPEC-0010/SPEC-0011's
  existing non-objectives).
- User-configurable time/node limits. The limits introduced here are fixed
  constants for this specification, analogous to `MAX_ITERATIONS = 200` on
  the `clarabel` path (SPEC-0010); making them user-configurable is a
  possible future enhancement, not required by `ISSUE-0019`.
- Any change to `MAX_VARIABLES` / `MAX_CONSTRAINTS` (still 200/200,
  reused unchanged for the `microlp` path — see Data Model).
- Any change to continuous-only (no domain restriction) problem behavior,
  results, performance, or error messages on the existing `clarabel` path.
- Declaring a domain restriction over an expression that is not a bare
  variable or a `CVX.INDEX` selection directly over a bare variable (e.g.
  over `x + y`, `CVX.SUM(x)`, or `CVX.MATMUL(A, x)`) — rejected with a
  descriptive error (see Error Handling), not silently applied to whichever
  variables happen to appear in the expression.
- A variable-bounds shorthand. As with SPEC-0005's existing non-objective,
  ordinary numeric bounds (e.g. `0 <= x <= 10`) are still expressed as
  affine constraints, not as part of `CVX.VARIABLE`/`CVX.INTEGER`/
  `CVX.BINARY`. `CVX.BINARY` is the one exception required by its own
  definition: it always implies the bound `0 <= x <= 1` natively in the
  solver, in addition to integrality (see Data Model), because "binary"
  would otherwise be indistinguishable from an unbounded integer variable.

## Interface

### Excel functions

```
CVX.INTEGER(variable, [name])
CVX.BINARY(variable, [name])
```

- `variable`: a `cvx:var:<uuid>` handle or registered variable name (the
  whole variable), or a `cvx:expr:<uuid>` handle or registered name whose
  stored expression is exactly `Expression::Index { expr: Variable(_), .. }`
  — i.e. a `CVX.INDEX` selection built directly over a bare variable, with
  no other operation (`CVX.ADD`, `CVX.SUM`, `CVX.MATMUL`, `CVX.TRANSPOSE`,
  a nested `CVX.INDEX` over anything but a variable, etc.) anywhere in its
  construction. Resolved the same way as `CVX.LESS_THAN`'s `left`/`right`
  operands (SPEC-0005), except numeric literals are not accepted (a domain
  restriction is meaningless without a variable to restrict) and the
  resolved expression is further restricted to this variable-or-plain-index
  shape.
- `name`: optional string identifier supplied as the last argument,
  following the same overwrite-by-name convention as SPEC-0003 through
  SPEC-0006.
- Returns: a string handle `cvx:dom:<uuid>` on success, or `#VALUE!` (see
  Error Handling).

Both functions declare a restriction on a rectangular sub-block of a single
variable's scalar entries: the whole variable when `variable` is a bare
`cvx:var:` reference, or exactly the sub-block selected by the `CVX.INDEX`
expression otherwise. `CVX.INTEGER` restricts those entries to integer
values (any whole number, positive, negative, or zero, unless additionally
bounded by ordinary constraints). `CVX.BINARY` restricts those entries to
exactly `0` or `1`.

A domain restriction, like a constraint (SPEC-0005), is declared
independently of any particular problem and does not mutate the
`VariableEntry` it refers to — the same variable may be used unrestricted
in one problem and integer/binary-restricted in another, the same way it
may be bounded differently by different constraint sets in different
problems.

### `CVX.PROBLEM`'s `constraints` argument, extended

SPEC-0006's `constraints` argument to `CVX.PROBLEM` (a single
`cvx:constrset:<uuid>` cell, or a range of individual handles/names) is
extended to also accept `cvx:dom:<uuid>` handles and registered domain
names, mixed freely alongside `cvx:constr:<uuid>` handles/names in the same
range or constraint set. Each resolved domain reference is collected into
a new `domains: Vec<Uuid>` field on `ProblemEntry`, positionally
independent of the existing `constraints: Vec<Uuid>` field — domain
references never become ordinary `cvxrust::Constraint`s. A `constraints`
range or constraint set containing no domain references at all produces a
`ProblemEntry` with `domains: vec![]`, which is the existing, unaffected
continuous-only path.

### `CVX.CONSTRAINTS`, extended

SPEC-0005's `CVX.CONSTRAINTS` is extended identically: its input range may
contain `cvx:dom:<uuid>` handles/names alongside `cvx:constr:<uuid>`
handles/names. `ConstraintSetEntry`'s stored ordered list becomes a list of
tagged references (`enum ConstraintSetItem { Constraint(Uuid), Domain(Uuid)
}`) instead of a bare `Vec<Uuid>`, so that `CVX.PROBLEM` can later split a
referenced set back into its `constraints`/`domains` buckets. The "at least
one item must resolve" rule from SPEC-0005 is unchanged (an empty result,
counting both kinds together, is still an error for `CVX.CONSTRAINTS`,
though not for `CVX.PROBLEM`'s inline range, per SPEC-0006).

### Registry entries

A new registry table is added, following the `RegistryTable<T>` pattern:

```rust
pub struct DomainEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub domain: cvxrust::DomainConstraint,
}
```

The registry supports lookup by UUID or by name, with the same
overwrite-by-name and cross-table uniqueness rules as every other entry
kind (SPEC-0002's amendment).

### Handle format

One new `HandleKind` variant is added: `HandleKind::Domain` → `"dom"`.

### `cvxrust` interface additions

```rust
/// Which discrete values a variable's elements are restricted to.
/// Ordered so that `Binary` is treated as strictly more restrictive than
/// `Integer` when overlapping domain restrictions apply to the same
/// scalar entry (see Data Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    Integer,
    Binary,
}

/// Restricts a rectangular sub-block of a variable's scalar entries to a
/// discrete domain. Uses the same row/col addressing as
/// `Expression::Index` (SPEC-0015): `rows` rows starting at `row_start`,
/// `cols` columns starting at `col_start`, all 0-based, within `variable`'s
/// shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainConstraint {
    pub variable: Variable,
    pub row_start: usize,
    pub col_start: usize,
    pub rows: usize,
    pub cols: usize,
    pub domain: Domain,
}

pub struct Problem {
    pub sense: Sense,
    pub objective: Expression,
    pub constraints: Vec<Constraint>,
    /// New. Empty for every problem built before this specification;
    /// non-empty problems route to the `microlp` translation path instead
    /// of `clarabel` (see Data Model).
    pub domains: Vec<DomainConstraint>,
    pub variables: Vec<Variable>,
}

pub enum SolveStatus {
    Optimal,
    Infeasible,
    Unbounded,
    /// New. A feasible solution honoring every domain restriction was
    /// found, but the `microlp` branch-and-bound search was stopped (time
    /// limit or node limit) before optimality could be proven.
    /// `Solution::objective_value` and `Solution::variable_values` are
    /// populated with the best incumbent found, exactly as for `Optimal`.
    StoppedAtLimit,
    Error(String),
}
```

`Solution` is unchanged in shape; its doc comment is updated to note that
`objective_value`/`variable_values` are also populated (not empty) for
`StoppedAtLimit`.

`cvxx` translates a `core::registry::DomainEntry` into a
`cvxrust::DomainConstraint` once, at declaration time (`CVX.INTEGER`/
`CVX.BINARY`), the same way `ConstraintEntry` already stores a resolved
`cvxrust::Constraint`; no re-resolution happens when the domain reference
is later used by `CVX.PROBLEM`.

## Data Model

### Resolving `CVX.INTEGER(variable)` / `CVX.BINARY(variable)`

1. Resolve `variable` via the shared operand resolver (handle or name), as
   `CVX.LESS_THAN`'s operands are resolved, but restricted to `cvx:var:`
   and `cvx:expr:` handles/names (reject a bare numeric literal or any
   other handle kind with `#VALUE!`).
2. If the resolved object is a `VariableEntry`, the restriction covers the
   whole variable: `row_start = col_start = 0`, `rows`/`cols` = the
   variable's shape.
3. If the resolved object is an `ExpressionEntry`, inspect its stored
   `cvxrust::Expression`:
   - `Expression::Index { expr, row_start, col_start, rows, cols }` where
     `*expr == Expression::Variable(v)` (a plain index directly over a bare
     variable, no further nesting) → the restriction covers that
     sub-block, over `v`.
   - `Expression::Variable(v)` (a stored expression is a bare variable
     reference without an index — possible if a user builds one via
     `CVX.EXPRESSION("x")`) → treated the same as step 2, over `v`.
   - Anything else (`Add`, `Sub`, `Mul`, `Div`, `Neg`, `Scale`, `Sum`,
     `MatMul`, `Transpose`, `Parameter`, `Constant`, or an `Index` whose
     inner `expr` is not a bare `Variable`) → `#VALUE!`.
4. Build `cvxrust::DomainConstraint { variable: v, row_start, col_start,
   rows, cols, domain }` with `domain` = `Domain::Integer` for
   `CVX.INTEGER`, `Domain::Binary` for `CVX.BINARY`.
5. Insert a `DomainEntry` (with cross-table name uniqueness and
   overwrite-by-name, as usual) and return its `cvx:dom:<uuid>` handle.

### Building `Problem::domains` in `CVX.PROBLEM`

When resolving the (extended) `constraints` argument, each resolved
`cvx:dom:` reference's UUID is appended to `ProblemEntry::domains` instead
of `ProblemEntry::constraints`; its referenced variable is added to
`ProblemEntry::variables` the same way a constraint's variables already
are, so it is included in the `cvxrust::Problem::variables` list (and thus
in `Solution::variable_values`) even if that variable never otherwise
appears in the objective or an ordinary constraint. When `cvxrust::solve`
is invoked, `ProblemEntry::domains` is translated into
`Vec<cvxrust::DomainConstraint>` by looking up each `DomainEntry` by UUID,
positionally independent of `ProblemEntry::constraints`.

### `cvxrust::solve` routing

```
solve(problem):
    if problem.domains.is_empty():
        # unchanged: existing clarabel translation path (SPEC-0010/0011)
        return solve_continuous(problem)
    return solve_mixed_integer(problem)
```

`solve_mixed_integer`:

1. Reduce the objective and every constraint row with the existing
   `quadratize` step (SPEC-0011), exactly as `solve_continuous` does, to
   reuse all of its existing validation (shape/broadcast errors, degree- 3
   rejection, non-affine-with-vector/matrix-operand rejection, and the
   `MAX_VARIABLES`/`MAX_CONSTRAINTS = 200` size limit, all unchanged).
2. If any resulting `QuadraticForm` (the objective, or any constraint row)
   has a non-empty `quad` list, return
   `SolveStatus::Error("mixed-integer solving (CVX.INTEGER/CVX.BINARY) \
   does not yet support quadratic objectives or constraints; remove the \
   quadratic term(s) or remove the integer/binary declaration(s)")`
   — this is the plain-language explanation `ISSUE-0019` requires for an
   unsupported problem-type combination.
3. Otherwise every `QuadraticForm` is purely affine (`constant` + `linear`,
   empty `quad`); build a flattened, per-scalar-entry domain map: for each
   `DomainConstraint`, expand its `(row_start, col_start, rows, cols)`
   sub-block over `variable` into the set of global scalar indices (the
   same global numbering `quadratize` already assigns to every scalar
   entry of every problem variable) it covers, tagged with its `domain`.
   Where two or more `DomainConstraint`s cover the same scalar index with
   different domains, the more restrictive one wins: `Domain::Binary` over
   `Domain::Integer` (reflecting `Domain`'s `Ord`), so a user may tighten
   an already-integer sub-block to binary with a second, narrower
   declaration without that being treated as a conflict/error.
4. Build a `microlp::Problem` with `OptimizationDirection::Minimize`/
   `Maximize` matching `problem.sense`, one `microlp` variable per global
   scalar index:
   - An index with no domain entry, or tagged `Domain::Integer`: added via
     `add_var`/`add_integer_var` respectively, with native bounds
     `(f64::NEG_INFINITY, f64::INFINITY)` / `(i64::MIN, i64::MAX)` — i.e.
     unbounded at the solver level, exactly like a `clarabel`-path free
     variable; any numeric bound the user wants is still expressed as an
     ordinary affine constraint row, translated in the next step, keeping
     one bounding mechanism across both solver paths.
   - An index tagged `Domain::Binary`: added via `add_integer_var` with
     native bounds `(0, 1)` — binary's `0`/`1` restriction is encoded as a
     native solver bound (not left to an affine constraint row), since it
     is intrinsic to what "binary" means, per this specification's one
     exception to the "bounds are ordinary constraints" rule (Non-
     Objective).
   - the objective's linear coefficients set each variable's objective
     coefficient (`microlp`'s `add_var`/`add_integer_var` take the
     coefficient directly).
   - each affine constraint row added via `add_constraint` with the
     row's linear coefficients, the matching `ComparisonOp`
     (`Le`/`Ge`/`Eq` for `Relation::LessEqual`/`GreaterEqual`/`Equal`), and
     its constant right-hand side (`-constant` from the row's
     `QuadraticForm`, following the same affine-form convention as the
     `clarabel` path).
5. Set a fixed time limit (`MILP_TIME_LIMIT_SECS: f64 = 10.0`) and a fixed
   node limit (`MILP_NODE_LIMIT: u64 = 100_000`) on the `microlp::Problem`
   before solving — the hang-prevention mechanism `ISSUE-0019` requires,
   analogous in spirit to `clarabel`'s existing `MAX_ITERATIONS = 200`.
6. Call `solve()` and translate the outcome:
   - `Err(microlp::Error::Infeasible)` → `SolveStatus::Infeasible`.
   - `Err(microlp::Error::Unbounded)` → `SolveStatus::Unbounded`.
   - `Err(microlp::Error::InvalidOptions | microlp::Error::InternalError)`
     → `SolveStatus::Error(message)`, logged the same way other solver
     failures are (`%TEMP%/cvxx.log`).
   - `Ok(outcome)` where the search proved optimality
     (`TerminationReason::ProvenOptimal`) → `SolveStatus::Optimal`, with
     `objective_value`/`variable_values` read from the solution.
   - `Ok(outcome)` where the search stopped at a limit
     (`TerminationReason::TimeLimit`, `NodeLimit`, or `MipGap`) **and** a
     feasible incumbent exists → `SolveStatus::StoppedAtLimit`, with
     `objective_value`/`variable_values` read from that incumbent.
   - `Ok(outcome)` where the search stopped at a limit **without** ever
     finding a feasible incumbent → `SolveStatus::Error("mixed-integer \
     solve did not find a feasible solution within the time/node limit; \
     feasibility is undetermined (not proven infeasible)")` — an honest
     "don't know" is reported as a failure rather than guessed at, since
     neither `Infeasible` nor `StoppedAtLimit` would be accurate here.
7. Map each `microlp` variable's solved value back to
   `Solution::variable_values` using the same global-scalar-index-to-
   `(variable, offset)` bookkeeping `quadratize` already maintains, so the
   row-major layout matches the `clarabel` path exactly.

### `CVX.STATUS` / `CVX.VALUE` / `CVX.OBJECTIVE_VALUE`, extended

- `CVX.STATUS` gains a fourth possible return value: `"stopped_at_limit"`,
  for a `ResultEntry` whose `status` is `SolveStatus::StoppedAtLimit`.
- `CVX.VALUE` and `CVX.OBJECTIVE_VALUE` treat `StoppedAtLimit` the same way
  as `Optimal`: they return the stored variable/objective values rather
  than `#VALUE!`, since `ISSUE-0019` requires the best-found answer to be
  readable, not hidden behind a non-`Optimal` status the way a hard
  `Infeasible`/`Unbounded` result already is.
- `CVX.DESCRIBE`/`CVX.TYPE` gain a `"domain"` kind for `cvx:dom:` handles
  (e.g. `CVX.DESCRIBE` renders `"x": integer domain over 1x1` for a whole-
  variable restriction, or `"x"[2:3, 1]: binary domain` for a sub-block
  one, reusing the existing `CVX.INDEX` rendering from SPEC-0007).
  `CVX.SHAPE` on a `cvx:dom:` handle returns `#VALUE!`, consistent with
  constraints, constraint sets, objectives, problems, and results (none of
  which have a "shape" in `CVX.SHAPE`'s sense).
- `ResultEntry` is now also created for a `StoppedAtLimit` outcome (not
  just `Optimal`/`Infeasible`/`Unbounded`), since it is not a solver
  failure.

## Error Handling

- `CVX.INTEGER`/`CVX.BINARY`:
  - Unresolvable `variable` handle/name → `#VALUE!` via
    `CvxError::UnknownIdentifier`.
  - A numeric literal or a handle kind other than `cvx:var:`/`cvx:expr:` →
    `#VALUE!`.
  - A resolved `cvx:expr:` whose expression is not a bare variable or a
    plain `CVX.INDEX` directly over a bare variable (step 3's "anything
    else" case) → `#VALUE!` with a message naming the unsupported
    construction, e.g. "CVX.INTEGER/CVX.BINARY can only restrict a variable
    or a CVX.INDEX selection directly over a variable".
  - Reusing a name already registered under a different object table →
    `#VALUE!` via `CvxError::AmbiguousIdentifier` (SPEC-0002's amendment),
    checked before the same-table `DuplicateName` rule.
  - Duplicate name within the domain table → `#VALUE!` via
    `CvxError::DuplicateName`.
- `CVX.CONSTRAINTS` / `CVX.PROBLEM`'s `constraints` argument: a non-blank
  cell that is neither a known constraint, constraint-set, nor domain
  handle/name → `#VALUE!` via `CvxError::UnknownIdentifier` (the existing
  SPEC-0005/SPEC-0006 rule, now also covering `cvx:dom:` lookups).
- `cvxrust::solve` on a problem with at least one domain restriction and a
  quadratic objective/constraint term → `SolveStatus::Error("...")` (exact
  message in Data Model, step 2) → `#VALUE!` at the `CVX.SOLVE` boundary,
  logged, not stored as a `ResultEntry` — the same "hard solver failure"
  handling SPEC-0006 already defines for any other `SolveStatus::Error`.
- `cvxrust::solve` exceeding `MAX_VARIABLES`/`MAX_CONSTRAINTS` on the
  `microlp` path → `SolveStatus::Error(...)`, reusing SPEC-0011's existing
  oversize-problem message and `#VALUE!` handling, unchanged.
- A mixed-integer solve that stops at the time/node limit without any
  feasible incumbent → `SolveStatus::Error("...")` (Data Model, step 6) →
  `#VALUE!`, consistent with "an honest status, not a generic error" being
  interpreted as: report `Infeasible`/`Unbounded`/`StoppedAtLimit` only
  when the search can actually support that conclusion, and fail
  explicitly (with a message that says *why* — feasibility undetermined)
  otherwise.
- `Infeasible`, `Unbounded`, and `StoppedAtLimit` outcomes are not
  failures: each stores a `ResultEntry` and returns a `cvx:result:<uuid>`
  handle normally, exactly as `Optimal` already does.
- Diagnostics are logged to `%TEMP%/cvxx.log`, unchanged.

## Test Approach

- Unit tests in `cvxrust`:
  - `Domain`/`DomainConstraint` construction and `Ord` (`Binary >
    Integer`).
  - `solve` routing: a `Problem` with empty `domains` produces bit-for-bit
    identical `Solution`s to before this specification (regression test
    against existing SPEC-0010/0011 fixtures).
  - A small all-integer problem (e.g. a knapsack-style 0/1 selection) with
    a known optimal objective/variable values, solved via the `microlp`
    path and asserted `Optimal`.
  - A problem combining continuous, integer, and binary variables in one
    objective/constraint set, asserted `Optimal` with correct values.
  - An infeasible all-integer problem (e.g. `x + y == 0.5` with `x`, `y`
    integer) asserted `SolveStatus::Infeasible`.
  - An unbounded mixed-integer problem asserted `SolveStatus::Unbounded`.
  - A problem engineered to exceed the fixed time/node limit before
    proving optimality but after finding a feasible incumbent (e.g. a
    deliberately hard bin-packing-style instance), asserted
    `SolveStatus::StoppedAtLimit` with non-`None`
    `objective_value`/`variable_values`.
  - A domain restriction combined with a quadratic objective term (`x *
    x` with `x` integer) asserted `SolveStatus::Error` with the specific
    message from Data Model step 2.
  - Overlapping `Integer`/`Binary` domain restrictions on the same scalar
    entry resolve to `Binary`, verified by checking the built `microlp`
    variable's bounds/kind.
- Unit tests in `cvxx`:
  - `CVX.INTEGER`/`CVX.BINARY` over a whole variable and over a `CVX.INDEX`
    sub-block; rejecting a numeric literal, a non-`cvx:var:`/`cvx:expr:`
    handle, and an expression that is not a bare variable or plain index
    over one.
  - `CVX.PROBLEM`'s extended `constraints` resolution: a range mixing
    ordinary constraint handles and domain handles; a `CVX.CONSTRAINTS`
    set mixing both kinds, referenced by name from `CVX.PROBLEM`.
  - `CVX.STATUS` returning `"stopped_at_limit"`; `CVX.VALUE`/
    `CVX.OBJECTIVE_VALUE` succeeding (not `#VALUE!`) for a
    `StoppedAtLimit` result.
  - `CVX.DESCRIBE`/`CVX.TYPE`/`CVX.SHAPE` on a `cvx:dom:` handle.
  - Registry tests: insertion/lookup by UUID/name, overwrite-by-name, and
    cross-table uniqueness for the new domain table.
- Integration test: a sample workbook declares an integer and a binary
  variable, builds a small mixed-integer problem (e.g. a simple project-
  selection knapsack), solves it, and confirms `CVX.STATUS` is
  `"optimal"` with the expected whole-number/binary variable values.

## Documentation Impact

- `docs/variables.md`: add a section introducing integer/binary
  restrictions, cross-referencing the new `CVX.INTEGER`/`CVX.BINARY`
  functions and explaining, for a non-optimization-background reader,
  what "integer" and "binary" decisions mean (truck counts, yes/no
  project selection) — mirroring `ISSUE-0019`'s own background examples.
- `docs/constraints.md`: document that `CVX.CONSTRAINTS` and `CVX.PROBLEM`
  now also accept `cvx:dom:` handles/names alongside ordinary constraints.
- `docs/problems.md`: document the new `microlp`-backed mixed-integer
  solve path, the fixed time/node limit, the new `StoppedAtLimit` /
  `"stopped_at_limit"` status and its plain-language meaning ("best answer
  found so far, not a guaranteed optimum" vs. "proven optimal"), and the
  quadratic-plus-integer/binary error case.
- `docs/inspection.md`: document `CVX.STATUS`'s fourth possible value,
  `CVX.VALUE`/`CVX.OBJECTIVE_VALUE` now also succeeding for
  `StoppedAtLimit`, the new `"domain"` `CVX.TYPE` kind, and
  `CVX.DESCRIBE`/`CVX.SHAPE` behavior for `cvx:dom:` handles.
- `docs/SUMMARY.md`: no new page needed; existing pages are extended in
  place.
- `tools/gen-examples`: a new generated example workbook demonstrating a
  simple mixed-integer problem (e.g. project selection or truck
  assignment, per `ISSUE-0019`'s own examples) should be added alongside
  the existing generated workbooks, consistent with SPEC-0017's
  "every exported `CVX.*` function is mentioned" documentation-coverage
  discipline.

## Dependencies

- `docs/architecture.md`'s 2026-10-07 "Mixed-integer linear programming
  needs a second, dedicated solver crate" decision, which this
  specification implements.
- SPEC-0002 for the registry/handle conventions, overwrite-by-name, and
  cross-table uniqueness rules reused for the new domain table.
- SPEC-0003 for `cvxrust::Variable` and variable handles.
- SPEC-0005 for constraint/constraint-set handles and the operand
  resolution helpers (`resolve_operand`, `resolve_handle_arg`) reused by
  `CVX.INTEGER`/`CVX.BINARY`.
- SPEC-0006 for `ProblemEntry`, `CVX.PROBLEM`'s `constraints` resolution,
  and the `cvxrust::Problem`/`SolveStatus`/`solve` contract extended here.
- SPEC-0007 for `CVX.STATUS`/`CVX.VALUE`/`CVX.OBJECTIVE_VALUE` and the
  generic `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` machinery extended here.
- SPEC-0010/SPEC-0011 for the existing `clarabel` translation layer
  (`quadratize`, `QuadraticForm`, `MAX_VARIABLES`/`MAX_CONSTRAINTS`,
  `MAX_ITERATIONS`) that the `microlp` path reuses for reduction and size
  limits, and which remains completely unchanged for problems with no
  domain restrictions.
- SPEC-0015 for `Expression::Index` and its row/col addressing, reused by
  `DomainConstraint` and by `CVX.INTEGER`/`CVX.BINARY`'s sub-block
  resolution.
- SPEC-0017 for the generated-example-workbook convention the new
  mixed-integer example should follow.
- New external dependency: [`microlp`](https://crates.io/crates/microlp)
  (Apache-2.0, pure Rust), added to `cvxrust/Cargo.toml` only — `cvxx`
  continues to depend solely on `cvxrust`'s public API and must never
  import `microlp` directly, mirroring the existing `clarabel` boundary.

## Status

Implemented. `cvxrust` gained `Domain`/`DomainConstraint` (`model.rs`,
`Domain::Binary > Domain::Integer` via `Ord`), `Problem::domains`, and
`SolveStatus::StoppedAtLimit`. `cvxrust::solve` (`solver.rs`) routes a
`Problem` with a non-empty `domains` list to a new `solver_milp` module
instead of `clarabel`, translating each variable's scalar entries into
`microlp` columns with the domain-appropriate bounds/kind (continuous by
default, overlapping restrictions resolving to the stricter `Binary`), a
fixed time/node limit before reporting `StoppedAtLimit` on a found-but-
unproven incumbent, and the quadratic-plus-domain rejection from Data
Model step 2. A `Problem` with empty `domains` is unaffected and still
solves via `clarabel`, confirmed by a regression test against the
existing SPEC-0010/0011 fixtures. `solver_milp_tests.rs` covers an
all-integer knapsack, a mixed continuous/integer/binary problem,
infeasible and unbounded mixed-integer problems, a
time/node-limit-exceeded `StoppedAtLimit` case, the quadratic-plus-domain
error, and overlapping `Integer`/`Binary` resolving to `Binary`.

`cvxx` gained a `cvx:dom:` handle kind (`src/core/handle.rs`), a
`DomainEntry`/`domains` registry table and a `ConstraintSetItem` enum
distinguishing ordinary constraints from domain references within a
`ConstraintSetEntry`'s (renamed) `items` list (`src/core/registry.rs`),
and a new `src/excel/domain.rs` implementing `CVX.INTEGER`/`CVX.BINARY`
(accepting a bare variable or a plain `CVX.INDEX` sub-block, rejecting
numeric literals, non-variable handles, and nested expressions).
`CVX.CONSTRAINTS` and `CVX.PROBLEM`'s `constraints` argument both accept
a mix of constraint and domain handles/names (`src/excel/constraint.rs`,
`src/excel/problem.rs`); `CVX.PROBLEM` sweeps domain-referenced variables
into the problem's variable list the same way it does for constraints,
and `CVX.SOLVE` forwards the resolved `DomainConstraint`s into
`cvxrust::Problem::domains`. `CVX.STATUS` returns `"stopped_at_limit"`;
`CVX.VALUE`/`CVX.OBJECTIVE_VALUE` succeed (rather than `#VALUE!`) for
both `Optimal` and `StoppedAtLimit` results; `CVX.DESCRIBE`/`CVX.TYPE`/
`CVX.SHAPE` all handle `cvx:dom:` handles (`src/excel/inspect.rs`). All
of the Test Approach's unit tests are implemented, including a
`src/excel/problem.rs` integration test that builds and solves a binary
0/1 knapsack entirely through the registry/resolver path `CVX.PROBLEM`/
`CVX.SOLVE` use internally, confirming `"optimal"` status and the
expected binary variable values.

`docs/variables.md`, `docs/constraints.md`, `docs/problems.md`, and
`docs/inspection.md` were updated per the Documentation Impact section
above; `docs/SUMMARY.md` needed no change. `tools/gen-examples` gained
`08-mixed-integer-programming.xlsx` (project-selection knapsack using
`CVX.BINARY`), and `scripts/check-examples-generated.ps1` was updated for
the new nine-workbook count; `docs/examples/*.xlsx` were regenerated.

Verified via `cargo build --workspace --all-targets`, `cargo test
--workspace` (328 tests passed across `cvxrust`, `cvxx`, and
`gen-examples`, 0 failed), `cargo clippy --workspace --all-targets`
(clean), `cargo fmt --all -- --check` (clean), and
`scripts/check-public-functions-documented.ps1` /
`scripts/check-examples-generated.ps1` (both pass).

Known limitation, inherited from `microlp`: integer/binary variable
bounds are internally represented as `i32`, so domain restrictions on
variables expected to take values outside that range are not supported;
this is not separately validated/surfaced as a distinct error today.
