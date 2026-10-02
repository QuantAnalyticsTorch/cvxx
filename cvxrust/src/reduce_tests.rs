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
fn linearize_shaped_mul_of_two_variable_vectors_is_an_error() {
    let w = Variable::new(1, (3, 1));
    let v = Variable::new(2, (3, 1));
    let (offsets, n_total) = offsets_of(&[w, v]);
    let expr = Expression::mul(Expression::from_variable(w), Expression::from_variable(v));
    let err = linearize_shaped(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(err, VECTOR_MUL_ERROR);
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
    assert_eq!(err, VECTOR_DIV_ERROR);
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
fn is_all_scalar_true_for_all_1x1_leaves_even_with_quadratic_term() {
    let x = scalar_var(1);
    let expr = Expression::mul(Expression::from_variable(x), Expression::from_variable(x));
    assert!(is_all_scalar(&expr));
}

#[test]
fn is_all_scalar_false_once_any_leaf_is_non_scalar() {
    let w = Variable::new(1, (3, 1));
    assert!(!is_all_scalar(&Expression::from_variable(w)));
}

#[test]
fn is_all_scalar_false_for_any_expression_containing_sum() {
    let x = scalar_var(1);
    let expr = Expression::sum(Expression::from_variable(x));
    assert!(!is_all_scalar(&expr));
}

#[test]
fn scalar_quadratic_nested_in_a_vector_tree_is_rejected() {
    // Add(x * x, w) with w a (3, 1) variable: once routed through
    // linearize_shaped, the nested x * x product is restricted to
    // affine terms (SPEC-0014 Non-Objective), even though x * x alone
    // would be fine via quadratize.
    let x = scalar_var(1);
    let w = Variable::new(2, (3, 1));
    let (offsets, n_total) = offsets_of(&[x, w]);
    let x_expr = Expression::from_variable(x);
    let expr = Expression::add(
        Expression::mul(x_expr.clone(), x_expr),
        Expression::from_variable(w),
    );
    let err = reduce_expression(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(err, VECTOR_MUL_ERROR);
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
fn sum_of_scalar_quadratic_is_rejected() {
    // Sum(x * x): an all-(1,1)-leaf tree containing Sum is still routed
    // through linearize_shaped (is_all_scalar's Sum arm), so the nested
    // quadratic product is rejected.
    let x = scalar_var(1);
    let (offsets, n_total) = offsets_of(&[x]);
    let x_expr = Expression::from_variable(x);
    let expr = Expression::sum(Expression::mul(x_expr.clone(), x_expr));
    let err = reduce_expression(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(err, VECTOR_MUL_ERROR);
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
fn is_all_scalar_false_for_any_expression_containing_index() {
    let x = scalar_var(1);
    let expr = Expression::index(Expression::from_variable(x), 0, 0, 1, 1);
    assert!(!is_all_scalar(&expr));
}

#[test]
fn index_of_scalar_quadratic_is_rejected() {
    // Index(x * x, 0, 0, 1, 1): an all-(1,1)-leaf tree containing Index is
    // still routed through linearize_shaped (is_all_scalar's Index arm),
    // so the nested quadratic product is rejected, same as Sum.
    let x = scalar_var(1);
    let (offsets, n_total) = offsets_of(&[x]);
    let x_expr = Expression::from_variable(x);
    let expr = Expression::index(Expression::mul(x_expr.clone(), x_expr), 0, 0, 1, 1);
    let err = reduce_expression(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(err, VECTOR_MUL_ERROR);
}

#[test]
fn mul_of_two_index_derived_scalars_is_rejected() {
    // CVX.INDEX(X, 1, 1) * CVX.INDEX(X, 1, 2): two non-constant Index
    // operands multiplied together, forced through linearize_shaped,
    // fails with the existing VECTOR_MUL_ERROR (no new quadratic support
    // over Index is introduced).
    let x = Variable::new(1, (1, 2));
    let (offsets, n_total) = offsets_of(&[x]);
    let x_expr = Expression::from_variable(x);
    let left = Expression::index(x_expr.clone(), 0, 0, 1, 1);
    let right = Expression::index(x_expr, 0, 1, 1, 1);
    let expr = Expression::mul(left, right);
    let err = reduce_expression(&expr, &offsets, n_total).unwrap_err();
    assert_eq!(err, VECTOR_MUL_ERROR);
}
