---
id: SPEC-0002
title: Create a named parameter from an Excel range
issue: ISSUE-0002
status: draft
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

## Test Approach

- Unit tests for the registry: insert, lookup by UUID, lookup by name, duplicate name rejection, content hash equality.
- Unit tests for range parsing: empty cells treated as zero, scalar, vector, and matrix shapes preserved.
- Integration test: a sample workbook calls `CVX.PARAMETER` and the returned handle matches the expected prefix.

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
