---
id: ISSUE-0009
title: Display, list, and clear cached optimization objects in Excel
priority: should
status: draft
created: 2026-09-29
---

## Background

As users build optimization models in Excel, they create many opaque handles for parameters, variables, expressions, constraints, problems, and results. Over the course of a modeling session the workbook can accumulate a large number of these objects, and users need a way to see what is currently stored, what each object represents, and how objects relate to one another. Users also need a safe way to remove stale or unwanted objects without restarting Excel. Without inventory and cleanup capabilities, users struggle to debug formulas, manage memory, or share workbooks with colleagues.

## Goal

Give Excel users clear visibility into all cached optimization objects, let them inspect individual objects directly from the worksheet, and let them clear the cache selectively.

## Acceptance Criteria

- [ ] A worksheet function returns a list of all objects currently in the cache.
- [ ] The list includes, at minimum, each object's handle, user-defined name (if any), object type, and shape or size.
- [ ] The list can be filtered by object type (for example, show only variables or only constraints).
- [ ] The list can be sorted by name or by creation order.
- [ ] A worksheet function displays the full details of a single object given its handle or name.
- [ ] Object details include the object's type, dimensions, user-defined name, and a human-readable summary of its contents.
- [ ] For expression, constraint, and problem objects, the summary shows how named parameters and variables are referenced.
- [ ] The functions handle empty caches gracefully by returning a clear indicator rather than an error.
- [ ] Unknown, expired, or invalid handles return an Excel-visible error with a logged diagnostic.
- [ ] A worksheet function clears objects from the cache.
- [ ] The clear function accepts an optional filter argument that limits clearing to one or more object types.
- [ ] The clear function accepts an optional filter argument that limits clearing to specific handles or names.
- [ ] When no filter is provided, the clear function removes all cached objects.
- [ ] Clearing objects that are referenced by other cached objects is handled safely (for example, by returning an error or by cascading the removal).
- [ ] Clearing an unknown, expired, or invalid handle returns an Excel-visible error with a logged diagnostic.
- [ ] The inventory and clearing capabilities are documented with examples showing how to audit and clean up a model.

## Notes

- The list output should fit naturally into an Excel spill range or array formula.
- Consider whether the ribbon should offer a dedicated "Model Explorer" button that dumps the inventory to a new worksheet and optionally clears selected objects.
- Define the exact behavior for clearing referenced objects during the technical analysis phase.
