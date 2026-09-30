---
id: SPEC-0002
title: Create a named parameter from an Excel range
issue: ISSUE-0002
status: implemented
created: 2026-09-29
---

## Objective

Implement the first Excel-facing `cvxx` function, `CVX.PARAMETER`, which reads numeric data from an Excel range, stores it as a cached parameter object in a Rust registry, and returns an opaque string handle.

## Non-Objective

- No expression parsing.
- No variables, constraints, problems, or results.
- No optimization or solver invocation.
- No ribbon or XLAM changes.

## Interface

### Excel function

```
CVX.PARAMETER(range, [name])
```

- `range`: a rectangular Excel range containing numeric values. Empty cells are treated as zero.
- `name`: optional string identifier supplied as the last argument. If omitted, the parameter has no user-visible name and can only be referenced by its handle.
- Returns: a string handle of the form `cvx:param:<uuid>`.

### Registry lookup

The Rust registry stores parameter entries keyed by a generated UUID. Each entry contains:

- `uuid`: the UUID embedded in the handle.
- `name`: optional user-supplied name (unique within the registry).
- `shape`: `(rows, cols)`.
- `data`: dense matrix of `f64` values in a defined storage order (row-major or column-major; document the choice).
- `content_hash`: hash of the flattened data.

The registry supports lookup by UUID or by name.

### Handle format

All handles follow the pattern `cvx:<kind>:<uuid>`, where `<kind>` for parameters is `param`.

## Data Model

- Input: `XLOPER12` multi-cell reference.
- Parsed input: a validated 2-D matrix of `f64` values.
- Stored object: parameter entry in the registry.
- Output: string handle.

## Error Handling

- Empty or non-rectangular input → `#VALUE!`.
- Non-numeric cell → `#VALUE!`.
- Duplicate name → `#VALUE!` with message indicating the conflict.
- Internal registry failure → `#VALUE!` with logged diagnostic.

## Amendment (2026-09-30): Cross-table name uniqueness

Names were originally only checked for uniqueness *within* each object
kind's own table (e.g. two calls to `CVX.PARAMETER` with the same name
conflict, but a parameter and a variable could share a name without
error). `SPEC-0007`'s `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` need to resolve
a bare name to exactly one registry object, so this ambiguity is closed
here, at the root registry spec all other specifications build on:

- A new error variant is added: `CvxError::AmbiguousIdentifier(String)`,
  `#[error("name '{0}' is already registered under a different object type")]`.
- Every `insert_*` method on `Registry` (parameters, variables, expressions,
  constraints, constraint sets, objectives, problems, results — added
  cumulatively by SPEC-0002 through SPEC-0006) checks, before inserting a
  *named* entry, whether that name already exists in **any other** table's
  `by_name` map (not just its own). If so, the insertion is rejected with
  `CvxError::AmbiguousIdentifier(name)` and nothing is stored — this check
  runs *before* the existing same-table `DuplicateName`/overwrite-by-name
  logic, so reusing a name within its own table still behaves exactly as
  each object kind's own specification describes (overwrite for
  variables/expressions/constraints/etc., `DuplicateName` rejection for
  content-addressed parameters), and only a name that would newly appear in
  a *second* table is rejected.
- This is a pure additional guard: it does not change `content_hash`
  computation, UUID generation, or any existing lookup method's signature.
- Every specification that defines an `insert_*` method (SPEC-0003 through
  SPEC-0006) is amended with a one-line cross-reference to this rule in its
  own Error Handling section, rather than restating it.
- Status: pending implementation (not yet reflected in the `## Status`
  section below, which describes the original `CVX.PARAMETER` delivery).

## Test Approach

- Unit tests for the registry: insert, lookup by UUID, lookup by name, duplicate name rejection, content hash equality.
- Unit tests for range parsing: empty cells treated as zero, scalar, vector, and matrix shapes preserved.
- Integration test: a sample workbook calls `CVX.PARAMETER` and the returned handle matches the expected prefix.
- Unit tests for the cross-table name uniqueness amendment: inserting a
  parameter named `"x"` then a variable named `"x"` (and every other
  cross-kind pairing among parameter/variable/expression/constraint/
  constraint-set/objective/problem/result) returns
  `CvxError::AmbiguousIdentifier`; reusing a name within its own table is
  unaffected and still follows that table's existing rule (overwrite or
  `DuplicateName`).

## Dependencies

- `xladd` for XLL function registration and `XLOPER12` access.
- A hashing crate such as `sha2` or `blake3`, or `std::collections::hash_map::DefaultHasher`.
- A thread-safe registry implementation, likely `std::sync::RwLock` or `dashmap`.

## Implementation Notes

1. Create the module structure:
   - `src/excel/` — XLL registration and `XLOPER12` adapters.
   - `src/data/` — Excel range parsing and normalization.
   - `src/core/` — registry, handle generation, parameter entry, and shared errors.
2. Implement `core::Registry` as a thread-safe map from UUID to a parameter entry.
3. Implement `data::parse_range` to convert an `XLOPER12` array reference into a dense `f64` matrix.
4. Implement `excel::parameter` as the exported XLL function.
5. Register the function in the XLL entry point with `xladd`.
6. Add `#[cfg(test)]` unit tests for registry and parsing logic.

## Status

Implemented. `CVX.PARAMETER` is registered in `src/excel/mod.rs` (`xlAutoOpen`) and
exported as `src/excel/parameter.rs::cvx_parameter`. Range parsing lives in
`src/data/mod.rs` (row-major storage, blank cells treated as zero). The
thread-safe registry, content hashing/caching, handle formatting, and parsing
live in `src/core/registry.rs` and `src/core/handle.rs`. Diagnostics are logged
via `tracing` to `%TEMP%/cvxx.log` (see `src/logging.rs`); Excel only ever sees
`#VALUE!`. 13 unit tests cover the registry, handle round-tripping, and range
parsing; `cargo fmt` and `cargo clippy --all-targets` are clean. Integration
testing against a live Excel workbook is still pending and requires a Windows
machine with Excel installed.

**2026-09-30 amendment update**: the cross-table name uniqueness check
described above is now implemented. `CvxError::AmbiguousIdentifier` was added
to `src/core/error.rs`; `Registry::check_name_available` in
`src/core/registry.rs` checks all eight tables (in fixed `HandleKind` order,
releasing each read lock immediately) before every `insert_*` method's
existing same-table logic. New unit tests cover every adjacent cross-kind
pairing plus same-table-reuse regression cases; all pass, and `cargo fmt`/
`cargo clippy --all-targets -- -D warnings` remain clean.
