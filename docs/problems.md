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
`Unbounded` outcomes. **`cvxrust` does not yet implement a solver**, so
`CVX.SOLVE` currently always fails and returns `#VALUE!`; no result is
stored in that case.

## Error conditions

- Unknown or wrong-kind objective/problem handles return `#VALUE!`.
- Unresolvable constraint handles/names within `constraints` return `#VALUE!`.
- A solver failure (including the current "not implemented" placeholder)
  returns `#VALUE!` and does not create a result entry.
- Duplicate names return `#VALUE!`.
- Diagnostics are logged to `%TEMP%/cvxx.log`.
