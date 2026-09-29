---
id: ISSUE-0005
title: Create optimization constraints from Excel formulas
priority: must
status: draft
created: 2026-09-29
---

## Background

An optimization problem is defined by its objective and its constraints. Users should be able to write constraints naturally as strings, referencing named parameters, variables, and expressions. Multiple constraints must be combinable and passed to the problem builder.

## Goal

Expose Excel functions that parse constraint strings and return constraint handles. Support combining multiple constraints into a constraint set that can be passed to the problem builder.

## Acceptance Criteria

- [ ] A `CVX.CONSTRAINT` function is registered with Excel.
- [ ] The function accepts a constraint string and an optional name as the last argument.
- [ ] The function returns a string handle such as `cvx:constr:<uuid>`.
- [ ] Constraint strings support `<=`, `>=`, and `==` operators.
- [ ] Constraint strings can reference named parameters, variables, and expressions.
- [ ] Functional builders such as `CVX.LESS_THAN` and `CVX.EQUAL` are available as alternatives.
- [ ] Multiple constraints can be passed as a cell range to the problem builder.
- [ ] A `CVX.CONSTRAINTS` function is available to combine handles into a single constraint set handle.
- [ ] Invalid constraint strings or unknown names return a clear error.

## Notes

- Constraint bounds and variable bounds are out of scope for this issue; they will be handled as constraints here.
