---
id: ISSUE-0015
title: Refer to a single entry or sub-section of a vector or matrix
priority: should
status: draft
created: 2026-10-01
---

## Background

Today, expressions can only combine a vector or matrix with another value
entry-by-entry (for example, adding two equal-sized lists together, or
scaling every entry of a list by the same number), or — once the companion
solving issue (ISSUE-0014) lands — collapse all of a list or table's entries
down to one combined total. There is still no way to refer to just one
entry, row, column, or sub-section of a list/table on its own. Without this,
users cannot express formulas that single out part of a larger list or
table — for example "check that just this one entry is below a limit," or
"use only the first three entries of this list in a separate calculation."

## Goal

Users can refer to a single entry, row, column, or sub-section of a
list/table — whether it is a parameter, a variable, or the result of another
formula — for use elsewhere in a `cvxx` expression, objective, or
constraint.

## Acceptance Criteria

- [ ] Users can refer to a single entry, row, column, or sub-section of a
  list/table — whether it is a parameter, a variable, or the result of
  another formula — for use elsewhere in a calculation.
- [ ] Clear, plain-language errors are returned when a requested entry, row,
  column, or sub-section falls outside the list/table's actual size, naming
  the requested position and the actual size.
- [ ] Inspecting an expression that refers to a single entry or sub-section
  correctly reports its resulting size, consistent with how existing
  expressions are already described today.
- [ ] Existing single-number and whole-list/table expression behavior is
  unaffected.
- [ ] Both referring to a single entry, row, column, or sub-section, and
  combining a list/table's entries into one combined total, can be written
  directly inside a single typed-out formula (for example, something like
  `sum(v)`, or singling out one entry of `v` in the middle of a larger
  formula) — not only by chaining together separate handle-producing
  building-block steps one at a time.

## Notes

- This issue is about building expressions only; actually solving problems
  that use vector/matrix variables is covered by ISSUE-0014. It does not
  depend on ISSUE-0014 and could be delivered before, after, or alongside it.
- Combining several entries into a single total (a weighted sum, or adding up
  all of a list/table's entries) is covered by ISSUE-0014, not here — that
  capability is needed to make vector/matrix variables usable at all, so it
  was moved out of this issue and folded into ISSUE-0014's scope. The one
  exception is the last acceptance criterion above: making that
  already-delivered combining-into-a-total capability usable directly
  inside a single typed-out formula, alongside this issue's own
  single-entry/sub-section capability, is in scope here, since both are the
  same kind of "usable directly inside a formula" request and are most
  naturally delivered together.
