---
id: ISSUE-0006
title: Build and solve optimization problems from Excel
priority: must
status: draft
created: 2026-09-29
---

## Background

Once users have defined expressions and constraints, they need to assemble them into an optimization problem and run a solver. The result must be storable in the registry because Excel cannot receive Rust objects directly.

## Goal

Expose Excel functions that build a problem from an objective expression and constraints, invoke `cvxrust` to solve it, and return a result handle.

## Acceptance Criteria

- [ ] A `CVX.PROBLEM` function is registered with Excel.
- [ ] The function accepts an objective handle and a range or handle representing constraints.
- [ ] The function returns a problem handle such as `cvx:prob:<uuid>`.
- [ ] Objective wrappers `CVX.MINIMIZE` and `CVX.MAXIMIZE` are available.
- [ ] A `CVX.SOLVE` function is registered with Excel.
- [ ] `CVX.SOLVE` accepts a problem handle and returns a result handle such as `cvx:result:<uuid>`.
- [ ] Solving respects the cached parameter hashes and does not rebuild unchanged parameters.
- [ ] Solver status is captured in the result object.
- [ ] Solver failures return an Excel-visible error with logged diagnostics.

## Notes

- Dual values and sensitivity analysis are out of scope for this issue.
- The recalculation behavior should be controlled by Excel; the add-in should not force automatic recalculation.
