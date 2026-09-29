---
id: ISSUE-0004
title: Build optimization expressions from Excel formulas
priority: must
status: draft
created: 2026-09-29
---

## Background

Users want to write optimization expressions naturally, similar to how they write formulas in `cvxpy` or `cvxrust`. Since Excel cannot overload operators on string handles, `cvxx` will accept expression strings and parse them into expression objects. Named parameters and variables can be referenced inside those strings.

## Goal

Expose an Excel function that parses a mathematical expression string, resolves named parameters and variables from the registry, and returns an expression handle that can be reused in other expressions, constraints, or objectives.

## Acceptance Criteria

- [ ] A `CVX.EXPRESSION` function is registered with Excel.
- [ ] The function accepts an expression string and an optional name as the last argument.
- [ ] The function returns a string handle such as `cvx:expr:<uuid>`.
- [ ] Expression strings support `+`, `-`, `*`, `/`, parentheses, and numeric constants.
- [ ] Expression strings can reference named parameters and variables.
- [ ] Named expression handles can be referenced inside later expression strings.
- [ ] The parser returns a clear error for invalid syntax or unknown names.
- [ ] Functional builders such as `CVX.ADD` and `CVX.MUL` are available as alternatives.
- [ ] Expressions are lazy: no numerical evaluation happens until solve time.

## Notes

- Transpose syntax (`x'` or `x.T`) is out of scope for this issue.
- Function calls inside expressions (`sum_squares`, `norm`, etc.) may be included here or deferred to a follow-up issue.
