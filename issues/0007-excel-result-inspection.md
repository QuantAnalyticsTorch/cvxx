---
id: ISSUE-0007
title: Inspect optimization results and handles from Excel
priority: must
status: draft
created: 2026-09-29
---

## Background

After solving, users need to extract the optimal values of their variables and understand the solver status. They also need visibility into the opaque handles they have created, so they can debug expressions and constraints.

## Goal

Expose Excel functions that extract solution values from a result handle and describe the contents of any handle in the registry.

## Acceptance Criteria

- [ ] A `CVX.VALUE` function returns the optimal value of a variable referenced by its handle.
- [ ] `CVX.VALUE` returns a scalar for scalar variables and an array for vector or matrix variables.
- [ ] A `CVX.STATUS` function returns the solver status as a readable string.
- [ ] A `CVX.OBJECTIVE_VALUE` function returns the optimal objective value.
- [ ] A `CVX.DESCRIBE` function returns a human-readable description of any handle.
- [ ] A `CVX.SHAPE` function returns the dimensions of a handle as a string.
- [ ] A `CVX.TYPE` function returns the object type of a handle (`variable`, `parameter`, `expression`, `constraint`, `problem`, `result`).
- [ ] Requests for unknown or expired handles return `#VALUE!`.

## Notes

- Dual values will be added in a later issue.
- `CVX.DESCRIBE` should handle large expressions gracefully, possibly truncating very long strings.
