# Building and solving optimization problems

Use `CVX.MINIMIZE`/`CVX.MAXIMIZE` to declare an objective, `CVX.PROBLEM` to
combine an objective with constraints, and `CVX.SOLVE` to solve it.

## Objectives

### Syntax

```excel
=CVX.MINIMIZE(objective, [name])
=CVX.MAXIMIZE(objective, [name])
```

- `objective`: a handle, name, or numeric literal for the expression to
  optimize (resolved the same way as `CVX.LESS_THAN`'s operands).
- `name`: optional unique name for the objective.

Returns a `cvx:obj:<uuid>` handle.

## Problems

### Syntax

```excel
=CVX.PROBLEM(objective, constraints, [name])
```

- `objective`: handle or name of an objective created by `CVX.MINIMIZE` or
  `CVX.MAXIMIZE`.
- `constraints`: one of:
  - blank — the problem has no constraints;
  - a single cell naming a constraint set (from `CVX.CONSTRAINTS`) — that
    set's constraints are used directly;
  - a range of constraint handles or names — flattened row-major, blank
    cells skipped.
- `name`: optional unique name for the problem.

Returns a `cvx:prob:<uuid>` handle.

## Solving

### Syntax

```excel
=CVX.SOLVE(problem, [name])
```

- `problem`: handle or name of a problem created by `CVX.PROBLEM`.
- `name`: optional unique name for the result.

Returns a `cvx:result:<uuid>` handle on `Optimal`, `Infeasible`, or
`Unbounded` outcomes.

`cvxrust` solves affine or convex quadratic objectives subject to affine
`<=`/`>=`/`=` constraints and convex quadratic `<=`/`>=` constraints, by
translating the problem into a conic program and delegating to the
`clarabel` solver. A quadratic term is any product of two
variable-dependent sub-expressions (e.g. `x * x` or `x * y`).

Variables and parameters may be a vector or a matrix, not just a single
number (`1x1`): constraints broadcast per-entry, the same way
`CVX.ADD`/`CVX.SUB`/`CVX.MUL`/`CVX.DIV` already broadcast when building
expressions — a `(1, 1)` operand broadcasts against the other side's
shape, equal shapes combine entrywise, and a declared constraint produces
one solved row per output entry. The **objective**, however, must still
evaluate to a single value (shape `1x1`); use `CVX.SUM(operand, [name])` to
reduce a vector/matrix expression's entries down to one number (e.g.
`=CVX.SUM(CVX.MUL(weights, x))` for a weighted total) — a vector/matrix
variable may otherwise be declared and freely used in constraints without
ever appearing in the objective. Quadratic support (`x * x`, `x * y`)
remains limited to objectives/constraint sides built exclusively from
`1x1` variables and parameters — once any vector/matrix (non-`1x1`)
variable or parameter, or `CVX.SUM`, appears anywhere in an objective or a
constraint side, that side is restricted to affine (degree-1) terms.

Problems outside this class fail with a descriptive error and return
`#VALUE!` (no result is stored):

- a term of degree 3 or higher (a product or quotient involving three or
  more variable-dependent sub-expressions);
- a product or quotient of two variable-dependent terms where a
  vector/matrix (non-`1x1`) variable or parameter, or `CVX.SUM`, is
  involved anywhere in that objective or constraint side (quadratic terms
  remain supported only between operands built exclusively from `1x1`
  variables and parameters);
- an objective that does not evaluate to a single value (e.g. a bare
  vector/matrix variable used directly as the objective, without
  `CVX.SUM`);
- mismatched, non-broadcastable shapes on the two sides of a constraint,
  or the two operands of `CVX.ADD`/`CVX.SUB`/`CVX.MUL`/`CVX.DIV`;
- a quadratic **equality** constraint (`==`) — only quadratic `<=`/`>=`
  constraints are supported;
- a quadratic constraint whose coefficient matrix is not positive
  semidefinite (i.e. not convex);
- division by zero, or division by a variable-dependent term;
- a problem exceeding the solver's size limit (200 scalar variables / 200
  scalar constraint rows — counting every entry of every vector/matrix
  variable and every broadcast constraint row, not just the number of
  declared variables/constraints).

## Error conditions

- Unknown or wrong-kind objective/problem handles return `#VALUE!`.
- Unresolvable constraint handles/names within `constraints` return `#VALUE!`.
- A solver failure (an unsupported problem type, reported as a descriptive
  error message) returns `#VALUE!` and does not create a result entry.
  `Infeasible` and `Unbounded` outcomes are not failures: they store a
  result entry with that status, same as `Optimal`.
- Duplicate names return `#VALUE!`.
- Diagnostics are logged to `%TEMP%/cvxx.log`.

## Inspecting results

Once `CVX.SOLVE` returns a result handle, use:

```excel
=CVX.STATUS(result)
=CVX.OBJECTIVE_VALUE(result)
=CVX.VALUE(result, variable)
```

to read back the solve status, the objective value, and each variable's
solved value(s) (a scalar for a `(1, 1)` variable, a row-major array
otherwise). `CVX.OBJECTIVE_VALUE` and `CVX.VALUE` return `#VALUE!` for a
non-`Optimal` result, since no objective/variable values are stored for
`Infeasible`/`Unbounded` outcomes. `CVX.DESCRIBE`/`CVX.TYPE` (see
[inspection.md](inspection.md)) also work on result, problem, and objective
handles, alongside every other object kind. See
[inspection.md](inspection.md) for full details.
