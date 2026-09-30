---
id: SPEC-0006
title: Build and solve optimization problems from Excel formulas
issue: ISSUE-0006
status: implemented
created: 2026-09-30
---

## Objective

Implement Excel-facing functions that assemble a `cvx:obj:<uuid>` objective, a
`cvx:prob:<uuid>` problem, and a `cvx:result:<uuid>` solver result from
existing parameter, variable, expression, and constraint handles, following
the registry/handle conventions established in SPEC-0002 through SPEC-0005.
This specification also defines the minimal `cvxrust` interface (`Sense`,
`Constraint`, `Problem`, `SolveStatus`, `Solution`, `solve`) that `cvxx` calls
to perform the solve.

## Non-Objective

- No actual convex optimization algorithm (simplex, interior-point, ADMM,
  etc.). `cvxrust` remains the "minimal stand-in" described in its own module
  header; this specification defines only the `Problem`/`solve` plumbing
  contract. Until a real solver is wired into `cvxrust`, `solve` returns
  `SolveStatus::Error("not implemented")` and `CVX.SOLVE` surfaces `#VALUE!`.
- No result inspection functions (e.g., reading a solved variable's value or
  the objective value back into a cell, or dual values/sensitivity). These
  are covered by SPEC-0007 (`ISSUE-0007`).
- No convenience solvers (`ISSUE-0008`).
- No re-parsing of Excel ranges at solve time; `CVX.SOLVE` operates entirely
  on registry state already resolved by `CVX.PARAMETER`, `CVX.VARIABLE`,
  `CVX.EXPRESSION`, and `CVX.CONSTRAINT`/`CVX.CONSTRAINTS`.
- No ribbon or XLAM changes.

## Interface

### Excel functions

```
CVX.MINIMIZE(objective, [name])
CVX.MAXIMIZE(objective, [name])
CVX.PROBLEM(objective, constraints, [name])
CVX.SOLVE(problem, [name])
```

- `objective`: a `cvx:param:`, `cvx:var:`, or `cvx:expr:` handle, a registered
  name, or a bare numeric literal, resolved the same way as the `left`/
  `right` operands of `CVX.LESS_THAN` (SPEC-0005) — i.e. via a shared
  `resolve_operand` helper that tries a numeric literal first, then falls
  back to handle/name resolution.
- `CVX.MINIMIZE`/`CVX.MAXIMIZE` return a `cvx:obj:<uuid>` handle wrapping the
  resolved objective expression with a `Sense` of `Minimize`/`Maximize`.
- `objective` (in `CVX.PROBLEM`): a `cvx:obj:<uuid>` handle or registered
  name produced by `CVX.MINIMIZE`/`CVX.MAXIMIZE`. A bare expression, without
  an explicit sense, is rejected — the sense must always be stated.
- `constraints` (in `CVX.PROBLEM`): optional. One of:
  - blank/missing — the problem has no constraints;
  - a single cell containing a `cvx:constrset:<uuid>` handle or a registered
    constraint-set name — the problem uses that set directly;
  - a range containing `cvx:constr:<uuid>` handles or registered constraint
    names, flattened row-major with blank cells skipped, exactly as
    `CVX.CONSTRAINTS` does in SPEC-0005 (an empty result here is *not* an
    error, unlike `CVX.CONSTRAINTS`, since an unconstrained problem is
    valid).
- `problem` (in `CVX.SOLVE`): a `cvx:prob:<uuid>` handle or registered name
  produced by `CVX.PROBLEM`.
- `name`: optional string identifier supplied as the last argument, following
  the same overwrite-by-name convention as SPEC-0003/0004/0005.
- Returns: `CVX.MINIMIZE`/`CVX.MAXIMIZE` return `cvx:obj:<uuid>`.
  `CVX.PROBLEM` returns `cvx:prob:<uuid>`. `CVX.SOLVE` returns
  `cvx:result:<uuid>` on a stored solver outcome (`Optimal`, `Infeasible`, or
  `Unbounded`), or an Excel error on a hard solver failure (see Error
  Handling).

### Registration constraints

- None of `CVX.MINIMIZE`, `CVX.MAXIMIZE`, `CVX.PROBLEM`, or `CVX.SOLVE` are
  registered with the volatile (`!`) flag. Recalculation is left entirely to
  Excel's normal dependency tracking, per the parent issue's note that the
  add-in must not force automatic recalculation.

### Registry entries

Three new registry tables are added, following the `RegistryTable<T>`
pattern established in SPEC-0002 through SPEC-0005:

```rust
pub struct ObjectiveEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub sense: cvxrust::Sense,
    pub expression: cvxrust::Expression,
    pub dependencies: Vec<String>,
}

pub struct ProblemEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub objective: Uuid,          // references an ObjectiveEntry
    pub constraints: Vec<Uuid>,   // ordered ConstraintEntry references; may be empty
    /// Distinct variable UUIDs referenced (directly or transitively) by the
    /// objective and constraints, in first-seen order. Positionally aligned
    /// with the `variables` field of the `cvxrust::Problem` passed to
    /// `cvxrust::solve`, so solved values can be mapped back to the
    /// registry's `VariableEntry` UUIDs without `cvxrust::Variable` needing
    /// any identity of its own.
    pub variables: Vec<Uuid>,
}

pub struct ResultEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub problem: Uuid,            // references a ProblemEntry
    pub status: cvxrust::SolveStatus,
    pub objective_value: Option<f64>,
    /// Solved values keyed by `VariableEntry` UUID, row-major per the
    /// variable's shape. Empty (and `objective_value: None`) when `status`
    /// is not `Optimal`.
    pub variable_values: HashMap<Uuid, Vec<f64>>,
}
```

- The registry supports lookup by UUID or by name for all three new tables.
- `CVX.MINIMIZE`/`CVX.MAXIMIZE`, `CVX.PROBLEM`, and `CVX.SOLVE` all use the
  overwrite-by-name insertion convention (reusing a name replaces the
  previous entry), matching `ConstraintEntry`/`ConstraintSetEntry`.
- `ResultEntry` is only inserted when `cvxrust::solve` returns `Optimal`,
  `Infeasible`, or `Unbounded`. A `SolveStatus::Error(_)` outcome is not
  stored; `CVX.SOLVE` returns an Excel error instead (see Error Handling).

### Handle format

Three new `HandleKind` variants are added, following the `cvx:<kind>:<uuid>`
pattern from SPEC-0002:

- `HandleKind::Obj` → `"obj"`
- `HandleKind::Prob` → `"prob"`
- `HandleKind::Result` → `"result"`

### `cvxrust` interface additions

`cvxrust` is extended with the minimal types needed to represent and "solve"
a problem, consistent with its role as a stand-in for the real solver crate:

```rust
pub enum Sense {
    Minimize,
    Maximize,
}

pub enum Relation {
    LessEqual,
    GreaterEqual,
    Equal,
}

pub struct Constraint {
    pub relation: Relation,
    pub lhs: Expression,
    pub rhs: Expression,
}

pub struct Problem {
    pub sense: Sense,
    pub objective: Expression,
    pub constraints: Vec<Constraint>,
    /// Ordered, positionally aligned with `Solution::variable_values`.
    pub variables: Vec<Variable>,
}

pub enum SolveStatus {
    Optimal,
    Infeasible,
    Unbounded,
    Error(String),
}

pub struct Solution {
    pub status: SolveStatus,
    pub objective_value: Option<f64>,
    /// Aligned by index with `Problem::variables`; each inner `Vec<f64>` is
    /// row-major data matching that variable's shape. Empty when `status`
    /// is not `Optimal`.
    pub variable_values: Vec<Vec<f64>>,
}

pub fn solve(problem: &Problem) -> Solution;
```

`cvxrust::Relation` is distinct from `cvxx`'s `analytics::ast::Relation`
(SPEC-0005); `cvxx` translates one into the other when building a
`cvxrust::Constraint` from a `core::registry::ConstraintEntry`. Until a real
solver is implemented, `solve` always returns
`Solution { status: SolveStatus::Error("not implemented".to_string()), objective_value: None, variable_values: vec![] }`.

## Data Model

- Input: an objective operand (handle/name/literal) plus a sense, for
  `CVX.MINIMIZE`/`CVX.MAXIMIZE`; an objective handle plus an optional
  constraints range/handle, for `CVX.PROBLEM`; a problem handle, for
  `CVX.SOLVE`. All with an optional trailing name.
- Intermediate: the ordered, deduplicated list of variable UUIDs referenced
  by the objective and constraints (built while assembling `ProblemEntry`)
  and the corresponding `Vec<cvxrust::Variable>` passed to `cvxrust::solve`.
- Stored object: `ObjectiveEntry`, `ProblemEntry`, or `ResultEntry` in the
  registry.
- Output: string handle, or an Excel error for a hard solver failure.

## Error Handling

- Invalid/unknown objective operand → `#VALUE!` via `CvxError::UnknownIdentifier`.
- `CVX.PROBLEM` given a handle that is not `cvx:obj:` for its `objective`
  argument → `#VALUE!` via `CvxError::UnknownIdentifier`.
- `CVX.PROBLEM` constraints argument containing a non-blank cell that is
  neither a known constraint handle/name nor a known constraint-set
  handle/name → `#VALUE!` via `CvxError::UnknownIdentifier` (reusing the
  SPEC-0005 constraint-resolution error path).
- `CVX.SOLVE` given a handle that is not `cvx:prob:` or an unknown
  problem name/UUID → `#VALUE!` via `CvxError::UnknownIdentifier`.
- `cvxrust::solve` returning `SolveStatus::Error(message)` → `#VALUE!`; the
  `message` is logged via `tracing::error!` but not stored in the registry
  and not written to the worksheet, matching the existing `to_xloper_result`
  convention.
- `cvxrust::solve` returning `Optimal`, `Infeasible`, or `Unbounded` is *not*
  an error: a `ResultEntry` is created and its handle returned normally, so
  callers can distinguish "the solver ran and reported infeasible" from "the
  solver could not run at all". Reading the stored status is out of scope
  here (SPEC-0007).
- Duplicate name on any of the three new entries → `#VALUE!` via
  `CvxError::DuplicateName`.
- Internal registry failure (lock poisoned) → `#VALUE!` via
  `CvxError::Registry`.

## Test Approach

- Unit tests for `CVX.MINIMIZE`/`CVX.MAXIMIZE`: resolving a handle, a name,
  and a numeric literal objective operand; correct `Sense` stored; duplicate
  name rejection and overwrite-by-name behavior.
- Unit tests for `CVX.PROBLEM`: building a problem from an objective handle
  and (a) no constraints, (b) a single constraint-set handle, (c) a range of
  individual constraint handles; rejecting a non-`cvx:obj:` objective
  argument; rejecting an unresolvable constraint entry; deduplicating shared
  variables referenced by both the objective and a constraint into a single
  entry in `ProblemEntry::variables`.
- Unit tests for `CVX.SOLVE`: given the current `cvxrust::solve` stub,
  confirm it surfaces `#VALUE!` (since the stub always returns
  `SolveStatus::Error`) and that no `ResultEntry` is created in that case.
  Add a second test that drives `Registry::insert_result` (or equivalent)
  directly with a synthetic `Solution { status: SolveStatus::Optimal, .. }`
  to confirm the `Optimal`/`Infeasible`/`Unbounded` success path stores a
  `ResultEntry` and returns a `cvx:result:` handle, independent of the real
  solver landing later.
- Unit tests for the registry: insertion and lookup by UUID/name for
  `ObjectiveEntry`, `ProblemEntry`, and `ResultEntry`; overwrite-by-name
  behavior for each.
- Unit tests for `cvxrust`: `Problem`/`Solution` construction and the
  placeholder `solve` always returning `SolveStatus::Error`.
- Integration test: a sample workbook builds an objective with
  `CVX.MINIMIZE`, a problem with `CVX.PROBLEM`, calls `CVX.SOLVE`, and
  confirms the result is the expected `#VALUE!` placeholder today (to be
  revisited once `cvxrust` gains a real solver) and that handle prefixes
  (`cvx:obj:`, `cvx:prob:`) match expectations for the non-solve steps.

## Dependencies

- SPEC-0002 for the registry/handle conventions and overwrite-by-name
  pattern.
- SPEC-0003 for variable handles and `cvxrust::Variable`.
- SPEC-0004 for the expression AST, parser, resolver, and the
  `resolve_operand`-style handle/name/literal resolution reused for the
  objective argument.
- SPEC-0005 for constraint handles, constraint sets, and the
  `resolve_handle_arg`/`to_xloper_result` helpers this specification's Excel
  functions reuse.
- `xladd` for XLL function registration and `XLOPER12` access.
- `cvxrust` for `Sense`, `Relation`, `Constraint`, `Problem`, `SolveStatus`,
  `Solution`, and `solve`, all added by this specification as described
  above.

## Implementation Notes

1. Extend `cvxrust/src/lib.rs` with `Sense`, `Relation`, `Constraint`,
   `Problem`, `SolveStatus`, `Solution`, and a `solve` function that always
   returns `SolveStatus::Error("not implemented".to_string())` with empty
   `variable_values` and `objective_value: None`. Add `#[cfg(test)]` unit
   tests confirming this placeholder behavior and basic struct
   construction/equality.
2. Add `ObjectiveEntry`, `ProblemEntry`, and `ResultEntry` to
   `src/core/registry.rs`, each with their own `RegistryTable`,
   `insert_*`/`get_*_by_uuid`/`get_*_by_name` methods, following the
   overwrite-by-name pattern already used for `ConstraintEntry` and
   `ConstraintSetEntry`.
3. Add `HandleKind::Obj`, `HandleKind::Prob`, and `HandleKind::Result` to
   `src/core/handle.rs` (`"obj"`, `"prob"`, `"result"`), with round-trip unit
   tests, and extend every existing exhaustive `match kind { ... }` over
   `HandleKind` (in `src/analytics/resolve.rs` and `src/excel/expression.rs`)
   with arms rejecting these new kinds as invalid expression operands.
4. Add `src/excel/problem.rs` exporting `CVX.MINIMIZE`, `CVX.MAXIMIZE`,
   `CVX.PROBLEM`, and `CVX.SOLVE`, following the
   `run_*`/`to_xloper_result` structure from `src/excel/expression.rs` and
   `src/excel/constraint.rs`:
   - `CVX.MINIMIZE`/`CVX.MAXIMIZE` reuse `resolve_operand` (SPEC-0005) to
     resolve the objective, then call a new `Registry::insert_objective`.
   - `CVX.PROBLEM` resolves the `objective` argument to a `cvx:obj:` UUID
     (rejecting other handle kinds), resolves `constraints` per the rules
     above (reusing/extending the SPEC-0005 constraint-set flattening logic,
     relaxed to allow an empty result), walks the objective's and each
     constraint's `dependencies` to build the deduplicated
     `ProblemEntry::variables` list, and calls a new
     `Registry::insert_problem`.
   - `CVX.SOLVE` looks up the `ProblemEntry`, looks up its `ObjectiveEntry`
     and each `ConstraintEntry`, translates `analytics::ast::Relation` to
     `cvxrust::Relation`, builds a `cvxrust::Problem` (with `variables`
     populated from the corresponding `VariableEntry::variable` values, in
     `ProblemEntry::variables` order), and calls `cvxrust::solve`. On
     `SolveStatus::Error`, returns an Excel error without touching the
     registry. Otherwise, zips `ProblemEntry::variables` with
     `Solution::variable_values` into `ResultEntry::variable_values` and
     calls a new `Registry::insert_result`.
5. Register all four new functions in the XLL entry point (`xlAutoOpen` in
   `src/excel/mod.rs`), matching the existing registration pattern, and
   explicitly omitting the volatile (`!`) flag on all of them.
6. Add a new `docs/problems.md` describing `CVX.MINIMIZE`/`CVX.MAXIMIZE`,
   `CVX.PROBLEM`, and `CVX.SOLVE` for end users, including a note that
   `CVX.SOLVE` currently always returns `#VALUE!` until `cvxrust` gains a
   real solver.
7. Add `#[cfg(test)]` unit tests for `cvxrust`, the registry, and the
   Excel-facing functions, matching the coverage described above.

## Status

Implemented as specified, with these notes:

- `cvxrust::Constraint`/`Problem`/`SolveStatus`/`Solution`/`solve` and
  `Sense`/`Relation` were added exactly as described, with the placeholder
  `solve` always returning `SolveStatus::Error("not implemented")`.
- `HandleKind::Obj`/`Prob`/`Result` were added; all three previously
  exhaustive `match kind` sites (`src/analytics/resolve.rs`,
  `src/excel/expression.rs` x2) were extended to reject them as expression
  operands.
- `ObjectiveEntry`, `ProblemEntry`, and `ResultEntry` were added to the
  registry with overwrite-by-name insertion, matching `ConstraintEntry`.
- `src/excel/constraint.rs`'s `resolve_operand` and `resolve_constraint_uuid`
  were changed from private to `pub(crate)` so `src/excel/problem.rs` could
  reuse them, per the spec's intent.
- `CVX.PROBLEM`'s variable discovery walks `dependencies` transitively
  through named expressions (an expression dependency is expanded into that
  expression's own `dependencies`), so variables referenced through a named
  sub-expression are found even though only the outer expression's name is
  recorded as a dependency. **Known limitation**: `dependencies` is only
  populated by the string-parsing builders (`CVX.EXPRESSION`,
  `CVX.CONSTRAINT`); the functional builders (`CVX.ADD`, `CVX.LESS_THAN`,
  `CVX.MINIMIZE`, etc.) always pass an empty dependency list, matching their
  existing SPEC-0004/0005 behavior. Variables referenced only through those
  functional-builder paths are therefore not discovered by `CVX.PROBLEM`'s
  variable-collection step. This pre-existing gap was not in scope to fix
  here; a future spec could close it by having the functional builders
  compute and store dependencies too.
- A new `CvxError::SolveFailed(String)` variant represents a
  `SolveStatus::Error` outcome; it is logged via `tracing::error!` and
  translated to `#VALUE!` like every other `CvxError` variant.
- All 87 unit tests pass; `cargo fmt` and `cargo clippy --all-targets` are
  clean.

