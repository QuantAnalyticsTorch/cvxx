---
id: SPEC-0013
title: Prefer registered names over placeholders inside rendered expression bodies
issue: ISSUE-0013
status: implemented
created: 2026-10-01
---

## Status

Implemented: `cvxrust::Expression::Parameter` gained an opaque `id` field
(mirroring `Variable::id`); `Registry` gained
`get_variable_by_variable_id`/`get_parameter_by_parameter_id`;
`render_expression` (`src/analytics/shape.rs`) now takes a `&Registry` and
prefers a leaf's registered name when one exists. All call sites
(`src/analytics/resolve.rs`, `src/excel/expression.rs`,
`src/excel/inspect.rs`) and test literals were updated accordingly.
`docs/inspection.md` documents the new behavior with an example. `cargo
fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo
test --workspace` all pass (35 `cvxrust` tests, 137 `cvxx` tests).

## Objective

Change `render_expression` (SPEC-0007, `src/analytics/shape.rs`) so that
each `Variable`/`Parameter` leaf it renders shows the referenced object's
registered short name — quoted, as already done for cross-references in
SPEC-0012 — when the registry currently has a name for it, instead of the
structural `var#<id>`/`param(<rows>x<cols>)` placeholder. An object without
a current registered name renders exactly as it does today. This applies
everywhere `render_expression` is used: `CVX.DESCRIBE` output for
expressions, constraints (both operands), and objectives
(`describe_body`, `src/excel/inspect.rs`).

Because `cvxrust::Expression::Variable` already carries an opaque identity
(`Variable::id`, SPEC-0010) that the registry can map back to a
`VariableEntry`, resolving a variable's name only requires a new
registry lookup — no change to `cvxrust`. `cvxrust::Expression::Parameter`
carries no such identity today (only `shape` and `data`), so a parameter
leaf cannot currently be mapped back to the `ParameterEntry` it came from
at all once embedded in an expression tree. This specification closes that
gap by giving `Expression::Parameter` the same kind of opaque identity
`Expression::Variable` already has.

## Non-Objective

- No change to how users assign or remove names, or to the cross-table
  name uniqueness invariant (SPEC-0002) — this specification only adds a
  read-time name lookup.
- No change to the handle format (`format_handle`/`parse_handle`,
  `src/core/handle.rs`) or to any `CVX.*` function signature. `CVX.SHAPE`,
  `CVX.TYPE`, `CVX.VALUE`, `CVX.STATUS`, and `CVX.OBJECTIVE_VALUE` are
  unaffected (unchanged by SPEC-0012 for the same reason: none of them
  render a handle, name, or formula).
- No change to `CVX.EXPRESSION`'s parser, to `resolve_expr`/
  `resolve_constraint` (`src/analytics/resolve.rs`), or to how an AST
  identifier is resolved into an `Expression` — this specification changes
  what already-resolved `Expression` trees carry as *identity* and what
  `render_expression` *displays*, not identifier resolution rules.
  `render_expression`'s output continues to not be guaranteed to round-trip
  through `CVX.EXPRESSION`'s parser (SPEC-0007's Non-Objective): a quoted
  name is shown for readability only, and is not itself re-parsable syntax
  if it contains characters the parser's identifier grammar rejects.
- No change to `cvxrust`'s solving behavior, numerical results, or
  `clarabel` translation (`validate_shapes`, `check_expr_shapes`,
  `quadratize`, `QuadraticForm`). The new `Expression::Parameter` identity
  field is opaque to the solver, exactly as `Variable::id` already is for
  non-identity purposes — both are only ever read for *comparison*
  (`Variable::id` against the problem's variable index; the new parameter
  id against the registry), never arithmetically.
- No disambiguation beyond an exact identity match. If a `Parameter` or
  `Variable` leaf's id no longer corresponds to any current registry entry
  (see Error Handling), the placeholder is shown — same graceful
  degradation already established by SPEC-0012's `display_ref`. Two
  different registry objects are never conflated: SPEC-0002's cross-table
  name uniqueness means a resolvable name is always unambiguous.
- No change to `CVX.DESCRIBE`'s truncation rule (`MAX_DESCRIBE_LEN = 500`,
  SPEC-0007), which continues to apply unchanged to the new, generally
  shorter output.

## Interface

### `cvxrust` (`cvxrust/src/lib.rs`): `Expression::Parameter` gains an id

```rust
pub enum Expression {
    ...
    /// A reference to a parameter (dense numeric data).
    Parameter {
        /// Opaque identity distinguishing this parameter reference from any
        /// other, even one of the same shape and data. Not used by the
        /// solver; read only for display purposes by `cvxx`.
        id: u64,
        shape: (usize, usize),
        data: Vec<f64>,
    },
    ...
}

impl Expression {
    /// Creates a parameter expression with the given opaque identity.
    pub fn from_parameter(id: u64, shape: (usize, usize), data: Vec<f64>) -> Self {
        Expression::Parameter { id, shape, data }
    }
}
```

`check_expr_shapes` and `quadratize` (both in `cvxrust/src/lib.rs`) already
destructure `Expression::Parameter { shape, .. }` / `{ data, .. }` with
`..`, so neither needs to change: the added field is ignored by both.

Every existing call site that constructs an `Expression::Parameter` (either
via `Expression::from_parameter(..)` or a literal `Expression::Parameter {
.. }`) is updated to supply an id:

- `src/analytics/resolve.rs` (`resolve_identifier`, `resolve_handle`): pass
  `parameter_id(entry.uuid)` (new helper, below) using the `Uuid` already
  available on the resolved `ParameterEntry`.
- `src/excel/expression.rs` (dependency-shape inference and inlining,
  lines constructing `Expression::from_parameter(e.shape, e.data)` /
  `Expression::from_parameter(entry.shape, entry.data)`): same, using the
  `Uuid` already available on the corresponding `ParameterEntry`.
- Existing unit test literals in `cvxrust/src/lib.rs`,
  `src/analytics/shape.rs`, `src/analytics/resolve.rs`, and
  `src/excel/expression.rs` that construct a bare `Expression::from_parameter`
  without a registry round-trip: pass any fixed `u64` test id (the specific
  value is immaterial — these tests do not exercise name lookup).

### `src/core/registry.rs`: id ↔ entry lookups

```rust
/// Derives the opaque `cvxrust` identity for the parameter stored at
/// `uuid`, by the same convention already used for
/// `cvxrust::Variable::id` (the uuid's first 64 bits).
fn parameter_id(uuid: Uuid) -> u64 {
    uuid.as_u64_pair().0
}
```

`insert_variable` keeps constructing `Variable::new(uuid.as_u64_pair().0,
shape)` exactly as today (unchanged); the new `parameter_id` helper is used
only at the two call sites listed above (`resolve.rs`, `expression.rs`),
not inside `insert_parameter` itself — `ParameterEntry` stores `uuid`
already, and the id is a pure, cheap function of it, so no new field is
added to `ParameterEntry`.

```rust
impl Registry {
    /// Finds the variable entry whose `cvxrust`-level identity is `id`, if
    /// one currently exists. Diagnostic-only; not performance-critical
    /// (SPEC-0007), so a linear scan over the variable table is
    /// sufficient — no new index is maintained.
    pub fn get_variable_by_variable_id(&self, id: u64) -> Option<VariableEntry> {
        self.variables
            .read()
            .ok()?
            .by_uuid
            .values()
            .find(|e| e.variable.id == id)
            .cloned()
    }

    /// Finds the parameter entry whose derived `cvxrust`-level identity
    /// (`parameter_id`) is `id`, if one currently exists. Diagnostic-only;
    /// same linear-scan rationale as `get_variable_by_variable_id`.
    pub fn get_parameter_by_parameter_id(&self, id: u64) -> Option<ParameterEntry> {
        self.parameters
            .read()
            .ok()?
            .by_uuid
            .values()
            .find(|e| parameter_id(e.uuid) == id)
            .cloned()
    }
}
```

(Exact field/accessor names for the internal `by_uuid` table follow
whatever `get_variable_by_uuid`/`get_parameter_by_uuid` already use
internally in this file; the signatures and behavior above are normative,
the internal traversal is not.)

### `src/analytics/shape.rs`: `render_expression` takes the registry

```rust
/// Renders an `Expression` as a fully-parenthesized diagnostic string for
/// `CVX.DESCRIBE`. Not guaranteed to round-trip through `CVX.EXPRESSION`'s
/// parser. A `Variable`/`Parameter` leaf shows its current registered name
/// (quoted) when `registry` has one, else the structural placeholder.
pub fn render_expression(expr: &Expression, registry: &Registry) -> String {
    match expr {
        Expression::Constant(c) => format!("{c}"),
        Expression::Variable(v) => registry
            .get_variable_by_variable_id(v.id)
            .and_then(|e| e.name)
            .map(|name| format!("\"{name}\""))
            .unwrap_or_else(|| format!("var#{}", v.id)),
        Expression::Parameter { id, shape, .. } => registry
            .get_parameter_by_parameter_id(*id)
            .and_then(|e| e.name)
            .map(|name| format!("\"{name}\""))
            .unwrap_or_else(|| format!("param({}x{})", shape.0, shape.1)),
        Expression::Add(l, r) => format!(
            "{} + {}",
            render_expression(l, registry),
            render_expression(r, registry)
        ),
        Expression::Sub(l, r) => format!(
            "{} - ({})",
            render_expression(l, registry),
            render_expression(r, registry)
        ),
        Expression::Mul(l, r) => format!(
            "({}) * ({})",
            render_expression(l, registry),
            render_expression(r, registry)
        ),
        Expression::Div(l, r) => format!(
            "({}) / ({})",
            render_expression(l, registry),
            render_expression(r, registry)
        ),
        Expression::Neg(e) => format!("-({})", render_expression(e, registry)),
        Expression::Scale { scalar, expr } => {
            format!("{scalar} * ({})", render_expression(expr, registry))
        }
    }
}
```

`infer_shape` (same file) is unaffected — it does not render identifiers
and needs no registry access.

### `src/excel/inspect.rs`: callers pass the registry

The four `render_expression(&e.expression)` / `render_expression(&e.lhs)` /
`render_expression(&e.rhs)` call sites inside `describe_body`'s
`Expression`, `Constraint`, and `Objective` arms are updated to
`render_expression(&e.expression, Registry::global())` (and similarly for
`e.lhs`/`e.rhs`), matching the pattern already used by `display_ref`
(SPEC-0012) for registry access inside this module.

## Data Model

No new registry tables or stored fields. `ParameterEntry` and
`VariableEntry` are unchanged; `parameter_id`/`get_parameter_by_parameter_id`
derive everything from the existing `uuid` field.

Example output changes (illustrative, not exhaustive), reproducing the
originally reported case:

- Before: `"total" (cvx:expr:...): expression 1x1 = (var#16880882605295944462) * (var#16880882605295944462) + var#3771668566231105626`
- After, assuming the repeated variable is named `"x"` and the other is
  named `"y"`: `"total" (cvx:expr:...): expression 1x1 = ("x") * ("x") + "y"`
- After, assuming neither variable is named: output is unchanged from
  today — `"total" (cvx:expr:...): expression 1x1 = (var#...) * (var#...) + var#...`.
- A constraint with a named left-hand variable `"x"` and an unnamed
  right-hand parameter: `constraint: "x" <= param(1x1)`.
- A problem's objective referencing a named parameter `"budget"` and an
  unnamed variable: `objective minimize: ("budget") * (var#7)`.

A variable or parameter referenced twice in the same formula (e.g. `x * x`)
renders identically at both occurrences, since both leaves carry the same
`id` and are looked up independently but identically.

## Error Handling

No new error conditions; `render_expression` cannot fail (unchanged
signature return type: plain `String`, no `Result`). `get_variable_by_variable_id`/
`get_parameter_by_parameter_id` returning `None` (an id with no current
registry entry — e.g. a variable whose name was reused via
`insert_variable`'s overwrite-on-same-name behavior, which removes the old
entry) degrades to the existing placeholder, matching SPEC-0012's
graceful-degradation philosophy. A variable or parameter whose name is
changed or removed is picked up correctly on the *next* `CVX.DESCRIBE`
call, since lookup happens at render time against live registry state —
`render_expression` performs no caching.

## Test Approach

Unit tests in `cvxrust/src/lib.rs`'s existing test module: update literal
`Expression::from_parameter`/`Expression::Parameter` constructions to
include an id; add/keep at least one test confirming `quadratize`/
`check_expr_shapes` output is unaffected by the id's value (regression:
solver behavior unchanged for both named/registered-looking and arbitrary
ids).

Unit tests in `src/core/registry.rs`'s existing test module:

- `get_variable_by_variable_id` finds the entry for a just-inserted named
  or unnamed variable by its `variable.id`, and returns `None` for an
  id that was never inserted.
- `get_variable_by_variable_id` returns `None` for the id of a variable
  whose name was later reused by `insert_variable` (the old entry was
  removed by the existing overwrite behavior).
- `get_parameter_by_parameter_id` finds the entry for a just-inserted named
  or unnamed parameter by its derived `parameter_id(uuid)`, and returns
  `None` for an id that was never inserted.

Unit tests in `src/analytics/shape.rs`'s existing test module, extended
with a `Registry` fixture per test:

- `render_expression` shows the quoted name for a registered, named
  variable and for a registered, named parameter.
- `render_expression` on an unregistered/unnamed variable or parameter
  produces byte-for-byte identical output to today's placeholder format
  (regression check against the SPEC-0007 format for the no-name case).
- A nested expression mixing named and unnamed variables/parameters
  renders each leaf independently and correctly, in the same structural
  format (parenthesization, operators) as today.
- The same variable or parameter referenced twice in one expression (e.g.
  `x * x`) renders identically at both occurrences.

Unit tests in `src/excel/inspect.rs`'s existing test module:

- `CVX.DESCRIBE` on an expression/constraint/objective built from a mix of
  named and unnamed variables/parameters shows names in the formula body
  where available and placeholders elsewhere, reproducing the shape of the
  originally reported case.
- Truncation (`MAX_DESCRIBE_LEN`) still applies correctly to the new,
  generally shorter output.

## Dependencies

- SPEC-0007 (result inspection, `render_expression`, and `CVX.DESCRIBE`) —
  this specification amends `render_expression`'s signature and the
  `describe_body` call sites in place.
- SPEC-0010 (convex solver backend) — establishes `cvxrust::Variable::id`
  as the existing precedent this specification mirrors for
  `Expression::Parameter`.
- SPEC-0012 (registered names over handles in `CVX.DESCRIBE`) — establishes
  the quoted-name display convention and the `Registry::global()`-inside-
  `inspect.rs` access pattern this specification reuses.
- ISSUE-0013 (parent issue).

Documentation: `docs/inspection.md`'s `CVX.DESCRIBE` description (the
paragraph noting "variables are shown as `var#<id>` and parameters as
`param(<rows>x<cols>)`") is updated to state that a variable or parameter
with a current registered name is shown by that name instead, with one
short example formula showing a mix of named and unnamed references.
