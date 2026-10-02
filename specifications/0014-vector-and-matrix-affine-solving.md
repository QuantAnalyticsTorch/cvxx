---
id: SPEC-0014
title: Affine solving for vector and matrix variables and parameters
issue: ISSUE-0014
status: implemented
created: 2026-10-02
---

## Objective

Extend `cvxrust::solve` so that `Variable`s and `Parameter`s with a shape
other than `(1, 1)` may be used anywhere a scalar one is used today — most
usefully in constraints — instead of universally failing with
`"solver only supports scalar (1x1) variables and parameters"`
(SPEC-0010/SPEC-0011). The existing elementwise `Add`/`Sub`/`Mul`/`Div`/
`Neg`/`Scale` operators are extended to operate per-entry over
broadcast-compatible shapes (the same broadcasting rule `cvxx`'s
diagnostic-only `src/analytics/shape.rs::broadcast_shape` already uses for
`CVX.DESCRIBE`/`CVX.SHAPE`, reimplemented here since `cvxrust` does not
depend on `cvxx`), and a declared `Constraint` between two differently- or
identically-shaped sides broadcasts to one solved row per output entry.
`Solution::variable_values` (already `Vec<Vec<f64>>`, SPEC-0010) is
populated with every entry of each variable's own shape, in the same
row-major order `CVX.VALUE` (SPEC-0007) already expects. This is additive
to the existing affine/quadratic translation layer (SPEC-0010/SPEC-0011),
not a new solver integration.

A vector/matrix variable or parameter is of little practical use if it can
only be bounded entry-by-entry and never combined into a single number for
an objective or a total-vs-limit constraint — and, structurally, no such
combination is possible with only `Add`/`Sub`/`Mul`/`Div`/`Neg`/`Scale`,
since none of them can shrink a shape (see Data Model). This specification
therefore also introduces exactly one new, minimal reduction primitive,
`Expression::Sum`, and a corresponding `CVX.SUM` Excel function: summing
every entry of a vector/matrix expression down to a single number is a
purely proportional ("linear") operation — it is `Sum` composed with the
*existing* elementwise `Mul` that gives users a weighted total (a
"dot product with a parameter"), with no new quadratic machinery required.
Referring to a single entry or sub-section of a vector/matrix (indexing),
and genuine quadratic terms over vector/matrix shapes (e.g. `x' * Q * x`),
remain out of scope — see Non-Objective.

## Non-Objective

- Vector/matrix-shaped **objectives that do not go through `Sum`**. The
  objective must still reduce to a single value (shape `(1, 1)`); a bare
  vector/matrix `Variable`/`Parameter`, or any expression built only from
  `Add`/`Sub`/`Mul`/`Div`/`Neg`/`Scale`, can never produce shape `(1, 1)`
  unless every leaf already is `(1, 1)` (see Data Model) — `Sum` is the one
  way around this, and is in scope (see Objective/Interface/Data Model
  below). A vector/matrix variable may still be declared and freely used in
  constraints — summed, compared, bounded — without ever being wrapped in
  `Sum` or appearing in the objective at all.
- Quadratic (degree-2, "variable × variable") terms anywhere within an
  objective, or anywhere within a single side of a constraint, that also
  contains a non-`(1, 1)`-shaped `Variable`/`Parameter` leaf **or a `Sum`**.
  SPEC-0011's quadratic support continues to apply exactly as today,
  unchanged, but only when an entire objective, or an entire constraint
  side, is built exclusively from `(1, 1)`-shaped leaves and contains no
  `Sum`. (A quadratic scalar term compared, across a constraint's two
  sides, against a vector/matrix-shaped or `Sum`-containing term — e.g.
  `x*x <= w` where `w` is a `(3, 1)` vector, or `x*x <= CVX.SUM(w)` —
  remains supported and requires no new code; see Data Model.) General
  quadratic support for vector/matrix-shaped terms (e.g. a weighted sum of
  *squares*, or `x' * Q * x` for a matrix `Q`) is deferred to ISSUE-0016.
- Referring to a single entry, row, column, or sub-section of a
  vector/matrix (indexing/slicing) — the narrowed ISSUE-0015. `Sum` always
  reduces its *entire* operand to one number; it cannot single out part of
  it.
- Matrix-vector or matrix-matrix multiplication (as opposed to elementwise
  `Mul`) and transpose. A weighted total of a vector against a same-shape
  parameter is achievable today via `CVX.SUM(CVX.MUL(weights, x))`; a
  general matrix-vector product is left to a future specification.
- Any change to `CVX.PARAMETER`, `CVX.VARIABLE`, `CVX.PROBLEM`,
  `CVX.SOLVE`, `CVX.VALUE`, `CVX.STATUS`, `CVX.OBJECTIVE_VALUE`, or any
  Excel-facing function signature or behavior other than the one new
  `CVX.SUM` function (`src/excel/*.rs`). Everything else needed on that
  side — shape-aware storage, row-major value recovery — was already built
  generically in earlier specs (SPEC-0002/SPEC-0003/SPEC-0007) and requires
  no change; see Dependencies. `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE`
  (`src/analytics/shape.rs`) require a small, additive change — a new
  `Expression::Sum` match arm — covered in Data Model/Interface below, not
  a behavior change for any existing expression kind.
- No change to `CVX.EXPRESSION`'s string grammar (SPEC-0004 already
  deferred "general function calls inside expression strings" to a
  follow-up). `CVX.SUM` is a functional-builder-only addition, like
  `CVX.NEG`; a string-grammar form (e.g. `SUM(w)`) is left to a future
  specification extending the grammar itself.
- Raising `MAX_VARIABLES`/`MAX_CONSTRAINTS` beyond 200 each, or changing
  `MAX_ITERATIONS`/`PSD_TOLERANCE`. Only the accounting basis of the two
  size constants changes (declared-entity count → total scalar entry/row
  count — see Data Model), not their numeric value.
- The one-shot convenience solvers (`CVX.LP_SOLVE`/`CVX.QP_SOLVE`/
  `CVX.LEAST_SQUARES`, ISSUE-0008) — not yet implemented, unaffected
  either way.
- Dual values, sensitivity analysis, warm starts, or any solver tuning
  beyond the existing fixed iteration bound (unchanged from
  SPEC-0010/SPEC-0011).

## Interface

### `cvxrust::Expression` gains one new variant

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // ... existing variants unchanged ...
    /// The sum of every entry of a (possibly vector/matrix-shaped)
    /// expression, reduced to a `(1, 1)` value.
    Sum(Box<Expression>),
}

impl Expression {
    /// Creates a sum-reduction expression.
    pub fn sum(expr: Expression) -> Self {
        Expression::Sum(Box::new(expr))
    }
}
```

This is the only public `cvxrust` API change. `solve(&Problem) -> Solution`'s
signature, and every other public struct/enum (`Variable`, `Constraint`,
`Problem`, `Solution`, `SolveStatus`, `Sense`, `Relation`,
`MAX_VARIABLES`, `MAX_CONSTRAINTS`, `MAX_ITERATIONS`), are unchanged from
SPEC-0010/SPEC-0011.

### New Excel function

```
CVX.SUM(operand, [name])
```

- `operand`: a parameter, variable, or expression handle, a registered
  name, or a bare numeric literal (resolved identically to every other
  unary/binary builder's operand, via the existing
  `resolve_handle_arg` in `src/excel/expression.rs`).
- `name`: optional unique name for the resulting expression.
- Returns: a string handle of the form `cvx:expr:<uuid>`, exactly like
  `CVX.ADD`/`CVX.NEG`/etc.

Implementation is a direct, minimal addition alongside the existing
unary builder:

```rust
/// `CVX.SUM(operand, [name])` — sums every entry of an expression into a
/// single `(1, 1)` value.
#[export_name = "CVX.SUM"]
pub extern "system" fn cvx_sum(operand: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_unary(operand, name, Expression::sum);
    to_xloper_result(result, "CVX.SUM")
}
```

(`run_unary`'s existing signature, `fn(operand, name, op: fn(Expression) ->
Expression) -> Result<String, CvxError>`, already matches `Expression::sum`
exactly — no changes to `run_unary` itself.)

Like every other functional builder, `CVX.SUM` records no dependency names
(`run_unary` calls `insert_expression(name, expr, vec![])`, unchanged); a
variable reachable only through `CVX.SUM`/`CVX.ADD`/etc. handles (never
through a `CVX.EXPRESSION`/`CVX.CONSTRAINT` string) is therefore not swept
into `problem.variables` by `CVX.PROBLEM`'s existing
`expand_variable_dependencies` (`src/excel/problem.rs`, its own doc comment
already states this is deliberate: "functional builders ... do not track
[dependencies]"). This is a pre-existing characteristic of every functional
builder, unrelated to and unaffected by this specification; `CVX.SUM`
inherits it unchanged and does not need to work around it here.

The private `QuadraticForm` (SPEC-0011) gains `#[derive(Clone)]`:

```rust
#[derive(Debug, Clone)]
struct QuadraticForm { /* fields unchanged */ }
```

This is needed because, as described below, a broadcast binary operator
reads the same source entry of a smaller-shaped operand once per output
entry, and `QuadraticForm`'s arithmetic methods (`add`/`sub`/`negate`/
`scale`) consume `self` by value.

No new external crate dependencies (`clarabel`, `nalgebra` are already
present per SPEC-0010/SPEC-0011).

## Data Model

### Scalar-slot numbering (replaces `index: HashMap<u64, usize>` / `n`)

Today, `solve` builds `index: HashMap<u64, usize>` mapping each
`Variable::id` to its position in `problem.variables` (`n =
problem.variables.len()`), since every variable occupies exactly one
scalar slot. This is generalized to an **offset** map, since a variable of
shape `(r, c)` now occupies `r * c` contiguous scalar slots:

```rust
let mut offsets: HashMap<u64, usize> = HashMap::with_capacity(problem.variables.len());
let mut n_total = 0usize;
for v in &problem.variables {
    offsets.insert(v.id, n_total);
    n_total += v.shape.0 * v.shape.1;
}
```

For an all-scalar problem (every `Variable::shape == (1, 1)`), `offsets`
assigns exactly the same values `index` would have (0, 1, 2, ...) and
`n_total == problem.variables.len()`, so every existing all-scalar code
path is unaffected by this renaming. `offsets`/`n_total` replace `index`/`n`
everywhere in `solve`; `quadratize`'s signature and body (SPEC-0010/
SPEC-0011) are **unchanged** — it is simply called with an offset map
instead of a position map, which for the all-scalar expressions it is still
exclusively used for (see routing, below) means identical behavior.

### Size limit (replaces the `problem.variables.len() > MAX_VARIABLES` check)

```rust
if n_total > MAX_VARIABLES {
    return error_solution(SIZE_LIMIT_ERROR);
}
```

where `SIZE_LIMIT_ERROR = "problem exceeds solver size limit (200 scalar \
variables / 200 scalar constraint rows)"` (replacing today's "200
variables / 200 constraints" wording, since the limit now counts total
scalar entries/rows rather than declared `Variable`/`Constraint` count). For
an all-scalar problem the two countings coincide exactly, so this is not a
behavioral regression for any problem solvable today. The constraint-row
half of this check is applied incrementally; see "Constraint translation"
below. As today (SPEC-0011), a quadratic constraint's extra second-order-
cone rows (`r + 2`, see SPEC-0011) are not separately counted against this
limit — consistent with the existing, already-imprecise accounting for
quadratic constraints.

### Routing: which expressions keep quadratic support

Define, reusing the existing `check_expr_shapes` (SPEC-0010, body
unchanged except for one new arm) purely as a boolean predicate:

```rust
/// `true` when every `Variable`/`Parameter` leaf reachable from `expr` has
/// shape `(1, 1)` **and** `expr` contains no `Sum` node. (`Sum` always
/// collapses its operand to `(1, 1)`, but — like a non-scalar leaf — it
/// forces the affine-only `linearize_shaped` path; see below.)
fn is_all_scalar(expr: &Expression) -> bool {
    check_expr_shapes(expr).is_ok()
}
```

`check_expr_shapes` gains exactly one new match arm (its existing arms,
and `SHAPE_ERROR`'s role as a mere "not all-scalar" sentinel reused from
SPEC-0010, are otherwise untouched):

```rust
Expression::Sum(_) => Err(SHAPE_ERROR.to_string()),
```

`validate_shapes` (SPEC-0010 — the function that made *any* non-`(1, 1)`
leaf a hard failure for the whole problem) is removed; its blanket
rejection is what this specification replaces.

A single dispatcher reduces any (objective, or one constraint side)
expression to a per-entry shaped form:

```rust
/// Row-major reduction of a (possibly vector/matrix-shaped) `Expression`:
/// one `QuadraticForm` per output entry. Every entry's `quad` is empty
/// unless `shape == (1, 1)` (see routing below).
struct ShapedForm {
    shape: (usize, usize),
    /// Row-major, length == shape.0 * shape.1.
    entries: Vec<QuadraticForm>,
}

fn reduce(
    expr: &Expression,
    offsets: &HashMap<u64, usize>,
    n_total: usize,
) -> Result<ShapedForm, String> {
    if is_all_scalar(expr) {
        // Preserves SPEC-0011's quadratic support exactly, unchanged.
        let form = quadratize(expr, offsets, n_total)?;
        Ok(ShapedForm { shape: (1, 1), entries: vec![form] })
    } else {
        linearize_shaped(expr, offsets, n_total)
    }
}
```

`quadratize` (SPEC-0010/SPEC-0011) needs one new match arm purely for
exhaustiveness, since `Expression` gained a variant: given
`is_all_scalar`'s new `Sum` arm above, `quadratize` is, by construction,
never actually invoked on a tree containing `Sum` (routing always sends
such a tree through `linearize_shaped` instead — see below). Rather than
an `unreachable!()` (this crate avoids panics on malformed-but-well-typed
input; see Error Handling), the arm is implemented defensively, identically
to `linearize_shaped`'s own `Sum` handling below:

```rust
Expression::Sum(e) => {
    let inner = reduce(e, index, n)?;
    Ok(inner.entries.into_iter().fold(QuadraticForm::zero(n, 0.0), |acc, x| acc.add(&x)))
}
```

`reduce` is called exactly twice per constraint (once for `lhs`, once for
`rhs`) and once for `problem.objective` — i.e., the routing decision
(`is_all_scalar`) is made once per such top-level expression, not
separately for every sub-expression. `linearize_shaped`'s own recursive
calls (below) call `linearize_shaped` directly, not `reduce`: once a
non-`(1, 1)` leaf appears anywhere in an objective or constraint side, the
*entire* side is restricted to affine (degree ≤ 1) arithmetic, even for an
all-scalar sub-expression nested inside it (e.g. `Add(x * x, w)`, with `w`
a `(3, 1)` variable, rejects the `x * x` sub-product — see "Mul/Div" below
— even though `x * x` alone would be fine). This is a deliberate,
documented scope boundary (Non-Objective), not an oversight: it keeps the
affine-only vector/matrix path simple to specify, implement, and test, and
ISSUE-0014 explicitly defers all vector/matrix quadratic support to
ISSUE-0016 regardless of how it would arise.

This routing rule also explains why an objective must be wrapped in `Sum`
(or already scalar) to reference a vector/matrix variable at all: given
only `Constant`/`Variable`/`Parameter`/`Add`/`Sub`/`Mul`/`Div`/`Neg`/
`Scale`, an expression's shape is always either `(1, 1)` (when every leaf
is `(1, 1)`) or the shape of its largest non-scalar leaf (broadcasting
never *shrinks* a shape) — `Sum` is the only construct that can bring a
vector/matrix-shaped sub-expression back down to `(1, 1)`. A vector/matrix
expression *not* wrapped in `Sum` is therefore rejected by the explicit
objective-shape check below (a bare `CVX.MINIMIZE(w)` for a `(3, 1)` `w`
still errors; `CVX.MINIMIZE(CVX.SUM(w))` does not). The check is still
performed explicitly (rather than relying on it being implied by
`is_all_scalar`) so the behavior stays correct and forward-compatible once
a future specification adds further shape-changing operators (e.g.
indexing, ISSUE-0015, which produces a smaller, not necessarily `(1, 1)`,
shape).

### Shape broadcasting

```rust
/// Broadcasts two shapes per the existing `cvxx` diagnostic rule
/// (`src/analytics/shape.rs::broadcast_shape`, duplicated here since
/// `cvxrust` does not depend on `cvxx`): a `(1, 1)` operand broadcasts to
/// the other operand's shape; otherwise the two shapes must be equal.
fn broadcast_shape(a: (usize, usize), b: (usize, usize)) -> Result<(usize, usize), String> {
    if a == (1, 1) {
        Ok(b)
    } else if b == (1, 1) || a == b {
        Ok(a)
    } else {
        Err(format!("shape mismatch: {}x{} vs {}x{}", a.0, a.1, b.0, b.1))
    }
}

/// Reads shaped-form entry `k` (row-major), broadcasting a `(1, 1)` form's
/// single entry across every `k` when `form.shape != (1, 1)`.
fn entry_at(form: &ShapedForm, k: usize) -> &QuadraticForm {
    if form.shape == (1, 1) {
        &form.entries[0]
    } else {
        &form.entries[k]
    }
}
```

### `linearize_shaped` — the new affine, shape-broadcasting reduction

```rust
const VECTOR_MUL_ERROR: &str = "solver only supports linear (affine) \
objectives and constraints once a vector or matrix (non-1x1) variable or \
parameter is involved; a product of two variable-dependent terms was \
found";

const VECTOR_DIV_ERROR: &str = "solver only supports linear (affine) \
objectives and constraints once a vector or matrix (non-1x1) variable or \
parameter is involved; division by a variable-dependent term was found";

fn linearize_shaped(
    expr: &Expression,
    offsets: &HashMap<u64, usize>,
    n_total: usize,
) -> Result<ShapedForm, String> {
    match expr {
        Expression::Constant(c) => Ok(ShapedForm {
            shape: (1, 1),
            entries: vec![QuadraticForm::zero(n_total, *c)],
        }),
        Expression::Parameter { shape, data, .. } => Ok(ShapedForm {
            shape: *shape,
            entries: data.iter().map(|&c| QuadraticForm::zero(n_total, c)).collect(),
        }),
        Expression::Variable(v) => match offsets.get(&v.id) {
            Some(&off) => {
                let count = v.shape.0 * v.shape.1;
                let entries = (0..count)
                    .map(|k| {
                        let mut form = QuadraticForm::zero(n_total, 0.0);
                        form.linear[off + k] = 1.0;
                        form
                    })
                    .collect();
                Ok(ShapedForm { shape: v.shape, entries })
            }
            None => Err(
                "objective or constraint references a variable not included \
                 in the problem's variable list".to_string(),
            ),
        },
        Expression::Add(l, r) | Expression::Sub(l, r) => {
            let lf = linearize_shaped(l, offsets, n_total)?;
            let rf = linearize_shaped(r, offsets, n_total)?;
            let shape = broadcast_shape(lf.shape, rf.shape)?;
            let count = shape.0 * shape.1;
            let is_add = matches!(expr, Expression::Add(..));
            let entries = (0..count)
                .map(|k| {
                    let l_entry = entry_at(&lf, k).clone();
                    let r_entry = entry_at(&rf, k);
                    if is_add { l_entry.add(r_entry) } else { l_entry.sub(r_entry) }
                })
                .collect();
            Ok(ShapedForm { shape, entries })
        }
        Expression::Mul(l, r) => {
            let lf = linearize_shaped(l, offsets, n_total)?;
            let rf = linearize_shaped(r, offsets, n_total)?;
            let shape = broadcast_shape(lf.shape, rf.shape)?;
            let count = shape.0 * shape.1;
            let mut entries = Vec::with_capacity(count);
            for k in 0..count {
                let l_entry = entry_at(&lf, k);
                let r_entry = entry_at(&rf, k);
                entries.push(if l_entry.is_constant() {
                    r_entry.clone().scale(l_entry.constant)
                } else if r_entry.is_constant() {
                    l_entry.clone().scale(r_entry.constant)
                } else {
                    return Err(VECTOR_MUL_ERROR.to_string());
                });
            }
            Ok(ShapedForm { shape, entries })
        }
        Expression::Div(l, r) => {
            let lf = linearize_shaped(l, offsets, n_total)?;
            let rf = linearize_shaped(r, offsets, n_total)?;
            let shape = broadcast_shape(lf.shape, rf.shape)?;
            let count = shape.0 * shape.1;
            let mut entries = Vec::with_capacity(count);
            for k in 0..count {
                let l_entry = entry_at(&lf, k);
                let r_entry = entry_at(&rf, k);
                if !r_entry.is_constant() {
                    return Err(VECTOR_DIV_ERROR.to_string());
                }
                if r_entry.constant.abs() < 1e-12 {
                    return Err("division by zero in objective or constraint".to_string());
                }
                entries.push(l_entry.clone().scale(1.0 / r_entry.constant));
            }
            Ok(ShapedForm { shape, entries })
        }
        Expression::Neg(e) => {
            let f = linearize_shaped(e, offsets, n_total)?;
            Ok(ShapedForm {
                shape: f.shape,
                entries: f.entries.into_iter().map(|e| e.negate()).collect(),
            })
        }
        Expression::Scale { scalar, expr } => {
            let f = linearize_shaped(expr, offsets, n_total)?;
            Ok(ShapedForm {
                shape: f.shape,
                entries: f.entries.into_iter().map(|e| e.scale(*scalar)).collect(),
            })
        }
        Expression::Sum(e) => {
            let inner = linearize_shaped(e, offsets, n_total)?;
            let summed = inner
                .entries
                .into_iter()
                .fold(QuadraticForm::zero(n_total, 0.0), |acc, entry| acc.add(&entry));
            Ok(ShapedForm { shape: (1, 1), entries: vec![summed] })
        }
    }
}
```

`Sum`'s operand is reduced via `linearize_shaped` directly (not `reduce`),
consistent with every other `linearize_shaped` arm: a scalar quadratic
sub-expression inside a `Sum` (e.g. `CVX.SUM(CVX.MUL(x_times_x_expr, w))`)
is restricted to affine terms by the same rule as `Add(x * x, w)` above
(Non-Objective). `Sum` over an already-`(1, 1)` operand is legal but a
no-op (summing one entry); `Sum` is most useful over a genuinely
vector/matrix-shaped operand, most commonly `CVX.SUM(CVX.MUL(weights,
x))` for a weighted total.

`QuadraticForm::is_constant()` (SPEC-0011, unchanged) already checks both
`quad.is_empty()` and all-zero `linear`, so `Mul`/`Div`'s constant checks
above correctly require *no* variable dependence at all (not merely
affine), matching the existing scalar behavior.

### Objective reduction

```rust
let obj_form = match reduce(&problem.objective, &offsets, n_total) {
    Ok(form) => form,
    Err(message) => return error_solution(message),
};
if obj_form.shape != (1, 1) {
    return error_solution(format!(
        "objective must evaluate to a single value (shape 1x1); got shape {}x{}",
        obj_form.shape.0, obj_form.shape.1
    ));
}
let obj = obj_form.entries.into_iter().next().unwrap();
```

Everything after this (objective-vector/matrix `P`/`q` construction,
`Sense::Maximize` negation, recovered-objective-value computation) is
**unchanged** from SPEC-0010/SPEC-0011 — `obj` is the same `QuadraticForm`
those steps already consume.

### Constraint translation

For each `constraint` in `problem.constraints`, `lhs`/`rhs` are now reduced
independently via `reduce` (not `quadratize` directly), and may broadcast
to more than one row:

```rust
let lhs = match reduce(&constraint.lhs, &offsets, n_total) {
    Ok(form) => form,
    Err(message) => return error_solution(message),
};
let rhs = match reduce(&constraint.rhs, &offsets, n_total) {
    Ok(form) => form,
    Err(message) => return error_solution(message),
};
let shape = match broadcast_shape(lhs.shape, rhs.shape) {
    Ok(shape) => shape,
    Err(message) => return error_solution(message),
};
let count = shape.0 * shape.1;

total_constraint_rows += count;
if total_constraint_rows > MAX_CONSTRAINTS {
    return error_solution(SIZE_LIMIT_ERROR);
}

for k in 0..count {
    let mut diff = entry_at(&lhs, k).clone().sub(entry_at(&rhs, k));
    let mut relation = constraint.relation;

    // --- everything below is the existing SPEC-0011 per-row body,
    // unchanged, now invoked once per broadcast output entry `k` instead
    // of once per declared `Constraint` ---
    if !diff.is_affine() {
        if relation == Relation::Equal {
            return error_solution("quadratic equality constraints are not supported");
        }
        if relation == Relation::GreaterEqual {
            diff = diff.negate();
            relation = Relation::LessEqual;
        }
        match build_soc_block(&diff, n_total) {
            Ok(Some(block)) => { soc_blocks.push(block); continue; }
            Ok(None) => {}
            Err(message) => return error_solution(message),
        }
    }
    rows.push((relation, diff.linear.clone(), -diff.constant));
}
```

(`total_constraint_rows: usize`, initialized to `0` before the constraint
loop, is the only new piece of state; it realizes the incremental half of
the size-limit check described above, failing before `clarabel` is ever
invoked, consistent with today's fail-fast ordering.)

Because `diff` here is the same `QuadraticForm` SPEC-0011 already builds
and feeds into `build_soc_block`, a constraint whose scalar (all-`(1, 1)`-
leaf) side carries a quadratic term — e.g. `x * x <= w` with `w` a
`(3, 1)` vector parameter or variable — is handled automatically: `lhs`
(`x * x`) is reduced via `quadratize` (routing, above) and broadcasts
against `rhs`'s three entries, producing three independent per-entry
`diff`s, each with a non-empty `quad`, each routed into the existing
second-order-cone construction, one `SecondOrderConeT` block per output
row. No new code is needed for this case beyond the broadcasting and
looping already described.

The rest of `solve` (grouping `rows` by cone, appending `soc_blocks`,
building `P`/`q`/`A`/`b`, `DefaultSolver` construction and status mapping,
and the `n_total == 0` degenerate feasibility-only path) is **unchanged**
— it already operates generically over however many rows `rows`/
`soc_blocks` end up containing, and was already written as a loop over an
arbitrary number of rows.

### Recovery

```rust
let x = &solver.solution.x;
let mut variable_values = Vec::with_capacity(problem.variables.len());
let mut cursor = 0usize;
for v in &problem.variables {
    let count = v.shape.0 * v.shape.1;
    variable_values.push(x[cursor..cursor + count].to_vec());
    cursor += count;
}
```

(`cursor` tracks the same offsets as `offsets`, since both iterate
`problem.variables` in the same order; for an all-scalar problem this
produces exactly today's `vec![x[j]]` per variable.) The recovered
objective value computation (`obj.constant + dot(&obj.linear, x) +
quadratic_value`) is unchanged.

### `cvxx`-side diagnostic support for `Expression::Sum`

`src/analytics/shape.rs` (`infer_shape`/`render_expression`, SPEC-0007/
SPEC-0013, diagnostic-only, never called during solving) needs one new
match arm each, so `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE` recognize `Sum`
instead of failing to compile once the new `Expression` variant exists:

```rust
// infer_shape
Expression::Sum(_) => Ok((1, 1)),

// render_expression
Expression::Sum(e) => format!("sum({})", render_expression(e, registry)),
```

This mirrors the existing style of both functions exactly (compare
`Expression::Neg`'s single-operand arms) and is the only change to either
function.

## Error Handling

All failure paths return `Solution { status: SolveStatus::Error(message), \
objective_value: None, variable_values: vec![] }` via the existing
`error_solution` helper, unchanged. No panics: `offsets.get(&v.id)` and all
slicing in Recovery are derived directly from `problem.variables`'
own shapes and are therefore always in-bounds for a well-formed `Problem`.

New or changed error strings:

| Case | Message |
|---|---|
| Size limit (reworded; now total scalar entries/rows) | `"problem exceeds solver size limit (200 scalar variables / 200 scalar constraint rows)"` |
| Shape mismatch (new) | `"shape mismatch: {a0}x{a1} vs {b0}x{b1}"` |
| Objective not shape `(1, 1)` (new) | `"objective must evaluate to a single value (shape 1x1); got shape {r}x{c}"` |
| Product of two variable-dependent terms where a non-`(1,1)` leaf is involved (new) | `"solver only supports linear (affine) objectives and constraints once a vector or matrix (non-1x1) variable or parameter is involved; a product of two variable-dependent terms was found"` |
| Division by a variable-dependent term where a non-`(1,1)` leaf is involved (new) | `"solver only supports linear (affine) objectives and constraints once a vector or matrix (non-1x1) variable or parameter is involved; division by a variable-dependent term was found"` |

Unchanged from SPEC-0010/SPEC-0011 (still reachable, verbatim, only now
exclusively via the all-scalar `quadratize` path): division by zero,
dangling variable reference, degree ≥ 3 product, quadratic equality
constraint, non-convex quadratic constraint, unrecognized `clarabel` solver
status.

`"solver only supports scalar (1x1) variables and parameters"`
(SPEC-0010/SPEC-0011) is **removed** — this specification is precisely the
removal of that blanket restriction. `validate_shapes` (the function that
produced it) is deleted; `check_expr_shapes` is kept, repurposed as the
`is_all_scalar` predicate.

### Required test/message update

`src/excel/problem.rs`'s existing test
`solve_reports_excel_error_and_does_not_store_a_result` constructs a bare
`(2, 1)` variable used directly as the objective and asserts the old
blanket shape error. Under this specification that same scenario now hits
the *new* "objective must evaluate to a single value" error instead (the
variable itself is no longer rejected — using it as an entire objective
is). This test's expected `CvxError::SolveFailed(...)` message must be
updated to
`"objective must evaluate to a single value (shape 1x1); got shape 2x1"`.
No other existing test changes are expected.

### Documentation

`docs/problems.md` is updated to remove "a variable or parameter with a
shape other than `(1, 1)`" from the list of unsupported-problem bullets,
and to add: (a) constraints may freely use vector/matrix-shaped operands,
broadcasting per the rule above, with one solved row per output entry; (b)
the objective must still evaluate to a single value, achieved either by
using only `(1, 1)`-shaped operands or by wrapping a vector/matrix
expression in the new `CVX.SUM`; (c) the new shape-mismatch and
vector/matrix-nonlinear-term error messages.

`docs/expressions.md` is updated to add `CVX.SUM(operand, [name])` to the
"Functional builders" table, alongside `CVX.ADD`/`CVX.NEG`/etc., with an
example showing a weighted total:
`=CVX.SUM(CVX.MUL(weights_handle, x_handle))`.

## Test Approach

`cvxrust` unit tests (extending `mod tests` in `cvxrust/src/lib.rs`):

- `linearize_shaped` directly:
  - A `(3, 1)` parameter reduces to 3 entries whose constants match its
    (row-major) data, each with an all-zero `linear`.
  - A `(2, 2)` variable reduces to 4 entries, each a one-hot `linear`
    vector at its own offset within `n_total`.
  - `Add`/`Sub` of two equal-shape `(3, 1)` variables combines entrywise
    (no shape error).
  - `Add`/`Sub` of a `(1, 1)` constant and a `(3, 1)` variable broadcasts
    the constant's single entry across all 3 output entries.
  - `Add` of mismatched, non-broadcastable shapes (e.g. `(2, 1)` vs
    `(3, 1)`) returns `"shape mismatch: 2x1 vs 3x1"`.
  - `Mul` of a `(3, 1)` constant parameter by a `(3, 1)` variable scales
    each entry's one-hot coefficient correctly (affine, allowed).
  - `Mul` of two `(3, 1)` variables (both variable-dependent at every
    entry) returns `VECTOR_MUL_ERROR`.
  - `Div` of a `(3, 1)` variable by a `(1, 1)` constant scales every
    entry; division by a zero constant returns the existing
    division-by-zero error; division by a variable-dependent divisor
    returns `VECTOR_DIV_ERROR`.
  - `Neg`/`Scale` preserve shape and negate/scale every entry.
- `reduce`'s routing:
  - An all-`(1, 1)`-leaf expression containing `x * x` still produces a
    non-empty `quad` (i.e. is routed through `quadratize`, unchanged).
  - The same `x * x` nested inside `Add(x * x, w)` with `w` a `(3, 1)`
    variable now returns `VECTOR_MUL_ERROR` (routed through
    `linearize_shaped` once any leaf in the tree is non-scalar).
- `Expression::Sum` (`linearize_shaped`'s new arm, and `quadratize`'s
  defensive arm):
  - `Sum` of a `(3, 1)` variable produces a single entry whose `linear`
    has a `1.0` at each of that variable's 3 offsets (summing 3 one-hot
    vectors).
  - `Sum` of `Mul(weights_param, x_var)` (both `(3, 1)`) produces a single
    entry whose `linear[offset(x) + k] == weights_param.data[k]` for each
    `k` — the weighted-total case.
  - `Sum` of an already-`(1, 1)` expression is a no-op (identity).
  - `Sum(Mul(x, x))` for a scalar `x` (an all-`(1, 1)`-leaf tree containing
    `Sum`) still returns `VECTOR_MUL_ERROR`, confirming `is_all_scalar`'s
    new `Sum` arm correctly forces the affine-only path even with no
    non-scalar leaf present.
  - `is_all_scalar` returns `false` for any expression containing `Sum`
    anywhere, including nested inside `Add`/`Mul`/etc.
- End-to-end `solve` tests:
  - A feasibility-only problem (`CVX.MINIMIZE(0)`-equivalent) with a
    `(3, 1)` variable `w` constrained by `w >= [1, 2, 3]` and
    `w <= [1, 2, 3]` (two `(3, 1)`-parameter constraints) returns
    `Optimal` with `variable_values[w] == [1, 2, 3]` (within tolerance)
    and `objective_value == Some(0.0)`.
  - A mixed problem: minimize scalar `x` subject to `x >= 3`, together
    with an unrelated `(3, 1)` variable `w` tightly boxed to `[2, 2, 2]`
    by two vector constraints, confirms `x`'s recovered value (`3.0`) is
    correct independent of `w`'s presence, and `w`'s recovered values are
    `[2, 2, 2]`.
  - `2 .* w == [4, 6, 8]` (elementwise `CVX.MUL` of a `(3, 1)` constant
    parameter and a `(3, 1)` variable, compared via `Relation::Equal`
    against a `(3, 1)` parameter) recovers `w == [2, 3, 4]`.
  - A scalar quadratic constraint broadcast against a vector parameter
    (`x * x <= w` with `w` a `(3, 1)` parameter, e.g. `[4, 4, 4]`, and a
    linear objective on `x`) solves correctly, confirming the
    quadratic-term-broadcast case in "Constraint translation" needs no new
    code.
  - A budget-allocation-style LP: minimize `CVX.SUM(CVX.MUL(cost, x))` for
    a `(3, 1)` variable `x` and `(3, 1)` cost parameter, subject to
    `x >= [0, 0, 0]` and `CVX.SUM(x) >= 10`, converges to the expected
    least-cost allocation — the representative realistic use case this
    specification (combined with the `Sum` addition) unlocks.
  - A bare `(2, 1)`/`(3, 1)` variable used directly as the objective
    returns `"objective must evaluate to a single value (shape 1x1); got \
    shape 2x1"` / `"...got shape 3x1"`; `CVX.SUM` of the same variable as
    the objective succeeds instead.
  - A single `(201, 1)` variable (alone, `n_total = 201 > MAX_VARIABLES`)
    returns `SIZE_LIMIT_ERROR`.
  - Every existing SPEC-0010/SPEC-0011 all-scalar test (affine and
    quadratic, objectives and constraints) continues to pass unmodified,
    confirming no regression.

`cvxx` test update (not new coverage, a required fix):

- `src/excel/problem.rs::solve_reports_excel_error_and_does_not_store_a_result`
  is updated to expect the new objective-shape error message (see Error
  Handling).

`cvxx` unit tests (new coverage):

- `src/analytics/shape.rs`: `infer_shape(Sum(_))` returns `(1, 1)`;
  `render_expression` renders `Sum(e)` as `"sum(<e>)"`.
- `src/excel/expression.rs`: `CVX.SUM` resolves its operand identically to
  `CVX.NEG` (handle, name, or bare numeric literal) and stores an
  `Expression::Sum`.

## Dependencies

- SPEC-0010 (linear solver backend) and SPEC-0011 (quadratic objectives/
  constraints) — both implemented; this specification extends their
  translation layer in place and reuses `QuadraticForm`'s arithmetic
  (`add`/`sub`/`negate`/`scale`/`is_constant`/`is_affine`), `clarabel`
  settings, and size-limit/error-surfacing conventions unchanged.
- SPEC-0004 (expression builder) — `CVX.SUM` reuses its existing
  `run_unary`/`resolve_handle_arg` machinery in `src/excel/expression.rs`
  unchanged, the same way `CVX.NEG` does; SPEC-0004 explicitly deferred
  "general function calls inside expression strings" to a follow-up, which
  this specification does not revisit (`CVX.SUM` is functional-builder-only
  — see Non-Objective).
- SPEC-0007/SPEC-0013 (result inspection / short names) —
  `src/analytics/shape.rs` needs the one new `Sum` arm noted in Data Model;
  no other change.
- ISSUE-0002/ISSUE-0003 (parameter/variable shape creation) — already
  implemented; `CVX.PARAMETER`/`CVX.VARIABLE` already create non-`(1, 1)`
  handles today, so no change is needed there.
- `src/excel/problem.rs` (`CVX.PROBLEM`/`CVX.SOLVE`) and
  `src/excel/inspect.rs` (`CVX.VALUE`) — confirmed to already pass
  `Solution::variable_values: Vec<Vec<f64>>` through generically by shape
  (SPEC-0007); no changes needed beyond the one test message update above.
- Does not depend on, and does not implement, the narrowed ISSUE-0015
  (indexing/sub-range access) or ISSUE-0016 (quadratic vector/matrix
  support); both remain fully open follow-on specifications that this
  design does not block. The "combine entries into a total" portion
  formerly scoped to ISSUE-0015 is implemented here instead, folded into
  ISSUE-0014 (see that issue's updated Notes).
- No new external crate dependencies.

## Status

Implemented in `cvxrust/src/lib.rs`: `Expression` gained a `Sum(Box<Expression>)`
variant and `Expression::sum`; `validate_shapes` was removed and
`check_expr_shapes` repurposed as the boolean `is_all_scalar` predicate
(with a new `Sum` arm that always disqualifies); `QuadraticForm` gained
`#[derive(Clone)]`; a new `ShapedForm`/`broadcast_shape`/`entry_at`/
`linearize_shaped`/`reduce` set of private helpers implements the
affine, shape-broadcasting reduction and routing exactly as specified;
`quadratize` gained a defensive (never-reached) `Sum` arm for match
exhaustiveness; `solve` was updated to compute `offsets`/`n_total` from
variable shapes, check the reworded `SIZE_LIMIT_ERROR` against total scalar
variables and (incrementally, per-constraint) total scalar constraint rows,
route the objective and each constraint side through `reduce`, broadcast
constraint `lhs`/`rhs` into one row per output entry, and recover
`variable_values` by slicing the solver's solution vector per variable's
own shape. `src/analytics/shape.rs` (`infer_shape`/`render_expression`)
gained a `Sum` arm. `src/excel/expression.rs` gained `CVX.SUM` (reusing
`run_unary` unchanged); `src/excel/mod.rs` registers it. The one required
test update (`src/excel/problem.rs::solve_reports_excel_error_and_does_not_store_a_result`)
and the two `cvxrust` tests asserting the old blanket shape/size-limit
messages were updated to the new wording. `docs/problems.md` and
`docs/expressions.md` were updated per Error Handling/Documentation above.

Added 26 new `cvxrust` unit/end-to-end tests (61 total, up from 35) covering
`linearize_shaped`'s every arm, `is_all_scalar`/`reduce` routing (including
the documented nested-quadratic-in-a-vector-tree and `Sum`-forces-affine
cases), and end-to-end solves (boxed vector *and* matrix variables, a mixed
scalar/vector problem, an elementwise-`Mul` equality constraint, a scalar
quadratic constraint broadcast against a vector parameter, a
budget-allocation LP using `CVX.SUM`, the bare-vector-objective error vs.
`Sum`-wrapped success, and the reworded size-limit error) plus 1 new
`src/analytics/shape.rs` test for `Sum`'s shape/rendering. `cargo fmt` and
`cargo clippy --all-targets -- -D warnings` are clean; the full workspace
test suite (61 + 138 tests) passes with no regressions.
