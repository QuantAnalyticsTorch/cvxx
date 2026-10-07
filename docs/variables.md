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

Use `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` (see [inspection.md](inspection.md))
to inspect a variable's shape or get a diagnostic description of it, by
handle or by name.

## Error conditions

- Non-numeric, zero, negative, or non-integer `rows`/`cols` return `#VALUE!`.
- `rows` or `cols` larger than the implementation limit (1,000,000) return
  `#VALUE!`; a diagnostic is written to `%TEMP%/cvxx.log`.
- A duplicate `name` returns `#VALUE!`.

## Restricting a variable to integer or binary values

By default a variable's entries may take any real value (subject to the
constraints you add). Some decisions are naturally whole numbers — how many
trucks to buy, how many units to produce — or yes/no choices — whether to
fund a particular project. `CVX.INTEGER` and `CVX.BINARY` declare that kind
of restriction on a variable (or part of one), turning on `cvxx`'s
mixed-integer solve path (see [problems.md](problems.md)) wherever the
restriction is used.

### Syntax

```excel
CVX.INTEGER(variable, [name])
CVX.BINARY(variable, [name])
```

- `variable`: a handle or name of a variable created by `CVX.VARIABLE`, or a
  `CVX.INDEX` selection directly over such a variable (to restrict only part
  of it). A numeric literal or any other kind of handle is rejected.
- `name`: optional unique name for the domain restriction.

`CVX.INTEGER` restricts the selected entries to whole numbers (positive,
negative, or zero — add ordinary constraints for any additional bounds).
`CVX.BINARY` restricts the selected entries to exactly `0` or `1`; this bound
is built into the restriction itself, since "binary" would otherwise be
indistinguishable from an unbounded integer entry.

A domain restriction is declared independently of any particular problem,
the same way a constraint is: the same variable may be used unrestricted in
one problem and integer/binary-restricted in another.

### Return value

A string handle of the form `cvx:dom:<uuid>` on success, or `#VALUE!` (see
Error conditions below).

### Examples

| Formula | Result |
|---|---|
| `=CVX.INTEGER("trucks")` | Restricts the whole `trucks` variable to integers |
| `=CVX.BINARY("select")` | Restricts the whole `select` variable to 0/1 |
| `=CVX.INTEGER(CVX.INDEX("x", 2, 1, 3, 1))` | Restricts a 3-row slice of `x` to integers |

Use the resulting handle anywhere `CVX.PROBLEM` or `CVX.CONSTRAINTS` accepts
a constraint handle/name (see [constraints.md](constraints.md)) to apply the
restriction to a specific problem.

### Error conditions

- An unresolvable `variable` handle/name, a numeric literal, or any handle
  kind other than a variable or a plain `CVX.INDEX` selection over one,
  returns `#VALUE!`.
- An expression built from anything other than a bare variable or a direct
  `CVX.INDEX` over one (e.g. `CVX.ADD`, `CVX.SUM`, a nested index) returns
  `#VALUE!`.
- A duplicate `name`, including one already used by a different kind of
  object, returns `#VALUE!`.
