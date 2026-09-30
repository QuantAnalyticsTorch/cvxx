---
id: SPEC-0011
title: Quadratic objectives and quadratic constraints for CVX.SOLVE
issue: ISSUE-0011
status: implemented
created: 2026-09-30
---

## Objective

Extend `cvxrust::solve` (SPEC-0010) so that, in addition to affine
objectives and affine `<=`/`>=`/`=` constraints, it also solves problems
whose objective and/or `<=`/`>=` constraints contain **convex quadratic**
terms — i.e. any expression built with `Expression::Mul` where both
operands are themselves variable-dependent (a case SPEC-0010 rejects with
"a product of two variable-dependent terms was found"). This is additive to
the existing linear translation layer inside `cvxrust`, not a new solver
integration: `clarabel` already accepts a quadratic objective matrix (`P`)
and already supports the second-order cone needed to represent convex
quadratic constraints. No Excel-facing function signature changes; the
existing `CVX.SOLVE` surface (SPEC-0006) is unchanged, only the class of
problems it can solve grows.

## Non-Objective

- Quadratic **equality** constraints (`x^T Q x + q^T x + c == 0` with
  `Q != 0`). These are rejected with a descriptive error; only `<=`/`>=`
  quadratic constraints are supported, since an equality between two convex
  quadratics is generally non-convex.
- Non-convex quadratic terms. A quadratic objective/constraint whose
  quadratic-coefficient matrix is not positive semidefinite (for a `<=`-
  oriented constraint or a `Minimize` objective; negative semidefinite for
  the `>=`/`Maximize` orientation) is detected and rejected with a
  descriptive error, not solved approximately.
- Cubic or higher-degree terms (three or more variable-dependent factors
  multiplied together). Still rejected, with an updated error message (see
  Error Handling).
- Non-scalar (shape other than `(1, 1)`) variables/parameters. Still out of
  scope, per SPEC-0010's existing shape restriction, unchanged here.
- Dual values, sensitivity analysis, and solver tuning beyond the existing
  fixed iteration bound (unchanged from SPEC-0010).
- Any change to `MAX_VARIABLES` / `MAX_CONSTRAINTS` (still 200/200) or to
  the `CVX.SOLVE`/`CVX.VALUE`/`CVX.STATUS`/`CVX.OBJECTIVE_VALUE` Excel
  interface.
- General conic support (SDP, exponential, power cones). Only the
  second-order cone (`SecondOrderConeT`), already a `clarabel` primitive, is
  used, purely as the mechanism for representing convex quadratic
  constraints.

## Interface

No new or changed Excel functions. The change is entirely internal to
`cvxrust`'s problem-translation layer (`cvxrust/src/lib.rs`).

### New/changed internal types

```rust
/// Replaces `AffineForm` as the result of reducing an `Expression`. A
/// quadratic form `constant + linear . x + sum_{i<=j} quad[(i,j)] * x_i * x_j`
/// over the problem's variable list. `quad` is empty for a purely affine
/// expression (the common case), so existing affine-only problems take the
/// same code path as before with no behavioral change.
struct QuadraticForm {
    constant: f64,
    linear: Vec<f64>,
    /// Sparse upper-triangular entries `(i, j, coeff)` with `i <= j`;
    /// `i == j` is the coefficient of `x_i^2`, `i < j` is the *total*
    /// coefficient of the `x_i * x_j` cross term (not halved).
    quad: Vec<(usize, usize, f64)>,
}
```

- `quadratize(expr, index, n) -> Result<QuadraticForm, String>` replaces
  `linearize` as the Step 2 reduction function, with the same error strings
  reused where still applicable (unknown variable, division-by-zero,
  division by a variable-dependent term).
- `QuadraticForm` supports `zero`, `negate`, `scale`, `add`, `sub` (same
  shape as `AffineForm`'s methods, extended to also combine the `quad`
  lists: `add`/`sub` concatenate/negate-and-concatenate entries for equal
  `(i, j)` keys, summed via a temporary `HashMap<(usize, usize), f64>` and
  re-flattened, dropping any resulting zero coefficients to keep the list
  sparse).
- `QuadraticForm::is_affine(&self) -> bool` — `true` when `quad.is_empty()`
  after this canonicalizing sum (used to route affine-only problems through
  the existing linear path unchanged).

### `quadratize` — `Expression::Mul` handling (the only behavioral change
from `linearize`)

```
quadratize(Mul(l, r)):
    lf = quadratize(l)
    rf = quadratize(r)
    if lf.is_affine() && lf.linear.is_all_zero():   # lf is a pure constant
        return rf.scale(lf.constant)
    if rf.is_affine() && rf.linear.is_all_zero():   # rf is a pure constant
        return lf.scale(rf.constant)
    if lf.quad.is_empty() && rf.quad.is_empty():
        # both sides are affine-but-not-constant: product is a new quadratic
        return QuadraticForm {
            constant: lf.constant * rf.constant,
            linear: lf.constant * rf.linear + rf.constant * lf.linear,  # elementwise
            quad: outer_product_symmetrized(lf.linear, rf.linear),
        }
    # at least one side already has a quadratic term and the other is not
    # a constant: this is a degree >= 3 product.
    return Err("solver only supports linear and quadratic (degree <= 2) \
                objectives and constraints; a product of three or more \
                variable-dependent terms was found")
```

`outer_product_symmetrized(a, b)` builds the sparse `(i, j, coeff)` list
for `sum_i sum_j a[i] * b[j] * x_i * x_j`, canonicalized to `i <= j` (folding
`(j, i)` into `(i, j)` by adding `a[i]*b[j] + a[j]*b[i]` when `i != j`, and
`a[i]*b[i]` directly when `i == j`), dropping zero-coefficient entries.

`Expression::Div` is unchanged in spirit: still only legal when the
divisor's `quadratize` result is a non-zero constant (`quad` empty and
`linear` all-zero), in which case the dividend's full `QuadraticForm`
(including any `quad` entries) is scaled by `1.0 / divisor.constant`;
division by a variable-dependent denominator (affine or quadratic) remains
an error, with the existing message unchanged.

### Objective translation (`solve`, Step 3)

- After `quadratize(problem.objective, ...)`, if `obj.quad.is_empty()`,
  proceed exactly as SPEC-0010 (`p_matrix = CscMatrix::zeros((n, n))`).
- Otherwise build the symmetric `P` matrix `clarabel` expects for
  `(1/2) x^T P x + q^T x`: for each `(i, j, coeff)` in `obj.quad`, set
  `P[i][i] += 2 * coeff` when `i == j`, else set both `P[i][j] += coeff` and
  `P[j][i] += coeff`. Assemble as a `CscMatrix` via the same dense-to-CSC
  helper pattern as `dense_rows_to_csc` (a new `dense_symmetric_to_csc`
  building a full `n x n` dense buffer from the sparse triples, then
  converting), reusing `q = obj.linear` (negated when `Sense::Maximize`,
  same as today) and `obj.constant` added back into the reported objective
  value exactly as SPEC-0010 already does for the affine part.
- When `Sense::Maximize`, both `P` and `q` are negated (matching `clarabel`
  minimizing `(1/2) x^T P x + q^T x`; negating `P` for a maximize sense is
  required in addition to the existing `q` negation, since the quadratic
  term's sign also flips under `max f(x) = -min(-f(x))`).

### Constraint translation (`solve`, Step 4) — quadratic branch

For each constraint, `lhs`/`rhs` are now reduced with `quadratize` instead
of `linearize`. Let `diff = lhs.sub(&rhs)` (a single `QuadraticForm`,
combining both sides' `quad`/`linear`/`constant` via the extended
`sub`/`add` above).

- If `diff.quad.is_empty()`: unchanged from SPEC-0010 — build the affine
  row exactly as today, from `diff.linear` and `-diff.constant`.
- If `diff.quad` is non-empty and `relation == Relation::Equal`: return
  `Err("quadratic equality constraints are not supported")`.
- If `diff.quad` is non-empty and `relation == Relation::GreaterEqual`:
  negate the whole `QuadraticForm` (`quad`, `linear`, `constant` all
  negated) and treat it as `LessEqual`, per the reduction below.
- If `diff.quad` is non-empty and `relation == Relation::LessEqual` (after
  the `GreaterEqual` negation above, both cases reach this same reduction):
  the constraint is `x^T Q x + q^T x + c <= 0` where `Q` is the dense
  symmetrization of `diff.quad` (same `P[i][i] = coeff`,
  `P[i][j] = P[j][i] = coeff / 2` construction as an ordinary quadratic
  form matrix — note the `/2` here, unlike the objective's `P`, since this
  `Q` is used directly as `x^T Q x`, not `(1/2) x^T Q x`), `q = diff.linear`,
  `c = diff.constant`:
  1. **Symmetric eigendecomposition** of `Q` (add `nalgebra`'s
     `SymmetricEigen` to `cvxrust/Cargo.toml`, restricted to the `n x n`
     dense `Q`, `n <= MAX_VARIABLES = 200`, so this is a bounded, one-off
     cost per quadratic constraint). Let `(lambda_k, v_k)` be the resulting
     eigenpairs.
  2. **Convexity check**: if any `lambda_k < -PSD_TOLERANCE` (a new
     constant `PSD_TOLERANCE: f64 = 1e-8`), return
     `Err("quadratic constraint is not convex (matrix is not positive \
          semidefinite)")`.
  3. **Factor rows**: for each `lambda_k > PSD_TOLERANCE`, emit one row
     `z_k = sqrt(lambda_k) * v_k` (a length-`n` vector over the problem's
     full variable list). Let `r` be the number of such rows. If `r == 0`
     (the quadratic part cancels out to within tolerance), fall back to the
     affine-only row construction using just `q`/`c`, exactly as the
     `diff.quad.is_empty()` case — this can legitimately happen when
     `Q`'s entries sum to (numerically) zero despite `diff.quad` being
     non-empty before combination.
  4. **Second-order cone rows**: with `t(x) = q . x + c` (affine), define
     two additional affine rows `p(x) = (1 - t(x)) / 2` and
     `w(x) = (-t(x) - 1) / 2`. Append, as one contiguous block to the
     constraint-row matrix `A`/`b` (after the existing `ZeroConeT`/
     `NonnegativeConeT` rows, same append-only pattern as today), the
     `r + 2` rows `[p(x); w(x); z_1(x); ...; z_r(x)]` in that order, and
     push `SecondOrderConeT(r + 2)` onto the `cones` vector (after the
     existing `ZeroConeT`/`NonnegativeConeT` entries, consistent with
     `clarabel` requiring the cones list to describe the row blocks of `A`
     in order). This encodes `x^T Q x + q(x) <= 0` exactly, via the
     standard identity `p >= sqrt(w^2 + ||z||_2^2) <=> z.z <= p^2 - w^2 =
     (p-w)(p+w) = (1 - t(x)) <=> t(x) <= -z.z`, i.e. `t(x) + z.z <= 0`,
     which is `Q`'s quadratic form plus `q(x) <= 0`.
- The size limits `MAX_VARIABLES`/`MAX_CONSTRAINTS` (SPEC-0010, unchanged)
  continue to bound `n`; no new size constant is introduced for the number
  of quadratic constraints (each contributes one `SecondOrderConeT` block,
  bounded the same way ordinary constraint rows already are).

## Data Model

- `Problem`, `Variable`, `Expression`, `Constraint`, `Sense`, `Relation`,
  `Solution`, `SolveStatus` (SPEC-0010) are all unchanged.
- `AffineForm` is removed and replaced by `QuadraticForm` (above); every
  existing affine-only call site continues to work because `QuadraticForm`
  with an empty `quad` list behaves identically to the old `AffineForm` for
  all arithmetic and for row/objective construction.
- No new public `cvxrust` API — `solve(&Problem) -> Solution` is unchanged;
  only its internal implementation grows to recognize a wider class of
  `Expression` trees.

## Error Handling

All errors continue to surface as `SolveStatus::Error(String)` via the
existing `error_solution` helper (unchanged), which `src/excel/problem.rs`
already translates to `#VALUE!` (SPEC-0006) — no new Excel-facing error
path is introduced.

New/changed error strings, replacing or added alongside SPEC-0010's:

- Degree >= 3 product (either factor already quadratic, and the other not
  a constant): `"solver only supports linear and quadratic (degree <= 2) \
  objectives and constraints; a product of three or more variable-dependent \
  terms was found"` (replaces SPEC-0010's degree >= 2 message, since degree
  2 is now supported).
- Division by a variable-dependent denominator: unchanged message from
  SPEC-0010 (`"solver only supports linear (affine) objectives and \
  constraints; division by a variable-dependent term was found"` — updated
  to say `"linear or quadratic"` instead of `"linear (affine)"`, for
  accuracy, since a quadratic dividend is now legal as long as the divisor
  is constant).
- Quadratic equality constraint: `"quadratic equality constraints are not \
  supported"`.
- Non-convex quadratic term: `"quadratic constraint is not convex (matrix \
  is not positive semidefinite)"`.
- Existing shape, division-by-zero, size-limit, unknown-variable, and
  solver-status-passthrough errors from SPEC-0010 are unchanged.

## Test Approach

`cvxrust` unit tests (extending the existing `mod tests` in
`cvxrust/src/lib.rs`), covering:

- `quadratize` directly:
  - A pure product of two distinct variables (`x * y`) yields
    `quad = [(i, j, 1.0)]` with `i < j`, `linear` all zero.
  - A self-product (`x * x`) yields `quad = [(i, i, 1.0)]`.
  - A constant times an affine-with-quad term scales `constant`, `linear`,
    and every `quad` coefficient.
  - A product where one side already has a `quad` term and the other is
    non-constant returns the degree->=3 error.
  - Division by a non-zero constant scales `quad` correctly; division by a
    variable-dependent term still errors.
- End-to-end `solve` tests:
  - Minimize a simple positive-definite quadratic (e.g.
    `minimize x^2 + y^2` subject to `x + y >= 1`) converges to the known
    analytic optimum (`x = y = 0.5`, objective `0.5`).
  - Maximize a concave quadratic (negative semidefinite) with a linear
    constraint, verifying the `Sense::Maximize` sign handling for `P`.
  - A quadratic `<=` constraint (e.g. `x^2 + y^2 <= 1` with a linear
    objective) converges to a solution on/inside the unit disk.
  - A quadratic `>=` constraint is accepted and correctly negated before
    the SOC reduction (e.g. `-(x^2 + y^2) >= -1`, equivalent to the above).
  - A quadratic equality constraint (`x^2 == 1`) returns the new
    unsupported-equality error.
  - An indefinite quadratic constraint (e.g. `x*y <= 1`, whose `Q` has
    eigenvalues `+0.5`/`-0.5`) returns the non-convex error.
  - A cubic term (`x * x * y`) returns the degree->=3 error.
  - A pre-existing SPEC-0010 affine-only test (e.g. a small LP) still
    produces the same result, confirming no regression for problems with an
    empty `quad` list throughout.

## Dependencies

- SPEC-0010 (linear solver backend) — this specification extends its
  translation layer and reuses its size limits, error-surfacing pattern,
  and `clarabel` settings (`MAX_ITERATIONS`, etc.) unchanged.
- New external crate dependency: `nalgebra` (or an equivalent small linear
  algebra crate providing a dense symmetric eigendecomposition), added to
  `cvxrust/Cargo.toml`. `clarabel` itself is unchanged — this specification
  uses only cone types (`SecondOrderConeT`) and the `P` matrix that
  `clarabel` 0.9 already exposes, per `docs/architecture.md`'s standing
  decision to extend the existing `clarabel` translation layer rather than
  add another solver crate.
- ISSUE-0011 (parent issue).

## Status

Implemented in `cvxrust/src/lib.rs`: `AffineForm`/`linearize` were replaced
by `QuadraticForm`/`quadratize`, `solve` now builds a quadratic `P` matrix
for the objective and reduces convex `<=`/`>=` quadratic constraints to
`SecondOrderConeT` blocks via `nalgebra::SymmetricEigen`, and `nalgebra` was
added to `cvxrust/Cargo.toml`. All error messages, the equality-constraint
rejection, and the non-convexity rejection are implemented as specified.
Extended the unit test suite (35 `cvxrust` tests, all passing) with
quadratization tests and end-to-end quadratic-objective/constraint solve
tests, including the degree->=3 and non-convex error cases; `cargo fmt` and
`cargo clippy` are clean.
