---
id: ISSUE-0010
title: Provide a working convex solver backend
priority: must
status: draft
created: 2026-09-30
---

## Background

Users can already define variables, expressions, and constraints, and assemble them into a problem via `CVX.PROBLEM` and `CVX.SOLVE`. However, solving currently always fails: there is no real optimization engine behind it yet. Without an actual solver, the add-in cannot deliver its core value proposition — helping users find optimal answers to their convex optimization problems directly in Excel.

## Goal

Users who call `CVX.SOLVE` on a well-formed convex problem get back a real, correct answer (or an honest, accurate status such as infeasible or unbounded) rather than an "unimplemented" error. The add-in should be able to solve the common convex problem types users are expected to build with the existing variable, expression, and constraint features (e.g., linear and quadratic objectives with linear (in)equality constraints).

## Acceptance Criteria

- [ ] A problem built from supported variables, expressions, and constraints can be solved and returns a correct optimal objective value and variable values.
- [ ] Problems that are infeasible are reported as infeasible, not as a generic error.
- [ ] Problems that are unbounded are reported as unbounded, not as a generic error.
- [ ] Solve results are consistent and reproducible for the same inputs.
- [ ] Users are told, in plain language, when a problem type is not yet supported by the solver, rather than getting a confusing failure.
- [ ] Solving a reasonably sized problem (as expected for typical Excel-based use) completes within a time frame that keeps Excel usable (no indefinite hangs).

## Notes

- This issue is about making solving actually work; it does not cover dual values, sensitivity analysis, or advanced solver tuning options (see future issues).
- Coordinate with the technical team on which problem classes (linear programming, quadratic programming, etc.) are prioritized first.
- Depends on the existing problem/solve surface described in ISSUE-0006.
