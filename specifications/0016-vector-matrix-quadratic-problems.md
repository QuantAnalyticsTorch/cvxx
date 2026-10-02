---
id: SPEC-0016
title: Quadratic objectives and constraints over vector/matrix variables
issue: ISSUE-0016
status: implemented
created: 2026-10-02
---

## Objective

Extend `cvxrust`'s reduction layer (`cvxrust/src/reduce.rs`) so that a
convex quadratic (degree-2) term may involve vector/matrix-shaped
`Variable`/`Parameter` leaves, not only `(1, 1)`-shaped ones — the one
remaining restriction SPEC-0014 ("non-`(1, 1)`-shaped leaf forces the
affine-only path") and SPEC-0018 ("matrix multiplication of two
variable-dependent operands remains out of scope, deferred to ISSUE-0016")
both explicitly called out and deferred here. This is what makes the two
flagship quadratic use cases actually expressible and solvable:

- **Least squares**: `CVX.SUM(CVX.MUL(diff, diff))` where
  `diff = CVX.SUB(CVX.MATMUL(a, x), b)` is a vector of per-observation
  residuals (`a` a known `(m, n)` parameter matrix, `x` an `(n, 1)`
  unknown variable, `b` an `(m, 1)` parameter of observations) — the sum
  of squared residuals.
- **Portfolio risk**: `x.T @ sigma @ x` (via the `@`/`.T` grammar,
  SPEC-0018) or the equivalent `CVX.MATMUL(CVX.TRANSPOSE(x),
  CVX.MATMUL(sigma, x))` builder form, where `sigma` is a known `(n, n)`
  covariance parameter matrix and `x` is an `(n, 1)` unknown allocation
  variable — the portfolio variance.

No `cvxrust::Expression` variant, `cvxrust` public struct/enum, or
Excel-facing function signature changes. Every piece of surface area this
specification's examples use (`Expression::Mul`/`Sum`/`MatMul`/`Transpose`,
`CVX.MUL`/`CVX.SUM`/`CVX.MATMUL`/`CVX.TRANSPOSE`, the `*`/`@`/`.T` grammar)
already exists (SPEC-0004/SPEC-0014/SPEC-0018); this specification only
widens what combinations of them the solver accepts, by changing how
`Expression::Mul` and `Expression::MatMul` are reduced internally when both
operands are variable-dependent.

### Why this is a small, mechanical change

`QuadraticForm` (the degree-2 numeric form every (objective, or one
constraint side) expression reduces to) and the conic-program assembly
that consumes it (`cvxrust/src/solver.rs`: the `P`-matrix construction for
a quadratic objective, and the eigendecomposition/second-order-cone
construction for a quadratic constraint, `build_soc_block`) are already
fully general over `QuadraticForm`'s flat `(i, j, coeff)` triples — they
have no notion of which `Expression` shape or variant produced those
triples, and are not changed by this specification at all. The only
reason a vector/matrix-involving product does not already produce a
`QuadraticForm` with populated `quad` entries is that `reduce.rs`
currently *routes* any expression containing a non-`(1, 1)` leaf, `Sum`,
`Index`, or `MatMul` through `linearize_shaped`, whose `Expression::Mul`
and `matmul_entries` reject (rather than reduce) a product of two
variable-dependent operands, and routes every other expression through
the scalar-only `quadratize`, whose `Expression::Mul` arm already performs
exactly the degree-2 reduction needed (`outer_product_symmetrized`). This
specification extracts `quadratize`'s product-reduction rule into one
shared helper used by both `Expression::Mul` and `matmul_entries` inside
`linearize_shaped`, which — because a scalar is simply the `(1, 1)`
special case of a shaped reduction — makes the parallel scalar-only
`quadratize` path redundant, and it is deleted (see Data Model).

## Non-Objective

- Quadratic **equality** constraints. Still rejected with the existing
  `"quadratic equality constraints are not supported"` message
  (SPEC-0011), unchanged in wording or condition — only now also reachable
  for a vector/matrix-originated quadratic equality constraint, not only a
  scalar one.
- Non-convex quadratic terms. Still detected and rejected via the existing
  eigendecomposition/`PSD_TOLERANCE` check in `build_soc_block`
  (SPEC-0011), with the existing `"quadratic constraint is not convex
  (matrix is not positive semidefinite)"` message, unchanged — this
  specification changes nothing about how, or how strictly, convexity is
  checked; it only changes which expressions reach that check with a
  non-empty `quad`.
- Cubic or higher-degree terms (three or more variable-dependent factors
  multiplied together, via any mix of `Mul`/`MatMul`). Still rejected,
  with one message reused verbatim for every shape (see Error Handling).
- Vector/matrix-**shaped objectives**. The objective must still reduce to
  a single value (shape `(1, 1)`) — unchanged from SPEC-0014. A bare
  quadratic vector expression (e.g. `CVX.MUL(x, x)` for a vector `x`, with
  no enclosing `CVX.SUM`/reducing `CVX.MATMUL`) still cannot be the
  objective directly; `CVX.SUM` (or a `MatMul` that itself reduces to
  `(1, 1)`, e.g. `x.T @ x`) remains the only way to reduce a vector/matrix
  quadratic down to the single value an objective requires. This is
  unchanged machinery (SPEC-0014), exercised by, not modified by, this
  specification's examples.
- Any change to `MAX_VARIABLES`/`MAX_CONSTRAINTS` (still 200/200) or
  `MAX_ITERATIONS` (still 200). A quadratic constraint's extra
  second-order-cone rows continue to be excluded from the constraint-row
  count, exactly as today (SPEC-0011/SPEC-0014) — this specification adds
  no new way to produce those rows that wasn't already counted the same
  (imprecise) way.
- Any Excel-facing function signature, `CVX.EXPRESSION`/`CVX.CONSTRAINT`
  string-grammar token, or new `cvxrust::Expression` variant. Every
  expression this specification makes solvable is already buildable today
  (SPEC-0004/SPEC-0014/SPEC-0015/SPEC-0018); only `cvxrust`'s internal
  reduction of `Mul`/`MatMul` changes.
- Dual values, sensitivity analysis, warm starts, or any solver tuning
  beyond the existing fixed iteration bound (unchanged from
  SPEC-0010/SPEC-0011/SPEC-0014).
- General conic support (SDP, exponential, power cones). Only the
  second-order cone (`SecondOrderConeT`), already used by SPEC-0011's
  scalar quadratic constraints, is used — unchanged, just reached from
  more expressions.
- `Expression::Div`'s rule: division remains legal only by a provably
  constant divisor (`QuadraticForm::is_constant()`, i.e. `quad` empty and
  `linear` all-zero), regardless of shape. This was already true of both
  `quadratize` and `linearize_shaped`'s existing `Div` arms; it is
  unchanged behavior, only its error message text is consolidated (Error
  Handling).

## Interface

No new or changed Excel functions, and no new or changed public
`cvxrust` types (`Expression`, `Variable`, `Parameter`, `Constraint`,
`Problem`, `Solution`, `SolveStatus`, `Sense`, `Relation`,
`MAX_VARIABLES`, `MAX_CONSTRAINTS`, `MAX_ITERATIONS` are all unchanged).
The entire change is internal to `cvxrust/src/reduce.rs`'s private
reduction functions.

### New private helper: `multiply_forms`

```rust
/// Reduces the product of two `QuadraticForm`s (already positioned over
/// the same `n_total`-long scalar-variable space) to a `QuadraticForm`.
///
/// - If either side is a pure constant (`is_constant()`), the product is
///   the other side scaled by that constant (degree unchanged).
/// - Else, if both sides are affine (`is_affine()`, i.e. `quad` empty)
///   but neither is constant, the product is a new degree-2
///   `QuadraticForm`: `constant = l.constant * r.constant`,
///   `linear[i] = l.constant * r.linear[i] + r.constant * l.linear[i]`,
///   `quad = outer_product_symmetrized(&l.linear, &r.linear)`.
/// - Else (at least one side already carries a quadratic term and the
///   other is non-constant) the product would be degree >= 3: returns
///   `Err(DEGREE_ERROR)`.
fn multiply_forms(l: QuadraticForm, r: QuadraticForm) -> Result<QuadraticForm, String> {
    if l.is_constant() {
        return Ok(r.scale(l.constant));
    }
    if r.is_constant() {
        return Ok(l.scale(r.constant));
    }
    if l.is_affine() && r.is_affine() {
        let linear: Vec<f64> = l
            .linear
            .iter()
            .zip(r.linear.iter())
            .map(|(&a, &b)| l.constant * b + r.constant * a)
            .collect();
        return Ok(QuadraticForm {
            constant: l.constant * r.constant,
            linear,
            quad: outer_product_symmetrized(&l.linear, &r.linear),
        });
    }
    Err(DEGREE_ERROR.to_string())
}
```

This is `quadratize`'s existing `Expression::Mul` arm, unchanged in logic,
extracted to a function so `linearize_shaped`'s `Expression::Mul` arm and
`matmul_entries` can call it too (see below). `outer_product_symmetrized`
and `QuadraticForm::{is_constant, is_affine, scale}` are the existing,
unchanged helpers already used by `quadratize` today.

`DEGREE_ERROR` is the existing message, unchanged and reused verbatim
(previously inline in `quadratize`'s `Mul` arm, now a named constant):

```rust
const DEGREE_ERROR: &str = "solver only supports linear and quadratic \
(degree <= 2) objectives and constraints; a product of three or more \
variable-dependent terms was found";
```

### `linearize_shaped`'s `Expression::Mul` arm — now calls `multiply_forms`

Per broadcast output entry `k`, instead of today's:

```rust
entries.push(if l_entry.is_constant() {
    r_entry.clone().scale(l_entry.constant)
} else if r_entry.is_constant() {
    l_entry.clone().scale(r_entry.constant)
} else {
    return Err(VECTOR_MUL_ERROR.to_string());
});
```

the arm becomes:

```rust
entries.push(multiply_forms(l_entry.clone(), r_entry.clone())?);
```

`VECTOR_MUL_ERROR` is deleted (no remaining caller); every call site that
asserted it (see Test Approach) is updated to assert either a correct
quadratic result (for a genuine degree-2 product) or `DEGREE_ERROR` (for a
genuine degree-3+ product).

### `matmul_entries` — per-term accumulation now calls `multiply_forms`

Today's per-`t` term computation (inside the `i`/`j`/`t` triple loop):

```rust
let term = if l_entry.is_constant() {
    r_entry.clone().scale(l_entry.constant)
} else if r_entry.is_constant() {
    l_entry.clone().scale(r_entry.constant)
} else {
    return Err(VECTOR_MUL_ERROR.to_string());
};
acc = acc.add(&term);
```

becomes:

```rust
let term = multiply_forms(l_entry.clone(), r_entry.clone())?;
acc = acc.add(&term);
```

Each of the `k` dot-product terms summed into one matrix-multiplication
output entry is treated as an independent product and accumulated via the
existing `QuadraticForm::add` (whose `quad`-combining `combine_quad` is
already general and unchanged) — so e.g. `x.T @ sigma @ x`'s single
`(1, 1)` output entry correctly accumulates the full
`sum_i sum_j sigma[i][j] * x_i * x_j` quadratic form across every `(i, j)`
summation term, exactly the standard `x^T Q x` expansion.

### `Expression::Div` — message consolidation only, no rule change

`linearize_shaped`'s existing `Div` arm already requires the divisor
entry to be a provable constant (`!r_entry.is_constant()` is the error
condition) — the same rule `quadratize`'s `Div` arm already enforces. Only
the error message is consolidated: `VECTOR_DIV_ERROR` is deleted and
replaced by `quadratize`'s existing division-error text, reused verbatim:

```rust
const DIVISION_ERROR: &str = "solver only supports linear or quadratic \
objectives and constraints; division by a variable-dependent term was \
found";
```

### Eliminating the `is_all_scalar`/`quadratize` routing split

Because `multiply_forms` makes `linearize_shaped` (with `matmul_entries`)
exactly as capable, for every expression, as `quadratize` was for the
`(1, 1)`-only subset it handled — a scalar is just the `(1, 1)` special
case of the general shape-broadcasting machinery — the parallel
scalar-only path is no longer needed for correctness and is removed:

- `quadratize`, `is_all_scalar`, and `check_expr_shapes` are deleted.
- `reduce_expression` (the only function `cvxrust::solver::solve` calls
  into this module) simplifies from today's two-way branch to:

  ```rust
  pub(crate) fn reduce_expression(
      expr: &Expression,
      offsets: &HashMap<u64, usize>,
      n_total: usize,
  ) -> Result<ShapedForm, String> {
      linearize_shaped(expr, offsets, n_total)
  }
  ```

  (kept as a thin wrapper, rather than inlined at every call site, purely
  so `solver.rs`'s existing `use crate::reduce::{..., reduce_expression,
  ...}` import and call sites need no change.)
- The module-level doc comment (currently describing "two parallel
  reduction paths" per SPEC-0014) is updated to describe the single,
  unified reducer.

No change to `linearize_shaped`'s existing `Constant`/`Parameter`/
`Variable`/`Add`/`Sub`/`Neg`/`Scale`/`Sum`/`Index`/`Transpose` arms, or to
`transpose_entries`/`select_sub_block`/`broadcast_shape`/`matmul_shape`/
`entry_at`/`combine_quad`/`outer_product_symmetrized` — all already fully
general and reused unchanged.

## Data Model

- `QuadraticForm { constant, linear, quad }` and `ShapedForm { shape,
  entries }` (SPEC-0011/SPEC-0014) are unchanged. A vector/matrix-shaped
  expression's `ShapedForm` may now have one or more `entries` with a
  non-empty `quad` (previously impossible outside the deleted
  `quadratize` path) — this is the entire data-level change.
- `cvxrust::solver::solve` (`solver.rs`) is **unchanged**: its objective
  translation (`obj.is_affine()` branch into `p_matrix`), per-constraint
  translation (`diff.is_affine()` branch into an affine row vs.
  `build_soc_block`), and `build_soc_block` itself (eigendecomposition,
  `PSD_TOLERANCE` convexity check, second-order-cone row construction,
  SPEC-0011) already operate purely on a `QuadraticForm`'s flat `(i, j,
  coeff)` triples over the problem's full scalar-variable space, with no
  dependency on which `Expression` shape or variant produced them. A
  constraint whose two sides broadcast to a `(r, c)` shape and whose
  per-entry `diff` is quadratic already produces `r * c` independent rows,
  each its own affine row or `SecondOrderConeT` block — e.g. a `(3, 1)`
  quadratic vector constraint yields three independent second-order-cone
  blocks today for the all-scalar-leaf case (SPEC-0011/SPEC-0014's
  `scalar_quadratic_constraint_broadcasts_against_vector_parameter` test);
  this specification only changes which expressions can produce a
  non-empty `diff.quad` in the first place, not how `solve` consumes one.
- `check_expr_shapes`'s prior role — rejecting any `Variable`/`Parameter`
  leaf, `Sum`, `Index`, or `MatMul` as "not all-scalar" purely to route
  between two reducers — no longer exists; shape validity (mismatched
  broadcast shapes, out-of-bounds `Index`, incompatible `MatMul`
  dimensions) continues to be caught by `broadcast_shape`/
  `select_sub_block`/`matmul_shape`'s own existing, unchanged error
  returns, exactly as today for any non-quadratic shaped expression.

### Error-message consolidation (old → new/kept)

| Old (removed)                                             | New / kept                                   | Condition                                                                 |
|-------------------------------------------------------------|-----------------------------------------------|----------------------------------------------------------------------------|
| `VECTOR_MUL_ERROR` ("...once a vector or matrix...is involved; a product of two...") | `DEGREE_ERROR` ("...degree <= 2...a product of three or more...") | A product/matrix-product of two variable-dependent operands where at least one side is already degree >= 2 (previously: any such product involving a non-scalar leaf, unconditionally). |
| `VECTOR_DIV_ERROR`                                         | `DIVISION_ERROR` (quadratize's existing text) | Division by a non-constant (variable-dependent) term, any shape.          |
| *(quadratize's scalar-only degree error, same text)*       | `DEGREE_ERROR` (unchanged text)               | Unchanged: a scalar product of three or more variable-dependent terms.   |
| *(quadratize's scalar-only division error, same text)*     | `DIVISION_ERROR` (unchanged text)             | Unchanged: scalar division by a variable-dependent term.                 |

A product of exactly two variable-dependent operands that was previously
rejected unconditionally by `VECTOR_MUL_ERROR` solely because a non-`(1,
1)` leaf, `Sum`, `Index`, or `MatMul` appeared somewhere in the
expression — e.g. `Expression::mul(w, v)` for two `(3, 1)` variables `w`,
`v`, or the SPEC-0014 regression example `Add(Mul(x, x), w)` for a scalar
`x` and `(3, 1)` variable `w` — now succeeds with a correctly-populated
`quad`, instead of erroring. This is the acceptance-criteria-driving
behavioral change; every other error condition (shape mismatch, degree
>= 3, non-constant division, quadratic equality, non-convexity, size
limit) is unchanged in both trigger condition and message text.

## Error Handling

- **Degree >= 3** (cubic or higher): `DEGREE_ERROR`, unchanged text,
  reached uniformly regardless of whether the offending product arose
  from `Expression::Mul` or `Expression::MatMul`, and regardless of
  operand shape.
- **Division by a variable-dependent term**: `DIVISION_ERROR`, unchanged
  text, reached uniformly regardless of operand shape.
- **Quadratic equality constraint**: unchanged existing message and
  condition (SPEC-0011), now also reachable when the quadratic term
  originates from a vector/matrix-shaped product.
- **Non-convex quadratic constraint**: unchanged existing message and
  condition (`PSD_TOLERANCE` eigenvalue check in `build_soc_block`,
  SPEC-0011), now also reachable for a constraint whose quadratic term
  originates from a vector/matrix-shaped product (e.g. an
  indefinite-matrix portfolio-style constraint is rejected the same way a
  scalar one with a negative coefficient is today).
- **Non-`(1, 1)` objective**: unchanged existing message and condition
  (SPEC-0014) — `"objective must evaluate to a single value (shape 1x1); \
  got shape {r}x{c}"` — reached when a user builds a quadratic vector/
  matrix expression but forgets to reduce it to one value (e.g. omits
  `CVX.SUM` around a sum-of-squares, or omits `.T`/`CVX.TRANSPOSE` around
  one side of a would-be dot product).
- **Shape mismatch / out-of-bounds index / incompatible `MatMul`
  dimensions**: unchanged existing messages and conditions
  (`broadcast_shape`/`select_sub_block`/`matmul_shape`, SPEC-0014/
  SPEC-0015/SPEC-0018).
- **Size limit**: unchanged existing message and condition
  (`n_total > MAX_VARIABLES` / accumulated constraint-row count >
  `MAX_CONSTRAINTS`, SPEC-0014).
- **Infeasible / unbounded**: unchanged — `solve`'s existing
  `ClarabelStatus` → `SolveStatus` mapping (`PrimalInfeasible`/
  `AlmostPrimalInfeasible` → `Infeasible`, `DualInfeasible`/
  `AlmostDualInfeasible` → `Unbounded`) is untouched by this
  specification and applies identically to a vector/matrix-originated
  quadratic problem, satisfying ISSUE-0016's "reported with the same
  honest, accurate status as single-number problems today" criterion with
  no new code.

## Test Approach

### Unit tests (`cvxrust/src/reduce_tests.rs`)

- Update tests whose asserted outcome changes from an error to a
  quadratic success, verifying the resulting `QuadraticForm`'s `quad`
  entries are numerically correct (not just `Ok(..)`):
  - `linearize_shaped_mul_of_two_variable_vectors_is_an_error` → becomes
    e.g. `linearize_shaped_mul_of_two_variable_vectors_is_quadratic`,
    asserting entry `k`'s `quad == vec![(offset_w + k, offset_v + k,
    1.0)]` (elementwise `w_k * v_k`, no cross terms between different
    `k`).
  - `scalar_quadratic_nested_in_a_vector_tree_is_rejected` → becomes e.g.
    `scalar_quadratic_nested_in_a_vector_tree_is_accepted`, asserting
    every broadcast entry `k` is `x*x`'s quadratic form (`quad ==
    vec![(offset_x, offset_x, 1.0)]`) plus `w`'s one-hot linear entry
    `k`, since `x` broadcasts identically across all three entries.
  - The analogous `matmul`-path tests at (today's) lines ~394, 444, 558,
    574, 710 asserting `VECTOR_MUL_ERROR` for a two-variable-dependent
    `MatMul`/`@` product — each updated to assert the correct resulting
    `quad` entries instead (e.g. a `(2, 1)`-by-`(1, 2)` outer product of
    two variable vectors, and a `x.T @ x` self dot product).
- Remove tests exercising deleted functions directly
  (`is_all_scalar_true_for_all_1x1_leaves_even_with_quadratic_term`,
  `is_all_scalar_false_once_any_leaf_is_non_scalar`,
  `is_all_scalar_false_for_any_expression_containing_sum`, and any
  `quadratize`-specific unit test) — their coverage is subsumed by
  `reduce_expression`/`linearize_shaped` tests asserting the same inputs
  now produce identical numeric results via the unified reducer.
- Add a new degree-3+ regression test using a vector/matrix-shaped
  operand (e.g. `Mul(Mul(w, w), w)` for a vector `w`), asserting
  `DEGREE_ERROR` — confirming the degree limit still applies once routed
  through the now-unified reducer, not only for scalar leaves.
- Add a `DIVISION_ERROR`-text regression test (renamed from the deleted
  `VECTOR_DIV_ERROR` assertion) confirming the message text matches the
  previously-scalar-only `quadratize` division error exactly.

### Integration tests (`cvxrust/src/solver_tests.rs`), end-to-end via `solve`

- **Least squares**: a small fixed `(m, n)` design matrix `a`, observation
  vector `b`, unknown `x`; objective
  `Expression::sum(Expression::mul(diff.clone(), diff))` with `diff =
  Expression::sub(Expression::matmul(a, x), b)`; assert `SolveStatus::
  Optimal` and recovered `x` matches the closed-form least-squares
  solution (e.g. a 2-observation, 1-unknown or 3x2 toy problem with a
  hand-computed optimum) to solver tolerance.
- **Portfolio risk**: a small fixed PSD `(n, n)` covariance parameter
  `sigma`, unknown allocation `x`; objective
  `Expression::matmul(Expression::transpose(x), Expression::matmul(sigma,
  x))` (equivalently exercised via the `@`/`.T` string grammar in an
  `cvxx`-level integration test, see below), plus an affine
  budget/allocation constraint (e.g. `CVX.SUM(x) == 1`); assert
  `SolveStatus::Optimal` and the recovered allocation matches a
  hand-computed (e.g. closed-form minimum-variance) optimum.
- **Quadratic vector/matrix constraint**: a `<=` constraint whose
  quadratic side originates from a vector/matrix product (e.g. `x.T @ x
  <= 1`, a unit-ball constraint on a vector variable, combined with a
  linear objective pushing against it), asserting the recovered `x` lies
  on the expected boundary.
- **Non-convex rejection**: a `MatMul`/`@`-originated quadratic
  constraint built from a parameter matrix with a deliberately negative
  eigenvalue, asserting `SolveStatus::Error` with the existing
  non-convexity message (confirming the unchanged `build_soc_block` check
  is reached from this new code path).
- **Infeasible/unbounded**: one infeasible and one unbounded
  vector/matrix-originated quadratic problem (mirroring the existing
  scalar-quadratic infeasible/unbounded tests, SPEC-0011), asserting
  `SolveStatus::Infeasible` / `SolveStatus::Unbounded` respectively, with
  `objective_value: None` and empty `variable_values` as today.
- Existing scalar-only quadratic tests (SPEC-0011, e.g.
  `scalar_quadratic_constraint_broadcasts_against_vector_parameter`) are
  re-run unchanged and must continue to pass bit-for-bit (within existing
  tolerances), confirming "existing single-number quadratic problem
  behavior is unaffected" (ISSUE-0016 acceptance criterion).

### Manual validation (`cvxx` Excel layer)

- Build and solve, from an Excel worksheet using only already-shipped
  functions/grammar (`CVX.VARIABLE`, `CVX.PARAMETER`, `CVX.MATMUL`/`@`,
  `CVX.TRANSPOSE`/`.T`, `CVX.SUM`, `CVX.MUL`, `CVX.CONSTRAINT`,
  `CVX.PROBLEM`, `CVX.SOLVE`, `CVX.VALUE`, `CVX.STATUS`,
  `CVX.OBJECTIVE_VALUE`), one least-squares workbook and one
  portfolio-risk workbook, confirming correct optimal values and
  `CVX.STATUS` reporting end-to-end through the XLL — no new ribbon or
  Excel-facing surface is introduced, so this is validation of existing
  surface against newly-solvable problem shapes, not new UI testing.

## Dependencies

- SPEC-0010 (convex solver backend) and SPEC-0011 (scalar quadratic
  support) — the `QuadraticForm`/`build_soc_block`/eigendecomposition
  machinery this specification reuses unchanged, and the exact
  degree-limit/division-error message text this specification
  consolidates onto.
- SPEC-0014 (vector/matrix affine solving) — `ShapedForm`, broadcasting,
  `Sum`, and the routing split this specification removes.
- SPEC-0015 (vector/matrix indexing) — `Index`/`select_sub_block`, reused
  unchanged; a quadratic term may still involve an `Index`-selected
  sub-block of a larger variable/parameter with no additional work, since
  `select_sub_block` already operates generically on `QuadraticForm`
  entries.
- SPEC-0018 (matrix multiplication and transpose) — `MatMul`/`Transpose`,
  `matmul_entries`, and the `@`/`.T` grammar; SPEC-0018 explicitly
  deferred "matrix multiplication of two variable-dependent operands" to
  ISSUE-0016, which this specification resolves.
- ISSUE-0016 — the business issue this specification implements.

## Status

Implemented in `cvxrust/src/reduce.rs` exactly as designed above:
`multiply_forms`/`DEGREE_ERROR`/`DIVISION_ERROR` added; `quadratize`,
`is_all_scalar`, `check_expr_shapes`, `VECTOR_MUL_ERROR`, and
`VECTOR_DIV_ERROR` deleted; `linearize_shaped`'s `Mul`/`Div` arms and
`matmul_entries` updated to call `multiply_forms`; `reduce_expression`
simplified to a thin wrapper around `linearize_shaped`. No changes were
needed in `cvxrust/src/solver.rs` or anywhere in the `cvxx` Excel crate,
confirming the specification's prediction that the conic-program assembly
was already fully general over `QuadraticForm`.

Tests: `cvxrust/src/reduce_tests.rs` updated per the Test Approach above
(renamed/rewritten assertions for now-quadratic-instead-of-rejected
expressions, removed `is_all_scalar`-specific tests, added
`sum_of_squared_differences_is_quadratic`,
`matmul_transpose_self_dot_product_is_quadratic`, and
`degree_three_vector_product_is_still_rejected`).
`cvxrust/src/solver_tests.rs` updated: the former
`matmul_chained_so_the_variable_appears_on_both_final_sides_is_rejected`
(a SPEC-0018 regression test explicitly deferring this case to
ISSUE-0016) now asserts the correct portfolio-risk optimum instead of an
error; added `sum_of_squares_least_squares_problem_solves_the_expected_
optimum`, `quadratic_vector_constraint_from_a_matmul_self_dot_product_
restricts_a_unit_ball`, and
`indefinite_matmul_quadratic_constraint_is_rejected_as_non_convex`. Full
workspace test suite passes: 93 tests in `cvxrust`, 209 in `cvxx`, 0
failures. `cargo fmt` and `cargo clippy --all-targets` are clean.
