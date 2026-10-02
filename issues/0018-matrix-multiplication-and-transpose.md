---
id: ISSUE-0018
title: Matrix multiplication and transpose for vector/matrix expressions
priority: should
status: draft
created: 2026-10-02
---

## Background

Today, `cvxx` expressions can combine a vector or matrix with another value
only entry-by-entry (adding two equal-sized lists together, scaling every
entry by the same number, or multiplying two equal-sized lists entry by
entry), can collapse all of a list or table's entries down to one combined
total, and can refer to a single entry, row, column, or sub-section of a
list or table. There is still no way to combine two lists or tables using
true matrix multiplication — the standard linear-algebra operation behind
"a table of relationships applied to a list of unknowns" (for example,
applying a table of weights to a list of allocations, or a table of
sensitivities to a list of observations) — nor any way to turn a row into a
column (or vice versa), which real-world matrix multiplication regularly
requires to line up the right table rows against the right list entries.

Without these, users who think in standard matrix/vector terms — most
commonly because they already know them from tools like Python's NumPy —
have no direct way to write the formulas they already know. Today they must
manually break a single matrix-times-vector formula into many separate
one-row-at-a-time formulas, re-enter or rearrange their source data to get
rows and columns lined up, and stitch the individual results back together
by hand. This does not scale past a handful of rows, is tedious and
error-prone to set up, and actively works against reusing a table of data
that is already laid out the "wrong" way round for a particular formula.

## Goal

Users can combine two lists/tables using true matrix multiplication (not
just entry-by-entry combination), and can turn a list/table's rows into
columns (or columns into rows), using syntax and naming that will feel
immediately familiar to anyone who already knows Python's NumPy — so that
standard matrix/vector formulas (for example, "a table of weights applied
to a list of allocations," or "the relationships in a table applied to both
a list of unknowns and its turned-around self," as used for measuring
combined risk) can be written directly, without manually decomposing them
row by row or re-entering data in a different orientation.

## Acceptance Criteria

- [ ] Users can combine two lists/tables — whether a parameter, a variable,
  or the result of another formula — using true matrix multiplication,
  using syntax and naming consistent with NumPy's conventions, both when
  typing out a full formula and when building up a formula from previously
  created pieces one step at a time.
- [ ] Users can turn a list/table's rows into columns (or columns into
  rows) — whether a parameter, a variable, or the result of another
  formula — using syntax and naming consistent with NumPy's conventions,
  both when typing out a full formula and when building up a formula from
  previously created pieces one step at a time.
- [ ] Matrix multiplication and the row/column turn-around can be combined
  with each other and with every other existing expression capability (for
  example, within a constraint, or to express a combined-risk-style
  formula) to write standard matrix/vector formulas directly, without
  manually decomposing them row by row.
- [ ] Clear, plain-language errors are returned when two lists/tables
  cannot be combined via matrix multiplication because their sizes do not
  line up for it, naming the sizes involved — distinct from, and not
  confused with, the existing entry-by-entry size-mismatch error, since the
  two operations have different size rules.
- [ ] Inspecting an expression that uses matrix multiplication or a
  row/column turn-around correctly reports its resulting size, consistent
  with how existing expressions are already described today.
- [ ] Existing entry-by-entry combination behavior (today's multiplication,
  and every other existing capability) is completely unaffected — matrix
  multiplication is an additional capability, not a replacement or
  redefinition of what multiplication already means today.

## Notes

- This issue is about building and describing these formulas. Actually
  solving a problem built from them may require further work and is not
  guaranteed by this issue alone — in particular, combining two lists/
  tables that are *both* still unknown (for example, multiplying two
  variables together via matrix multiplication) is a fundamentally harder
  case, the same kind of hard case already called out in ISSUE-0016
  (quadratic problems over vectors and matrices), and may need to wait on
  or feed into that work. Combining one still-unknown list/table with one
  already-known table (for example, a known table of weights applied to a
  list of unknown allocations) is a substantially easier case and should
  not need to wait on ISSUE-0016.
- This issue does not depend on the existing single-entry/sub-section
  referencing capability, but the two are expected to be used together in
  practice (for example, turning around just part of a larger table).
- The existing entry-by-entry combination's "scale by a single number"
  shortcut does not apply to matrix multiplication — standard matrix
  multiplication requires the two lists/tables' sizes to line up in a
  specific way (the first one's row length must match the second one's
  column length), not just be identical or allow one side to be a single
  number.
- Naming and syntax should be chosen to feel immediately familiar to
  NumPy users specifically (matrix multiplication and the row/column
  turn-around are two of the most frequently reached-for operations in
  that tool), rather than inventing unfamiliar new names or wording for
  operations that already have well-known, widely recognized conventions.
