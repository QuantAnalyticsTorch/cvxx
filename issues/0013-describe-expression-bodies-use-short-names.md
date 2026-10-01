---
id: ISSUE-0013
title: Show short names for variables and parameters inside described expression bodies
priority: should
status: draft
created: 2026-10-01
---

## Background

ISSUE-0012 made `CVX.DESCRIBE` identify objects by their short registered name instead of their full handle, both for the object being described and for other objects it refers to directly (for example, the constraints in a constraint set, or the objective, constraints and variables of a problem).

That work did not cover the formula text that `CVX.DESCRIBE` shows for expressions, constraints and objectives. Variables and parameters in that text still appear as system placeholders, even when the user has named them. A user reported this output:

`cvx:expr:6551135e-bcd0-4dab-907a-aa7d45a95a98 ("total"): expression 1x1 = (var#16880882605295944462) * (var#16880882605295944462) + var#3771668566231105626`

The expression's own name ("total") is shown, but the variables inside the formula appear as long numeric placeholders. The user cannot tell which variables they are without further investigation. Users read the formula text to check that their model is built correctly, so names they cannot recognise make the add-in harder to use and errors harder to diagnose. SPEC-0012 explicitly listed this as a Non-Objective, so this issue tracks it as a follow-on readability improvement.

## Goal

When `CVX.DESCRIBE` shows the formula of an expression, constraint or objective, each variable or parameter in it appears under the short name the user gave it. Users can read the formula in their own model's terms, for example `total = x * x + y` rather than a string of numeric placeholders. Variables and parameters without a name are shown as they are today.

## Acceptance Criteria

- [ ] When an expression is described, each named variable in its formula is shown by its name instead of the current placeholder.
- [ ] When an expression is described, each named parameter in its formula is shown by its name instead of the current placeholder.
- [ ] The same applies wherever a formula appears in `CVX.DESCRIBE` output, including constraint descriptions (both sides) and the objective in problem descriptions.
- [ ] Variables and parameters without a name keep their current placeholder text, so output for unnamed models does not change.
- [ ] A formula that mixes named and unnamed variables and parameters shows names where they exist and placeholders elsewhere.
- [ ] When a variable or parameter appears more than once in a formula, it is shown the same way every time.
- [ ] The formula's structure (operators, grouping, order of terms) is unchanged; only the labels for variables and parameters change.
- [ ] The change affects display only. It does not change the identity, behaviour or lifetime of any object, or any solve result.
- [ ] If a variable or parameter's name is changed or removed after an expression uses it, the next describe shows its current name, or the placeholder if it no longer has one.
- [ ] User-facing documentation for `CVX.DESCRIBE` is updated with an example of a formula that uses names.

## Notes

- Follows on from ISSUE-0012 / SPEC-0012, which listed formula text as a Non-Objective.
- Open question: what should be shown if two different variables or parameters have the same name, so that they stay distinguishable? The technical team should propose an approach.
- Open question: what should be shown if a variable or parameter used in a formula has been removed or has expired? Showing the current placeholder is an acceptable fallback.
- Open question: should named parameters also show their shape (for example, `weights (3x1)`), or only the name? Shape is shown elsewhere in describe output, so the name alone is probably enough.
- Out of scope: how users assign names, the handle format, and any outputs other than `CVX.DESCRIBE`. The technical team should flag any other outputs that would benefit from the same change.
