# Creating optimization parameters

Use `CVX.PARAMETER` to import fixed numeric data from an Excel range. Parameters
can be referenced by name in expressions and constraints, or inspected by
their returned handle.

## Syntax

```excel
CVX.PARAMETER(range, [name])
```

- `range`: a rectangular Excel range containing numeric values. Empty cells
  are treated as zero.
- `name`: optional unique name for the parameter. If omitted, use the
  returned handle to reference it.

## Return value

A string handle of the form `cvx:param:<uuid>` on success, or `#VALUE!` if the
input is invalid.

## Example

If `B2:B4` contains numeric demand values, create a named parameter with:

```excel
=CVX.PARAMETER(B2:B4, "demand")
```

The parameter can then be referenced by name in an expression, for example:

```excel
=CVX.EXPRESSION("demand * unit_price", "revenue")
```

Parameters can contain a scalar, vector, or matrix. Use
[`CVX.SHAPE`](inspection.md) or [`CVX.DESCRIBE`](inspection.md) to inspect the
stored object's shape or description.

## Error conditions

- An empty or non-rectangular range, or a range containing a non-numeric cell,
  returns `#VALUE!`.
- A duplicate name, including a name already used by another object kind,
  returns `#VALUE!`.
