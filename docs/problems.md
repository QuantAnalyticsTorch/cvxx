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

`cvxrust` solves problems built from scalar (`1x1`) variables and
parameters: affine or convex quadratic objectives subject to affine
`<=`/`>=`/`=` constraints and convex quadratic `<=`/`>=` constraints, by
translating the problem into a conic program and delegating to the
`clarabel` solver. A quadratic term is any product of two
variable-dependent sub-expressions (e.g. `x * x` or `x * y`). Problems
outside this class fail with a descriptive error and return `#VALUE!` (no
result is stored):

- a variable or parameter with a shape other than `(1, 1)`;
- a term of degree 3 or higher (a product or quotient involving three or
  more variable-dependent sub-expressions);
- a quadratic **equality** constraint (`==`) — only quadratic `<=`/`>=`
  constraints are supported;
- a quadratic constraint whose coefficient matrix is not positive
  semidefinite (i.e. not convex);
- division by zero, or division by a variable-dependent term;
- a problem exceeding the solver's size limit (200 variables / 200
  constraints).

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
