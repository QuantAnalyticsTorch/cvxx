---
id: ISSUE-0003
title: Create named optimization variables from Excel
priority: must
status: draft
created: 2026-09-29
---

## Background

Optimization problems need decision variables. In `cvxx`, these variables must be created in Rust and referenced from Excel by handles. Each variable needs a user-friendly name so it can be used in expression strings later.

## Goal

Expose an Excel function that creates an optimization variable with a given shape and an optional name, stores it in the registry, and returns a string handle.

## Acceptance Criteria

- [ ] A `CVX.VARIABLE` function is registered with Excel.
- [ ] The function accepts rows, columns, and an optional name as the last argument.
- [ ] The function returns a string handle such as `cvx:var:<uuid>`.
- [ ] If a name is provided, it is stored and must be unique in the registry.
- [ ] Variables support scalar, vector, and matrix shapes.
- [ ] Variables with duplicate names return an error.
- [ ] The variable object is unit-testable without Excel.

## Notes

- This issue covers only variable creation. Using variables in expressions is covered by the expression issue.
