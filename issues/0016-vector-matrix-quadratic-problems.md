---
id: ISSUE-0016
title: Support quadratic problems over vectors and matrices (portfolio risk, least squares)
priority: should
status: draft
created: 2026-10-01
---

## Background

`cvxx` already supports problems with a quadratic (non-proportional)
objective or constraint, but only when every variable involved is a single
number. This limits quadratic support to a narrow slice of its real
potential: the most common reasons someone wants a quadratic objective in
practice — minimizing risk across a portfolio of several investments, or
fitting a model to a set of observations (least-squares) — are inherently
about combining many unknowns at once, not just one. Once vectors and
matrices are usable in solved problems and in expression building (see
companion issues), extending quadratic support to them is the step that
makes these flagship, high-value use cases actually achievable for `cvxx`
users.

## Goal

Users can build and solve a problem with a quadratic objective or constraint
that involves vector or matrix variables and parameters — such as minimizing
a combined risk measure across several holdings, or minimizing the total
squared error between a model and a set of observations — and get back
correct optimal results, extending today's single-number-only quadratic
support.

## Acceptance Criteria

- [ ] A least-squares-style problem (minimizing the total squared difference
  across a list of unknowns and known observations) can be built and solved,
  returning correct optimal values.
- [ ] A portfolio-style risk-minimization problem (combining a table of
  relationships between investments with a list of unknown allocations) can
  be built and solved, returning correct optimal values.
- [ ] Quadratic constraints involving vector/matrix variables are supported
  to the same extent quadratic constraints are supported for single numbers
  today (e.g., convex problems only, inequality constraints only).
- [ ] Infeasible and unbounded vector/matrix quadratic problems are reported
  with the same honest, accurate status as single-number problems today.
- [ ] Users attempting an unsupported vector/matrix quadratic problem type
  receive a clear, plain-language explanation rather than a confusing
  failure.
- [ ] Existing single-number quadratic problem behavior is unaffected.

## Notes

- Depends on ISSUE-0014, which covers both solving problems with
  vector/matrix variables and parameters, and combining their entries into
  weighted/combined totals — the building blocks this issue extends with
  quadratic (squared/cross-term) support. Does not depend on the separate,
  narrower issue covering indexing into a single entry or sub-section of a
  vector/matrix.
- Out of scope: non-convex quadratic problems, and sensitivity/dual-value
  reporting (consistent with the existing scope of quadratic support).
