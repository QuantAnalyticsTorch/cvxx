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
- `CVX.STATUS`: `"optimal"`, `"infeasible"`, `"unbounded"`, or
  `"stopped_at_limit"` (a mixed-integer solve, see
  [problems.md](problems.md), that was stopped by its time/node limit
  after finding a feasible solution but before proving it optimal).
- `CVX.OBJECTIVE_VALUE`: a numeric cell.

### Error conditions

- Unknown/wrong-kind `result` or `variable` handles/names return `#VALUE!`.
- `CVX.VALUE`/`CVX.OBJECTIVE_VALUE` succeed for both `Optimal` and
  `StoppedAtLimit` results; on an `Infeasible`/`Unbounded` result they
  return `#VALUE!` (no variable values or objective value are stored).
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
  variable, expression, constraint, constraint set, objective, problem,
  domain, or result.

Returns:

- `CVX.DESCRIBE`: a human-readable diagnostic string describing the object
  (its handle, name if any, and kind-specific content). Truncated to 500
  characters (`"... (truncated)"` appended) for very large objects, such as
  parameters with many data points. This output is for diagnostics only and
  is not guaranteed to round-trip through `CVX.EXPRESSION`'s parser. A
  variable or parameter referenced by a formula (in an expression,
  constraint, or objective) is shown by its current registered name when it
  has one, else by its structural placeholder (`var#<id>` or
  `param(<rows>x<cols>)`) — for example, `"total": expression 1x1 =
  ("x") * ("x") + var#7` shows the named variable `x` twice and an
  unnamed variable as `var#7`. A `CVX.INDEX`/`index(...)`-built expression
  is shown as `<operand>[row, col]` for a single entry, or
  `<operand>[r1:r2, c1:c2]` for a row, column, or general sub-section —
  for example, `"v"[2, 1]` or `"M"[1:2, 1:3]`. A `CVX.MATMUL`/`@`-built
  expression is shown as `(<left>) @ (<right>)`, and a `CVX.TRANSPOSE`/
  `.T`-built expression as `(<operand>).T` — for example,
  `"weights" @ "x"` renders as `("weights") @ ("x")`. A domain restriction
  (from `CVX.INTEGER`/`CVX.BINARY`) is shown as `domain integer: <target>`
  or `domain binary: <target>`, reusing the same `CVX.INDEX` rendering for
  `<target>` — for example, `"x_int" (cvx:dom:...): domain integer: "x"`
  for a whole-variable restriction named `x_int` over a variable named
  `x`, or `domain binary: "x"[2:3, 1]` for an unnamed sub-block
  restriction.
- `CVX.SHAPE`: `"<rows>x<cols>"`, only for parameters, variables, and
  expressions (an expression's shape is inferred from its structure).
- `CVX.TYPE`: one of `"parameter"`, `"variable"`, `"expression"`,
  `"constraint"`, `"constraint_set"`, `"objective"`, `"problem"`,
  `"domain"`, `"result"`.

### Error conditions

- Unknown handle/name → `#VALUE!`.
- A name that matches more than one registry table → `#VALUE!` (should not
  normally occur — see "Cross-table name uniqueness" below).
- `CVX.SHAPE` on a constraint, constraint set, objective, problem, domain,
  or result → `#VALUE!` (those object kinds have no shape).
- `CVX.SHAPE` on an expression whose shape cannot be inferred (a genuine
  shape mismatch between non-scalar operands, an incompatible
  `CVX.MATMUL`/`@` pair, or a `CVX.INDEX`/`index(...)` selection that
  extends beyond its operand's actual shape) → `#VALUE!`.
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
