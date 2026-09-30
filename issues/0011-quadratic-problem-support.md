---
id: ISSUE-0011
title: Support quadratic objectives and quadratic constraints in solved problems
priority: should
status: done
created: 2026-09-30
---

## Background

ISSUE-0010 delivered a working solver backend, but it only covers linear problems: affine objectives with affine constraints. The underlying solver technology chosen for `cvxx` is also capable of handling problems with quadratic objectives and quadratic constraints, so this is a matter of extending what the add-in already does, not building a new backend from scratch. Many real-world use cases for an Excel-based optimization tool — such as portfolio risk minimization, least-squares fitting, or penalty-based objectives — are naturally expressed as quadratic problems, and today these remain unsupported.

## Goal

Users can build and solve problems whose objective and/or constraints involve quadratic terms (in addition to the linear ones already supported), and get back correct optimal results in the same way they do for linear problems today. This significantly broadens the set of real-world business and financial problems users can solve directly in Excel without needing a separate specialized tool.

## Acceptance Criteria

- [ ] A problem with a quadratic objective and linear constraints can be solved and returns a correct optimal objective value and variable values.
- [ ] A problem with a linear or quadratic objective and one or more quadratic constraints can be solved and returns a correct optimal objective value and variable values.
- [ ] Problems combining linear and quadratic objectives/constraints in the same problem are supported.
- [ ] Infeasible and unbounded quadratic problems are reported with the same honest, accurate status as linear problems (see ISSUE-0010), not a generic error.
- [ ] Users attempting to build a problem type that is still unsupported (e.g., non-convex quadratic forms) receive a clear, plain-language explanation rather than a confusing failure.
- [ ] Existing linear problem behavior is unaffected.

## Notes

- Builds on ISSUE-0010 (done): the linear solver backend is already in place, and this issue only extends its coverage to quadratic problems.
- Coordinate with the technical team on whether quadratic objective/constraint authoring requires new expression-builder capabilities, or whether existing expression/constraint features are already sufficient to express quadratic terms.
- Out of scope: general non-convex quadratic problems, and sensitivity/dual-value reporting for quadratic problems (may warrant future issues).
