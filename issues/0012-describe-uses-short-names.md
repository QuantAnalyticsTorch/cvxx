---
id: ISSUE-0012
title: Prefer short registered names over full handles when describing objects
priority: should
status: draft
created: 2026-09-30
---

## Background

Every object a user creates in `cvxx` (parameters, variables, expressions, constraints, problems, results, etc.) is tracked internally using a handle — a long, system-generated identifier. Users can optionally give these objects a short, memorable name instead. Today, when a user inspects an object with `CVX.DESCRIBE`, the output identifies the object primarily by its full handle, even when a much shorter name is available. This makes worksheets and diagnostic output harder to read and needlessly verbose, especially when a user is scanning many describe results at once.

## Goal

When a user inspects an object that has been given a short name, that name is used to identify the object wherever it is practical to do so, instead of the longer system handle. Handles remain available for objects that were never given a name, and the underlying reference/identity of the object is unaffected. Users get shorter, more readable, and easier-to-scan diagnostic output.

## Acceptance Criteria

- [ ] For an object that has been given a name, inspecting it shows the short name as its primary identifier instead of the full handle.
- [ ] For an object that has not been given a name, behavior is unchanged (the handle continues to be shown, since no shorter alternative exists).
- [ ] Users can still determine the object's underlying handle if needed (e.g., it remains visible or retrievable, just no longer the primary/leading identifier).
- [ ] This change does not alter the actual identity, behavior, or lifetime of any object — it only affects how it is displayed.
- [ ] Any other place in the product that currently surfaces a full handle where a name is available and would improve readability is reviewed for the same treatment.

## Notes

- Applies at least to `CVX.DESCRIBE`; the technical team should confirm whether other diagnostic or informational outputs have the same opportunity.
- Out of scope: changing how users assign names, or the underlying handle format itself.
