# Inspecting results and describing registry objects

Six functions let you read back solved values and describe any `cvxx`
registry object, regardless of kind.

## Reading solved results

### Syntax

```excel
=CVX.VALUE(result, variable)
=CVX.STATUS(result)
=CVX.OBJECTIVE_VALUE(result)
```

- `result`: handle or name of a result created by `CVX.SOLVE`.
- `variable` (`CVX.VALUE` only): handle or name of a variable that was part
  of the solved problem. `CVX.VALUE` takes both arguments explicitly
  because a variable may appear in several problems solved at different
  times — there is no notion of "the most recent result for a variable".

Returns:

- `CVX.VALUE`: a single numeric cell for a `(1, 1)` variable, otherwise a
  `rows x cols` array of numeric cells, row-major.
- `CVX.STATUS`: `"optimal"`, `"infeasible"`, or `"unbounded"`.
- `CVX.OBJECTIVE_VALUE`: a numeric cell.

### Error conditions

- Unknown/wrong-kind `result` or `variable` handles/names return `#VALUE!`.
- `CVX.VALUE`/`CVX.OBJECTIVE_VALUE` on a non-`Optimal` result return
  `#VALUE!` (no variable values or objective value are stored).
- `CVX.VALUE` for a variable that was not part of the solved problem
  returns `#VALUE!`.

## Describing any registry object

### Syntax

```excel
=CVX.DESCRIBE(handle)
=CVX.SHAPE(handle)
=CVX.TYPE(handle)
```

- `handle`: a handle or registered name of **any** kind — parameter,
  variable, expression, constraint, constraint set, objective, problem, or
  result.

Returns:

- `CVX.DESCRIBE`: a human-readable diagnostic string describing the object
  (its handle, name if any, and kind-specific content). Truncated to 500
  characters (`"... (truncated)"` appended) for very large objects, such as
  parameters with many data points. This output is for diagnostics only and
  is not guaranteed to round-trip through `CVX.EXPRESSION`'s parser —
  original Excel-facing identifier names are not recoverable from a stored
  expression, so variables are shown as `var#<id>` and parameters as
  `param(<rows>x<cols>)`.
- `CVX.SHAPE`: `"<rows>x<cols>"`, only for parameters, variables, and
  expressions (an expression's shape is inferred from its structure).
- `CVX.TYPE`: one of `"parameter"`, `"variable"`, `"expression"`,
  `"constraint"`, `"constraint_set"`, `"objective"`, `"problem"`,
  `"result"`.

### Error conditions

- Unknown handle/name → `#VALUE!`.
- A name that matches more than one registry table → `#VALUE!` (should not
  normally occur — see "Cross-table name uniqueness" below).
- `CVX.SHAPE` on a constraint, constraint set, objective, problem, or
  result → `#VALUE!` (those object kinds have no shape).
- `CVX.SHAPE` on an expression whose shape cannot be inferred (a genuine
  shape mismatch between non-scalar operands) → `#VALUE!`.
- `CVX.DESCRIBE` never fails for a handle that resolves to *some* registry
  object — it degrades gracefully even if an expression's shape can't be
  inferred, showing `"(shape unavailable: ...)"` instead of failing.

## Cross-table name uniqueness

Every `CVX.*` function that accepts an optional `name` rejects a name that
is already registered under a **different** object kind (for example, a
parameter and a variable can no longer share the same name) with
`#VALUE!`. This guarantees a bare name always resolves to exactly one
registry object for `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE`, and for any
other function that resolves operands by name. See SPEC-0002's amendment
for the full rule.
