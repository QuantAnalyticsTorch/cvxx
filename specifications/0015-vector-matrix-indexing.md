---
id: SPEC-0015
title: Indexing a vector/matrix expression, and sum()/index() keyword syntax in expression strings
issue: ISSUE-0015
status: implemented
created: 2026-10-02
---

## Objective

Let users refer to a single entry, row, column, or rectangular sub-section
of a vector/matrix parameter, variable, or expression — for use anywhere
else a `cvxx` expression, objective, or constraint operand is accepted —
via one new `Expression::Index` variant in `cvxrust` and one new Excel
function, `CVX.INDEX`. `Index` always selects a contiguous, rectangular,
row-major sub-block (`rows` rows starting at `row`, `cols` columns starting
at `col`) of its operand's shape; a single entry is the `rows == cols == 1`
case, a single row or column is the `rows == 1` or `cols == 1` case, and
any other rectangular region is the general case — one primitive covers
all four acceptance-criteria scenarios. `CVX.INDEX`'s operand may itself be
the result of another `CVX.INDEX` (or any other expression handle),
matching the issue's requirement to index "the result of another formula".

Because `cvxrust::Expression` and its diagnostic shape-inference/rendering
(`src/analytics/shape.rs`) and solve-time reduction (`cvxrust/src/
reduce.rs`) are already generalized to arbitrary vector/matrix shapes by
SPEC-0014 (implemented), adding `Index` is an additive extension of that
same, already-landed machinery — a new match arm in each of
`infer_shape`/`render_expression`/`check_expr_shapes`/`linearize_shaped`/
`quadratize`, following exactly the pattern SPEC-0014 established for
`Sum` — not a new subsystem. This specification supersedes ISSUE-0015's
note that it "does not depend on ISSUE-0014 and could be delivered before,
after, or alongside it": SPEC-0014 is implemented in this repository today,
so this specification is written against (and extends) it directly.

In addition, both the new `Index` capability and the existing `Sum`
capability (SPEC-0014, delivered so far only as the standalone `CVX.INDEX`/
`CVX.SUM` functional builders) must also be usable as `sum(...)`/
`index(...)` keyword syntax directly inside a `CVX.EXPRESSION` string —
e.g. `CVX.EXPRESSION("sum(v)")` or `CVX.EXPRESSION("index(v, 2, 1) + 3")`
— per the amended ISSUE-0015 acceptance criteria. This is the "future
specification extending the grammar itself" both SPEC-0004 and SPEC-0014
explicitly left open, and is delivered here as a single, minimal grammar
extension (one new AST node, `Expr::Call`) that recognizes exactly the two
keywords `sum` and `index` and resolves each to the same
`cvxrust::Expression::Sum`/`Expression::Index` construction the functional
builders already produce — not a general user-extensible function-call
mechanism. See Interface/"Grammar extension" below.

## Non-Objective

- Non-contiguous or strided selection (e.g. "every other entry", a
  reversed range, or an arbitrary list of positions). `Index` always
  selects one contiguous rectangular row/column range.
- Negative or "from-the-end" positions (e.g. "last row"). `row`/`col` are
  always 1-based positions counted from the start, matching `CVX.VARIABLE`'s
  `rows, cols` convention (`docs/variables.md`).
- Any change to `CVX.EXPRESSION`'s string grammar **beyond** recognizing
  exactly the two keywords `sum(...)` and `index(...)` as function-call
  syntax (see Interface/"Grammar extension"). In particular:
  - No bracket/subscript syntax (e.g. `v[1,2]`) — `index(...)` is the only
    spelling for indexing inside a formula string.
  - No other function keywords (e.g. `sum_squares`, `norm`) — SPEC-0004's
    deferred "general function calls inside expression strings" remains
    only partially addressed: `Expr::Call` resolves exactly `sum`/`index`
    and rejects every other name with a clear "unknown function" error
    (see Error Handling), rather than introducing a general,
    user-extensible function mechanism. A future specification may widen
    this to other named functions; this one does not.
  - No change to `CVX.CONSTRAINT`'s grammar beyond inheriting
    `parse`'s new `sum(...)`/`index(...)` support in each operand
    (`parse_constraint` already calls the same `parse_expr` used by
    `parse`, so this follows for free — see Interface).
- Quadratic (degree-2) terms built from one or more `Index` operands (e.g.
  `CVX.INDEX(X, 1, 1) * CVX.INDEX(X, 1, 2)`, or `CVX.INDEX(X, 1, 1) *
  CVX.INDEX(X, 1, 1)`). Like `Sum`, the presence of `Index` anywhere in an
  objective or one constraint side unconditionally forces that whole side
  through the affine-only `linearize_shaped` path (see Data Model); a
  product of two non-constant terms on that path already fails with the
  existing `VECTOR_MUL_ERROR` (SPEC-0014), unchanged and un-special-cased
  here. General quadratic support over indexed/vector/matrix terms remains
  ISSUE-0016's scope.
- Matrix-vector/matrix-matrix multiplication and transpose (SPEC-0014
  Non-Objective, unchanged and still out of scope).
- Fixing the pre-existing functional-builder dependency-tracking gap
  (SPEC-0014: a variable reachable only through functional-builder handles
  — never through a `CVX.EXPRESSION`/`CVX.CONSTRAINT` string — is not swept
  into `problem.variables` by `CVX.PROBLEM`'s `expand_variable_dependencies`,
  `src/excel/problem.rs`). The `CVX.INDEX`/`CVX.SUM` functional builders
  keep this limitation unchanged, exactly as `CVX.ADD`/etc. already do:
  `insert_expression` is called with an empty `dependencies` list. This
  specification does not change that. A `sum(...)`/`index(...)` call
  written inside a `CVX.EXPRESSION`/`CVX.CONSTRAINT` **string**, by
  contrast, already gets this correctly today, for free, simply because
  `resolve_expr`'s existing identifier-walking dependency tracking
  (`src/analytics/resolve.rs`) recurses into `Expr::Call`'s operand exactly
  like it does for `Add`/`Mul`/etc. — this is a direct, welcome consequence
  of the grammar extension below, not additional work it requires.
- Any change to `CVX.PARAMETER`, `CVX.VARIABLE`, `CVX.PROBLEM`,
  `CVX.SOLVE`, `CVX.VALUE`, `CVX.STATUS`, `CVX.OBJECTIVE_VALUE`, or any
  other existing Excel-facing function signature or behavior.
  `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` require only the same kind of
  small, additive match-arm change SPEC-0014 already made for `Sum`.
- Raising `MAX_VARIABLES`/`MAX_CONSTRAINTS`, or changing
  `MAX_ITERATIONS`/`PSD_TOLERANCE`/`MAX_DESCRIBE_LEN`. `Index` introduces
  no new variables or constraint rows; it is a pure selection over an
  already-reduced operand.
- Assigning a value to, or otherwise mutating, part of a vector/matrix
  through an index (there is no "write" side to this feature — `Index` is
  read-only, exactly like every other `Expression` node).

## Interface

### `cvxrust::Expression` gains one new variant (`cvxrust/src/model.rs`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // ... existing variants unchanged ...
    /// A contiguous, rectangular, row-major sub-block of a (possibly
    /// vector/matrix-shaped) expression: `rows` rows starting at
    /// `row_start`, `cols` columns starting at `col_start`, all 0-based
    /// (SPEC-0015). Covers a single entry (`rows == cols == 1`), a single
    /// row (`rows == 1`) or column (`cols == 1`), or any other rectangular
    /// sub-section.
    Index {
        expr: Box<Expression>,
        row_start: usize,
        col_start: usize,
        rows: usize,
        cols: usize,
    },
}

impl Expression {
    /// Creates a sub-block index expression. `rows`/`cols` are always
    /// `>= 1` for any `Index` built through `CVX.INDEX` (enforced at the
    /// `cvxx` boundary by reusing `data::parse_dimension`/
    /// `parse_optional_dimension`); this constructor does not itself
    /// re-validate positivity or bounds, consistent with how
    /// `Variable::new`/`Expression::from_parameter` never re-validate
    /// `shape` either.
    pub fn index(
        expr: Expression,
        row_start: usize,
        col_start: usize,
        rows: usize,
        cols: usize,
    ) -> Self {
        Expression::Index {
            expr: Box::new(expr),
            row_start,
            col_start,
            rows,
            cols,
        }
    }
}
```

No other public `cvxrust` struct/enum/function signature changes.

### New Excel function (`src/excel/expression.rs`)

```
CVX.INDEX(operand, row, col, [rows], [cols], [name])
```

- `operand`: a parameter, variable, or expression handle, or a registered
  name (resolved identically to every other functional builder's operand,
  via the existing `resolve_handle_arg`). Not a bare numeric literal,
  consistent with `CVX.ADD`/`CVX.NEG`/etc. (only `CVX.SCALE`'s `scalar`
  argument, and constraint relation operands via `resolve_operand`, accept
  bare numbers today).
- `row`, `col`: required positive integers — the 1-based starting row and
  column of the selection, matching `CVX.VARIABLE`'s existing `rows, cols`
  convention (`docs/variables.md`). Parsed with the existing
  `data::parse_dimension`.
- `rows`, `cols`: optional positive integers — how many rows/columns to
  select, each defaulting to `1` when omitted (so the common "single
  entry" case is just `CVX.INDEX(operand, row, col)`). Parsed with the new
  `data::parse_optional_dimension` (see Data Model).
- `name`: optional unique name for the resulting expression, as with every
  other functional builder.
- Returns: a string handle of the form `cvx:expr:<uuid>`, exactly like
  `CVX.SUM`/`CVX.NEG`/etc.

```rust
/// `CVX.INDEX(operand, row, col, [rows], [cols], [name])` — selects a
/// contiguous rectangular sub-block of a (possibly vector/matrix-shaped)
/// expression (SPEC-0015).
#[export_name = "CVX.INDEX"]
pub extern "system" fn cvx_index(
    operand: LPXLOPER12,
    row: LPXLOPER12,
    col: LPXLOPER12,
    rows: LPXLOPER12,
    cols: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    let result = run_index(operand, row, col, rows, cols, name);
    to_xloper_result(result, "CVX.INDEX")
}

fn run_index(
    operand: LPXLOPER12,
    row: LPXLOPER12,
    col: LPXLOPER12,
    rows: LPXLOPER12,
    cols: LPXLOPER12,
    name: LPXLOPER12,
) -> Result<String, CvxError> {
    let operand_expr = resolve_handle_arg(operand)?;
    let row = data::parse_dimension(&Variant::from_xloper(row))?;
    let col = data::parse_dimension(&Variant::from_xloper(col))?;
    let rows = data::parse_optional_dimension(&Variant::from_xloper(rows), 1)?;
    let cols = data::parse_optional_dimension(&Variant::from_xloper(cols), 1)?;
    let name = data::parse_optional_name(&Variant::from_xloper(name))?;

    let operand_shape = crate::analytics::shape::infer_shape(&operand_expr)?;
    crate::analytics::shape::check_index_bounds(operand_shape, row, col, rows, cols)?;

    let expr = Expression::index(operand_expr, row - 1, col - 1, rows, cols);
    Registry::global().insert_expression(name, expr, vec![])
}
```

Registered in `xlAutoOpen` (`src/excel/mod.rs`) alongside the other
functional builders, following the existing `Reg::add` call convention
(argument names `"operand, row, col, rows, cols, name"`, one help string
per argument).

### `data::parse_optional_dimension` (`src/data/mod.rs`)

```rust
/// Parses an optional positive-integer dimension argument (`CVX.INDEX`'s
/// `rows`/`cols`). Returns `default` when the argument is missing/blank;
/// otherwise applies the same validation as `parse_dimension` (non-numeric,
/// non-integer, zero, negative, or over `MAX_DIMENSION` all reject).
pub fn parse_optional_dimension(value: &Variant, default: usize) -> Result<usize, CvxError> {
    if is_blank(value) {
        return Ok(default);
    }
    parse_dimension(value)
}
```

(`is_blank` is already a private helper in this module, reused unchanged.)

### Grammar extension: `sum(...)`/`index(...)` keyword syntax (`src/analytics/ast.rs`, `src/analytics/parser.rs`, `src/analytics/resolve.rs`)

`CVX.EXPRESSION` (and, since `parse_constraint` already reuses the same
`parse_expr`, `CVX.CONSTRAINT`) strings gain exactly two recognized
function-call keywords, `sum` and `index`, resolving to the same
`cvxrust::Expression::Sum`/`Expression::Index` the functional builders
already produce. One new AST node covers both (and leaves room for a
future specification to recognize more names without another AST change):

```rust
// src/analytics/ast.rs
pub enum Expr {
    // ... existing variants unchanged ...
    /// A function-call-syntax node, e.g. `sum(v)` or `index(v, 2, 1)`.
    /// Only `name == "sum"` (exactly 1 argument) and `name == "index"`
    /// (exactly 3 or 5 arguments) are recognized by `resolve` (SPEC-0015);
    /// any other `name`, or a recognized `name` with the wrong argument
    /// count, is a resolve-time error (see Error Handling) — the parser
    /// itself does not validate arity, staying as generic/simple as the
    /// rest of this module.
    Call { name: String, args: Vec<ExprNode> },
}
```

Tokenizer (`parser.rs`) gains one new token, `Comma`, for `,` — the only
new lexical symbol needed.

Grammar (replacing the existing doc-comment grammar at the top of
`parser.rs`):

```text
primary  := number | call | identifier | '(' expr ')'
call     := identifier '(' (expr (',' expr)*)? ')'
```

`parse_primary` is extended to look ahead for `(` immediately following an
identifier (no whitespace-sensitivity — the tokenizer already discards
whitespace) and, when present, parse a comma-separated argument list
instead of returning a bare `Expr::Identifier`:

```rust
fn parse_primary(&mut self) -> Result<ExprNode, CvxError> {
    match self.advance() {
        Some(Token::Number(value)) => Ok(Expr::Constant(*value).node()),
        Some(Token::Identifier(name)) => {
            let name = name.clone();
            if matches!(self.peek(), Some(Token::LParen)) {
                self.advance();
                let args = self.parse_call_args()?;
                Ok(Expr::Call { name, args }.node())
            } else {
                Ok(Expr::Identifier(name).node())
            }
        }
        Some(Token::LParen) => {
            let inner = self.parse_expr()?;
            self.expect(Token::RParen)?;
            Ok(inner)
        }
        _ => Err(CvxError::InvalidExpression(
            "expected number, identifier, or '('".to_string(),
        )),
    }
}

fn parse_call_args(&mut self) -> Result<Vec<ExprNode>, CvxError> {
    let mut args = Vec::new();
    if matches!(self.peek(), Some(Token::RParen)) {
        self.advance();
        return Ok(args);
    }
    loop {
        args.push(self.parse_expr()?);
        match self.advance() {
            Some(Token::Comma) => continue,
            Some(Token::RParen) => break,
            _ => {
                return Err(CvxError::InvalidExpression(
                    "expected ',' or ')' in function call arguments".to_string(),
                ))
            }
        }
    }
    Ok(args)
}
```

`resolve.rs`'s `Resolver::resolve` gains one new arm dispatching on
`name`:

```rust
Expr::Call { name, args } => self.resolve_call(name, args),
```

```rust
fn resolve_call(&mut self, name: &str, args: &[ExprNode]) -> Result<Expression, CvxError> {
    match name {
        "sum" => {
            let [operand] = args else {
                return Err(CvxError::InvalidExpression(format!(
                    "sum() takes exactly 1 argument, got {}",
                    args.len()
                )));
            };
            let operand = self.resolve(operand)?;
            Ok(Expression::sum(operand))
        }
        "index" => {
            if args.len() != 3 && args.len() != 5 {
                return Err(CvxError::InvalidExpression(format!(
                    "index() takes exactly 3 or 5 arguments, got {}",
                    args.len()
                )));
            }
            let operand = self.resolve(&args[0])?;
            let row = literal_dimension(&args[1])?;
            let col = literal_dimension(&args[2])?;
            let rows = if args.len() == 5 { literal_dimension(&args[3])? } else { 1 };
            let cols = if args.len() == 5 { literal_dimension(&args[4])? } else { 1 };
            let operand_shape = crate::analytics::shape::infer_shape(&operand)?;
            crate::analytics::shape::check_index_bounds(operand_shape, row, col, rows, cols)?;
            Ok(Expression::index(operand, row - 1, col - 1, rows, cols))
        }
        other => Err(CvxError::InvalidExpression(format!(
            "unknown function '{other}'"
        ))),
    }
}

/// Reads a bare numeric-literal argument (e.g. `index()`'s `row`/`col`/
/// `rows`/`cols`) as a positive integer. Identifiers, arithmetic, and
/// non-integer or non-positive numbers are all rejected — these
/// arguments are positions/counts, never registry references, mirroring
/// `CVX.INDEX`'s own `row`/`col`/`rows`/`cols` (`data::parse_dimension`).
fn literal_dimension(node: &ExprNode) -> Result<usize, CvxError> {
    match node.as_ref() {
        Expr::Constant(value) if value.fract() == 0.0 && *value >= 1.0 => Ok(*value as usize),
        Expr::Constant(_) => Err(CvxError::InvalidExpression(
            "index() row/col/rows/cols arguments must be positive integers".to_string(),
        )),
        _ => Err(CvxError::InvalidExpression(
            "index() row/col/rows/cols arguments must be numeric literals".to_string(),
        )),
    }
}
```

Both `sum(...)` and `index(...)` recurse through the resolver's existing
`self.resolve(...)` for their operand argument exactly like every other
operator, so identifiers nested arbitrarily deep inside either — including
one `index(...)`/`sum(...)` nested inside another, e.g.
`"sum(index(X, 1, 1, 2, 2))"` — resolve and record dependencies (see
Non-Objective) with no special-casing.

## Data Model

### Bounds validation (`check_index_bounds`, `src/analytics/shape.rs`)

A single, shared helper — not duplicated between the Excel-facing handler
and the grammar resolver — computes the out-of-bounds error described by
ISSUE-0015's acceptance criterion, naming both the requested 1-based
range and the operand's actual shape:

```rust
/// Validates that a `rows x cols` sub-block starting at 1-based
/// `(row, col)` fits within `operand_shape`. Shared by `CVX.INDEX`
/// (`src/excel/expression.rs`) and the `index(...)` grammar keyword
/// (`src/analytics/resolve.rs`) so both report the identical error
/// message for the identical mistake (SPEC-0015).
pub fn check_index_bounds(
    operand_shape: (usize, usize),
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> Result<(), CvxError> {
    let (op_rows, op_cols) = operand_shape;
    let row_end = row + rows - 1;
    let col_end = col + cols - 1;
    if row_end > op_rows || col_end > op_cols {
        return Err(CvxError::InvalidExpression(format!(
            "requested rows {row}..{row_end} and columns {col}..{col_end} are \
             out of bounds for a {op_rows}x{op_cols} operand"
        )));
    }
    Ok(())
}
```

This runs once, eagerly — for `CVX.INDEX`, inside its handler; for
`index(...)`, inside `resolve_call` — not deferred to `CVX.DESCRIBE`/
`CVX.SOLVE`, by reusing the existing, already-`pub` `infer_shape`
(`src/analytics/shape.rs`, same module) to learn the operand's shape
before constructing the `Index` node. This matches ISSUE-0015's
acceptance criterion that an out-of-bounds request is reported with the
requested position and the actual size, as early as possible (at
`CVX.INDEX`/`index(...)` call time), rather than only when the resulting
expression is later described or solved.

### `src/analytics/shape.rs`: `infer_shape`/`render_expression` gain one arm each

`infer_shape` re-derives the same bound check (reusing the operand's own
inferred shape, recursively) so that an `Index` node built directly via
`cvxrust` test code (bypassing `CVX.INDEX`'s eager check above), or an
`Index` whose operand's shape only becomes invalid for an unrelated reason
further down the tree, is still reported correctly by `CVX.DESCRIBE`/
`CVX.SHAPE`, consistent with how every other shape error already surfaces
there (e.g. `rejects_mismatched_non_scalar_shapes`):

```rust
Expression::Index { expr, row_start, col_start, rows, cols } => {
    let (op_rows, op_cols) = infer_shape(expr)?;
    if row_start + rows > op_rows || col_start + cols > op_cols {
        return Err(CvxError::InvalidExpression(format!(
            "requested rows {}..{} and columns {}..{} are out of bounds for \
             a {op_rows}x{op_cols} operand",
            row_start + 1,
            row_start + rows,
            col_start + 1,
            col_start + cols
        )));
    }
    Ok((*rows, *cols))
}
```

`render_expression` renders an `Index` using 1-based, Excel-facing
positions (converted from the stored 0-based fields), with a compact form
for a single entry and a `start:end` range form otherwise — diagnostic
only, not guaranteed to round-trip through `CVX.EXPRESSION`'s parser
(SPEC-0007 Non-Objective, unchanged):

```rust
Expression::Index { expr, row_start, col_start, rows, cols } => {
    let inner = render_expression(expr, registry);
    let row_part = if *rows == 1 {
        format!("{}", row_start + 1)
    } else {
        format!("{}:{}", row_start + 1, row_start + rows)
    };
    let col_part = if *cols == 1 {
        format!("{}", col_start + 1)
    } else {
        format!("{}:{}", col_start + 1, col_start + cols)
    };
    format!("{inner}[{row_part}, {col_part}]")
}
```

E.g. `"x"[2, 1]` for a single entry of a column vector `x`, or
`"Z"[1:2, 1:3]` for a 2x3 sub-block of a matrix `Z`.

### `cvxrust/src/reduce.rs`: solve-time reduction

`check_expr_shapes` gains exactly one new arm, identical in spirit to the
existing `Sum` arm — `Index`'s presence anywhere in an expression
unconditionally routes that whole (objective, or one constraint side)
expression through `linearize_shaped` (affine-only), never `quadratize`:

```rust
Expression::Index { .. } => Err(SHAPE_ERROR.to_string()),
```

A shared, private helper performs the row-major sub-block selection over
an already-reduced `ShapedForm`, used by both reduction paths below:

```rust
/// Row-major sub-block selection: entries at rows
/// `[row_start, row_start + rows)` and columns
/// `[col_start, col_start + cols)` of `form`, into a new `ShapedForm` of
/// shape `(rows, cols)`. Returns a descriptive `Err`, never panics, when
/// the requested block does not fit `form.shape` — defensive only; `cvxx`
/// already rejects this eagerly at `CVX.INDEX`/`index(...)` construction
/// time (see `src/analytics/shape.rs::check_index_bounds`), so this is
/// normally unreachable in practice, same reasoning as every other
/// "defensive, not `unreachable!()`" arm in this module.
fn select_sub_block(
    form: &ShapedForm,
    row_start: usize,
    col_start: usize,
    rows: usize,
    cols: usize,
) -> Result<ShapedForm, String> {
    let (form_rows, form_cols) = form.shape;
    if row_start + rows > form_rows || col_start + cols > form_cols {
        return Err(format!(
            "requested rows {}..{} and columns {}..{} are out of bounds for \
             a {form_rows}x{form_cols} operand",
            row_start + 1,
            row_start + rows,
            col_start + 1,
            col_start + cols
        ));
    }
    let mut entries = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let k = (row_start + r) * form_cols + (col_start + c);
            entries.push(form.entries[k].clone());
        }
    }
    Ok(ShapedForm {
        shape: (rows, cols),
        entries,
    })
}
```

`linearize_shaped` gains one new arm calling it:

```rust
Expression::Index { expr, row_start, col_start, rows, cols } => {
    let inner = linearize_shaped(expr, offsets, n_total)?;
    select_sub_block(&inner, *row_start, *col_start, *rows, *cols)
}
```

`quadratize` gains one new, purely defensive arm (unreachable in practice
for the same reason `check_expr_shapes`'s `Index` arm above always forces
`linearize_shaped` routing instead — mirrors the existing defensive `Sum`
arm exactly):

```rust
Expression::Index { expr, row_start, col_start, rows, cols } => {
    let inner = reduce_expression(expr, index, n)?;
    let selected = select_sub_block(&inner, *row_start, *col_start, *rows, *cols)?;
    // `is_all_scalar` (and therefore this function) is only ever reached
    // for an `Index`-free expression, so `selected.shape == (1, 1)` here
    // in practice; `quadratize`'s return type is a single `QuadraticForm`,
    // so defensively fold in case it were not.
    Ok(selected
        .entries
        .into_iter()
        .fold(QuadraticForm::zero(n, 0.0), |acc, x| acc.add(&x)))
}
```

No change to `offsets`/`n_total` construction, `MAX_VARIABLES`/
`MAX_CONSTRAINTS` accounting, `Solution`, or `Problem` — `Index` consumes
an already-reduced operand and produces no new scalar variable slots or
constraint rows of its own.

## Error Handling

All errors below are translated to Excel `#VALUE!` by the existing
`to_xloper_result` (`src/excel/expression.rs`); diagnostics are logged to
`%TEMP%/cvxx.log`, never written to the worksheet, per project convention.

- `operand` is an unknown identifier, or resolves to a handle kind
  `resolve_handle_arg` does not support (constraint, problem, result,
  etc.) → `CvxError::UnknownIdentifier`, identical to every other
  functional builder.
- `row`/`col` is non-numeric, non-integer, zero, negative, or exceeds
  `MAX_DIMENSION` → `CvxError::InvalidDimension`, via `parse_dimension`.
- `rows`/`cols`, when supplied, fails the same validation as `row`/`col` →
  `CvxError::InvalidDimension`, via `parse_optional_dimension`. Omitted
  `rows`/`cols` default to `1` and never error.
- The requested row or column range extends beyond the operand's actual
  shape (e.g. `row + rows - 1 > operand_rows`) →
  `CvxError::InvalidExpression`, naming both the requested 1-based
  row/column range and the operand's actual `rows x cols` shape (the
  ISSUE-0015 acceptance criterion), checked eagerly at `CVX.INDEX` call
  time via `check_index_bounds`, and re-checked defensively (never
  panicking) wherever the resulting expression's shape is later
  inferred (`CVX.DESCRIBE`/`CVX.SHAPE`) or solved
  (`cvxrust::reduce::select_sub_block`).
- Duplicate `name` (or a name already registered under a different object
  table) → `CvxError::AmbiguousIdentifier` when `name` is already used by a
  **different** object table (the cross-table uniqueness rule, SPEC-0002);
  reusing a name already used by another **expression** is not an error —
  `insert_expression` overwrites the old entry, exactly as every other
  expression-producing function (`CVX.ADD`, `CVX.SUM`, etc.) already does,
  so repeated Excel edits/recalculation can keep reusing the same name.
- Internal registry failure → `CvxError::Registry`, with a logged
  diagnostic, as elsewhere.
- A solve that reaches a quadratic term built from two non-constant
  `Index`-derived operands fails with the existing `VECTOR_MUL_ERROR`
  (SPEC-0014), unchanged — not a new error introduced here.

### Grammar-specific errors (`sum(...)`/`index(...)` inside a string)

- A function-call name other than `sum`/`index` (e.g. `"foo(x)"`) →
  `CvxError::InvalidExpression("unknown function 'foo'")`.
- `sum(...)` called with any arity other than exactly 1, or `index(...)`
  called with any arity other than exactly 3 or 5 → `CvxError::
  InvalidExpression`, naming the function and the actual argument count.
- Any of `index(...)`'s `row`/`col`/`rows`/`cols` arguments is not a bare
  numeric literal (e.g. an identifier, or a sub-expression like `1 + 1`)
  → `CvxError::InvalidExpression("index() row/col/rows/cols arguments
  must be numeric literals")`.
- Any of `index(...)`'s `row`/`col`/`rows`/`cols` numeric-literal
  arguments is not a positive integer (zero, negative, or non-integer) →
  `CvxError::InvalidExpression("index() row/col/rows/cols arguments must
  be positive integers")`.
- `index(...)`'s selection extends beyond its (resolved) operand's actual
  shape → the same `CvxError::InvalidExpression` out-of-bounds message as
  `CVX.INDEX`, via the shared `check_index_bounds` (see Data Model),
  checked eagerly during `resolve_call`, before `CVX.EXPRESSION`/
  `CVX.CONSTRAINT` ever returns a handle.
- An unresolvable identifier nested inside `sum(...)`/`index(...)`'s
  operand → `CvxError::UnknownIdentifier`, exactly as for any other
  nested identifier today (no new behavior; `resolve` recurses the same
  way for every operator).
- A syntax error in the argument list itself (a missing `,`, a trailing
  `,` before `)`, or a missing closing `)`) → `CvxError::InvalidExpression`
  with a message naming the expected token, consistent with every other
  parser error in this module (e.g. `expect`'s existing message style).

## Test Approach

### `cvxrust` (`cvxrust/src/reduce_tests.rs`, `cvxrust/src/lib.rs` or a new
test module)

- `Expression::index` constructs the expected `Expression::Index` variant.
- `linearize_shaped`/`reduce_expression`, via `solve`, correctly select a
  single entry, a single row, a single column, and a general sub-block
  from a vector `Variable`, a matrix `Variable`, and a `Parameter`,
  confirmed against `Solution::variable_values`/`objective_value`.
- A constraint built from `Expression::index(x, ...)` restricts only the
  indexed entry/entries of a larger variable `x`, leaving the rest of `x`
  free (or governed only by other constraints) — an end-to-end `solve`
  test, mirroring SPEC-0014's
  `solve_result_variable_values_match_each_variables_own_coefficient`.
- `Index` wrapping another `Index` (nested selection) reduces correctly.
- An out-of-bounds `Index` (row/col/rows/cols exceeding the operand's
  shape) reduces to a descriptive `Err`, not a panic, via
  `select_sub_block`.
- `check_expr_shapes`/`is_all_scalar` returns `false`/`Err` for any
  expression containing `Index`, even one that is otherwise entirely
  `(1, 1)`-shaped.
- A quadratic term over two `Index`-derived operands fails with the
  existing `VECTOR_MUL_ERROR`, confirming no accidental new quadratic
  support was introduced.

### `cvxx` (`src/analytics/shape.rs`, `src/excel/expression.rs`)

- `infer_shape` returns the expected `(rows, cols)` for an in-bounds
  `Index` over a `Variable`/`Parameter`/nested `Expression` leaf, and a
  descriptive `CvxError::InvalidExpression` for an out-of-bounds one.
- `render_expression` renders a single-entry `Index` and a multi-row/
  column `Index` with the expected `[row, col]`/`[r1:r2, c1:c2]` syntax,
  for both named and unnamed operands (reusing SPEC-0013's name
  preference).
- `run_index`/`check_index_bounds` unit tests: single entry, single row,
  single column, general sub-block, and nested `Index`-of-`Index`, all
  in-bounds and producing the expected `Expression::Index` fields
  (confirming the 1-based-to-0-based conversion); each of `row`, `col`,
  `rows`, `cols` individually invalid (non-numeric, zero, negative,
  non-integer); `rows`/`cols` omitted and defaulting to `1`; an
  out-of-bounds request naming the requested range and actual shape;
  unknown/incompatible `operand`; reusing a name already used by a
  different object table (`AmbiguousIdentifier`); reusing a name already
  used by another expression (overwrites, no error).
- `data::parse_optional_dimension` unit tests: blank/missing input
  returns the default; present input is validated identically to
  `parse_dimension`.

### `cvxx` grammar (`src/analytics/parser.rs`, `src/analytics/resolve.rs`)

- Tokenizer: `,` lexes to `Token::Comma`; existing tokens unaffected.
- Parser: `"sum(v)"`/`"index(v, 2, 1)"`/`"index(v, 1, 1, 2, 3)"` each parse
  to the expected `Expr::Call { name, args }`; a bare identifier not
  followed by `(` still parses to `Expr::Identifier` unchanged (e.g.
  `"sum"` alone, with no registered object named `sum`, still fails at
  *resolve* time with `UnknownIdentifier`, not at parse time); nested
  calls (`"sum(index(X, 1, 1, 2, 2))"`) parse correctly; a call with zero
  arguments (`"sum()"`), a trailing comma (`"sum(v,)"`), and a missing
  closing paren (`"sum(v"`) each produce a descriptive parse error.
- Resolver: `"sum(v)"`/`"index(v, 2, 1)"` resolve to the exact same
  `cvxrust::Expression::Sum`/`Expression::Index` tree `CVX.SUM`/
  `CVX.INDEX` would build for the same operand; an unknown function name,
  wrong arity (for both `sum` and `index`, including `index` with 4
  arguments), a non-literal or non-positive-integer `row`/`col`/`rows`/
  `cols` argument, and an out-of-bounds `index(...)` selection each
  produce the documented error (see Error Handling).
- Dependency tracking: `resolve_expr("sum(v)")`/`resolve_expr("index(v,
  2, 1)")`, where `v` is a registered variable, include `v` in the
  returned `ResolvedExpr::dependencies` — confirming the Non-Objective
  claim that grammar-based `sum`/`index` get correct dependency tracking
  "for free", in contrast with the `CVX.SUM`/`CVX.INDEX` functional
  builders (which still record none).

### Integration

- A sample workbook creates a vector variable, builds a constraint via
  `CVX.LESS_THAN(CVX.INDEX(x, 2, 1), 5)`, solves the resulting problem,
  and confirms only the second entry of `x` is bounded while the other
  entries are unconstrained in the solution.
- A sample workbook repeats the same scenario using
  `CVX.CONSTRAINT("index(x, 2, 1) <= 5")` instead, and separately builds
  `CVX.EXPRESSION("sum(x)")`, confirming both grammar forms solve/describe
  identically to their functional-builder equivalents.

## Dependencies

- SPEC-0002/SPEC-0003 for parameter/variable handle and shape storage
  conventions (`(rows, cols)`, 1-based `rows, cols` argument convention).
- SPEC-0004 for the functional-builder convention
  (`resolve_handle_arg`/`insert_expression`, and the accepted
  dependency-tracking limitation this specification inherits unchanged
  for the functional builders only), and for the original recursive-
  descent parser/tokenizer (`src/analytics/parser.rs`) and resolver
  (`src/analytics/resolve.rs`) this specification extends with
  `Expr::Call`/`resolve_call`/`literal_dimension` — directly delivering
  the "general function calls inside expression strings ... deferred to
  a follow-up issue" note SPEC-0004 left open, narrowed here to exactly
  `sum`/`index`.
- SPEC-0007 for `CVX.DESCRIBE`/`CVX.SHAPE`/`infer_shape`/
  `render_expression`/`MAX_DESCRIBE_LEN`.
- SPEC-0013 for registered-name-preferring rendering, reused unchanged by
  `Index`'s `render_expression` arm.
- SPEC-0014 (implemented) for the `ShapedForm`/`linearize_shaped`/
  `quadratize`/`check_expr_shapes`/`broadcast_shape` machinery this
  specification extends, for the precedent (`Sum`) this specification's
  `Index` arms directly follow, and for `Expression::Sum`/`CVX.SUM`
  itself, whose own "a string-grammar form is left to a future
  specification extending the grammar itself" note this specification's
  `sum(...)` keyword directly delivers.
- `data::parse_dimension`/`MAX_DIMENSION` (`src/data/mod.rs`), reused
  unchanged for `row`/`col` and as the basis for the new
  `parse_optional_dimension`.
- No new external crate dependencies.

## Status

Implemented. `cvxrust::Expression` gained the `Index { expr, row_start,
col_start, rows, cols }` variant and `Expression::index` constructor
(`cvxrust/src/model.rs`). `cvxrust/src/reduce.rs` gained the shared
`select_sub_block` helper, an `Index` arm in `linearize_shaped`, a
defensive `Index` arm in `quadratize`, and an `Index` arm in
`check_expr_shapes` that forces the affine-only path, exactly mirroring
`Sum`. `src/analytics/shape.rs` gained `check_index_bounds` (now the
single, shared bounds-check helper used by both `CVX.INDEX` and the
`index(...)` grammar keyword) plus `Index` arms in `infer_shape` and
`render_expression`. `src/data/mod.rs` gained `parse_optional_dimension`.
`CVX.INDEX(operand, row, col, [rows], [cols], [name])` is implemented in
`src/excel/expression.rs` and registered in `src/excel/mod.rs`.

The grammar extension is implemented as specified: `src/analytics/ast.rs`
gained `Expr::Call { name, args }`; `src/analytics/parser.rs` gained a
`Comma` token and function-call parsing in `parse_primary`/
`parse_call_args`; `src/analytics/resolve.rs` gained `resolve_call` and
`literal_dimension`, recognizing exactly `sum`/`index` and resolving them
to the same `Expression::Sum`/`Expression::Index` the functional builders
produce, with correct dependency tracking (confirmed by test, in contrast
with the functional builders' pre-existing gap, unchanged).

`docs/expressions.md` documents `CVX.INDEX` and the `sum()`/`index()`
keyword syntax with examples; `docs/constraints.md` and
`docs/inspection.md` were updated with a cross-referencing example and
`CVX.DESCRIBE`/`CVX.SHAPE` behavior for `Index` expressions, respectively.
`issues/0015-vector-matrix-expression-operations.md` was amended with the
acceptance criterion covering formula-string usability.

One correction from the original draft: the "Duplicate `name`" Error
Handling bullet described `CvxError::DuplicateName` as applying to
same-table expression name reuse; in fact `insert_expression` has always
overwritten a reused name within the expression table (by design, for
convenient Excel re-editing/recalculation) — only **cross-table** name
reuse errors, with `AmbiguousIdentifier`. The Error Handling and Test
Approach sections above have been corrected accordingly; no implementation
behavior differs from any other expression-producing function.

`cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace` are all clean (74 `cvxrust` tests, 179 `cvxx`
tests, including an end-to-end `solve` test built entirely from
`sum(...)`/`index(...)` grammar strings). Integration testing against a
live Excel workbook is still pending and requires a Windows machine with
Excel installed.
