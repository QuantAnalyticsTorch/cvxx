---
id: SPEC-0018
title: Matrix multiplication and transpose for vector/matrix expressions
issue: ISSUE-0018
status: implemented
created: 2026-10-02
---

## Objective

Let users combine two vector/matrix parameters, variables, or expressions
using true matrix multiplication, and turn a vector/matrix's rows into
columns (or vice versa), via two new `cvxrust::Expression` variants
(`MatMul`, `Transpose`), two new Excel functional builders (`CVX.MATMUL`,
`CVX.TRANSPOSE`) for building up a formula from previously created pieces
one step at a time, and — for typing out a full formula directly, per
ISSUE-0018's acceptance criteria — two new pieces of syntax in the
`CVX.EXPRESSION`/`CVX.CONSTRAINT` string grammar chosen to be immediately
recognizable to Python/NumPy users as the real thing, not an approximation:
the infix `@` operator (`a @ b`, mirroring `numpy.matmul`/Python's own
`@` operator, same operator precedence as `*`/`/`) for matrix
multiplication, and the postfix `.T` property (`a.T`, mirroring
`numpy.ndarray.T`) for transpose. `CVX.MATMUL`/`CVX.TRANSPOSE` and
`@`/`.T` both resolve to the exact same `Expression::MatMul`/
`Expression::Transpose` construction; neither is a thin wrapper around a
`sum(...)`/`index(...)`-style function-call keyword (SPEC-0015) — NumPy
itself favors `@`/`.T` over `numpy.matmul(...)`/`numpy.transpose(...)` in
ordinary code, and introducing a third, function-call-syntax spelling
(e.g. `matmul(...)`/`transpose(...)`) alongside the operators would add
redundant surface area without a corresponding capability `sum`/`index`
needed it for (those have no natural infix/postfix spelling; `@`/`.T` do).

Because `cvxrust::Expression` and its diagnostic shape-inference/rendering
(`src/analytics/shape.rs`) and solve-time reduction (`cvxrust/src/
reduce.rs`) are already generalized to arbitrary vector/matrix shapes by
SPEC-0014 (implemented) and SPEC-0015 (implemented), adding `MatMul` and
`Transpose` is an additive extension of that same, already-landed
machinery — two new match arms in each of `infer_shape`/
`render_expression`/`check_expr_shapes`/`linearize_shaped`/`quadratize`,
following exactly the pattern SPEC-0014/SPEC-0015 established for `Sum`/
`Index` — not a new subsystem. Unlike `sum`/`index`, the string-grammar
side of this specification extends the AST (`Expr::MatMul`, `Expr::
Transpose`) and tokenizer (two new tokens, `At` and `Transpose` — see
Interface) directly, rather than reusing `Expr::Call`/`resolve_call`,
because `@`/`.T` are true operators with their own precedence/
associativity, not function-call syntax.

Per ISSUE-0018's notes, this specification covers matrix multiplication
where at most one operand depends on a variable (the "known table applied
to an unknown list" case): `linearize_shaped`'s existing restriction —
that a product of two variable-dependent terms is rejected — extends
unchanged to `MatMul`, reusing the existing `VECTOR_MUL_ERROR` message.
Matrix multiplication of two variable-dependent operands remains out of
scope (deferred to, or pending, ISSUE-0016), exactly as elementwise `Mul`
of two variable-dependent vector/matrix operands already is today
(SPEC-0014).

## Non-Objective

- Matrix multiplication (or any other combination) of two
  variable-dependent operands (e.g. `CVX.MATMUL(x, y)` where both `x` and
  `y` are variables, or either is an expression that is not entirely
  constant/parameter-derived). This remains rejected with the existing
  `VECTOR_MUL_ERROR` ("solver only supports linear (affine) objectives and
  constraints once a vector or matrix (non-1x1) variable or parameter is
  involved; a product of two variable-dependent terms was found",
  `cvxrust/src/reduce.rs`), unchanged and not specially distinguished for
  `MatMul` — the same restriction SPEC-0014 already applies to elementwise
  `Mul`. General support for this case is ISSUE-0016's scope, not this
  specification's.
- Any other NumPy operator/property syntax: no `@=` compound-assignment
  (there is no assignment syntax of any kind in this grammar), no `.H`
  (conjugate transpose — `cvxx` has no complex numbers), and no general
  attribute-access syntax (`.T` is recognized as one specific, hard-coded
  postfix token, not an instance of a general `expr '.' identifier`
  attribute mechanism — see Interface/Data Model).
- A function-call-syntax spelling of matrix multiplication/transpose
  (e.g. `matmul(...)`/`transpose(...)`, mirroring SPEC-0015's `sum(...)`/
  `index(...)`). `@`/`.T` are the only string-grammar spellings introduced
  (see Objective); a future specification may add a function-call
  alternative, but none is introduced here, to avoid two redundant
  spellings of the same capability.
- Full backward-compatible preservation of every existing identifier that
  already uses a literal, trailing `.T` as part of its own dotted name
  (e.g. a parameter or variable registered under the literal name
  `"cost.T"`). The tokenizer's existing "identifiers may contain dots"
  rule (`src/analytics/parser.rs`'s doc-comment grammar) is narrowed by
  exactly one case to make `.T` recognizable as a postfix operator — see
  Interface/Data Model for the precise (minimal, deliberately narrow)
  disambiguation rule and Error Handling for the resulting edge case.
  Every other existing dotted identifier (including one with a `.T` in
  the *middle*, e.g. `"a.T.b"`) is completely unaffected.
- 1-D ("bare vector", shape-less) semantics of any kind. Every `cvxx`
  vector is already a `(rows, 1)` or `(1, cols)` matrix (SPEC-0002/
  SPEC-0003); `MatMul`'s shape rule (Data Model) is the standard 2-D
  matrix-multiplication rule applied directly to these shapes, with no
  NumPy-style 1-D-array special-casing (e.g. no automatic "promote a 1-D
  operand to a row/column and demote the result" behavior) required or
  introduced.
- Broadcasting a `(1, 1)` operand through `MatMul` the way the existing
  elementwise `Add`/`Sub`/`Mul`/`Div`/`broadcast_shape` do. Standard matrix
  multiplication has no "scale by a single number" shortcut (ISSUE-0018
  Notes); `MatMul`'s shape rule (Data Model) is evaluated independently of,
  and is never confused with, `broadcast_shape`.
- Eager shape validation of `CVX.MATMUL`'s operands at call time (unlike
  `CVX.INDEX`/SPEC-0015, which does validate eagerly). `CVX.MATMUL`
  follows the existing `CVX.ADD`/`CVX.SUB`/`CVX.MUL`/`CVX.DIV` precedent
  (SPEC-0004/SPEC-0014): the handle is built unconditionally (operands may
  not even have inferable shapes yet, e.g. both are themselves unresolved
  `MatMul`/`Index` expressions built in an arbitrary order), and an
  incompatible-shape error surfaces lazily, the first time the resulting
  expression's shape is actually needed — `CVX.DESCRIBE`/`CVX.SHAPE`
  (`infer_shape`) or a solve (`linearize_shaped`) — exactly like today's
  elementwise-mismatch error. See Data Model/Error Handling.
- Quadratic (degree-2) terms built from one or more `MatMul`/`Transpose`
  operands. Like `Sum`/`Index`, the presence of `MatMul`/`Transpose`
  anywhere in an objective or one constraint side unconditionally forces
  that whole side through the affine-only `linearize_shaped` path (see
  Data Model); this is unchanged, existing behavior, not a new
  restriction introduced here.
- Any change to `CVX.PARAMETER`, `CVX.VARIABLE`, `CVX.PROBLEM`,
  `CVX.SOLVE`, `CVX.VALUE`, `CVX.STATUS`, `CVX.OBJECTIVE_VALUE`, or any
  other existing Excel-facing function signature or behavior.
  `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` require only the same kind of
  small, additive match-arm change SPEC-0014/SPEC-0015 already made for
  `Sum`/`Index`.
- Raising `MAX_VARIABLES`/`MAX_CONSTRAINTS`/`MAX_DIMENSION`, or changing
  `MAX_ITERATIONS`/`PSD_TOLERANCE`/`MAX_DESCRIBE_LEN`. `MatMul`/
  `Transpose` introduce no new variables or constraint rows; both are pure
  reductions/rearrangements over already-reduced operands.
- Fixing the pre-existing functional-builder dependency-tracking gap
  (SPEC-0014/SPEC-0015: a variable reachable only through
  functional-builder handles is not swept into `problem.variables` by
  `CVX.PROBLEM`). `CVX.MATMUL`/`CVX.TRANSPOSE` keep this limitation
  unchanged, exactly as `CVX.SUM`/`CVX.INDEX` already do. An `@`/`.T`
  usage written inside a `CVX.EXPRESSION`/`CVX.CONSTRAINT` **string**
  already gets this correctly, for free, via `resolve_expr`'s existing
  recursive dependency tracking (`src/analytics/resolve.rs`), exactly as
  SPEC-0015 already delivered for `sum(...)`/`index(...)`.
- Assigning a value to, or otherwise mutating, part of a vector/matrix
  through a `MatMul`/`Transpose` result (there is no "write" side to
  either feature — both are read-only, exactly like every other
  `Expression` node).

## Interface

### `cvxrust::Expression` gains two new variants (`cvxrust/src/model.rs`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // ... existing variants unchanged ...
    /// Standard (2-D) matrix multiplication of two (possibly
    /// vector/matrix-shaped) expressions: `left` contributes `rows`, which
    /// must match `right`'s row count against `left`'s column count (see
    /// Data Model); the result has `left`'s row count and `right`'s column
    /// count (SPEC-0018).
    MatMul(Box<Expression>, Box<Expression>),
    /// The row/column transpose of a (possibly vector/matrix-shaped)
    /// expression: a `rows x cols` operand becomes `cols x rows`
    /// (SPEC-0018).
    Transpose(Box<Expression>),
}

impl Expression {
    /// Creates a matrix-multiplication expression.
    pub fn matmul(left: Expression, right: Expression) -> Self {
        Expression::MatMul(Box::new(left), Box::new(right))
    }

    /// Creates a transpose expression.
    pub fn transpose(expr: Expression) -> Self {
        Expression::Transpose(Box::new(expr))
    }
}
```

No other public `cvxrust` struct/enum/function signature changes.

### New Excel functions (`src/excel/expression.rs`)

```
CVX.MATMUL(left, right, [name])
CVX.TRANSPOSE(operand, [name])
```

- `left`, `right` (`CVX.MATMUL`), `operand` (`CVX.TRANSPOSE`): a parameter,
  variable, or expression handle, or a registered name (resolved
  identically to every other functional builder's operand, via the
  existing `resolve_handle_arg`). Not a bare numeric literal, consistent
  with `CVX.ADD`/`CVX.MUL`/`CVX.NEG`/etc.
- `name`: optional unique name for the resulting expression, as with every
  other functional builder.
- Returns: a string handle of the form `cvx:expr:<uuid>`, exactly like
  `CVX.MUL`/`CVX.NEG`/etc.

```rust
/// `CVX.MATMUL(left, right, [name])` — standard (2-D) matrix
/// multiplication of two existing expressions (SPEC-0018).
#[export_name = "CVX.MATMUL"]
pub extern "system" fn cvx_matmul(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    run_binary(left, right, name, Expression::matmul, "CVX.MATMUL")
}

/// `CVX.TRANSPOSE(operand, [name])` — the row/column transpose of an
/// existing expression (SPEC-0018).
#[export_name = "CVX.TRANSPOSE"]
pub extern "system" fn cvx_transpose(operand: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_unary(operand, name, Expression::transpose);
    to_xloper_result(result, "CVX.TRANSPOSE")
}
```

`CVX.MATMUL` reuses the existing `run_binary` helper unchanged (same
pattern as `CVX.ADD`/`CVX.SUB`/`CVX.MUL`/`CVX.DIV`: build the expression
unconditionally, no eager shape check — see Non-Objective). `CVX.TRANSPOSE`
reuses the existing `run_unary` helper unchanged (same pattern as
`CVX.NEG`/`CVX.SUM`).

Registered in `xlAutoOpen` (`src/excel/mod.rs`) alongside the other
functional builders, following the existing `Reg::add` call convention:

```rust
reg.add(
    "CVX.MATMUL",
    "QQQ$",
    "left, right, name",
    "cvxx",
    "Matrix-multiplies two cvxx expressions and returns the resulting handle.",
    &[
        "Handle of the left-hand expression.",
        "Handle of the right-hand expression.",
        "Optional unique name for the result.",
    ],
);

reg.add(
    "CVX.TRANSPOSE",
    "QQ$",
    "operand, name",
    "cvxx",
    "Transposes a cvxx expression (rows become columns and vice versa) and returns the resulting handle.",
    &[
        "Handle of the expression to transpose.",
        "Optional unique name for the result.",
    ],
);
```

### Grammar extension: `@`/`.T` operator syntax
(`src/analytics/ast.rs`, `src/analytics/parser.rs`, `src/analytics/resolve.rs`)

Updated grammar (replacing the doc-comment grammar at the top of
`parser.rs`; changes from today's grammar in **bold** prose below the
block):

```text
expr     := add_sub
add_sub  := mul_div (('+' | '-') mul_div)*
mul_div  := unary (('*' | '/' | '@') unary)*
unary    := '-' unary | postfix
postfix  := primary ('.T')*
primary  := number | call | identifier | '(' expr ')'
call     := identifier '(' (expr (',' expr)*)? ')'
```

- **`@` joins `mul_div` at the same precedence tier as `*`/`/`**, with the
  same left-to-right associativity, exactly matching Python's own operator
  precedence table (`@`, `*`, `/` are all one tier, binding tighter than
  `+`/`-` and looser than unary `-`). `"A @ B * C"` therefore parses as
  `"(A @ B) * C"`, and `"A * B @ C"` as `"(A * B) @ C"` — matching what a
  NumPy user already expects from `A @ B * C`/`A * B @ C` in Python itself.
- **`postfix` is a new precedence tier between `unary` and `primary`**:
  zero or more trailing `.T` tokens applied to one `primary`, allowing
  chained transposition (`"A.T.T"`, a no-op double transpose, valid and
  accepted, like NumPy's own `a.T.T`) and transposition of any primary,
  including a parenthesized sub-expression (`"(A + B).T"`) or a function
  call (`"sum(X).T"`, transposing a `(1, 1)` result — a no-op, but not an
  error). `.T` binds *tighter* than unary `-`, matching Python's own
  attribute-access-over-unary-minus precedence: `"-A.T"` parses as
  `"-(A.T)"`, not `"(-A).T"`.

One new AST variant each for `MatMul`/`Transpose` (`src/analytics/ast.rs`),
parallel to the existing `Add`/`Sub`/`Mul`/`Div`/`Neg`:

```rust
pub enum Expr {
    // ... existing variants unchanged ...
    /// Standard matrix multiplication, `a @ b` (SPEC-0018).
    MatMul(ExprNode, ExprNode),
    /// Postfix transpose, `a.T` (SPEC-0018).
    Transpose(ExprNode),
}
```

Two new tokens (`src/analytics/parser.rs`):

```rust
enum Token {
    // ... existing variants unchanged ...
    /// `@`, matrix multiplication (SPEC-0018).
    At,
    /// `.T`, postfix transpose (SPEC-0018). Lexed as a single token (not
    /// `Dot` + `Identifier("T")`) so the parser never needs to reason
    /// about a general attribute-access grammar — see the tokenizer rule
    /// below.
    Transpose,
}
```

Tokenizer changes. `@` is unambiguous (not used anywhere else in the
grammar) and lexes trivially: `'@' => { tokens.push(Token::At); i += 1; }`.
`.T` requires one small, deliberately narrow disambiguation rule against
the *existing* "identifiers may contain dots" behavior (today's
`parser.rs` doc comment: `identifier := sequence of letters, digits,
underscores, or dots; must not start with a digit or dot`): a shared
lookahead helper decides, at any position `i` where `chars[i] == '.'`,
whether this exact dot begins a postfix-transpose token rather than
continuing an in-progress identifier —

```rust
/// `true` when the two characters at `chars[i..]` are exactly `.T` and
/// are not themselves followed by another identifier-continuation
/// character (alphanumeric, `_`, or `.`) — i.e. `.T` is a *maximal-munch*
/// suffix, not the start of a longer dotted segment like `.Total`
/// (SPEC-0018).
fn is_transpose_suffix(chars: &[char], i: usize) -> bool {
    chars.get(i) == Some(&'.')
        && chars.get(i + 1) == Some(&'T')
        && !matches!(
            chars.get(i + 2),
            Some(c) if c.is_alphanumeric() || *c == '_' || *c == '.'
        )
}
```

used in exactly two places:

1. The identifier-scanning loop's continuation condition gains one more
   exception — stop *before* consuming a `.` that begins a transpose
   suffix, instead of folding it into the identifier as today:
   ```rust
   while i < chars.len() {
       let ch = chars[i];
       if ch == '.' && is_transpose_suffix(&chars, i) {
           break;
       }
       if ch.is_alphanumeric() || ch == '_' || ch == '.' {
           i += 1;
       } else {
           break;
       }
   }
   ```
2. A new top-level dispatch arm, checked *before* the existing
   digit-or-dot number-literal arm (since a bare `.` at the top level
   today only ever starts a number literal like `.5`):
   ```rust
   '.' if is_transpose_suffix(&chars, i) => {
       tokens.push(Token::Transpose);
       i += 2;
   }
   ```

Because both sites share the same `is_transpose_suffix` check, `"A.T"`
tokenizes as `Identifier("A")` + `Transpose` (`"A .T"`, with a space
before the dot, is *not* supported — no internal whitespace inside `.T`,
matching how the tokenizer already disallows whitespace inside any
identifier), `"(A + B).T"` and `"A.T.T"` tokenize analogously, and
`"a.Total"` (an ordinary dotted identifier, unaffected — `.T` here is
immediately followed by `o`, so `is_transpose_suffix` is `false` and
scanning continues as today) tokenizes exactly as before, with no other
identifier's tokenization changed. See Data Model for the one narrow,
documented behavior change this introduces, and Error Handling for its
edge case.

Parser changes (`src/analytics/parser.rs`):

```rust
fn parse_mul_div(&mut self) -> Result<ExprNode, CvxError> {
    let mut left = self.parse_unary()?;
    while let Some(token) = self.peek() {
        match token {
            Token::Star => { /* unchanged */ }
            Token::Slash => { /* unchanged */ }
            Token::At => {
                self.advance();
                let right = self.parse_unary()?;
                left = Expr::MatMul(left, right).node();
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_unary(&mut self) -> Result<ExprNode, CvxError> {
    match self.peek() {
        Some(Token::Minus) => {
            self.advance();
            let operand = self.parse_unary()?;
            Ok(Expr::Neg(operand).node())
        }
        _ => self.parse_postfix(),
    }
}

/// Parses one `primary` followed by zero or more postfix `.T` tokens
/// (SPEC-0018).
fn parse_postfix(&mut self) -> Result<ExprNode, CvxError> {
    let mut expr = self.parse_primary()?;
    while matches!(self.peek(), Some(Token::Transpose)) {
        self.advance();
        expr = Expr::Transpose(expr).node();
    }
    Ok(expr)
}
```

(`parse_primary` itself — numbers, `call`, bare identifiers, parenthesized
sub-expressions — is unchanged from SPEC-0015.)

Resolver changes (`src/analytics/resolve.rs`): `resolve` gains two more
match arms, parallel to `Add`/`Sub`/`Mul`/`Div`/`Neg` (not routed through
`resolve_call`, since `MatMul`/`Transpose` are now dedicated AST nodes,
not `Expr::Call`):

```rust
Expr::MatMul(left, right) => {
    let l = self.resolve(left)?;
    let r = self.resolve(right)?;
    Ok(Expression::matmul(l, r))
}
Expr::Transpose(operand) => {
    let expr = self.resolve(operand)?;
    Ok(Expression::transpose(expr))
}
```

Because `parse_constraint` already reuses the same `parse_expr` as
`CVX.EXPRESSION`'s parser, `CVX.CONSTRAINT` strings gain `@`/`.T` support
for free, exactly as SPEC-0015 already established for `sum(...)`/
`index(...)`.

### Shape inference and rendering (`src/analytics/shape.rs`)

```rust
/// Validates the standard 2-D matrix-multiplication shape rule: `a`'s
/// column count must equal `b`'s row count. Distinct from
/// `broadcast_shape`'s elementwise rule (SPEC-0018) — the two error
/// messages are never confused with one another (ISSUE-0018 acceptance
/// criterion).
pub fn matmul_shape(a: (usize, usize), b: (usize, usize)) -> Result<(usize, usize), CvxError> {
    if a.1 != b.0 {
        return Err(CvxError::InvalidExpression(format!(
            "matrix multiplication requires the left operand's column \
             count to match the right operand's row count: {}x{} \
             (columns={}) vs {}x{} (rows={})",
            a.0, a.1, a.1, b.0, b.1, b.0
        )));
    }
    Ok((a.0, b.1))
}
```

`infer_shape` gains:

```rust
Expression::MatMul(l, r) => matmul_shape(infer_shape(l)?, infer_shape(r)?),
Expression::Transpose(e) => {
    let (rows, cols) = infer_shape(e)?;
    Ok((cols, rows))
}
```

`render_expression` gains, using the exact same `@`/`.T` syntax the
grammar now parses (Interface's grammar extension) — unlike `Index`'s
`[row, col]` rendering (SPEC-0015, which intentionally does not round-trip
through the parser), `MatMul`/`Transpose`'s rendering is, as a convenient
side effect, valid `CVX.EXPRESSION` input for the identical expression
(registered-name leaves aside, exactly like every other operator's
rendering already is today):

```rust
Expression::MatMul(l, r) => format!(
    "({}) @ ({})",
    render_expression(l, registry),
    render_expression(r, registry)
),
Expression::Transpose(e) => format!("({}).T", render_expression(e, registry)),
```

### `cvxrust` reduction (`cvxrust/src/reduce.rs`)

`check_expr_shapes` (the `is_all_scalar` predicate) gains two arms that
force the affine-only path, mirroring `Sum`/`Index` exactly:

```rust
Expression::MatMul(..) => Err(SHAPE_ERROR.to_string()),
Expression::Transpose(e) => check_expr_shapes(e),
```

(`Transpose` alone never prevents the quadratic `quadratize` path on its
own — it recurses into its operand, exactly like `Neg`/`Scale` — but see
below: `linearize_shaped`'s `Transpose` arm is still needed because
`Transpose` may appear nested inside a `MatMul`/`Sum`/`Index` that itself
forces the affine path. `MatMul` unconditionally forces the affine path,
exactly like `Sum`/`Index`, since `quadratize` has no notion of a
matrix-shaped product.)

A new, duplicated-from-`cvxx` `matmul_shape` helper (crate-private,
`String`-returning, `cvxrust` does not depend on `cvxx` — the same
duplication rationale as the existing `broadcast_shape`):

```rust
pub(crate) fn matmul_shape(a: (usize, usize), b: (usize, usize)) -> Result<(usize, usize), String> {
    if a.1 != b.0 {
        return Err(format!(
            "matrix multiplication requires the left operand's column \
             count to match the right operand's row count: {}x{} \
             (columns={}) vs {}x{} (rows={})",
            a.0, a.1, a.1, b.0, b.1, b.0
        ));
    }
    Ok((a.0, b.1))
}
```

`linearize_shaped` gains:

```rust
Expression::Transpose(e) => {
    let inner = linearize_shaped(e, offsets, n_total)?;
    Ok(transpose_entries(&inner))
}
Expression::MatMul(l, r) => {
    let lf = linearize_shaped(l, offsets, n_total)?;
    let rf = linearize_shaped(r, offsets, n_total)?;
    matmul_entries(&lf, &rf, n_total)
}
```

with two new row-major helpers alongside `select_sub_block`:

```rust
/// Row-major transpose: a `rows x cols` form becomes `cols x rows`,
/// entry `(r, c)` moving to `(c, r)` (SPEC-0018).
fn transpose_entries(form: &ShapedForm) -> ShapedForm {
    let (rows, cols) = form.shape;
    let mut entries = Vec::with_capacity(rows * cols);
    for c in 0..cols {
        for r in 0..rows {
            entries.push(form.entries[r * cols + c].clone());
        }
    }
    ShapedForm {
        shape: (cols, rows),
        entries,
    }
}

/// Standard (2-D) matrix multiplication: entry `(i, j)` of the
/// `m x n` result is `sum_k l[i, k] * r[k, j]` over the shared `k x`
/// dimension. Each individual product term follows the same
/// "at most one variable-dependent side" rule as elementwise `Mul`
/// (SPEC-0014); any term violating it fails with the existing
/// `VECTOR_MUL_ERROR`, accumulated via repeated `QuadraticForm::add`
/// (SPEC-0018).
fn matmul_entries(
    l: &ShapedForm,
    r: &ShapedForm,
    n_total: usize,
) -> Result<ShapedForm, String> {
    let (m, k) = l.shape;
    let (k2, n) = r.shape;
    if k != k2 {
        return Err(matmul_shape(l.shape, r.shape).unwrap_err());
    }
    let mut entries = Vec::with_capacity(m * n);
    for i in 0..m {
        for j in 0..n {
            let mut acc = QuadraticForm::zero(n_total, 0.0);
            for t in 0..k {
                let l_entry = &l.entries[i * k + t];
                let r_entry = &r.entries[t * n + j];
                let term = if l_entry.is_constant() {
                    r_entry.clone().scale(l_entry.constant)
                } else if r_entry.is_constant() {
                    l_entry.clone().scale(r_entry.constant)
                } else {
                    return Err(VECTOR_MUL_ERROR.to_string());
                };
                acc = acc.add(&term);
            }
            entries.push(acc);
        }
    }
    Ok(ShapedForm {
        shape: (m, n),
        entries,
    })
}
```

`quadratize` gains two defensive (never reached in practice, since
`is_all_scalar`/`check_expr_shapes` already routes any expression
containing `MatMul`/`Transpose` through `linearize_shaped` before
`quadratize` is ever called on it) arms, exhaustively matching the enum
without panicking, mirroring the existing defensive `Sum`/`Index` arms:

```rust
Expression::Transpose(e) => {
    let inner = reduce_expression(e, index, n)?;
    let transposed = transpose_entries(&inner);
    Ok(transposed
        .entries
        .into_iter()
        .fold(QuadraticForm::zero(n, 0.0), |acc, x| acc.add(&x)))
}
Expression::MatMul(l, r) => {
    let lf = reduce_expression(l, index, n)?;
    let rf = reduce_expression(r, index, n)?;
    let result = matmul_entries(&lf, &rf, n)?;
    Ok(result
        .entries
        .into_iter()
        .fold(QuadraticForm::zero(n, 0.0), |acc, x| acc.add(&x)))
}
```

(These two arms sum every entry of the sub-result down to one number, the
same simplification the existing defensive `Sum`/`Index` arms already use,
since `quadratize`'s return type is a single `QuadraticForm` and these
arms are unreachable in practice — see above.)

## Data Model

- **Shapes stay `(rows, cols)` throughout** (SPEC-0002/SPEC-0003/
  SPEC-0014); `MatMul`/`Transpose` introduce no new shape representation.
- **`MatMul` shape rule**: for operand shapes `(m, k)` and `(k2, n)`, valid
  exactly when `k == k2`, producing shape `(m, n)`. This is the standard
  linear-algebra rule and is *independent of* `broadcast_shape`'s
  elementwise rule — a `(1, 1)` operand does **not** broadcast through
  `MatMul` (e.g. `CVX.MATMUL` of a `(1, 1)` and a `(3, 2)` operand is
  invalid: `1 != 3`), unlike `CVX.MUL`. This directly delivers the
  ISSUE-0018 Notes' "the existing ... 'scale by a single number' shortcut
  does not apply to matrix multiplication" requirement.
  - A `(1, n)` row times an `(n, 1)` column produces a `(1, 1)` scalar
    (the standard "dot product" case, achievable today only via
    `CVX.SUM(CVX.MUL(...))` for same-shape operands — `MatMul` additionally
    covers the general matrix case and does not require the two operands
    to already share one dimension's orientation).
  - An `(n, 1)` column times a `(1, n)` row produces an `(n, n)` outer
    product (the shape used by, e.g., a combined-risk-style formula
    `"w.T @ Sigma @ w"` for a weight column `w` and a covariance matrix
    `Sigma`).
- **`Transpose` shape rule**: a `(rows, cols)` operand always produces
  `(cols, rows)`; never fails. Transposing a `(1, 1)` operand is a no-op
  (produces the same single value), consistent with NumPy's own
  `.T`/`transpose()` behavior on a `1x1` array.
- **Forcing the affine-only path**: exactly like `Sum`/`Index`
  (SPEC-0014/SPEC-0015), the presence of `MatMul` anywhere in an objective
  or one constraint side forces that entire side through
  `linearize_shaped`, even for an otherwise entirely `(1, 1)`-shaped
  sub-expression nested inside it. `Transpose` alone does not force this
  by itself when it is the *only* non-scalar-triggering construct present
  (`check_expr_shapes`'s `Transpose` arm recurses into its operand rather
  than unconditionally erroring, mirroring `Neg`/`Scale`) — but any
  `MatMul`, `Sum`, `Index`, or non-`(1, 1)` leaf anywhere in the same side
  still forces it, exactly as today.
- **Variable-dependent product restriction**: `matmul_entries` accumulates
  one `QuadraticForm` per output entry as a sum of up to `k` pairwise
  products (the shared dimension); each individual pairwise product must
  still have at least one constant side, exactly like elementwise `Mul`'s
  existing restriction — a dot product of two variable-dependent vectors
  (e.g. `CVX.MATMUL(x, y)` for two variable columns) is rejected with the
  existing `VECTOR_MUL_ERROR`, unchanged.
- **Lazy shape validation**: `CVX.MATMUL` does not validate `matmul_shape`
  at call time (Non-Objective); the first `matmul_shape`/`infer_shape`
  call (`CVX.DESCRIBE`/`CVX.SHAPE`) or `linearize_shaped`/`matmul_entries`
  call (a solve) that reaches the `MatMul` node is what surfaces an
  incompatible-shape error. `CVX.TRANSPOSE` never fails regardless of when
  its shape is inspected. `@`/`.T` used inside a string behave identically
  — a parsed `Expr::MatMul`/`Expr::Transpose` resolves unconditionally to
  `Expression::matmul`/`Expression::transpose` (Interface), with no eager
  shape check performed by the resolver either.
- **`@`/`.T` precedence and associativity**: `@` sits in the same
  precedence tier as `*`/`/` (left-associative); `.T` binds tighter than
  unary `-` and every binary operator, applying only to the immediately
  preceding `primary` (Interface). This is exactly Python's own operator
  precedence table for these two spellings, so a formula written the way
  a NumPy user already writes it in Python parses the way they already
  expect, with no `cvxx`-specific precedence surprises.
- **`.T`/dotted-identifier disambiguation**: the tokenizer's pre-existing
  "identifiers may contain dots" rule is narrowed by exactly one
  maximal-munch exception (Interface's `is_transpose_suffix`): an
  identifier's trailing `.T`, when not itself followed by another
  identifier-continuation character, is lexed as a separate `Transpose`
  token instead of being folded into the identifier. Every other dotted
  identifier — including one containing `.T` in the middle (e.g.
  `"a.T.b"`, still one identifier token, since `.T` here is followed by
  `.`, an identifier-continuation character) — tokenizes exactly as
  before. The only behavior change is for an identifier whose *entire*
  name literally ends in `.T` (e.g. a registered parameter or variable
  named `"cost.T"`); see Error Handling for the resulting, narrow,
  intentional edge case.

## Error Handling

- `left`/`right`/`operand` resolves to an unknown identifier or handle
  (wrong kind, unregistered name, malformed handle string, etc.) →
  `CvxError::UnknownIdentifier`, identical to every other functional
  builder.
- Duplicate `name` (or a name already registered under a different object
  table) → `CvxError::AmbiguousIdentifier` when `name` is already used by
  a **different** object table (SPEC-0002's cross-table uniqueness rule);
  reusing a name already used by another **expression** is not an error —
  `insert_expression` overwrites the old entry, exactly as every other
  expression-producing function already does.
- `CVX.MATMUL`'s two operand shapes fail `matmul_shape` (`a.1 != b.0`) →
  `CvxError::InvalidExpression`, naming both operand shapes and which
  counts failed to match (the distinct, ISSUE-0018-required error,
  distinguishable by wording and by the fact that it is never raised for
  a `(1, 1)` operand against anything — unlike `broadcast_shape`'s
  "shape mismatch" message), surfaced lazily the first time the
  resulting expression's shape is inferred or solved (Data Model).
- A solve that reaches a `MatMul`/`Sum`/`Index`-derived term built from
  two variable-dependent operands fails with the existing
  `VECTOR_MUL_ERROR` (SPEC-0014), unchanged — not a new error introduced
  here, and not confused with the `matmul_shape` error above (different
  wording, different trigger: one is about element *sizes*, the other
  about *both sides depending on an unknown*).
- `CVX.TRANSPOSE` never produces an expression-construction error of its
  own (its operand resolution can still fail with
  `CvxError::UnknownIdentifier`/`AmbiguousIdentifier` as above).
- Internal registry failure → `CvxError::Registry`, with a logged
  diagnostic, as elsewhere.

### Grammar-specific errors (`@`/`.T` inside a string)

- `@`/`.T` lexing and parsing never themselves raise a new error class
  beyond the existing `CvxError::InvalidExpression` parse-error family:
  a malformed operand on either side of `@` (e.g. a trailing `@` at end
  of input, `"A @"`) produces the same "expected number, identifier, or
  '('" parse error `parse_primary` already raises for any other missing
  operand.
- An unresolvable identifier nested as an `@`/`.T` operand →
  `CvxError::UnknownIdentifier`, exactly as for any other nested
  identifier today.
- A `matmul_shape` mismatch reached through `@` surfaces identically to
  `CVX.MATMUL`'s (lazily, at describe/solve time) — the operator performs
  no additional eager validation, consistent with `CVX.MATMUL` itself
  (Non-Objective).
- **The one narrow, intentional edge case**: an identifier registered
  under a name whose entire text ends in literal `.T` (e.g. a parameter
  or variable named `"cost.T"`) can no longer be referenced by that full
  dotted name inside a `CVX.EXPRESSION`/`CVX.CONSTRAINT` string — the
  tokenizer now always splits a trailing `.T` into a separate `Transpose`
  token (Interface/Data Model). Writing `"cost.T"` in a string now parses
  as `Expr::Transpose(Expr::Identifier("cost"))`, which resolves
  successfully only if `"cost"` (without the suffix) also happens to be a
  separately registered name — otherwise it fails with the ordinary
  `CvxError::UnknownIdentifier("cost")`, naming `cost`, not `cost.T`. This
  is a deliberate, narrow trade-off (Non-Objective) in exchange for
  idiomatic `.T` syntax; the object itself is never renamed or otherwise
  affected, it is simply no longer nameable by its full literal text
  inside this one string grammar — `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE`,
  the functional builders, and every other handle-based function continue
  to accept the literal `"cost.T"` name unchanged, since none of them
  parse it through this tokenizer.

## Test Approach

### `cvxrust` (`cvxrust/src/reduce_tests.rs`)

- `Expression::matmul`/`Expression::transpose` construct the expected
  `Expression::MatMul`/`Expression::Transpose` variants.
- `linearize_shaped`/`reduce_expression`, via `solve`, correctly compute:
  a `(1, n)` row times an `(n, 1)` column (scalar/"dot product" result),
  an `(n, 1)` column times a `(1, n)` row (outer-product result), and a
  general `(m, k)` times `(k, n)` matrix product — each against a constant
  `Parameter` operand and a `Variable` operand, confirmed against
  `Solution::variable_values`/`objective_value`.
- `transpose_entries` correctly permutes a row vector into a column
  vector, a column vector into a row vector, and a general non-square
  matrix; transposing a `(1, 1)` operand is a no-op.
- A constraint built from `Expression::matmul(weights_param, x_variable)`
  (a known weights table applied to an unknown variable column) restricts
  the resulting combined values while `x` itself remains otherwise free —
  an end-to-end `solve` test, mirroring SPEC-0014/SPEC-0015's equivalent
  tests.
- `MatMul`/`Transpose` nested inside `Sum`/`Index`, and `Index`/`Sum`
  nested inside `MatMul`'s operands, each reduce correctly (e.g. a
  combined-risk-style `Sum` of a `MatMul` chain).
- A `MatMul` of two incompatible shapes (`a.1 != b.0`) reduces to a
  descriptive `Err` from `matmul_shape`/`matmul_entries`, not a panic,
  naming both shapes.
- A `MatMul` of two variable-dependent operands fails with the existing
  `VECTOR_MUL_ERROR`, confirming no accidental new quadratic support was
  introduced.
- `check_expr_shapes`/`is_all_scalar` returns `false`/`Err` for any
  expression containing `MatMul`, even one that is otherwise entirely
  `(1, 1)`-shaped; a bare `Transpose` of an otherwise-scalar-only
  expression does not by itself force this (it recurses), but `Transpose`
  nested inside a `MatMul`/`Sum`/`Index` does (transitively, via the
  outer construct).

### `cvxx` (`src/analytics/shape.rs`, `src/excel/expression.rs`)

- `matmul_shape` returns the expected `(rows, cols)` for a compatible pair
  of shapes (including the `(1, n) x (n, 1)` and `(n, 1) x (1, n)` cases)
  and a descriptive `CvxError::InvalidExpression`, naming both shapes, for
  an incompatible pair — including confirming it is never satisfied by a
  `(1, 1)` operand the way `broadcast_shape` is.
- `infer_shape` propagates `matmul_shape`'s result/error for a `MatMul`
  node, and swaps `(rows, cols)` for a `Transpose` node (including a
  `(1, 1)` operand producing `(1, 1)`).
- `render_expression` renders `MatMul` as `(<left>) @ (<right>)` and
  `Transpose` as `(<operand>).T`, for both named and unnamed operands
  (reusing SPEC-0013's name preference).
- `CVX.MATMUL`/`CVX.TRANSPOSE` unit tests: successful construction for
  parameter/variable/expression operands in every combination; unknown/
  incompatible `left`/`right`/`operand`; reusing a name already used by a
  different object table (`AmbiguousIdentifier`); reusing a name already
  used by another expression (overwrites, no error); confirming
  `CVX.MATMUL` does **not** error at call time even when the two operands'
  shapes are already known to be incompatible (lazy-validation
  confirmation, Non-Objective) — the error only appears via a subsequent
  `CVX.DESCRIBE`/`CVX.SHAPE`/solve.

### `cvxx` grammar (`src/analytics/parser.rs`, `src/analytics/resolve.rs`)

- Tokenizer: `@` lexes to `Token::At`; `.T` at the end of an identifier or
  immediately after `)` lexes to `Token::Transpose`; an ordinary dotted
  identifier not ending in `.T` (e.g. `"my.variable"`) and one containing
  `.T` in the middle (e.g. `"a.T.b"`) each still lex as a single
  `Token::Identifier` unchanged; an identifier whose entire name is
  literally `.T`-suffixed (e.g. `"cost.T"`) now lexes as
  `Identifier("cost")` followed by `Token::Transpose` (the documented edge
  case, Error Handling); `"A.T.T"` lexes as `Identifier("A")` followed by
  two `Token::Transpose` tokens; existing tokens unaffected.
- Parser: `"A @ B"` parses to `Expr::MatMul`; `"A.T"` parses to
  `Expr::Transpose(Expr::Identifier("A"))`; `"A @ B * C"` parses as
  `(A @ B) * C` and `"A * B @ C"` as `(A * B) @ C` (same-tier,
  left-associative precedence); `"-A.T"` parses as `Neg(Transpose(A))`,
  not `Transpose(Neg(A))` (`.T` binds tighter than unary `-`); `"(A +
  B).T"` and `"sum(X).T"` each parse to a `Transpose` wrapping the
  parenthesized/call sub-expression; `"A.T.T"` parses to a doubly-nested
  `Transpose(Transpose(Identifier("A")))`; a trailing `@` or `.T` with no
  valid following/preceding operand produces a descriptive parse error.
- Resolver: `"A @ B"`/`"A.T"` resolve to the exact same
  `cvxrust::Expression::MatMul`/`Expression::Transpose` tree
  `CVX.MATMUL`/`CVX.TRANSPOSE` would build for the same operands; an
  unresolvable identifier nested in either operand produces the documented
  `UnknownIdentifier` error; nested combinations (e.g.
  `"sum(w.T @ Sigma @ w)"`, a combined-risk-style formula) resolve
  correctly.
- Dependency tracking: `resolve_expr("w.T @ Sigma @ w")`, where `w`/
  `Sigma` are registered, include both in the returned
  `ResolvedExpr::dependencies` — confirming correct "for free" dependency
  tracking, in contrast with the `CVX.MATMUL`/`CVX.TRANSPOSE` functional
  builders (which still record none, per the pre-existing limitation,
  unchanged).

### Integration

- A sample workbook creates a parameter weights table and a variable
  allocations column of compatible shapes, builds
  `CVX.CONSTRAINT("sum(weights @ x) <= 100")`, solves the resulting
  problem, and confirms the solved allocation respects the combined
  (matrix-multiplied, then summed) limit.
- A sample workbook separately builds a combined-risk-style formula
  (`CVX.EXPRESSION("w.T @ Sigma @ w")` for a parameter covariance matrix
  `Sigma` and a variable weight column `w`), confirming `CVX.DESCRIBE`/
  `CVX.SHAPE` report a `(1, 1)` result and a solve against it succeeds
  when `Sigma`'s side of every `@` is constant.
- A sample workbook attempts `CVX.EXPRESSION("A @ B")` with incompatible
  shapes and confirms `CVX.DESCRIBE`/`CVX.SHAPE` (or a `CVX.SOLVE` built
  from it) surfaces `#VALUE!` with the matrix-multiplication-specific
  error message, distinct from an elementwise `CVX.ADD`/`CVX.MUL`
  shape-mismatch error on a similarly-shaped pair of operands.

## Dependencies

- SPEC-0002/SPEC-0003 for parameter/variable handle and shape storage
  conventions (`(rows, cols)`).
- SPEC-0004 for the functional-builder convention
  (`resolve_handle_arg`/`insert_expression`/`run_binary`/`run_unary`, and
  the accepted dependency-tracking limitation this specification
  inherits unchanged for the functional builders only).
- SPEC-0007 for `CVX.DESCRIBE`/`CVX.SHAPE`/`infer_shape`/
  `render_expression`/`MAX_DESCRIBE_LEN`.
- SPEC-0013 for registered-name-preferring rendering, reused unchanged by
  `MatMul`'s/`Transpose`'s `render_expression` arms.
- SPEC-0014 (implemented) for the `ShapedForm`/`linearize_shaped`/
  `quadratize`/`check_expr_shapes`/`broadcast_shape`/`VECTOR_MUL_ERROR`
  machinery this specification extends, and for the precedent (`Sum`)
  this specification's `MatMul`/`Transpose` arms directly follow.
- SPEC-0015 (implemented) for the `check_index_bounds`/`select_sub_block`
  precedent `matmul_shape`/`matmul_entries`/`transpose_entries` directly
  follow, for the "forces the affine-only path" precedent `MatMul` follows
  exactly, and for the recursive-descent parser/tokenizer
  (`src/analytics/parser.rs`) and resolver (`src/analytics/resolve.rs`)
  this specification further extends with new `At`/`Transpose` tokens and
  `Expr::MatMul`/`Expr::Transpose` AST nodes/resolver arms — a sibling
  extension to, not a reuse of, SPEC-0015's own `Expr::Call`/
  `resolve_call` mechanism (`@`/`.T` are operators, not function calls).
- ISSUE-0016 (quadratic problems over vectors and matrices), which the
  two-variable-dependent-operand `MatMul` case (Non-Objective) is deferred
  to or pending on, per ISSUE-0018's own notes.
- No new external crate dependencies.

## Status

Implemented. `cvxrust::Expression` gained the `MatMul(Box<Expression>,
Box<Expression>)` and `Transpose(Box<Expression>)` variants and the
`Expression::matmul`/`Expression::transpose` constructors
(`cvxrust/src/model.rs`). `cvxrust/src/reduce.rs` gained a crate-private
`matmul_shape` helper, `transpose_entries`/`matmul_entries` row-major
helpers, `MatMul`/`Transpose` arms in `linearize_shaped` (built on those
helpers), a `MatMul` arm in `check_expr_shapes` that unconditionally forces
the affine-only path (mirroring `Sum`/`Index`) and a `Transpose` arm that
recurses into its operand instead (mirroring `Neg`/`Scale`, since a bare
`Transpose` of an otherwise-scalar expression does not by itself force the
restriction), and defensive `MatMul`/`Transpose` arms in `quadratize` —
`Transpose`'s is actually reachable in practice (e.g. `Transpose` of an
all-scalar sub-expression at the top of an objective/constraint side),
unlike `Sum`/`Index`/`MatMul`'s, confirmed by test.

`src/analytics/shape.rs` gained the public `matmul_shape` helper (the same
rule, duplicated for `cvxx` per the existing `broadcast_shape`
precedent) plus `MatMul`/`Transpose` arms in `infer_shape` and
`render_expression` (rendering `(<left>) @ (<right>)` and
`(<operand>).T`, both valid `CVX.EXPRESSION` input for the same
expression). `CVX.MATMUL(left, right, [name])` and
`CVX.TRANSPOSE(operand, [name])` are implemented in
`src/excel/expression.rs` (reusing the existing `run_binary`/`run_unary`
helpers unchanged) and registered in `src/excel/mod.rs`.

The grammar extension is implemented as specified: `src/analytics/ast.rs`
gained `Expr::MatMul(ExprNode, ExprNode)`/`Expr::Transpose(ExprNode)`;
`src/analytics/parser.rs` gained `Token::At`/`Token::Transpose`, the
`is_transpose_suffix` maximal-munch tokenizer helper, an `At` arm in
`parse_mul_div`, and a new `parse_postfix` precedence tier between
`parse_unary` and `parse_primary`. One tokenizer refinement beyond the
original draft: `is_transpose_suffix` recurses one segment ahead when the
character immediately after a `.T` is itself `.` — this correctly
distinguishes a *chained* `.T.T` (where the second `.T` is its own,
separate maximal-munch suffix, so the first `.T` is still recognized as a
postfix token) from a genuinely longer dotted identifier like `"a.T.b"`
(where the segment after the first `.T` is not itself a `.T`, so the dot
continues the identifier as before); both cases are covered by test.
`src/analytics/resolve.rs` gained `Expr::MatMul`/`Expr::Transpose` arms in
`resolve`, resolving to the same `Expression::matmul`/`Expression::
transpose` the functional builders produce, with correct dependency
tracking confirmed by test (including the `"w.T @ Sigma @ w"`
combined-risk-style case).

`docs/expressions.md` documents `CVX.MATMUL`/`CVX.TRANSPOSE` and the
`@`/`.T` operator syntax (including the one narrow, intentional
`.T`-suffixed-identifier edge case) with examples; `docs/inspection.md`
was updated with `CVX.DESCRIBE`/`CVX.SHAPE` behavior for `MatMul`/
`Transpose` expressions.
