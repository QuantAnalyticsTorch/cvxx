# Creating optimization variables

Use `CVX.VARIABLE` to create decision variables that can later be referenced by
handles in expressions, constraints, and problems.

## Syntax

```excel
CVX.VARIABLE(rows, cols, [name])
```

- `rows`: positive integer number of rows.
- `cols`: positive integer number of columns.
- `name`: optional unique name for the variable. If omitted, the variable can
  only be referenced by its handle.

## Return value

A string handle of the form `cvx:var:<uuid>` on success, or `#VALUE!` if the
input is invalid.

## Examples

| Formula | Result |
|---|---|
| `=CVX.VARIABLE(1, 1)` | Scalar variable handle, e.g. `cvx:var:...` |
| `=CVX.VARIABLE(5, 1, "x")` | 5-element column vector named `x` |
| `=CVX.VARIABLE(1, 5, "y")` | 5-element row vector named `y` |
| `=CVX.VARIABLE(3, 4, "Z")` | 3×4 matrix variable named `Z` |

## Error conditions

- Non-numeric, zero, negative, or non-integer `rows`/`cols` return `#VALUE!`.
- `rows` or `cols` larger than the implementation limit (1,000,000) return
  `#VALUE!`; a diagnostic is written to `%TEMP%/cvxx.log`.
- A duplicate `name` returns `#VALUE!`.
