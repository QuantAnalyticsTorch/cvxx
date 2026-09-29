---
id: ISSUE-0008
title: Provide one-shot convenience solver functions in Excel
priority: should
status: draft
created: 2026-09-29
---

## Background

Many users do not want to build variables, expressions, constraints, and problems manually. They want familiar one-shot functions similar to Excel's own solver-style calls: linear programming, quadratic programming, and least squares.

## Goal

Expose high-level Excel functions that hide the handle model and return the solution directly for common problem templates.

## Acceptance Criteria

- [ ] A `CVX.LP_SOLVE` function solves a linear program from cost, inequality, and equality inputs.
- [ ] A `CVX.QP_SOLVE` function solves a quadratic program.
- [ ] A `CVX.LEAST_SQUARES` function solves an ordinary least-squares problem.
- [ ] Each function accepts numeric ranges and returns the solution array directly.
- [ ] Each function returns an Excel error if the problem is infeasible, unbounded, or fails numerically.
- [ ] Empty cells in input ranges are treated as zero.
- [ ] The functions are documented with examples.

## Notes

- These functions internally use the same registry, expression builder, and solver as the handle-based API.
- Bounds on variables may be supported as optional arguments or deferred to a later issue.
