---
id: ISSUE-0017
title: Autogenerate a fuller set of branded example workbooks
priority: should
status: draft
created: 2026-10-01
---

## Background

New users learn `cvxx` largely by opening the example workbooks linked from
the ribbon's Examples gallery. Today there is only a single, small
introductory workbook, which barely scratches the surface of what the add-in
can do (parameters, variables, expressions, constraints, problems and
solving, quadratic problems, convenience solvers, and result inspection).
This under-sells the product, leaves most features undiscovered, and risks
examples drifting out of date or inconsistent in appearance if they have to
be hand-built and hand-maintained one at a time. A broader, consistently
branded set of examples would make it much easier for a new user to see what
`cvxx` can do and copy a working pattern for their own problem.

## Goal

A richer library of example workbooks is produced automatically (as part of
the existing build/release process, not hand-maintained one-off files),
covering the breadth of `cvxx` functionality, with a consistent, professional
blue-and-white visual style that reflects the `cvxx` brand.

## Acceptance Criteria

- [ ] Example workbooks are provided covering each major area of
  functionality: creating parameters and variables, building expressions,
  defining constraints, assembling and solving problems, quadratic problems,
  the one-shot convenience solvers, and inspecting results.
- [ ] Each example workbook is self-explanatory to a non-technical user: it
  states what business problem it illustrates, walks through the relevant
  formulas, and shows the resulting answer.
- [ ] Workbooks are generated automatically rather than maintained by hand,
  so they cannot silently go stale as functions change or are added.
- [ ] All example workbooks share a consistent blue-and-white visual style
  (headers, input cells, and result cells are visually distinguishable and
  consistent across every example).
- [ ] New and updated example workbooks are discoverable through the
  existing ribbon Examples gallery without further changes to that
  mechanism.
- [ ] The example workbooks are included in the release archive alongside
  the existing documentation, as already established for examples in
  ISSUE-0001.

## Notes

- Builds on the existing ribbon Examples gallery and release packaging,
  which already discover and ship whatever workbooks are present in
  `docs/examples/`; this issue is about having many more, better, and
  automatically produced workbooks for it to surface.
- Coordinate with the technical team on where the generation step fits in
  the existing build pipeline, and on the exact blue/white palette to use
  consistently across workbooks and (ideally) alongside the existing
  HTML documentation styling.
