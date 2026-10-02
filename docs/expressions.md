# Building optimization expressions

Use `CVX.EXPRESSION` or the functional builders to construct lazy convex
optimization expressions that can later be used as objectives or constraints.

## String expressions

### Syntax

```excel
CVX.EXPRESSION(expr_string, [name])
```

- `expr_string`: a mathematical expression such as `"x + y"` or `"2.5 * (A - b)"`.
- `name`: optional unique name for the expression.

### Supported grammar

- Numeric constants: `1`, `2.5`, `-3`
- Identifier references: names of registered parameters, variables, or other
  named expressions
- Binary operators: `+`, `-`, `*`, `/` with standard precedence
- `@` for matrix multiplication, at the same precedence tier as `*`/`/`
  (left-associative)
- Postfix `.T` for transpose, binding tighter than unary minus and every
  binary operator (see "`@`/`.T` matrix operator syntax" below)
- Parentheses for grouping
- Unary minus for negation
- `sum(expr)` and `index(expr, row, col[, rows, cols])` function-call syntax
  (see "`sum()`/`index()` keyword syntax" below)

### Examples

| Formula | Meaning |
|---|---|
| `=CVX.EXPRESSION("x + y")` | Sum of variables `x` and `y` |
| `=CVX.EXPRESSION("2.5 * (A - b)", "profit")` | Named expression |
| `=CVX.EXPRESSION("-x / 3")` | Unary minus and division |
| `=CVX.EXPRESSION("sum(v)")` | Sum of every entry of vector `v` |
| `=CVX.EXPRESSION("index(v, 2, 1) + 3")` | Second entry of `v`, plus 3 |
| `=CVX.EXPRESSION("weights @ x")` | Matrix product of `weights` and `x` |
| `=CVX.EXPRESSION("x.T")` | Transpose of `x` (rows become columns) |

## Functional builders

These functions compose expressions from existing handles instead of strings:

```excel
=CVX.ADD(left_handle, right_handle, [name])
=CVX.SUB(left_handle, right_handle, [name])
=CVX.MUL(left_handle, right_handle, [name])
=CVX.DIV(left_handle, right_handle, [name])
=CVX.NEG(operand_handle, [name])
=CVX.SCALE(operand_handle, scalar, [name])
=CVX.SUM(operand_handle, [name])
=CVX.INDEX(operand_handle, row, col, [rows], [cols], [name])
=CVX.MATMUL(left_handle, right_handle, [name])
=CVX.TRANSPOSE(operand_handle, [name])
```

`left`, `right`, and `operand` can be parameter, variable, or expression
handles. `scalar` must be a numeric constant.

`CVX.ADD`/`CVX.SUB`/`CVX.MUL`/`CVX.DIV` work entrywise when either operand
is a vector or a matrix: a `(1, 1)` operand broadcasts against the other
side's shape, and operands of equal shape combine entry-by-entry (mismatched
non-broadcastable shapes return `#VALUE!`). `CVX.SUM` reduces every entry of
a (possibly vector/matrix-shaped) expression down to a single value —
combined with `CVX.MUL`, this gives a weighted total, e.g.
`=CVX.SUM(CVX.MUL(weights_handle, x_handle))`.

`CVX.INDEX` selects a contiguous rectangular sub-block of a (possibly
vector/matrix-shaped) expression: `rows` rows starting at the 1-based `row`,
and `cols` columns starting at the 1-based `col`. `rows`/`cols` each default
to `1` when omitted, so a single entry is just
`=CVX.INDEX(v_handle, 2, 1)`. A single row or column, or a general
sub-section, is selected by passing larger `rows`/`cols`:

| Formula | Selects |
|---|---|
| `=CVX.INDEX(v_handle, 2, 1)` | The single entry at row 2, column 1 |
| `=CVX.INDEX(M_handle, 2, 1, 1, 4)` | Row 2 of a matrix with 4 columns |
| `=CVX.INDEX(M_handle, 1, 3, 3, 1)` | Column 3 of a matrix with 3 rows |
| `=CVX.INDEX(M_handle, 2, 2, 2, 2)` | The 2x2 sub-block starting at row 2, column 2 |

`CVX.INDEX`'s operand may itself be the result of another `CVX.INDEX` (or
any other expression handle), so a single entry or sub-section of an
already-built expression can be singled out in turn.

`CVX.MATMUL` performs standard (2-D) matrix multiplication: `left`'s column
count must match `right`'s row count, producing a result with `left`'s row
count and `right`'s column count. Unlike `CVX.MUL`, a `(1, 1)` operand does
**not** broadcast through `CVX.MATMUL` — standard matrix multiplication has
no "scale by a single number" shortcut, so a mismatch between a `(1, 1)`
operand and anything else is still an error. `CVX.TRANSPOSE` turns a
`rows x cols` operand's rows into columns (and vice versa), producing a
`cols x rows` result; transposing a `(1, 1)` operand is a no-op.

| Formula | Result shape |
|---|---|
| `=CVX.MATMUL(weights_handle, x_handle)` | A `(1, 3)` weights row times a `(3, 1)` variable column: a `(1, 1)` scalar (a "dot product") |
| `=CVX.MATMUL(M_handle, x_handle)` | A `(2, 3)` matrix times a `(3, 1)` column: a `(2, 1)` column |
| `=CVX.TRANSPOSE(v_handle)` | A `(3, 1)` column becomes a `(1, 3)` row |

Both operand shapes are only checked lazily, the first time the resulting
expression's shape is actually needed (`CVX.DESCRIBE`/`CVX.SHAPE` or a
solve) — not when `CVX.MATMUL` itself is called — exactly like today's
`CVX.ADD`/`CVX.SUB`/`CVX.MUL`/`CVX.DIV`.

Like elementwise `CVX.MUL`, matrix multiplication of two operands that both
depend on an unknown variable (e.g. `CVX.MATMUL(x_handle, y_handle)` for two
variables) is not supported and returns `#VALUE!`; only a known
parameter/constant table applied to an unknown variable (or vice versa) is
supported today.

## `@`/`.T` matrix operator syntax

`CVX.MATMUL`/`CVX.TRANSPOSE` are also available as the infix `@` operator
and the postfix `.T` property directly inside a `CVX.EXPRESSION`/
`CVX.CONSTRAINT` string, mirroring NumPy's own `a @ b`/`a.T` syntax:

```excel
=CVX.EXPRESSION("weights @ x")
=CVX.EXPRESSION("x.T")
=CVX.EXPRESSION("sum(w.T @ Sigma @ w)")
=CVX.CONSTRAINT("weights @ x <= 100")
```

- `@` sits at the same precedence tier as `*`/`/` (left-associative), so
  `"A @ B * C"` parses as `"(A @ B) * C"` and `"A * B @ C"` as
  `"(A * B) @ C"` — exactly like Python itself.
- `.T` binds tighter than unary minus and every binary operator, and
  applies to the immediately preceding number, identifier, parenthesized
  expression, or function call: `"-A.T"` parses as `"-(A.T)"`,
  `"(A + B).T"` transposes the parenthesized sum, and `"A.T.T"` is a valid
  (no-op) double transpose.
- An identifier whose entire registered name literally ends in `.T` (e.g. a
  parameter named `"cost.T"`) can no longer be referenced by that full name
  inside a `CVX.EXPRESSION`/`CVX.CONSTRAINT` string — `"cost.T"` is always
  parsed as `Transpose(cost)` instead. The object itself is unaffected and
  remains fully usable by every handle-based function (including
  `CVX.MATMUL`/`CVX.TRANSPOSE` themselves); it is only unreachable by that
  literal text inside this one string grammar.

## `sum()`/`index()` keyword syntax

`CVX.SUM`/`CVX.INDEX` are also available as `sum(...)`/`index(...)`
function-call syntax directly inside a `CVX.EXPRESSION`/`CVX.CONSTRAINT`
string, instead of first building a separate handle:

```excel
=CVX.EXPRESSION("sum(v)")
=CVX.EXPRESSION("index(v, 2, 1)")
=CVX.CONSTRAINT("index(x, 2, 1) <= 5")
```

- `sum(expr)` takes exactly one argument — a parameter, variable, or
  expression identifier, or a nested call.
- `index(expr, row, col)` or `index(expr, row, col, rows, cols)` takes
  exactly 3 or 5 arguments. `row`/`col`/`rows`/`cols` must be bare positive
  integer literals (not identifiers or sub-expressions); omitting `rows`/
  `cols` (the 3-argument form) defaults both to `1`, exactly like
  `CVX.INDEX`.
- Both can be nested arbitrarily, e.g. `=CVX.EXPRESSION("sum(index(X, 1, 1,
  2, 2))")`.
- No other function names are recognized; an unrecognized name (e.g.
  `"foo(x)"`) returns `#VALUE!`.
- Unlike `CVX.SUM`/`CVX.INDEX` used as functional builders, a variable
  referenced only through `sum(...)`/`index(...)` *inside a string* is
  still swept into a problem's variable list by `CVX.PROBLEM`, since its
  name is correctly recorded as a dependency during string resolution.

## Error conditions

- Invalid expression syntax returns `#VALUE!`.
- Unknown identifiers return `#VALUE!` and name the unresolved item.
- Non-numeric `scalar` in `CVX.SCALE` returns `#VALUE!`.
- Mismatched, non-broadcastable shapes between the two operands of
  `CVX.ADD`/`CVX.SUB`/`CVX.MUL`/`CVX.DIV` return `#VALUE!`.
- An incompatible `CVX.MATMUL`/`@` pair (left operand's column count not
  equal to the right operand's row count) returns `#VALUE!` with a
  matrix-multiplication-specific message, naming both operands' shapes —
  distinct from, and never confused with, the elementwise shape-mismatch
  error above.
- Matrix multiplication (`CVX.MATMUL`/`@`) of two operands that both depend
  on an unknown variable returns `#VALUE!`, the same restriction elementwise
  `CVX.MUL`/`*` already has today.
- Non-numeric, non-integer, zero, or negative `row`/`col`/`rows`/`cols` in
  `CVX.INDEX` or `index(...)` return `#VALUE!`.
- A `CVX.INDEX`/`index(...)` selection that extends beyond the operand's
  actual shape returns `#VALUE!`, naming the requested row/column range and
  the operand's actual shape.
- An unrecognized `sum(...)`/`index(...)` function name, or the wrong
  number of arguments, returns `#VALUE!`.
- Duplicate names return `#VALUE!`.
- Diagnostics are logged to `%TEMP%/cvxx.log`.

Use `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` (see [inspection.md](inspection.md))
to inspect an expression's inferred shape or get a diagnostic rendering of
it, by handle or by name.
