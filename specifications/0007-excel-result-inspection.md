---
id: SPEC-0007
title: Inspect solved results and describe any registry handle
issue: ISSUE-0007
status: implemented
created: 2026-09-30
---

## Objective

Implement Excel-facing functions that read solved variable/objective values
out of a `cvx:result:<uuid>` (`CVX.VALUE`, `CVX.STATUS`,
`CVX.OBJECTIVE_VALUE`), and functions that describe **any** registry handle
regardless of kind — parameter, variable, expression, constraint,
constraint set, objective, problem, or result (`CVX.DESCRIBE`, `CVX.SHAPE`,
`CVX.TYPE`). "Every object should allow for display" (this issue's guiding
requirement) means `CVX.DESCRIBE` and `CVX.TYPE` are implemented once,
generically, against all eight `HandleKind` variants, not just results.

## Non-Objective

- Dual values and sensitivity analysis (explicitly deferred by
  `ISSUE-0007`'s Notes to a future issue).
- Any change to how `CVX.SOLVE` or the registry stores results (SPEC-0006 is
  unchanged).
- Enforcing or changing expression shape rules at *construction* time
  (`CVX.EXPRESSION`, `CVX.ADD`, etc. from SPEC-0004 are unchanged and still
  perform no shape validation). The shape-inference helper introduced here
  (`infer_shape`) is a read-only, display-time utility used only by
  `CVX.SHAPE`/`CVX.DESCRIBE`; it never rejects or mutates a stored
  `ExpressionEntry`.
- Re-parseable expression rendering. The human-readable rendering used by
  `CVX.DESCRIBE` is for diagnostic display only (see Data Model); it is not
  guaranteed to round-trip through `CVX.EXPRESSION`'s parser, and original
  identifier names are not recoverable (an `Expression` tree carries no
  names, only structural `Variable`/`Parameter` data — see Data Model).
- Excel array output for anything other than `CVX.VALUE`. `CVX.DESCRIBE`,
  `CVX.SHAPE`, `CVX.TYPE`, and `CVX.STATUS` always return a single string
  cell; `CVX.OBJECTIVE_VALUE` always returns a single numeric cell.

## Interface

### Excel functions

```
CVX.VALUE(result, variable)
CVX.STATUS(result)
CVX.OBJECTIVE_VALUE(result)
CVX.DESCRIBE(handle)
CVX.SHAPE(handle)
CVX.TYPE(handle)
```

- `result`: a `cvx:result:<uuid>` handle or registered name produced by
  `CVX.SOLVE`.
- `variable` (in `CVX.VALUE`): a `cvx:var:<uuid>` handle or registered name.
  `CVX.VALUE` takes **two** arguments (a decision the issue's one-argument
  wording left ambiguous): the registry has no notion of "the most recent
  result for a variable" — a variable may appear in several problems solved
  at different times — so the caller must say which solve's values to read.
- `handle` (in `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE`): a handle or registered
  name of **any** kind (`cvx:param:`, `cvx:var:`, `cvx:expr:`, `cvx:constr:`,
  `cvx:constrset:`, `cvx:obj:`, `cvx:prob:`, or `cvx:result:`).
- Returns:
  - `CVX.VALUE`: a single numeric cell when the variable's shape is
    `(1, 1)`; otherwise a `rows x cols` array of numeric cells, row-major,
    matching the variable's shape (per `ISSUE-0007`'s acceptance criterion).
  - `CVX.STATUS`: `"optimal"`, `"infeasible"`, or `"unbounded"` (a
    `ResultEntry` is never stored for any other status — SPEC-0006).
  - `CVX.OBJECTIVE_VALUE`: a numeric cell.
  - `CVX.DESCRIBE`: a string (see Data Model for its format).
  - `CVX.SHAPE`: a string of the form `"<rows>x<cols>"`.
  - `CVX.TYPE`: one of `"parameter"`, `"variable"`, `"expression"`,
    `"constraint"`, `"constraint_set"`, `"objective"`, `"problem"`,
    `"result"`.

### Registration constraints

None of the six functions are registered with the volatile (`!`) flag,
consistent with SPEC-0006/SPEC-0005.

### New shared resolution helpers (`src/excel/inspect.rs`)

```rust
/// One entry from any registry table, used only by CVX.DESCRIBE/CVX.SHAPE/
/// CVX.TYPE, which must work uniformly across every `HandleKind`.
enum RegistryObject {
    Parameter(ParameterEntry),
    Variable(VariableEntry),
    Expression(ExpressionEntry),
    Constraint(ConstraintEntry),
    ConstraintSet(ConstraintSetEntry),
    Objective(ObjectiveEntry),
    Problem(ProblemEntry),
    Result(ResultEntry),
}

/// Resolves `text` (a `cvx:<kind>:<uuid>` handle or a registered name)
/// against every registry table.
fn resolve_any(text: &str) -> Result<RegistryObject, CvxError>;
```

- If `text` starts with `cvx:`, `parse_handle` determines the exact table to
  look up; an unknown UUID is `CvxError::UnknownIdentifier`.
- Otherwise `text` is tried against every table's by-name lookup —
  parameter, variable, expression, constraint, constraint set, objective,
  problem, result. Thanks to SPEC-0002's cross-table name uniqueness
  amendment (every `insert_*` now rejects a name that already exists in a
  *different* table with `CvxError::AmbiguousIdentifier`), a name can exist
  in at most one table at any given time, so at most one of the eight
  lookups can succeed. `resolve_any` still checks all eight defensively
  (rather than assuming the invariant always holds) and returns
  `CvxError::AmbiguousIdentifier(text)` if it ever finds a name in more than
  one table — a belt-and-suspenders check against the invariant being
  violated by a future bug, not the primary mechanism for avoiding
  ambiguity.
- No match in any table → `CvxError::UnknownIdentifier(text.to_string())`.

`CVX.VALUE`, `CVX.STATUS`, and `CVX.OBJECTIVE_VALUE` do **not** use
`resolve_any`; they reuse the existing narrowly-typed pattern from
`src/excel/problem.rs` (e.g. `resolve_problem_uuid`) — a `resolve_result_uuid`
and `resolve_variable_uuid`-style lookup restricted to `cvx:result:`/
`cvx:var:` handles or names, rejecting any other kind as
`CvxError::UnknownIdentifier`.

### Shape inference (`src/analytics/shape.rs`)

```rust
/// Infers the `(rows, cols)` shape of an `Expression` for display purposes
/// only. Never called during expression construction (SPEC-0004) or solving
/// (SPEC-0010); read-only and side-effect-free.
pub fn infer_shape(expr: &cvxrust::Expression) -> Result<(usize, usize), CvxError>;
```

- `Constant(_)` → `(1, 1)`.
- `Variable(v)` → `v.shape`.
- `Parameter { shape, .. }` → `shape`.
- `Add`/`Sub`/`Mul`/`Div`: infer both sides. If either side is `(1, 1)`
  (scalar broadcasting), the result is the *other* side's shape. If neither
  side is `(1, 1)`, the two shapes must be equal (elementwise semantics; no
  matrix multiplication is defined anywhere in `cvxrust`), and the result is
  that shared shape; otherwise
  `CvxError::InvalidExpression("shape mismatch: <a>x<b> vs <c>x<d>")`.
- `Neg(e)` → same shape as `e`.
- `Scale { expr, .. }` → same shape as `expr`.

### Excel-array output helper (`src/excel/inspect.rs`)

```rust
/// Converts a solved variable's values into a scalar or row-major array
/// XLOPER matching `shape`.
fn to_xloper_value_result(
    result: Result<((usize, usize), Vec<f64>), CvxError>,
    context: &str,
) -> LPXLOPER12;
```

- `Ok(((1, 1), values))` → a single numeric `Variant` (`values[0]`).
- `Ok(((rows, cols), values))` with `rows * cols > 1` → an array `Variant`
  built via `Variant::from_array(cols, rows, &cells)` (matching the
  `cols, rows` argument order already used in `src/data/mod.rs`'s tests),
  `cells[row * cols + col]` taken row-major from `values`.
- `Err(err)` → logged via `tracing::error!`, and an Excel `#VALUE!` error
  `Variant` (`Variant::from_err(xlerrValue)`), matching the
  `xlerrValue`-based error convention already used by
  `src/excel/variable.rs::cvx_variable` (as opposed to
  `src/excel/expression.rs::to_xloper_result`, which returns the error
  message as plain text — this specification's new functions all use the
  `xlerrValue` convention, since it is the one the parent issue's "return
  `#VALUE!`" acceptance criterion literally describes).

## Data Model

### `CVX.VALUE(result, variable)`

1. Resolve `result` to a `cvx:result:` UUID; look up the `ResultEntry`.
2. Resolve `variable` to a `cvx:var:` UUID; look up the `VariableEntry` (for
   its `shape`).
3. If `result_entry.status != SolveStatus::Optimal`, return
   `CvxError::InvalidExpression("result status is <status>; no variable values are available")`.
4. If `variable_uuid` is not a key of `result_entry.variable_values`, return
   `CvxError::UnknownIdentifier(variable text)` (the variable was not part
   of the solved problem).
5. Otherwise return `Ok((variable_entry.shape, values.clone()))` for
   `to_xloper_value_result`.

### `CVX.STATUS(result)`

Resolve `result`, map `ResultEntry.status` to `"optimal"` / `"infeasible"` /
`"unbounded"` (a helper `status_str(&SolveStatus) -> &'static str`, total
over all four `SolveStatus` variants for reuse by `CVX.DESCRIBE`, even
though `Error(_)` is unreachable for a stored `ResultEntry` — it maps to
`"error"` defensively rather than panicking).

### `CVX.OBJECTIVE_VALUE(result)`

Resolve `result`; if `objective_value` is `Some(v)`, return `v`; if `None`,
return `CvxError::InvalidExpression("result status is <status>; no objective value is available")`.

### `CVX.DESCRIBE(handle)`

Resolve via `resolve_any`. Build a string
`format!("{handle}{name_suffix}: {body}")` where:

- `handle` is `format_handle(kind, uuid)` for the resolved object.
- `name_suffix` is `format!(" (\"{name}\")")` when the entry has a name, else
  empty.
- `body` is kind-specific:
  - **Parameter**: `"parameter {r}x{c}, data=[{csv}]"` — `csv` is
    `data.iter().map(f64::to_string).collect::<Vec<_>>().join(", ")`.
  - **Variable**: `"variable {r}x{c}"`.
  - **Expression**: `"expression {shape_part} = {rendered}"`, where
    `rendered` is `render_expression(&entry.expression)` (below) and
    `shape_part` is `"{r}x{c}"` from `infer_shape`, or the literal string
    `"(shape unavailable: {err})"` when `infer_shape` fails — `CVX.DESCRIBE`
    never fails just because a stored expression's shape can't be inferred;
    it degrades gracefully as this issue's Notes require ("handle large
    expressions gracefully").
  - **Constraint**: `"constraint: {lhs} {op} {rhs}"`, `op` is `"<="`, `">="`,
    or `"="`; `lhs`/`rhs` via `render_expression`.
  - **ConstraintSet**: `"constraint_set: [{n} constraint(s)]: {handles}"`,
    `handles` is the member UUIDs formatted with `format_handle(Constr, _)`,
    comma-joined.
  - **Objective**: `"objective {sense}: {rendered}"`, `sense` is
    `"minimize"`/`"maximize"`.
  - **Problem**: `"problem: objective={obj_handle}, constraints=[{n}]: {c_handles}, variables=[{m}]: {v_handles}"`,
    each `*_handle(s)` formatted the same way as ConstraintSet's.
  - **Result**: `"result: status={status}, objective_value={ov}, variables=[{n}]"`,
    `ov` is the value or the literal `"n/a"` when `None`, `n` is
    `variable_values.len()`.
- **Truncation**: after building the full string, if its length in `char`s
  exceeds `MAX_DESCRIBE_LEN = 500`, truncate to the first 500 `char`s (using
  `.chars().take(500).collect::<String>()`, never splitting a multi-byte
  `char`) and append `"... (truncated)"`. This is the one, uniform
  truncation point for every kind — large parameter data lists and long
  expression renderings are both cut by this same rule rather than needing
  per-field truncation logic.

### Expression rendering (`render_expression`, `src/analytics/shape.rs`)

A diagnostic-only pretty-printer that always fully parenthesizes
multiplicative/unary operands, sidestepping operator-precedence tracking
entirely (unambiguous to implement, more verbose than the input grammar —
acceptable since this output is never re-parsed, per Non-Objective):

- `Constant(c)` → `format!("{c}")` (Rust's default `f64` `Display`).
- `Variable(v)` → `format!("var#{}", v.id)` (original Excel-facing names are
  not recoverable — see Non-Objective — so the `cvxrust::Variable::id`
  introduced by SPEC-0010 is shown instead).
- `Parameter { shape, .. }` → `format!("param({}x{})", shape.0, shape.1)`
  (the parameter's own data is shown by describing the parameter's own
  handle directly, not inlined here, to keep expression renderings
  bounded).
- `Add(l, r)` → `format!("{} + {}", render(l), render(r))`.
- `Sub(l, r)` → `format!("{} - ({})", render(l), render(r))`.
- `Mul(l, r)` → `format!("({}) * ({})", render(l), render(r))`.
- `Div(l, r)` → `format!("({}) / ({})", render(l), render(r))`.
- `Neg(e)` → `format!("-({})", render(e))`.
- `Scale { scalar, expr }` → `format!("{scalar} * ({})", render(expr))`.

### `CVX.SHAPE(handle)`

Resolve via `resolve_any`:

- Parameter/Variable → `"{r}x{c}"` from the stored `shape`.
- Expression → `"{r}x{c}"` from `infer_shape`, propagating its
  `InvalidExpression("shape mismatch: ...")` error as-is (unlike
  `CVX.DESCRIBE`, `CVX.SHAPE` promises "the dimensions of a handle", so a
  genuinely undefined shape is a hard error here).
- Constraint/ConstraintSet/Objective/Problem/Result →
  `CvxError::InvalidExpression("object of type '<type>' has no shape")`,
  where `<type>` is the same string `CVX.TYPE` would return. Shape is only
  meaningful for numeric/matrix-like objects.

### `CVX.TYPE(handle)`

Resolve via `resolve_any`; map the resolved variant to the string listed in
Interface (`"parameter"`, `"variable"`, `"expression"`, `"constraint"`,
`"constraint_set"`, `"objective"`, `"problem"`, `"result"`). This is a total
mapping over all eight `HandleKind`s, extending the six kinds
`ISSUE-0007` names explicitly to also cover `constraint_set` and
`objective` (added by SPEC-0005/SPEC-0006 after the issue was written),
consistent with this specification's "every object" requirement.

## Error Handling

- Unknown/unresolvable handle or name, for any of the six functions →
  `#VALUE!` via `CvxError::UnknownIdentifier`, as an `xlerrValue` `Variant`
  (see the array-output helper's note on the two coexisting error
  conventions in this codebase; every function added by this specification
  uses `xlerrValue`, not the plain-text convention).
- `CVX.VALUE`/`CVX.STATUS`/`CVX.OBJECTIVE_VALUE` given a handle that is not
  `cvx:result:` (or, for `CVX.VALUE`'s second argument, not `cvx:var:`) →
  `#VALUE!` via `CvxError::UnknownIdentifier`.
- `CVX.VALUE`/`CVX.OBJECTIVE_VALUE` on a result whose status is not
  `Optimal` → `#VALUE!` via `CvxError::InvalidExpression` (a real, resolved
  result — not an "unknown identifier" — so a different message than the
  handle-resolution errors, still surfaced as `#VALUE!`).
- `CVX.VALUE` for a variable not part of the solved problem →
  `#VALUE!` via `CvxError::UnknownIdentifier`.
- `CVX.SHAPE` on a non-shaped object kind, or on an expression whose shape
  cannot be inferred → `#VALUE!` via `CvxError::InvalidExpression`.
- `CVX.DESCRIBE` never fails for a handle that resolves to *some* registry
  object, even one with an internally inconsistent expression shape (see
  Data Model); it only fails (`#VALUE!` via `CvxError::UnknownIdentifier`)
  when the handle/name does not resolve at all.
- Registry lock poisoned → `#VALUE!` via `CvxError::Registry`, matching
  every prior specification.
- `docs/problems.md` gains a "## Inspecting results" section and each of
  `docs/variables.md`, `docs/expressions.md`, `docs/constraints.md` gains a
  short cross-reference to `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` under a new
  shared `docs/inspection.md` page (created by this specification)
  documenting all six functions, their argument/return conventions, and
  error conditions.

## Test Approach

- Unit tests for `resolve_any`: resolving a handle and a name for each of
  the eight kinds; unknown handle/name → `UnknownIdentifier`; a name
  artificially forced to collide across two tables (bypassing the normal
  insertion guard, to simulate the invariant being violated) →
  `AmbiguousIdentifier`, exercising `resolve_any`'s defensive check.
- Unit tests for `infer_shape`: scalar constant; bare variable; bare
  parameter; scalar-broadcast `Add`/`Mul` against a non-scalar operand;
  equal-shape elementwise `Add`/`Mul`; mismatched non-scalar shapes →
  `InvalidExpression`; `Neg`/`Scale`/`Sub`/`Div` passthrough/broadcast cases.
- Unit tests for `render_expression`: each node kind individually and a
  nested combination, asserting the exact fully-parenthesized string.
- Unit tests for `CVX.VALUE`: scalar variable → numeric `Variant`;
  multi-element variable → array `Variant` with correct row-major layout;
  non-`Optimal` result → error; variable not in the solved problem → error;
  wrong-kind handles for either argument → error.
- Unit tests for `CVX.STATUS`/`CVX.OBJECTIVE_VALUE`: `Optimal` with a value,
  `Infeasible`/`Unbounded` with `objective_value: None` → error for
  `CVX.OBJECTIVE_VALUE` but a valid string for `CVX.STATUS`.
- Unit tests for `CVX.DESCRIBE`: one test per registry kind asserting the
  exact expected string (including the name-suffix and no-name cases); a
  parameter with data long enough to trigger the 500-char truncation,
  asserting the `"... (truncated)"` suffix and exact length; an expression
  with an inferable and an un-inferable shape (asserting the graceful
  `"(shape unavailable: ...)"` substitution in the latter case).
- Unit tests for `CVX.SHAPE`: parameter/variable/expression success cases;
  constraint/constraint-set/objective/problem/result → `InvalidExpression`;
  an expression with a genuine shape mismatch → `InvalidExpression`.
- Unit tests for `CVX.TYPE`: one assertion per kind, both via handle and via
  name.
- Integration test: a sample workbook builds a small solved problem (as in
  SPEC-0010's integration test) and confirms `CVX.STATUS` returns
  `"optimal"`, `CVX.VALUE` returns the expected scalar/array for each of two
  variables, `CVX.OBJECTIVE_VALUE` matches the known optimum, and
  `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` produce sane output for every handle
  created along the way (parameter, variable, expression, constraint,
  objective, problem, result).

## Dependencies

- SPEC-0002 through SPEC-0006 for every registry table, handle kind, and
  resolution convention this specification builds on and generalizes.
- SPEC-0010 for `cvxrust::Variable::id`, used by `render_expression` to
  label variable leaves, and for the `SolveStatus`/`ResultEntry` values
  `CVX.VALUE`/`CVX.STATUS`/`CVX.OBJECTIVE_VALUE` read.
- `xladd` for `XLOPER12`/`Variant` array construction
  (`Variant::from_array`) and the `xlerrValue` error convention.
- No new external crate dependencies.

## Status

Implemented. Shape inference and expression rendering live in
`src/analytics/shape.rs` (`infer_shape`, `render_expression`), exercised by
unit tests for every node kind, scalar broadcasting, and mismatched-shape
rejection. `RegistryObject`, `resolve_any`, and the six Excel functions
(`CVX.VALUE`, `CVX.STATUS`, `CVX.OBJECTIVE_VALUE`, `CVX.DESCRIBE`,
`CVX.SHAPE`, `CVX.TYPE`) live in `src/excel/inspect.rs`, registered in
`src/excel/mod.rs`'s `xlAutoOpen`. `resolve_any`'s ambiguity branch is
split into a separately-testable `resolve_from_matches` helper, since the
cross-table uniqueness invariant (SPEC-0002) normally prevents more than
one match from ever occurring at insertion time; `resolve_any` still checks
all eight tables defensively rather than relying solely on that invariant.
All six functions use the `xlerrValue` Excel error convention, matching
`src/excel/variable.rs`'s existing pattern (not the plain-text convention
used by `src/excel/expression.rs`). 24 new unit tests cover `resolve_any`
(by handle/name/unknown/forced-ambiguous), `CVX.DESCRIBE` (per kind, no-name
case, 500-char truncation, graceful shape-unavailable degradation),
`CVX.SHAPE` (success and non-shaped-kind rejection), `CVX.TYPE`, and
`CVX.STATUS`/`CVX.OBJECTIVE_VALUE`/`CVX.VALUE` (optimal/non-optimal/
variable-not-in-problem cases). `docs/inspection.md` documents all six
functions; `docs/problems.md`, `docs/variables.md`, `docs/expressions.md`,
and `docs/constraints.md` cross-reference it. `cargo test --workspace`:
146 passed, 0 failed. `cargo fmt` and `cargo clippy --all-targets -- -D
warnings` are clean. Live-Excel integration testing is still pending and
requires a Windows machine with Excel installed (consistent with every
prior specification's Status section).
