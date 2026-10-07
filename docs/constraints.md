# Building optimization constraints

Use `CVX.CONSTRAINT` or the relational functional builders to construct
constraints from parameters, variables, and expressions, then combine them
into a constraint set with `CVX.CONSTRAINTS`.

## String constraints

### Syntax

```excel
CVX.CONSTRAINT(constraint_string, [name])
```

- `constraint_string`: a relational expression such as `"x + y <= 10"`.
- `name`: optional unique name for the constraint.

### Supported grammar

A constraint string is an [expression](expressions.md) on each side of a
single relational operator:

- `<=`
- `>=`
- `==`

Exactly one relational operator is allowed per constraint string; it cannot
be nested inside parentheses or repeated.

### Examples

| Formula | Meaning |
|---|---|
| `=CVX.CONSTRAINT("x + y <= 10")` | Sum of `x` and `y` is at most 10 |
| `=CVX.CONSTRAINT("profit >= cost * 1.1", "margin")` | Named constraint |
| `=CVX.CONSTRAINT("A == b")` | Equality constraint |
| `=CVX.CONSTRAINT("index(x, 2, 1) <= 5")` | Only the second entry of vector `x` is at most 5 |

## Functional builders

These functions build a constraint from two operands instead of parsing a
string:

```excel
=CVX.LESS_THAN(left, right, [name])
=CVX.GREATER_THAN(left, right, [name])
=CVX.EQUAL(left, right, [name])
```

`left` and `right` can be a parameter, variable, or expression handle, a
registered name, or a bare numeric literal.

## Combining constraints

```excel
CVX.CONSTRAINTS(constraints, [name])
```

- `constraints`: a range containing constraint handles/names and/or domain
  handles/names (from `CVX.INTEGER`/`CVX.BINARY`, see
  [variables.md](variables.md)), mixed freely. Blank cells are skipped.
- `name`: optional unique name for the resulting constraint set.

Returns a `cvx:constrset:<uuid>` handle referencing the ordered list of
resolved constraints and/or domain restrictions, ready to pass to the
problem builder. `CVX.PROBLEM`'s `constraints` argument accepts the same mix
of constraint and domain handles/names directly, with or without going
through `CVX.CONSTRAINTS` first (see [problems.md](problems.md)).

### Example

```excel
=CVX.CONSTRAINTS(A1:A5, "my_constraints")
=CVX.CONSTRAINTS({"cvx:constr:...", "cvx:dom:..."}, "mixed_set")
```

## Error conditions

- Invalid constraint syntax (missing, duplicated, or nested relational
  operator) returns `#VALUE!`.
- Unknown identifiers return `#VALUE!` and name the unresolved item.
- A `CVX.CONSTRAINTS` entry that is not a known constraint, constraint-set,
  or domain handle/name returns `#VALUE!`.
- An empty constraint set (no resolvable constraints or domains) returns
  `#VALUE!`.
- Duplicate names return `#VALUE!`.
- Diagnostics are logged to `%TEMP%/cvxx.log`.

Use `CVX.DESCRIBE`/`CVX.TYPE` (see [inspection.md](inspection.md)) to get a
diagnostic description of a constraint, constraint set, or domain
restriction, by handle or by name (constraints, constraint sets, and domain
restrictions have no shape, so `CVX.SHAPE` does not apply to them).
