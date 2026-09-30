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
- Parentheses for grouping
- Unary minus for negation

### Examples

| Formula | Meaning |
|---|---|
| `=CVX.EXPRESSION("x + y")` | Sum of variables `x` and `y` |
| `=CVX.EXPRESSION("2.5 * (A - b)", "profit")` | Named expression |
| `=CVX.EXPRESSION("-x / 3")` | Unary minus and division |

## Functional builders

These functions compose expressions from existing handles instead of strings:

```excel
=CVX.ADD(left_handle, right_handle, [name])
=CVX.SUB(left_handle, right_handle, [name])
=CVX.MUL(left_handle, right_handle, [name])
=CVX.DIV(left_handle, right_handle, [name])
=CVX.NEG(operand_handle, [name])
=CVX.SCALE(operand_handle, scalar, [name])
```

`left`, `right`, and `operand` can be parameter, variable, or expression
handles. `scalar` must be a numeric constant.

## Error conditions

- Invalid expression syntax returns `#VALUE!`.
- Unknown identifiers return `#VALUE!` and name the unresolved item.
- Non-numeric `scalar` in `CVX.SCALE` returns `#VALUE!`.
- Duplicate names return `#VALUE!`.
- Diagnostics are logged to `%TEMP%/cvxx.log`.

Use `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` (see [inspection.md](inspection.md))
to inspect an expression's inferred shape or get a diagnostic rendering of
it, by handle or by name.
