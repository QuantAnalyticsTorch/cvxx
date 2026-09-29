---
id: ISSUE-0002
title: Create a named parameter from an Excel range
priority: must
status: draft
created: 2026-09-29
---

## Background

`cvxx` needs to bring data from Excel worksheets into Rust so it can be used in optimization problems. The first building block is a parameter: a numeric object that lives in an in-memory registry and is referenced from Excel by an opaque handle. Because Excel cannot receive Rust objects directly, the handle must be a string. Parameters should be cacheable by content hash so that unchanged data does not force a rebuild of downstream expressions or problems.

## Goal

Expose an Excel function that takes a worksheet range and an optional name, stores the data in a Rust registry, and returns a stable string handle. The function must compute a content hash, cache the parameter, and allow the parameter to be looked up later by either its handle or its name.

## Acceptance Criteria

- [ ] A `CVX.PARAMETER` function is registered with Excel.
- [ ] The function accepts a range of numeric cells and an optional name as the last argument.
- [ ] The function returns a string handle such as `cvx:param:<uuid>`.
- [ ] If a name is provided, it is stored with the parameter and must be unique in the registry.
- [ ] The function computes a hash of the range contents and reuses a cached parameter when the same range data and name are supplied again.
- [ ] Empty cells within the range are treated as zero.
- [ ] Non-numeric or invalid inputs return `#VALUE!` with a logged diagnostic.
- [ ] The parameter object is unit-testable without Excel.

## Notes

- This issue intentionally covers only parameters. Variables, expressions, constraints, problems, and results will follow in later issues.
- The registry must be thread-safe because Excel may recalculate in parallel.
