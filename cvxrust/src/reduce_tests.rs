use super::*;
use crate::model::Variable;

fn var(id: u64) -> Variable {
    Variable::new(id, (1, 1))
}

fn scalar_var(id: u64) -> Variable {
    Variable::new(id, (1, 1))
}

fn index_of(vars: &[Variable]) -> HashMap<u64, usize> {
    vars.iter().enumerate().map(|(i, v)| (v.id, i)).collect()
}

fn offsets_of(vars: &[Variable]) -> (HashMap<u64, usize>, usize) {
    let mut offsets = HashMap::with_capacity(vars.len());
    let mut n_total = 0usize;
    for v in vars {
        offsets.insert(v.id, n_total);
        n_total += v.shape.0 * v.shape.1;
    }
    (offsets, n_total)
}

/// Deterministic ordering for a sparse `quad` list, since `combine_quad`'s
/// `HashMap`-based accumulation does not guarantee entry order.
fn sorted_quad(mut quad: Vec<(usize, usize, f64)>) -> Vec<(usize, usize, f64)> {
    quad.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    quad
}

/// Test-only wrapper preserving the pre-SPEC-0016 `quadratize(expr,
/// offsets, n_total) -> Result<QuadraticForm, String>` signature used
/// throughout this file's scalar-only test section, now backed by the
/// unified `linearize_shaped` (SPEC-0016). Every expression under test
/// here is scalar (`(1, 1)`), so this always has exactly one entry.
fn quadratize(
    expr: &Expression,
    offsets: &HashMap<u64, usize>,
    n_total: usize,
) -> Result<QuadraticForm, String> {
    let form = linearize_shaped(expr, offsets, n_total)?;
    Ok(form.entries.into_iter().next().unwrap())
}

// --- quadratization tests ---

#[test]
fn quadratizes_a_constant() {
    let form = quadratize(&Expression::constant(2.5), &HashMap::new(), 0).unwrap();
    assert_eq!(form.constant, 2.5);
    assert!(form.linear.is_empty());
    assert!(form.quad.is_empty());
}

#[test]
fn quadratizes_a_single_variable() {
    let vars = vec![var(1), var(2)];
    let index = index_of(&vars);
    let form = quadratize(&Expression::from_variable(vars[1]), &index, vars.len()).unwrap();
    assert_eq!(form.constant, 0.0);
    assert_eq!(form.linear, vec![0.0, 1.0]);
    assert!(form.quad.is_empty());
}

#[test]
fn quadratizes_a_parameter() {
    let form = quadratize(
        &Expression::from_parameter(1, (1, 1), vec![7.0]),
        &HashMap::new(),
        0,
    )
    .unwrap();
    assert_eq!(form.constant, 7.0);
}

#[test]
fn quadratizes_add_and_sub() {
    let vars = vec![var(1)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);
    let add = quadratize(
        &Expression::add(x.clone(), Expression::constant(3.0)),
        &index,
        1,
    )
    .unwrap();
    assert_eq!(add.constant, 3.0);
    assert_eq!(add.linear, vec![1.0]);

    let sub = quadratize(&Expression::sub(Expression::constant(3.0), x), &index, 1).unwrap();
    assert_eq!(sub.constant, 3.0);
    assert_eq!(sub.linear, vec![-1.0]);
}

#[test]
fn quadratizes_neg_and_scale() {
    let vars = vec![var(1)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);

    let neg = quadratize(&Expression::neg(x.clone()), &index, 1).unwrap();
    assert_eq!(neg.linear, vec![-1.0]);

    let scaled = quadratize(&Expression::scale(4.0, x), &index, 1).unwrap();
    assert_eq!(scaled.linear, vec![4.0]);
}

#[test]
fn quadratizes_mul_by_constant_either_side() {
    let vars = vec![var(1)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);

    let left = quadratize(
        &Expression::mul(Expression::constant(2.0), x.clone()),
        &index,
        1,
    )
    .unwrap();
    assert_eq!(left.linear, vec![2.0]);

    let right = quadratize(&Expression::mul(x, Expression::constant(3.0)), &index, 1).unwrap();
    assert_eq!(right.linear, vec![3.0]);
}

#[test]
fn quadratizes_div_by_constant() {
    let vars = vec![var(1)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);

    let form = quadratize(&Expression::div(x, Expression::constant(2.0)), &index, 1).unwrap();
    assert_eq!(form.linear, vec![0.5]);
}

#[test]
fn nested_combination_quadratizes_correctly() {
    let vars = vec![var(1), var(2)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);
    let y = Expression::from_variable(vars[1]);

    // 2 * (x - y) + 3
    let expr = Expression::add(
        Expression::scale(2.0, Expression::sub(x, y)),
        Expression::constant(3.0),
    );
    let form = quadratize(&expr, &index, 2).unwrap();
    assert_eq!(form.constant, 3.0);
    assert_eq!(form.linear, vec![2.0, -2.0]);
    assert!(form.quad.is_empty());
}

#[test]
fn mul_of_two_distinct_variables_is_quadratic() {
    let vars = vec![var(1), var(2)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);
    let y = Expression::from_variable(vars[1]);

    let form = quadratize(&Expression::mul(x, y), &index, 2).unwrap();
    assert_eq!(form.constant, 0.0);
    assert_eq!(form.linear, vec![0.0, 0.0]);
    assert_eq!(form.quad, vec![(0, 1, 1.0)]);
}

#[test]
fn mul_of_a_variable_with_itself_is_quadratic() {
    let vars = vec![var(1)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);

    let form = quadratize(&Expression::mul(x.clone(), x), &index, 1).unwrap();
    assert_eq!(form.quad, vec![(0, 0, 1.0)]);
}

#[test]
fn mul_scales_an_existing_quadratic_term() {
    let vars = vec![var(1), var(2)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);
    let y = Expression::from_variable(vars[1]);
    let xy = Expression::mul(x.clone(), y);

    let form = quadratize(&Expression::mul(Expression::constant(3.0), xy), &index, 2).unwrap();
    assert_eq!(form.quad, vec![(0, 1, 3.0)]);
}

#[test]
fn mul_of_three_variable_dependent_terms_is_a_degree_error() {
    let vars = vec![var(1), var(2)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);
    let y = Expression::from_variable(vars[1]);
    let xy = Expression::mul(x.clone(), y);

    let err = quadratize(&Expression::mul(xy, x), &index, 2).unwrap_err();
    assert_eq!(
            err,
            "solver only supports linear and quadratic (degree <= 2) objectives and constraints; a product of three or more variable-dependent terms was found"
        );
}

#[test]
fn div_by_variable_is_nonlinear_error() {
    let vars = vec![var(1), var(2)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);
    let y = Expression::from_variable(vars[1]);

    let err = quadratize(&Expression::div(x, y), &index, 2).unwrap_err();
    assert_eq!(
            err,
            "solver only supports linear or quadratic objectives and constraints; division by a variable-dependent term was found"
        );
}

#[test]
fn div_by_quadratic_constant_scales_quad_term() {
    let vars = vec![var(1), var(2)];
    let index = index_of(&vars);
    let x = Expression::from_variable(vars[0]);
    let y = Expression::from_variable(vars[1]);
    let xy = Expression::mul(x, y);

    let form = quadratize(&Expression::div(xy, Expression::constant(2.0)), &index, 2).unwrap();
    assert_eq!(form.quad, vec![(0, 1, 0.5)]);
}

#[test]
fn div_by_zero_is_an_error() {
    let form = quadratize(
        &Expression::div(Expression::constant(1.0), Expression::constant(0.0)),
        &HashMap::new(),
        0,
    );
    assert_eq!(
        form.unwrap_err(),
        "division by zero in objective or constraint"
    );
}

// --- vector/matrix affine solving + Sum tests (SPEC-0014) ---

#[test]
fn linearize_shaped_parameter_matches_its_data() {
    let form = linearize_shaped(
        &Expression::from_parameter(1, (3, 1), vec![10.0, 20.0, 30.0]),
        &HashMap::new(),
        0,
    )
    .unwrap();
    assert_eq!(form.shape, (3, 1));
    assert_eq!(
        form.entries.iter().map(|e| e.constant).collect::<Vec<_>>(),
        vec![10.0, 20.0, 30.0]
    );
    assert!(form
        .entries
        .iter()
        .all(|e| e.linear.is_empty() || e.linear.iter().all(|c| *c == 0.0)));
}

#[test]
fn linearize_shaped_variable_is_one_hot_per_entry() {
    let w = Variable::new(1, (2, 2));
    let (offsets, n_total) = offsets_of(&[w]);
    let form = linearize_shaped(&Expression::from_variable(w), &offsets, n_total).unwrap();
    assert_eq!(form.shape, (2, 2));
    assert_eq!(form.entries.len(), 4);
    for (k, entry) in form.entries.iter().enumerate() {
        assert_eq!(entry.constant, 0.0);
        assert_eq!(entry.linear[k], 1.0);
        assert_eq!(entry.linear.iter().filter(|c| **c != 0.0).count(), 1);
    }
}

#[test]
fn linearize_shaped_add_broadcasts_scalar_across_vector() {
    let w = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let expr = Expression::add(Expression::from_variable(w), Expression::constant(5.0));
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (3, 1));
    for (k, entry) in form.entries.iter().enumerate() {
        assert_eq!(entry.constant, 5.0);
        assert_eq!(entry.linear[k], 1.0);
    }
}

#[test]
fn linearize_shaped_add_combines_equal_shapes_entrywise() {
    let w = Variable::new(1, (2, 1));
    let v = Variable::new(2, (2, 1));
    let (offsets, n_total) = offsets_of(&[w, v]);
    let expr = Expression::add(Expression::from_variable(w), Expression::from_variable(v));
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (2, 1));
    assert_eq!(form.entries[0].linear, vec![1.0, 0.0, 1.0, 0.0]);
    assert_eq!(form.entries[1].linear, vec![0.0, 1.0, 0.0, 1.0]);
}

#[test]
fn linearize_shaped_mismatched_shapes_is_a_shape_mismatch_error() {
    let w = Variable::new(1, (2, 1));
    let v = Variable::new(2, (3, 1));
    let (offsets, n_total) = offsets_of(&[w, v]);
    let expr = Expression::add(Expression::from_variable(w), Expression::from_variable(v));
    let err = linearize_shaped(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(err, "shape mismatch: 2x1 vs 3x1");
}

#[test]
fn linearize_shaped_mul_by_constant_vector_is_affine() {
    let w = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let weights = Expression::from_parameter(2, (3, 1), vec![2.0, 3.0, 4.0]);
    let expr = Expression::mul(weights, Expression::from_variable(w));
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (3, 1));
    assert_eq!(form.entries[0].linear, vec![2.0, 0.0, 0.0]);
    assert_eq!(form.entries[1].linear, vec![0.0, 3.0, 0.0]);
    assert_eq!(form.entries[2].linear, vec![0.0, 0.0, 4.0]);
}

#[test]
fn linearize_shaped_mul_of_two_variable_vectors_is_quadratic() {
    // SPEC-0016: a product of two variable-dependent vectors is no longer
    // unconditionally rejected; each broadcast entry is its own degree-2
    // (elementwise) quadratic term, w_k * v_k.
    let w = Variable::new(1, (3, 1));
    let v = Variable::new(2, (3, 1));
    let (offsets, n_total) = offsets_of(&[w, v]);
    let expr = Expression::mul(Expression::from_variable(w), Expression::from_variable(v));
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (3, 1));
    for k in 0..3 {
        assert_eq!(form.entries[k].quad, vec![(k, 3 + k, 1.0)]);
        assert_eq!(form.entries[k].constant, 0.0);
        assert!(form.entries[k].linear.iter().all(|c| *c == 0.0));
    }
}

#[test]
fn linearize_shaped_div_by_constant_scales_every_entry() {
    let w = Variable::new(1, (2, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let expr = Expression::div(Expression::from_variable(w), Expression::constant(2.0));
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.entries[0].linear, vec![0.5, 0.0]);
    assert_eq!(form.entries[1].linear, vec![0.0, 0.5]);
}

#[test]
fn linearize_shaped_div_by_variable_vector_is_an_error() {
    let w = Variable::new(1, (2, 1));
    let v = Variable::new(2, (2, 1));
    let (offsets, n_total) = offsets_of(&[w, v]);
    let expr = Expression::div(Expression::from_variable(w), Expression::from_variable(v));
    let err = linearize_shaped(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(err, DIVISION_ERROR);
}

#[test]
fn linearize_shaped_neg_and_scale_preserve_shape() {
    let w = Variable::new(1, (2, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let neg = linearize_shaped(
        &Expression::neg(Expression::from_variable(w)),
        &offsets,
        n_total,
    )
    .unwrap();
    assert_eq!(neg.shape, (2, 1));
    assert_eq!(neg.entries[0].linear, vec![-1.0, 0.0]);

    let scaled = linearize_shaped(
        &Expression::scale(4.0, Expression::from_variable(w)),
        &offsets,
        n_total,
    )
    .unwrap();
    assert_eq!(scaled.entries[1].linear, vec![0.0, 4.0]);
}

#[test]
fn scalar_quadratic_nested_in_a_vector_tree_is_accepted() {
    // Add(x * x, w) with w a (3, 1) variable: SPEC-0016 lifts the
    // "any non-scalar leaf forces affine-only" restriction, so the
    // nested x * x product now produces a genuine quadratic term,
    // broadcast identically across every entry of w.
    let x = scalar_var(1);
    let w = Variable::new(2, (3, 1));
    let (offsets, n_total) = offsets_of(&[x, w]);
    let x_expr = Expression::from_variable(x);
    let expr = Expression::add(
        Expression::mul(x_expr.clone(), x_expr),
        Expression::from_variable(w),
    );
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (3, 1));
    for k in 0..3 {
        assert_eq!(form.entries[k].quad, vec![(0, 0, 1.0)]);
        let mut expected_linear = vec![0.0; n_total];
        expected_linear[1 + k] = 1.0;
        assert_eq!(form.entries[k].linear, expected_linear);
    }
}

#[test]
fn sum_of_a_vector_variable_sums_its_one_hot_entries() {
    let w = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let form = reduce_expression(
        &Expression::sum(Expression::from_variable(w)),
        &offsets,
        n_total,
    )
    .unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].linear, vec![1.0, 1.0, 1.0]);
}

#[test]
fn sum_of_weighted_vector_gives_a_weighted_total() {
    let w = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let weights = Expression::from_parameter(2, (3, 1), vec![2.0, 3.0, 4.0]);
    let expr = Expression::sum(Expression::mul(weights, Expression::from_variable(w)));
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].linear, vec![2.0, 3.0, 4.0]);
}

#[test]
fn sum_of_a_scalar_is_an_identity() {
    let form = reduce_expression(
        &Expression::sum(Expression::constant(7.0)),
        &HashMap::new(),
        0,
    )
    .unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].constant, 7.0);
}

#[test]
fn sum_of_scalar_quadratic_is_accepted() {
    // Sum(x * x): SPEC-0016 lifts the "Sum forces affine-only"
    // restriction, so this now reduces to the same quadratic form x * x
    // alone would.
    let x = scalar_var(1);
    let (offsets, n_total) = offsets_of(&[x]);
    let x_expr = Expression::from_variable(x);
    let expr = Expression::sum(Expression::mul(x_expr.clone(), x_expr));
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].quad, vec![(0, 0, 1.0)]);
}

#[test]
fn sum_of_squared_differences_is_quadratic() {
    // CVX.SUM(CVX.MUL(diff, diff)) where diff = x - b (a (2, 1) vector
    // variable minus a (2, 1) parameter): the least-squares building
    // block (SPEC-0016, ISSUE-0016), summing each entry's squared
    // residual into one aggregated quadratic objective.
    let x = Variable::new(1, (2, 1));
    let (offsets, n_total) = offsets_of(&[x]);
    let b = Expression::from_parameter(2, (2, 1), vec![1.0, 2.0]);
    let diff = Expression::sub(Expression::from_variable(x), b);
    let expr = Expression::sum(Expression::mul(diff.clone(), diff));
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(
        sorted_quad(form.entries[0].quad.clone()),
        vec![(0, 0, 1.0), (1, 1, 1.0)]
    );
    assert_eq!(form.entries[0].linear, vec![-2.0, -4.0]);
    assert_eq!(form.entries[0].constant, 5.0);
}

#[test]
fn matmul_transpose_self_dot_product_is_quadratic() {
    // x.T @ x for a (3, 1) variable x: the sum-of-squares building block
    // via MatMul/Transpose instead of Sum/Mul, producing the identity
    // quadratic form (SPEC-0016, ISSUE-0016's portfolio-risk-style
    // x.T @ sigma @ x is the same pattern with a weighting parameter
    // matrix between the two x's).
    let x = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[x]);
    let x_expr = Expression::from_variable(x);
    let expr = Expression::matmul(Expression::transpose(x_expr.clone()), x_expr);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(
        sorted_quad(form.entries[0].quad.clone()),
        vec![(0, 0, 1.0), (1, 1, 1.0), (2, 2, 1.0)]
    );
}

#[test]
fn degree_three_vector_product_is_still_rejected() {
    // w .* w .* w for a (3, 1) variable w: still degree 3, rejected with
    // the same DEGREE_ERROR a scalar degree-3 product uses (SPEC-0016
    // unifies the message across shapes).
    let w = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let w_expr = Expression::from_variable(w);
    let expr = Expression::mul(Expression::mul(w_expr.clone(), w_expr.clone()), w_expr);
    let err = reduce_expression(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(err, DEGREE_ERROR);
}

// --- Index tests (SPEC-0015) ---

#[test]
fn index_single_entry_of_a_vector_selects_its_one_hot_row() {
    let w = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let expr = Expression::index(Expression::from_variable(w), 1, 0, 1, 1);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].linear, vec![0.0, 1.0, 0.0]);
}

#[test]
fn index_single_row_of_a_matrix_selects_the_expected_entries() {
    // 2x3 matrix, row-major entries 0..=5. Row 1 (0-based) is [3, 4, 5].
    let w = Variable::new(1, (2, 3));
    let (offsets, n_total) = offsets_of(&[w]);
    let expr = Expression::index(Expression::from_variable(w), 1, 0, 1, 3);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 3));
    assert_eq!(form.entries[0].linear, vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    assert_eq!(form.entries[1].linear, vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    assert_eq!(form.entries[2].linear, vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn index_single_column_of_a_matrix_selects_the_expected_entries() {
    // 2x3 matrix; column 2 (0-based) is entries 2 and 5.
    let w = Variable::new(1, (2, 3));
    let (offsets, n_total) = offsets_of(&[w]);
    let expr = Expression::index(Expression::from_variable(w), 0, 2, 2, 1);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (2, 1));
    assert_eq!(form.entries[0].linear, vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
    assert_eq!(form.entries[1].linear, vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn index_general_sub_block_of_a_matrix_selects_the_expected_entries() {
    // 3x3 matrix; rows 1..=2, cols 1..=2 (0-based) is the bottom-right 2x2.
    let w = Variable::new(1, (3, 3));
    let (offsets, n_total) = offsets_of(&[w]);
    let expr = Expression::index(Expression::from_variable(w), 1, 1, 2, 2);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (2, 2));
    let mut zeros = vec![0.0; 9];
    zeros[4] = 1.0;
    assert_eq!(form.entries[0].linear, zeros);
}

#[test]
fn index_of_a_parameter_selects_its_data() {
    let param = Expression::from_parameter(1, (3, 1), vec![10.0, 20.0, 30.0]);
    let expr = Expression::index(param, 1, 0, 1, 1);
    let form = reduce_expression(&expr, &HashMap::new(), 0).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].constant, 20.0);
}

#[test]
fn index_nested_inside_index_reduces_correctly() {
    let w = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    // First select the last two entries, then the first of those.
    let inner = Expression::index(Expression::from_variable(w), 1, 0, 2, 1);
    let expr = Expression::index(inner, 0, 0, 1, 1);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].linear, vec![0.0, 1.0, 0.0]);
}

#[test]
fn index_constraint_restricts_only_the_selected_entry() {
    let w = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[w]);
    let expr = Expression::index(Expression::from_variable(w), 1, 0, 1, 1);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    // Only the second entry's coefficient is set; the rest are untouched.
    assert_eq!(form.entries[0].linear, vec![0.0, 1.0, 0.0]);
    assert_eq!(form.entries[0].constant, 0.0);
}

#[test]
fn index_out_of_bounds_is_a_descriptive_error_not_a_panic() {
    let w = Variable::new(1, (2, 2));
    let (offsets, n_total) = offsets_of(&[w]);
    let expr = Expression::index(Expression::from_variable(w), 1, 1, 2, 2);
    let err = reduce_expression(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(
        err,
        "requested rows 2..3 and columns 2..3 are out of bounds for a 2x2 operand"
    );
}

#[test]
fn index_of_scalar_quadratic_is_accepted() {
    // Index(x * x, 0, 0, 1, 1): SPEC-0016 lifts the "Index forces
    // affine-only" restriction, same as Sum above.
    let x = scalar_var(1);
    let (offsets, n_total) = offsets_of(&[x]);
    let x_expr = Expression::from_variable(x);
    let expr = Expression::index(Expression::mul(x_expr.clone(), x_expr), 0, 0, 1, 1);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].quad, vec![(0, 0, 1.0)]);
}

#[test]
fn mul_of_two_index_derived_scalars_is_quadratic() {
    // CVX.INDEX(X, 1, 1) * CVX.INDEX(X, 1, 2): two non-constant Index
    // operands multiplied together now produce the expected cross-term
    // quadratic form (SPEC-0016).
    let x = Variable::new(1, (1, 2));
    let (offsets, n_total) = offsets_of(&[x]);
    let x_expr = Expression::from_variable(x);
    let left = Expression::index(x_expr.clone(), 0, 0, 1, 1);
    let right = Expression::index(x_expr, 0, 1, 1, 1);
    let expr = Expression::mul(left, right);
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].quad, vec![(0, 1, 1.0)]);
}

// --- MatMul/Transpose tests (SPEC-0018) ---

#[test]
fn matmul_and_transpose_construct_the_expected_variants() {
    let a = Expression::constant(1.0);
    let b = Expression::constant(2.0);
    assert_eq!(
        Expression::matmul(a.clone(), b.clone()),
        Expression::MatMul(Box::new(a.clone()), Box::new(b.clone()))
    );
    assert_eq!(
        Expression::transpose(a.clone()),
        Expression::Transpose(Box::new(a))
    );
}

#[test]
fn transpose_entries_permutes_row_to_column_and_back() {
    let w = Variable::new(1, (1, 3));
    let (offsets, n_total) = offsets_of(&[w]);
    let row_form = linearize_shaped(&Expression::from_variable(w), &offsets, n_total).unwrap();
    let column = transpose_entries(&row_form);
    assert_eq!(column.shape, (3, 1));
    assert_eq!(
        column
            .entries
            .iter()
            .map(|e| e.linear.clone())
            .collect::<Vec<_>>(),
        row_form
            .entries
            .iter()
            .map(|e| e.linear.clone())
            .collect::<Vec<_>>()
    );

    let back = transpose_entries(&column);
    assert_eq!(back.shape, (1, 3));
}

#[test]
fn transpose_entries_permutes_a_general_non_square_matrix() {
    // 2x3 matrix, row-major entries 0..=5 as constants; transposing gives
    // the 3x2 matrix with entries read column-major from the original.
    let param = Expression::from_parameter(1, (2, 3), vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    let form = linearize_shaped(&param, &HashMap::new(), 0).unwrap();
    let transposed = transpose_entries(&form);
    assert_eq!(transposed.shape, (3, 2));
    assert_eq!(
        transposed
            .entries
            .iter()
            .map(|e| e.constant)
            .collect::<Vec<_>>(),
        vec![0.0, 3.0, 1.0, 4.0, 2.0, 5.0]
    );
}

#[test]
fn transpose_of_a_1x1_operand_is_a_no_op() {
    let form = linearize_shaped(&Expression::constant(7.0), &HashMap::new(), 0).unwrap();
    let transposed = transpose_entries(&form);
    assert_eq!(transposed.shape, (1, 1));
    assert_eq!(transposed.entries[0].constant, 7.0);
}

#[test]
fn matmul_of_row_parameter_and_column_variable_is_a_dot_product() {
    // (1, 3) weights times (3, 1) variable column -> (1, 1) scalar.
    let x = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[x]);
    let weights = Expression::from_parameter(2, (1, 3), vec![2.0, 3.0, 4.0]);
    let expr = Expression::matmul(weights, Expression::from_variable(x));
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].linear, vec![2.0, 3.0, 4.0]);
}

#[test]
fn matmul_of_column_variable_and_row_parameter_is_an_outer_product() {
    // (3, 1) variable column times (1, 3) weights row -> (3, 3) outer
    // product: entry (i, j) is x_i scaled by weights[j].
    let x = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[x]);
    let weights = Expression::from_parameter(2, (1, 3), vec![5.0, 6.0, 7.0]);
    let expr = Expression::matmul(Expression::from_variable(x), weights);
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (3, 3));
    // Row i (entries 3*i..3*i+3) is x_i * [5, 6, 7].
    for i in 0..3 {
        for j in 0..3 {
            let entry = &form.entries[i * 3 + j];
            let mut expected = vec![0.0; 3];
            expected[i] = [5.0, 6.0, 7.0][j];
            assert_eq!(entry.linear, expected);
        }
    }
}

#[test]
fn matmul_of_a_general_constant_matrix_and_variable_column_is_the_expected_product() {
    // [[1, 2], [3, 4]] @ [x0, x1] -> [x0 + 2*x1, 3*x0 + 4*x1].
    let x = Variable::new(1, (2, 1));
    let (offsets, n_total) = offsets_of(&[x]);
    let matrix = Expression::from_parameter(2, (2, 2), vec![1.0, 2.0, 3.0, 4.0]);
    let expr = Expression::matmul(matrix, Expression::from_variable(x));
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (2, 1));
    assert_eq!(form.entries[0].linear, vec![1.0, 2.0]);
    assert_eq!(form.entries[1].linear, vec![3.0, 4.0]);
}

#[test]
fn matmul_of_incompatible_shapes_is_a_descriptive_error_not_a_panic() {
    let a = Expression::from_parameter(1, (2, 3), vec![0.0; 6]);
    let b = Expression::from_parameter(2, (2, 2), vec![0.0; 4]);
    let expr = Expression::matmul(a, b);
    let err = linearize_shaped(&expr, &HashMap::new(), 0).unwrap_err();
    assert_eq!(
        err,
        "matrix multiplication requires the left operand's column count \
         to match the right operand's row count: 2x3 (columns=3) vs 2x2 \
         (rows=2)"
    );
}

#[test]
fn matmul_of_two_variable_dependent_operands_is_quadratic() {
    // y @ x for y (1, 3) and x (3, 1) variables: a genuine dot product of
    // two variable vectors, now reduced to the expected quadratic form
    // (SPEC-0016, resolving what SPEC-0018 explicitly deferred) instead
    // of being rejected.
    let x = Variable::new(1, (3, 1));
    let y = Variable::new(2, (1, 3));
    let (offsets, n_total) = offsets_of(&[x, y]);
    let expr = Expression::matmul(Expression::from_variable(y), Expression::from_variable(x));
    let form = linearize_shaped(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(
        sorted_quad(form.entries[0].quad.clone()),
        vec![(0, 3, 1.0), (1, 4, 1.0), (2, 5, 1.0)]
    );
}

#[test]
fn matmul_nested_inside_sum_reduces_correctly() {
    // sum(weights @ x) for weights (1, 3) constant and x (3, 1) variable
    // is the same scalar the bare matmul already produces (its own
    // shape is already (1, 1), so summing it is a no-op).
    let x = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[x]);
    let weights = Expression::from_parameter(2, (1, 3), vec![2.0, 3.0, 4.0]);
    let expr = Expression::sum(Expression::matmul(weights, Expression::from_variable(x)));
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].linear, vec![2.0, 3.0, 4.0]);
}

#[test]
fn index_nested_inside_matmul_operand_reduces_correctly() {
    // Select row 2 of a 2x3 constant matrix via Index, then matmul it
    // against a (3, 1) variable column.
    let x = Variable::new(1, (3, 1));
    let (offsets, n_total) = offsets_of(&[x]);
    let matrix = Expression::from_parameter(2, (2, 3), vec![1.0, 1.0, 1.0, 2.0, 3.0, 4.0]);
    let row = Expression::index(matrix, 1, 0, 1, 3);
    let expr = Expression::matmul(row, Expression::from_variable(x));
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].linear, vec![2.0, 3.0, 4.0]);
}

#[test]
fn matmul_nested_inside_matmul_operand_reduces_correctly() {
    // (weights @ M) @ x: chaining two matmuls, the first entirely
    // constant, the second against a variable column.
    let x = Variable::new(1, (2, 1));
    let (offsets, n_total) = offsets_of(&[x]);
    let weights = Expression::from_parameter(1, (1, 2), vec![1.0, 1.0]);
    let m = Expression::from_parameter(2, (2, 2), vec![1.0, 2.0, 3.0, 4.0]);
    let combined_weights = Expression::matmul(weights, m);
    let expr = Expression::matmul(combined_weights, Expression::from_variable(x));
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    // combined_weights = [1,1] @ [[1,2],[3,4]] = [4, 6]
    assert_eq!(form.entries[0].linear, vec![4.0, 6.0]);
}

#[test]
fn transpose_of_scalar_quadratic_preserves_quadratic_support() {
    // Transpose(x * x) for a scalar x: Transpose is a no-op on a (1, 1)
    // operand, and the nested x * x product still reduces to the
    // expected quadratic form.
    let x = scalar_var(1);
    let (offsets, n_total) = offsets_of(&[x]);
    let x_expr = Expression::from_variable(x);
    let expr = Expression::transpose(Expression::mul(x_expr.clone(), x_expr));
    let form = reduce_expression(&expr, &offsets, n_total).unwrap();
    assert_eq!(form.shape, (1, 1));
    assert_eq!(form.entries[0].quad, vec![(0, 0, 1.0)]);
}
