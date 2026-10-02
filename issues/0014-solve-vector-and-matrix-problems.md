---
id: ISSUE-0014
title: Solve problems with vector and matrix variables and parameters
priority: must
status: draft
created: 2026-10-01
---

## Background

Users can already create variables and parameters that represent a vector (a
single row or column of numbers) or a matrix (a table of numbers), not just a
single number. However, solving a problem only works today when every
variable and parameter involved is a single number. This blocks the add-in
from being useful for the kinds of problems Excel users actually have, which
almost always involve a list or table of unknowns rather than just one — for
example, allocating a budget across several projects, choosing order
quantities across a product line, or setting weights across several
investments.

Critically, every one of those examples needs more than just "solve with a
vector variable": a budget allocation needs a *total* cost or a *total*
spend across every project to compare against a budget; a portfolio needs a
*total* value across every holding. Without a way to combine a vector or
matrix's entries into a single number, a vector/matrix variable could be
declared and bounded, but never meaningfully used to describe what a user
actually wants to optimize or limit — so this issue includes the minimum
"combine into a total" capability needed to make the rest of it useful, not
just the ability to declare and bound shaped variables.

## Goal

Users can build a problem whose variables and/or parameters are a vector or a
matrix, and solve it successfully — with a correct value returned for every
entry — for the straightforward ("linear"/proportional) objectives and
constraints already supported today, instead of being blocked by a
single-number-only limitation. Users can also combine all the entries of a
vector or matrix expression into a single total (e.g. a weighted sum of
costs, or a total portfolio value), so that a vector/matrix variable can
actually be used in an objective or in a constraint that compares a total
against a limit — not just bounded entry-by-entry.

## Acceptance Criteria

- [ ] A problem containing one or more vector-shaped variables can be solved,
  returning a correct result for every entry of the variable.
- [ ] A problem containing one or more matrix-shaped variables can be solved,
  returning a correct result for every entry.
- [ ] Vector- or matrix-shaped parameters can be used anywhere a single-number
  parameter is used today, with correct results.
- [ ] Users can combine all the entries of a vector or matrix expression
  (e.g. a list of costs multiplied entry-by-entry by a list of quantities)
  into a single total number, and use that total in an objective or in a
  constraint compared against a limit.
- [ ] Problems that mix single numbers, vectors, and matrices together behave
  correctly.
- [ ] Existing single-number-only problems continue to solve exactly as they
  do today (no regression).
- [ ] Users attempting a problem that is still too large or otherwise
  unsupported receive a clear, plain-language explanation rather than a
  confusing failure.
- [ ] Guidance is available on how problem size limits (which today are
  expressed in terms of individual numbers) apply once variables and
  parameters can hold many numbers at once.

## Notes

- Builds on the existing ability to create vector/matrix variables and
  parameters; this issue is about making solving actually honor those shapes
  rather than rejecting them, plus the minimum "combine into a total"
  capability needed to put that to real use.
- Referring to a single entry, row, column, or sub-section of a vector or
  matrix (rather than combining all of it into one total) is a separate,
  smaller, non-blocking capability and is covered by a follow-on issue.
- Quadratic (non-proportional) objectives and constraints over vectors and
  matrices — needed for things like portfolio risk or curve fitting — are
  intentionally out of scope here and covered by a follow-on issue. Combining
  entries into a total, as scoped here, is a proportional ("linear") operation
  and does not require that follow-on work.
