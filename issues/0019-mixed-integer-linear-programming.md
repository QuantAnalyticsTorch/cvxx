---
id: ISSUE-0019
title: Support mixed-integer and binary decision variables in solved problems
priority: should
status: draft
created: 2026-10-04
---

## Background

Today, every variable created with `CVX.VARIABLE` is continuous: its values
can be any real number (subject to the constraints the user adds) once a
problem is solved. This covers a large class of real-world planning and
allocation questions, but a common and important category of business
decisions is naturally "whole number" or "yes/no" in nature — for example,
how many trucks to assign to a route, which of a set of candidate projects
to fund, how many units of a product to produce, or whether to open a given
facility at all. These decisions cannot be expressed today: a user either
has to accept a fractional answer that doesn't make sense in the real world
(e.g., "assign 2.4 trucks") and round it by hand, or give up on using
`cvxx` for that question entirely.

Spreadsheet users who are used to Excel's own Solver add-in already expect
to be able to mark a cell/variable as "integer" or "binary" when setting up
an optimization model, so this is also a capability gap relative to the
tool many prospective users are migrating from.

## Goal

Users can mark some or all of the variables in a problem as restricted to
integer values, or specifically to 0/1 (binary) values, and then solve that
problem the same way they solve a continuous problem today — getting back a
correct optimal answer whose reported variable values honor those
restrictions, or an honest, accurate status (infeasible, unbounded, or
"stopped before proven optimal / best answer found so far") when a solution
can't be confirmed.

This extends optimization coverage to common whole-number and yes/no
business decisions — resource and crew assignment, project/investment
selection, facility location, scheduling, and similar planning problems —
directly in Excel, without requiring a separate specialized tool.

## Acceptance Criteria

- [ ] A user can designate a variable (or specific elements of a
      vector/matrix variable) as integer-valued when creating it.
- [ ] A user can designate a variable (or specific elements) as binary
      (0 or 1 only) when creating it.
- [ ] A problem containing a mix of continuous, integer, and binary
      variables, with linear objective and linear constraints, can be
      solved and returns a correct optimal objective value and variable
      values that honor the integer/binary restrictions.
- [ ] Problems that are infeasible are reported as infeasible, not as a
      generic error, consistent with existing continuous-problem behavior
      (ISSUE-0010).
- [ ] Problems that are unbounded are reported as unbounded, not as a
      generic error.
- [ ] If solving a mixed-integer problem is stopped before the best
      possible answer is proven optimal (for example, due to a time or
      size limit), the user is told plainly that the result is the best
      answer found so far rather than a guaranteed optimum, instead of
      silently presenting it as a final answer or failing with a generic
      error.
- [ ] Users attempting to mark a variable as integer/binary in a problem
      type that doesn't yet support it receive a clear, plain-language
      explanation rather than a confusing failure.
- [ ] Existing continuous-only problem behavior (results, performance,
      error messages) is unaffected when no variable is marked
      integer/binary.
- [ ] User-facing documentation explains what integer/binary variables are,
      how to declare them, and what "best answer found so far" vs.
      "proven optimal" means for someone without an optimization
      background.

## Notes

- Builds on ISSUE-0010 (continuous linear solver backend) and ISSUE-0011
  (quadratic objectives/constraints); this issue's initial scope is linear
  objectives/constraints with integer or binary variables (i.e.,
  mixed-integer *linear* programming). Whether quadratic problems combined
  with integer/binary variables are in scope for this issue or a follow-up
  is a coordination question for the technical team.
- The user has requested `HiGHS` specifically as the solving technology for
  this capability. Evaluating and deciding on the actual solver/backend
  used to deliver mixed-integer solving — including whether it fits
  alongside the solver framework already adopted for continuous problems
  (see `docs/architecture.md`) — is a technical/architectural decision, not
  a business requirement, and should be worked out by the technical and
  architecture review as part of specifying this issue.
- Mixed-integer problems can take substantially longer to solve than
  continuous ones, and in the worst case may not finish in a reasonable
  time at all. The acceptance criteria above assume some form of practical
  limit (time and/or problem size) is needed to keep Excel usable; the
  exact limit and how it's surfaced to users is a technical design
  question, but the business expectation is that Excel must never hang
  indefinitely.
- Out of scope for this issue: general nonlinear/non-convex integer
  problems, and sensitivity/dual-value reporting for mixed-integer
  problems (may warrant future issues).
