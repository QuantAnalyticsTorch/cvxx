---
id: SPEC-0003
title: Create named optimization variables from Excel
issue: ISSUE-0003
status: implemented
created: 2026-09-29
---

## Objective

Implement `CVX.VARIABLE`, an Excel-facing function that creates a `cvxrust` optimization variable of a specified shape, stores it in the Rust registry, and returns an opaque string handle.

## Non-Objective

- No parameter creation (covered by SPEC-0002).
- No expression parsing, constraint building, problem construction, or solver invocation.
- No numerical evaluation or assignment of variable values.
- No ribbon or XLAM changes.

## Interface

### Excel function

```
CVX.VARIABLE(rows, cols, [name])
```

- `rows`: positive integer specifying the number of rows in the variable.
- `cols`: positive integer specifying the number of columns in the variable.
- `name`: optional string identifier supplied as the last argument. If omitted, the variable has no user-visible name and can only be referenced by its handle.
- Returns: a string handle of the form `cvx:var:<uuid>`.

A scalar variable is created with `rows = 1` and `cols = 1`. A vector variable is created with exactly one of `rows` or `cols` equal to `1`. A matrix variable is created with both dimensions greater than `1`.

### Registry lookup

The Rust registry stores variable entries keyed by a generated UUID. Each entry contains:

- `uuid`: the UUID embedded in the handle.
- `name`: optional user-supplied name (unique within the registry).
- `shape`: `(rows, cols)`.
- `variable`: a `cvxrust` variable object representing the decision variable.

The registry supports lookup by UUID or by name.

### Handle format

All handles follow the pattern `cvx:<kind>:<uuid>`, where `<kind>` for variables is `var`.

## Data Model

- Input: two positive integers for `rows` and `cols`, plus an optional string name.
- Parsed input: validated shape `(rows, cols)`.
- Stored object: variable entry in the registry.
- Output: string handle.

## Error Handling

- Non-integer, zero, negative, or non-numeric `rows`/`cols` → `#VALUE!`.
- `rows` or `cols` exceeding a defined implementation limit → `#VALUE!` with a logged diagnostic.
- Duplicate name → `#VALUE!` with message indicating the conflict.
- Internal registry failure → `#VALUE!` with logged diagnostic.

## Test Approach

- Unit tests for the registry: insert variable, lookup by UUID, lookup by name, duplicate name rejection, shape preservation.
- Unit tests for shape validation: scalar `(1,1)`, row vector `(1,n)`, column vector `(n,1)`, matrix `(m,n)`.
- Integration test: a sample workbook calls `CVX.VARIABLE(3, 1, "x")` and the returned handle matches the expected prefix.

## Dependencies

- `xladd` for XLL function registration and `XLOPER12` access.
- A thread-safe registry implementation, likely `std::sync::RwLock` or `dashmap`.
- `cvxrust` for the underlying variable representation.
- SPEC-0002 establishes the registry, handle format, and naming conventions reused here.

## Implementation Notes

1. Extend the module structure defined in SPEC-0002:
   - `src/core/` — add a variable entry type to the registry.
   - `src/excel/` — add the `CVX.VARIABLE` exported XLL function.
2. Implement `core::Registry` methods for variable insertion and lookup by UUID/name, reusing the same uniqueness rules as parameters.
3. Parse `rows` and `cols` from `XLOPER12` numeric scalars, rejecting invalid values before creating the variable.
4. Construct the `cvxrust` variable with the validated shape and store it in the registry.
5. Register `CVX.VARIABLE` in the XLL entry point with `xladd`.
6. Add `#[cfg(test)]` unit tests for registry and shape validation logic.

## Status

Implemented. `CVX.VARIABLE` is registered in `src/excel/mod.rs` (`xlAutoOpen`)
and exported as `src/excel/variable.rs::cvx_variable`. Shape validation and
dimension parsing live in `src/data/mod.rs` (`parse_dimension`, `MAX_DIMENSION`
= 1,000,000). The variable placeholder type lives in
`src/core/variable.rs::Variable`; registry insertion and lookup live in
`src/core/registry.rs::VariableEntry`. The handle kind `var` is supported in
`src/core/handle.rs`. User documentation is in `docs/variables.md`. 11 new unit
tests cover variable registry operations and dimension validation; `cargo fmt`
and `cargo clippy --all-targets` are clean. Integration testing against a live
Excel workbook is still pending and requires a Windows machine with Excel
installed.
