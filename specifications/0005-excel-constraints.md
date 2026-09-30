---
id: SPEC-0005
title: Build optimization constraints and constraint sets from Excel formulas
issue: ISSUE-0005
status: implemented
created: 2026-09-30
---

## Objective

Implement Excel-facing functions that let users build convex optimization
constraints from strings or from existing expression handles, and combine
multiple constraints into a single constraint set handle that can later be
passed to the problem builder (SPEC-0006). Constraints are stored lazily in
the Rust registry and returned as opaque string handles, consistent with the
parameter, variable, and expression handles defined in SPEC-0002, SPEC-0003,
and SPEC-0004.

## Non-Objective

- No problem construction or solver invocation (deferred to SPEC-0006).
- No variable bounds shorthand (e.g., `CVX.VARIABLE` bound arguments); bounds
  are expressed as ordinary constraints per the parent issue's notes.
- No result inspection of constraint dual values (deferred to SPEC-0007).
- No shape/broadcast validation beyond what `cvxrust::Expression` already
  performs; elementwise vs. scalar constraint semantics are inherited from
  the expression layer and not re-specified here.
- No ribbon or XLAM changes.

## Interface

### Excel functions

```
CVX.CONSTRAINT(constraint_string, [name])
CVX.LESS_THAN(left, right, [name])
CVX.GREATER_THAN(left, right, [name])
CVX.EQUAL(left, right, [name])
CVX.CONSTRAINTS(constraints, [name])
```

- `constraint_string`: a string containing a relational expression, e.g.
  `"x + y <= 10"`.
- `left`, `right`: string handles or names of existing parameters,
  variables, or expressions (resolved the same way as `CVX.ADD` and friends
  in SPEC-0004), or numeric literals.
- `constraints`: a single range argument, matching the `range` argument
  taken by `CVX.PARAMETER`. It may be a single cell or a multi-cell range,
  and each cell is expected to contain either a `cvx:constr:<uuid>` handle,
  a registered constraint name, or be blank. Excel's own array-construction
  functions (e.g. wrapping several individually-typed handle cells with
  `HSTACK`/`VSTACK`) can be used to combine handles that do not already sit
  in one contiguous range.
- `name`: optional string identifier supplied as the last argument. If
  omitted, the object has no user-visible name and can only be referenced by
  its handle.
- Returns: a string handle. `CVX.CONSTRAINT`, `CVX.LESS_THAN`,
  `CVX.GREATER_THAN`, and `CVX.EQUAL` return `cvx:constr:<uuid>`.
  `CVX.CONSTRAINTS` returns `cvx:constrset:<uuid>`.

### Constraint string grammar

Constraint strings extend the SPEC-0004 expression grammar with a single
top-level relational operator:

```
constraint := expr relop expr
relop      := '<=' | '>=' | '=='
expr       := (as defined in SPEC-0004: constants, identifiers, +, -, *, /,
               parentheses, unary minus)
```

- Exactly one relational operator is allowed per constraint string; it must
  not appear nested inside parentheses or on both sides.
- Whitespace around the operator is optional.
- The left- and right-hand sides are parsed and resolved using the same
  identifier resolution rules as `CVX.EXPRESSION` (registered parameters,
  variables, and named expressions).

Examples of valid constraint strings:

```
"x + y <= 10"
"profit >= cost * 1.1"
"A == b"
```

### Functional builders

`CVX.LESS_THAN`, `CVX.GREATER_THAN`, and `CVX.EQUAL` compose a constraint
from two existing handles (or numeric literals) without parsing a string:

- `left`, `right`: a `cvx:param:`, `cvx:var:`, or `cvx:expr:` handle, a
  registered name, or a bare numeric literal (treated as a constant
  expression, matching `CVX.SCALE`'s scalar argument handling).
- `CVX.LESS_THAN(left, right)` builds `left <= right`.
- `CVX.GREATER_THAN(left, right)` builds `left >= right`.
- `CVX.EQUAL(left, right)` builds `left == right`.

### Constraint sets

`CVX.CONSTRAINTS` combines one or more constraint handles into a single
constraint set handle:

- Accepts a single range argument, flattened row-major into an ordered list
  of resolved constraint UUIDs.
- Blank cells and empty strings within the range are ignored; a non-blank
  cell that does not resolve to a known constraint handle or name is an
  error.
- At least one constraint must resolve; an empty result is an error.
- Returns a `cvx:constrset:<uuid>` handle referencing the ordered list of
  resolved constraint UUIDs.
- Reusing a name overwrites the previous constraint set entry, matching the
  overwrite-by-name convention used for variables, expressions, and
  constraints (so editing and recalculating Excel formulas is convenient).
  Constraint sets are not content-addressed.

### Registry entries

Two new registry tables are added, following the `RegistryTable<T>` pattern
established in SPEC-0002/SPEC-0003/SPEC-0004:

```rust
pub struct ConstraintEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub relation: Relation,       // LessEqual | GreaterEqual | Equal
    pub lhs: Expression,
    pub rhs: Expression,
    pub dependencies: Vec<String>,
}

pub struct ConstraintSetEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub constraints: Vec<Uuid>,   // ordered, references into the constraint table
}
```

- `Relation` is a small enum (`LessEqual`, `GreaterEqual`, `Equal`) added to
  `src/analytics/ast.rs` alongside the expression AST.
- The registry supports lookup by UUID or by name for both new tables,
  mirroring `get_expression_by_uuid` / `get_expression_by_name`.
- Reusing a name for any of `CVX.CONSTRAINT`/`CVX.LESS_THAN`/
  `CVX.GREATER_THAN`/`CVX.EQUAL`/`CVX.CONSTRAINTS` overwrites the previous
  entry (same convention as variables and expressions, to support editable
  Excel formulas).

### Handle format

All handles follow the pattern `cvx:<kind>:<uuid>` established in
SPEC-0002. Two new `HandleKind` variants are added:

- `HandleKind::Constr` → `"constr"`
- `HandleKind::ConstrSet` → `"constrset"`

## Data Model

- Input: a constraint string plus optional name; or two expression
  handles/names/literals, a relation, and optional name; or a list of
  constraint handles/ranges plus optional name.
- Parsed input (string path): an AST with a top-level `Relation` node and
  two `Expr` operand subtrees (reusing the SPEC-0004 `Expr` AST).
- Resolved input: `lhs`/`rhs` as `cvxrust::Expression` values, with combined
  dependency list from both sides.
- Stored object: `ConstraintEntry` or `ConstraintSetEntry` in the registry.
- Output: string handle.

## Error Handling

- Invalid constraint string syntax (missing or duplicated relational
  operator, malformed operand) → `#VALUE!` with a logged diagnostic via
  `CvxError::InvalidExpression`.
- Unknown identifier in either operand → `#VALUE!` via
  `CvxError::UnknownIdentifier`, naming the unresolved identifier.
- Identifier found but referring to an incompatible object kind (e.g., a
  problem handle) → `#VALUE!` via `CvxError::UnknownIdentifier`.
- Duplicate name on a constraint or constraint set → `#VALUE!` via
  `CvxError::DuplicateName`.
- `CVX.CONSTRAINTS` argument that is not a constraint handle, not a known
  constraint name, and not blank → `#VALUE!` via
  `CvxError::UnknownIdentifier`.
- `CVX.CONSTRAINTS` called with no resolvable constraints → `#VALUE!` via a
  new `CvxError::EmptyRange`-equivalent path (reuse `CvxError::EmptyRange`).
- Internal registry failure (lock poisoned) → `#VALUE!` via
  `CvxError::Registry`, matching the existing pattern in `registry.rs`.

All errors are surfaced to Excel as `#VALUE!`-style string results using the
existing `to_xloper_result` helper pattern from `src/excel/expression.rs`;
detailed diagnostics are logged with `tracing::error!`, not written to the
worksheet.

## Test Approach

- Unit tests for the constraint string parser: `<=`, `>=`, `==` at the top
  level; rejection of missing operator, multiple operators, and operators
  nested in parentheses.
- Unit tests for identifier resolution reusing SPEC-0004 fixtures (named
  parameters, variables, expressions) on both sides of a constraint.
- Unit tests for the functional builders: `CVX.LESS_THAN`, `CVX.GREATER_THAN`,
  `CVX.EQUAL` with handle, name, and numeric-literal operands.
- Unit tests for the registry: constraint insertion, lookup by UUID/name,
  duplicate-name rejection and overwrite-by-name behavior.
- Unit tests for `CVX.CONSTRAINTS`: combining individual handles, combining a
  flattened range, skipping blanks, rejecting an unresolvable entry,
  rejecting an empty result, and content-addressed reuse for an identical
  ordered list and name.
- Unit tests for error cases: invalid syntax, unknown identifier, duplicate
  name, empty constraint set.
- Integration test: a sample workbook builds constraints with
  `CVX.CONSTRAINT` and a functional builder, combines them with
  `CVX.CONSTRAINTS`, and confirms the returned handles match the expected
  prefixes (`cvx:constr:` and `cvx:constrset:`).

## Dependencies

- SPEC-0002 for the registry/handle conventions and content-addressed
  insertion pattern.
- SPEC-0003 for variable handles.
- SPEC-0004 for the expression AST, parser, and identifier resolution that
  constraint operands reuse.
- `xladd` for XLL function registration and `XLOPER12` access, including
  variadic/range argument handling for `CVX.CONSTRAINTS`.
- `cvxrust` for the underlying expression representation; no new `cvxrust`
  types are required beyond `Expression`, since the relation itself is
  tracked in the `cvxx` registry, not in `cvxrust`.

## Implementation Notes

1. Extend `src/analytics/ast.rs` with a `Relation` enum (`LessEqual`,
   `GreaterEqual`, `Equal`) and a `Constraint { relation, lhs, rhs }` AST
   node composed of two `ExprNode`s.
2. Extend `src/analytics/parser.rs` with a constraint-level entry point
   (e.g., `parse_constraint`) that scans the token stream for exactly one
   top-level `<=`, `>=`, or `==` token, parses the operand expressions with
   the existing `parse_expr` logic, and rejects strings with zero or more
   than one top-level relational operator.
3. Extend `src/analytics/resolve.rs` to resolve both operands of a
   `Constraint` node the same way `resolve_expr` resolves `Expr` nodes,
   returning combined dependencies.
4. Add `ConstraintEntry` and `ConstraintSetEntry` to `src/core/registry.rs`,
   each with their own `RegistryTable`, `insert_constraint`,
   `get_constraint_by_uuid`, `get_constraint_by_name`, `insert_constraint_set`,
   `get_constraint_set_by_uuid`, and `get_constraint_set_by_name` methods,
   following the existing variable/expression insertion and overwrite-by-name
   conventions.
5. Add `HandleKind::Constr` and `HandleKind::ConstrSet` to
   `src/core/handle.rs`, with `as_str()` returning `"constr"` and
   `"constrset"` respectively, and corresponding round-trip unit tests.
6. Add `src/excel/constraint.rs` exporting `CVX.CONSTRAINT`,
   `CVX.LESS_THAN`, `CVX.GREATER_THAN`, `CVX.EQUAL`, and `CVX.CONSTRAINTS`,
   following the `run_expression`/`run_binary`/`to_xloper_result` structure
   already used in `src/excel/expression.rs`. Reuse `resolve_handle_arg`-style
   logic (extracted to a shared helper if convenient) for resolving
   `left`/`right` operands.
7. Implement `CVX.CONSTRAINTS` argument handling using `xladd`'s multi-value
   array support to accept a mix of scalar handle arguments and range
   arguments; flatten row-major and skip blank/empty cells before resolving
   each entry to a constraint UUID by handle or name.
8. Register all five new functions in the XLL entry point (`xlAutoOpen` in
   `src/excel/mod.rs`), matching the existing registration pattern for
   expression functions.
9. Update `docs/expressions.md` or add a new `docs/constraints.md` describing
   the constraint string grammar and functional builders for end users.
10. Add `#[cfg(test)]` unit tests for the parser, resolver, registry, and
    Excel-facing functions, matching the coverage described above.

## Status

Implemented. `Relation` and `Constraint` were added to
`src/analytics/ast.rs`; `src/analytics/parser.rs` extends the tokenizer with
`<=`, `>=`, and `==` and adds `parse_constraint`, which naturally rejects
nested or repeated relational operators because the expression grammar does
not recognize them. `src/analytics/resolve.rs` adds `resolve_constraint`,
reusing the existing identifier resolver. `ConstraintEntry` and
`ConstraintSetEntry` were added to `src/core/registry.rs` with their own
`RegistryTable`s and overwrite-by-name insertion, matching the
variable/expression convention; `HandleKind::Constr`
(`cvx:constr:`) and `HandleKind::ConstrSet` (`cvx:constrset:`) were added to
`src/core/handle.rs`. `src/excel/constraint.rs` exports `CVX.CONSTRAINT`,
`CVX.LESS_THAN`, `CVX.GREATER_THAN`, `CVX.EQUAL`, and `CVX.CONSTRAINTS`,
registered in `xlAutoOpen` in `src/excel/mod.rs`. `CVX.CONSTRAINTS` takes a
single `range` argument (like `CVX.PARAMETER`) rather than variadic
arguments, since `xladd`'s fixed-arity XLL registration does not support
true variadic parameters; a new `data::parse_optional_string_range` helper
flattens the range row-major and treats blanks as `None`. Relational
operands accept numeric literals in addition to handles/names via a new
`resolve_operand` helper that tries `data::parse_scalar` before falling back
to the existing `resolve_handle_arg` (now `pub(crate)` and shared from
`src/excel/expression.rs`, along with `to_xloper_result`). User
documentation is in `docs/constraints.md`. 26 new unit tests cover parsing,
resolution, registry behavior, and handle resolution, for 71 total; `cargo
fmt` and `cargo clippy --all-targets` are clean. Integration testing against
a live Excel workbook is still pending and requires a Windows machine with
Excel installed.
