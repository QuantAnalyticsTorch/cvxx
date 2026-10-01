---
id: SPEC-0012
title: Prefer registered names over handles in CVX.DESCRIBE output
issue: ISSUE-0012
status: implemented
created: 2026-09-30
---

## Status

Implemented in `src/excel/inspect.rs`: added `primary_identifier` and
`display_ref` helpers, updated `describe` and the `ConstraintSet`/`Problem`
arms of `describe_body` to use them. Added unit tests covering named/unnamed
objects and mixed named/unnamed cross-references. `cargo fmt`, `cargo
clippy --all-targets -- -D warnings`, and `cargo test --lib` all pass (128
tests).

## Objective

Change `CVX.DESCRIBE`'s output (SPEC-0007, `src/excel/inspect.rs::describe`)
so that a registry object's short registered name — when it has one — is
shown as the leading identifier instead of its full `cvx:<kind>:<uuid>`
handle, with the handle demoted to a secondary, parenthetical detail. Apply
the same treatment everywhere `describe_body` currently renders a *reference
to another* registry object by handle (a constraint set's member
constraints, a problem's objective/constraints/variables) — each such
reference should also prefer that referenced object's name when it has one.
Objects without a registered name are unaffected: the handle remains their
only identifier, exactly as today.

## Non-Objective

- No change to `CVX.SHAPE`, `CVX.TYPE`, `CVX.VALUE`, `CVX.STATUS`, or
  `CVX.OBJECTIVE_VALUE` output formats (SPEC-0007) — none of these render a
  handle or name as part of their return value today.
- No change to how a `handle` *argument* is resolved (`resolve_any` already
  accepts either a handle or a name interchangeably, per SPEC-0007; this
  specification only changes what is *printed*, not what is *accepted*).
- No change to `render_expression` (SPEC-0007) or its `var#<id>`/
  `param(<rows>x<cols>)` output. An `Expression` tree carries no names (only
  structural `Variable`/`Parameter` data), so there is no name available to
  prefer there — this remains a known, documented limitation (SPEC-0007's
  Non-Objective), unchanged by this specification.
- No change to the underlying handle format (`format_handle`/
  `parse_handle`, `src/core/handle.rs`), registry storage, or object
  identity/lifetime — this is a display-only change.
- No change to `CVX.DESCRIBE`'s truncation rule (`MAX_DESCRIBE_LEN = 500`,
  SPEC-0007), which continues to apply to the new output format unchanged.

## Interface

No Excel-facing function signatures change. `CVX.DESCRIBE(handle)` keeps
its existing signature and argument resolution (`run_handle`/`resolve_any`,
SPEC-0007); only the returned string's content changes.

### Changed internal helper (`src/excel/inspect.rs`)

```rust
/// Formats `obj`'s primary, human-readable identifier: its registered name
/// if it has one, else its handle. Used both for the leading identifier in
/// `describe()` and, via `display_ref`, for references to other objects
/// inside `describe_body`.
fn primary_identifier(obj: &RegistryObject) -> String {
    obj.name()
        .map(|name| format!("\"{name}\""))
        .unwrap_or_else(|| format_handle(obj.kind(), obj.uuid()))
}
```

`describe` (SPEC-0007) changes from:

```rust
fn describe(obj: &RegistryObject) -> String {
    let handle = format_handle(obj.kind(), obj.uuid());
    let name_suffix = obj
        .name()
        .map(|name| format!(" (\"{name}\")"))
        .unwrap_or_default();
    let body = describe_body(obj);
    truncate_describe(format!("{handle}{name_suffix}: {body}"))
}
```

to:

```rust
fn describe(obj: &RegistryObject) -> String {
    let primary = primary_identifier(obj);
    let handle = format_handle(obj.kind(), obj.uuid());
    // Only show the handle a second time when it isn't already the primary
    // identifier (i.e. only when the object has a name).
    let handle_suffix = if obj.name().is_some() {
        format!(" ({handle})")
    } else {
        String::new()
    };
    let body = describe_body(obj);
    truncate_describe(format!("{primary}{handle_suffix}: {body}"))
}
```

### New helper for cross-references inside `describe_body`

```rust
/// Resolves `uuid` (known to be of kind `kind`) to its registry entry and
/// returns its name (quoted, as in `primary_identifier`) if it has one,
/// else its handle. Falls back to the handle if the entry has since been
/// removed from the registry (should not normally occur, since referenced
/// objects are not independently deletable — see Error Handling).
fn display_ref(kind: HandleKind, uuid: Uuid) -> String {
    let name = match kind {
        HandleKind::Constr => Registry::global()
            .get_constraint_by_uuid(uuid)
            .and_then(|e| e.name),
        HandleKind::Var => Registry::global()
            .get_variable_by_uuid(uuid)
            .and_then(|e| e.name),
        HandleKind::Obj => Registry::global()
            .get_objective_by_uuid(uuid)
            .and_then(|e| e.name),
        _ => None, // only the three referenced kinds below are ever passed in
    };
    match name {
        Some(name) => format!("\"{name}\""),
        None => format_handle(kind, uuid),
    }
}
```

`describe_body`'s `ConstraintSet` and `Problem` arms (SPEC-0007) replace
every `format_handle(HandleKind::Constr, *uuid)` /
`format_handle(HandleKind::Var, *uuid)` /
`format_handle(HandleKind::Obj, e.objective)` call with the corresponding
`display_ref(HandleKind::Constr, *uuid)` /
`display_ref(HandleKind::Var, *uuid)` / `display_ref(HandleKind::Obj,
e.objective)` call; the surrounding `format!` templates, counts (`[{n}]`),
and comma-joining are otherwise unchanged.

## Data Model

Example output changes (illustrative, not exhaustive):

- Named variable, before: `cvx:var:1f2e... ("x"): variable 1x1`.
  After: `"x" (cvx:var:1f2e...): variable 1x1`.
- Unnamed variable: unchanged in both cases —
  `cvx:var:1f2e...: variable 1x1` (no name to prefer, no parenthetical
  added).
- Named problem referencing a named objective `"profit"` and two named
  constraints `"budget"`/`"nonneg"`, before:
  `cvx:prob:... ("myproblem"): problem: objective=cvx:obj:..., \
  constraints=[2]: cvx:constr:..., cvx:constr:..., variables=[1]: cvx:var:...`.
  After: `"myproblem" (cvx:prob:...): problem: objective="profit", \
  constraints=[2]: "budget", "nonneg", variables=[1]: "x"` (assuming the
  variable is also named `"x"`; an unnamed member of any of these lists
  still falls back to its handle, e.g. `constraints=[2]: "budget", \
  cvx:constr:9ab...`).

No changes to `ConstraintEntry`, `ProblemEntry`, or any other registry
struct — this is purely a formatting change over existing stored data.

## Error Handling

No new error conditions. `display_ref` cannot itself fail: a missing
registry entry (which should not occur, since `ConstraintSetEntry`/
`ProblemEntry` only ever store UUIDs of objects that existed at the time
they were built, and no function removes a registry entry once inserted)
degrades to showing the handle, matching `CVX.DESCRIBE`'s existing
graceful-degradation philosophy (SPEC-0007: "`CVX.DESCRIBE` never fails for
a handle that resolves to *some* registry object").

## Test Approach

Unit tests in `src/excel/inspect.rs`'s existing test module (SPEC-0007),
extending it with:

- `primary_identifier` returns the quoted name for a named object and the
  handle for an unnamed object.
- `describe()` on a named parameter/variable/expression/constraint/
  constraint-set/objective/problem/result shows the name first with the
  handle in parentheses immediately after; on an unnamed object of each
  kind, output is byte-for-byte identical to today's (regression check
  against the SPEC-0007 format for the no-name case).
- A constraint set containing a mix of named and unnamed member constraints
  renders each member independently (name where available, handle
  otherwise) in the same order as today.
- A problem whose objective, constraints, or variables are a mix of named
  and unnamed objects renders each independently, same rule.
- Truncation (`MAX_DESCRIBE_LEN`) still applies correctly to the new,
  generally-shorter output (a case with many long names still truncates at
  500 `char`s, same as SPEC-0007).

## Dependencies

- SPEC-0007 (result inspection and `CVX.DESCRIBE`) — this specification
  amends its `describe`/`describe_body` implementation in place; no new
  registry tables, handles, or Excel functions are introduced.
- ISSUE-0012 (parent issue).
