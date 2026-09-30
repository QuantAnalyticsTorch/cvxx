---
id: SPEC-0004
title: Build optimization expressions from Excel formulas
issue: ISSUE-0004
status: implemented
created: 2026-09-29
---

## Objective

Implement Excel-facing functions that let users build convex optimization expressions from strings or from existing handles. Expressions are stored lazily in the Rust registry and returned as opaque string handles.

## Non-Objective

- No parameter creation (covered by SPEC-0002).
- No variable creation (covered by SPEC-0003).
- No constraint building, problem construction, or solver invocation.
- No expression evaluation or numeric substitution.
- No transpose syntax (`x'` or `x.T`).
- No general function calls inside expression strings (e.g., `sum_squares`, `norm`); these are deferred to a follow-up issue.
- No ribbon or XLAM changes.

## Interface

### Excel function

```
CVX.EXPRESSION(expr_string, [name])
```

- `expr_string`: a string containing a mathematical expression.
- `name`: optional string identifier supplied as the last argument. If omitted, the expression has no user-visible name and can only be referenced by its handle.
- Returns: a string handle of the form `cvx:expr:<uuid>`.

### Expression string grammar

Expression strings support the following constructs:

- Numeric constants: decimal numbers such as `1`, `2.5`, `-3`.
- Identifier references: names of registered parameters, variables, or other named expressions.
- Binary operators: `+`, `-`, `*`, `/` with standard arithmetic precedence.
- Parentheses for grouping.
- Unary minus for negation.

Whitespace may appear between tokens and is ignored.

Examples of valid expressions:

```
"x + y"
"2.5 * (A - b)"
"-x / 3"
"profit - cost"
```

### Functional builders

In addition to string parsing, the following functions compose expressions from existing handles:

```
CVX.ADD(left, right, [name])
CVX.SUB(left, right, [name])
CVX.MUL(left, right, [name])
CVX.DIV(left, right, [name])
CVX.NEG(operand, [name])
CVX.SCALE(operand, scalar, [name])
```

- `left`, `right`, `operand`: string handles of existing parameters, variables, or expressions.
- `scalar`: a numeric constant.
- `name`: optional string identifier supplied as the last argument.
- Returns: a string handle of the form `cvx:expr:<uuid>`.

### Registry lookup

The Rust registry stores expression entries keyed by a generated UUID. Each entry contains:

- `uuid`: the UUID embedded in the handle.
- `name`: optional user-supplied name (unique within the registry).
- `expression`: a lazy expression object (e.g., an AST or a `cvxrust` expression) that has not been numerically evaluated.
- `dependencies`: the set of parameter/variable/expression handles referenced by the expression.

The registry supports lookup by UUID or by name.

### Handle format

All handles follow the pattern `cvx:<kind>:<uuid>`, where `<kind>` for expressions is `expr`.

## Data Model

- Input: an expression string plus optional name, or existing handles plus optional name.
- Parsed input: a validated expression tree with resolved identifier references.
- Stored object: expression entry in the registry.
- Output: string handle.

## Error Handling

- Reusing a name that is already registered under a **different** object
  table → `#VALUE!` via `CvxError::AmbiguousIdentifier`, per SPEC-0002's
  cross-table name uniqueness amendment. This check runs before any of the
  rules below.
- Invalid expression syntax → `#VALUE!` with a logged diagnostic.
- Unknown identifier → `#VALUE!` with a message naming the unresolved identifier.
- Identifier found but referring to an incompatible object kind (e.g., a problem handle) → `#VALUE!`.
- Duplicate name → `#VALUE!` with message indicating the conflict.
- Type mismatch in functional builders (e.g., `scalar` argument is not numeric) → `#VALUE!`.
- Internal registry failure → `#VALUE!` with logged diagnostic.

## Test Approach

- Unit tests for the parser: numeric constants, identifiers, binary operators with precedence, parentheses, and unary minus.
- Unit tests for name resolution: resolving parameter names, variable names, and named expression handles.
- Unit tests for the functional builders: `ADD`, `SUB`, `MUL`, `DIV`, `NEG`, and `SCALE`.
- Unit tests for the registry: expression insertion, lookup by UUID, lookup by name, duplicate name rejection.
- Unit tests for error cases: invalid syntax, unknown identifier, duplicate name.
- Integration test: a sample workbook builds an expression with `CVX.EXPRESSION` and a functional builder, confirming the returned handles match the expected prefix.

## Dependencies

- `xladd` for XLL function registration and `XLOPER12` access.
- A thread-safe registry implementation, likely `std::sync::RwLock` or `dashmap`.
- `cvxrust` for the underlying expression representation.
- A parser implementation (e.g., a hand-written recursive-descent parser or a crate such as `nom` or `pest`).
- SPEC-0002 for parameter handles and registry conventions.
- SPEC-0003 for variable handles.

## Implementation Notes

1. Extend the module structure defined in SPEC-0002 and SPEC-0003:
   - `src/core/` — add an expression entry type and dependency tracking to the registry.
   - `src/analytics/` — expression AST definition, parser, and conversion to `cvxrust` expressions.
   - `src/excel/` — add the exported XLL functions `CVX.EXPRESSION`, `CVX.ADD`, `CVX.SUB`, `CVX.MUL`, `CVX.DIV`, `CVX.NEG`, and `CVX.SCALE`.
2. Define a small AST for expressions with nodes for constant, parameter reference, variable reference, named expression reference, and the supported binary/unary operators.
3. Implement name resolution against the registry: identifiers that match a registered name are replaced by the corresponding object; identifiers that do not match any name cause a clear error.
4. When an expression string references another expression by its handle or name, store the dependency so the dependency graph can be traversed at solve time.
5. Keep expressions lazy: parsing produces an AST or a `cvxrust` expression object, but no numeric values are substituted.
6. Register all expression functions in the XLL entry point with `xladd`.
7. Add `#[cfg(test)]` unit tests for parser, name resolution, functional builders, and registry behavior.

## Status

Implemented. Added `cvxrust` as a workspace member crate under `cvxrust/`
and wired it into `cvxx` as a path dependency. `CVX.EXPRESSION`, `CVX.ADD`,
`CVX.SUB`, `CVX.MUL`, `CVX.DIV`, `CVX.NEG`, and `CVX.SCALE` are registered in
`src/excel/mod.rs` (`xlAutoOpen`) and exported from
`src/excel/expression.rs`. The expression AST, recursive-descent parser, and
registry name resolver live in `src/analytics/ast.rs`,
`src/analytics/parser.rs`, and `src/analytics/resolve.rs`. Expression entries
(`ExpressionEntry`) are stored in `src/core/registry.rs` with dependency
tracking and lookup by UUID/name. `HandleKind::Expr` is supported in
`src/core/handle.rs`. User documentation is in `docs/expressions.md`. 45 unit
tests cover parsing, resolution, registry behavior, and handle resolution;
`cargo fmt` and `cargo clippy --all-targets` are clean. Integration testing
against a live Excel workbook is still pending and requires a Windows machine
with Excel installed.

**2026-09-30 amendment update**: `insert_expression` now rejects a name
already registered in any other table with `CvxError::AmbiguousIdentifier`,
per SPEC-0002's cross-table name uniqueness amendment; verified by unit tests
in `src/core/registry.rs`.
