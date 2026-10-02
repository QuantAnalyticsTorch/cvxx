//! Excel-facing expression builders: `CVX.EXPRESSION` and functional helpers.

use xladd::variant::Variant;
use xladd::xlcall::LPXLOPER12;

use crate::analytics::parser;
use crate::analytics::resolve::resolve_expr;
use crate::core::error::CvxError;
use crate::core::handle::{parse_handle, HandleKind};
use crate::core::registry::{parameter_id, Registry};
use crate::data;
use cvxrust::Expression;

/// `CVX.EXPRESSION(expr_string, [name])` — parses an expression string,
/// resolves identifiers against the registry, and returns a
/// `cvx:expr:<uuid>` handle.
#[export_name = "CVX.EXPRESSION"]
pub extern "system" fn cvx_expression(expr: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    run_expression(expr, name)
}

fn run_expression(expr: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = (|| {
        let expr = data::parse_string(&Variant::from_xloper(expr))?;
        let name = data::parse_optional_name(&Variant::from_xloper(name))?;

        let ast = parser::parse(&expr)?;
        let registry = Registry::global();
        let resolved = resolve_expr(registry, &ast)?;

        registry.insert_expression(name, resolved.expression, resolved.dependencies)
    })();

    to_xloper_result(result, "CVX.EXPRESSION")
}

pub(crate) fn to_xloper_result(result: Result<String, CvxError>, context: &str) -> LPXLOPER12 {
    match result {
        Ok(handle) => Box::into_raw(Box::new(Variant::from_str(&handle))) as LPXLOPER12,
        Err(err) => {
            tracing::error!(error = %err, "{context} failed");
            let message = err.to_string();
            Box::into_raw(Box::new(Variant::from_str(&message))) as LPXLOPER12
        }
    }
}

/// `CVX.ADD(left, right, [name])` — adds two existing expressions.
#[export_name = "CVX.ADD"]
pub extern "system" fn cvx_add(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    run_binary(left, right, name, Expression::add, "CVX.ADD")
}

/// `CVX.SUB(left, right, [name])` — subtracts two existing expressions.
#[export_name = "CVX.SUB"]
pub extern "system" fn cvx_sub(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    run_binary(left, right, name, Expression::sub, "CVX.SUB")
}

/// `CVX.MUL(left, right, [name])` — multiplies two existing expressions.
#[export_name = "CVX.MUL"]
pub extern "system" fn cvx_mul(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    run_binary(left, right, name, Expression::mul, "CVX.MUL")
}

/// `CVX.DIV(left, right, [name])` — divides two existing expressions.
#[export_name = "CVX.DIV"]
pub extern "system" fn cvx_div(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    run_binary(left, right, name, Expression::div, "CVX.DIV")
}

/// `CVX.NEG(operand, [name])` — negates an existing expression.
#[export_name = "CVX.NEG"]
pub extern "system" fn cvx_neg(operand: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_unary(operand, name, Expression::neg);
    to_xloper_result(result, "CVX.NEG")
}

/// `CVX.SUM(operand, [name])` — sums every entry of a (possibly
/// vector/matrix-shaped) expression into a single value (SPEC-0014).
#[export_name = "CVX.SUM"]
pub extern "system" fn cvx_sum(operand: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_unary(operand, name, Expression::sum);
    to_xloper_result(result, "CVX.SUM")
}

/// `CVX.SCALE(operand, scalar, [name])` — scales an expression by a scalar.
#[export_name = "CVX.SCALE"]
pub extern "system" fn cvx_scale(
    operand: LPXLOPER12,
    scalar: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    let result = run_scale(operand, scalar, name);
    to_xloper_result(result, "CVX.SCALE")
}

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

fn run_binary(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
    op: fn(Expression, Expression) -> Expression,
    context: &str,
) -> LPXLOPER12 {
    let result = (|| {
        let left = resolve_handle_arg(left)?;
        let right = resolve_handle_arg(right)?;
        let name = data::parse_optional_name(&Variant::from_xloper(name))?;
        let expr = op(left, right);
        Registry::global().insert_expression(name, expr, vec![])
    })();

    to_xloper_result(result, context)
}

fn run_unary(
    operand: LPXLOPER12,
    name: LPXLOPER12,
    op: fn(Expression) -> Expression,
) -> Result<String, CvxError> {
    let operand = resolve_handle_arg(operand)?;
    let name = data::parse_optional_name(&Variant::from_xloper(name))?;
    let expr = op(operand);
    Registry::global().insert_expression(name, expr, vec![])
}

fn run_scale(
    operand: LPXLOPER12,
    scalar: LPXLOPER12,
    name: LPXLOPER12,
) -> Result<String, CvxError> {
    let operand = resolve_handle_arg(operand)?;
    let scalar_value = data::parse_scalar(&Variant::from_xloper(scalar))?;
    let name = data::parse_optional_name(&Variant::from_xloper(name))?;
    let expr = Expression::scale(scalar_value, operand);
    Registry::global().insert_expression(name, expr, vec![])
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

pub(crate) fn resolve_handle_arg(arg: LPXLOPER12) -> Result<Expression, CvxError> {
    let text = data::parse_string(&Variant::from_xloper(arg))?
        .trim()
        .to_string();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(&text)?;
        let registry = Registry::global();
        match kind {
            HandleKind::Param => registry
                .get_parameter_by_uuid(uuid)
                .map(|e| Expression::from_parameter(parameter_id(e.uuid), e.shape, e.data))
                .ok_or_else(|| CvxError::UnknownIdentifier(text.clone())),
            HandleKind::Var => registry
                .get_variable_by_uuid(uuid)
                .map(|e| Expression::from_variable(e.variable))
                .ok_or_else(|| CvxError::UnknownIdentifier(text.clone())),
            HandleKind::Expr => registry
                .get_expression_by_uuid(uuid)
                .map(|e| e.expression)
                .ok_or_else(|| CvxError::UnknownIdentifier(text.clone())),
            HandleKind::Constr
            | HandleKind::ConstrSet
            | HandleKind::Obj
            | HandleKind::Prob
            | HandleKind::Result => Err(CvxError::UnknownIdentifier(text.clone())),
        }
    } else {
        // Allow referencing named objects by name as a convenience.
        let registry = Registry::global();
        if let Some(entry) = registry.get_parameter_by_name(&text) {
            return Ok(Expression::from_parameter(
                parameter_id(entry.uuid),
                entry.shape,
                entry.data,
            ));
        }
        if let Some(entry) = registry.get_variable_by_name(&text) {
            return Ok(Expression::from_variable(entry.variable));
        }
        if let Some(entry) = registry.get_expression_by_name(&text) {
            return Ok(entry.expression);
        }
        Err(CvxError::UnknownIdentifier(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_parameter_handle_to_expression() {
        let handle = Registry::global()
            .insert_parameter(Some("A".to_string()), (1, 1), vec![5.0])
            .unwrap();
        let expr = resolve_handle_for_test(&handle).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        assert_eq!(
            expr,
            Expression::from_parameter(parameter_id(uuid), (1, 1), vec![5.0])
        );
    }

    #[test]
    fn resolves_variable_handle_to_expression() {
        let handle = Registry::global()
            .insert_variable(Some("x".to_string()), (1, 1))
            .unwrap();
        let variable = Registry::global()
            .get_variable_by_name("x")
            .unwrap()
            .variable;
        let expr = resolve_handle_for_test(&handle).unwrap();
        assert_eq!(expr, Expression::from_variable(variable));
    }

    fn resolve_handle_for_test(handle: &str) -> Result<Expression, CvxError> {
        // The real resolve_handle_arg takes an LPXLOPER12; for unit tests we
        // bypass the XLOPER wrapper and use the registry directly.
        let (kind, uuid) = parse_handle(handle)?;
        let registry = Registry::global();
        match kind {
            HandleKind::Param => registry
                .get_parameter_by_uuid(uuid)
                .map(|e| Expression::from_parameter(parameter_id(e.uuid), e.shape, e.data))
                .ok_or_else(|| CvxError::UnknownIdentifier(handle.to_string())),
            HandleKind::Var => registry
                .get_variable_by_uuid(uuid)
                .map(|e| Expression::from_variable(e.variable))
                .ok_or_else(|| CvxError::UnknownIdentifier(handle.to_string())),
            HandleKind::Expr => registry
                .get_expression_by_uuid(uuid)
                .map(|e| e.expression)
                .ok_or_else(|| CvxError::UnknownIdentifier(handle.to_string())),
            HandleKind::Constr
            | HandleKind::ConstrSet
            | HandleKind::Obj
            | HandleKind::Prob
            | HandleKind::Result => Err(CvxError::UnknownIdentifier(handle.to_string())),
        }
    }

    // `run_index` itself takes `LPXLOPER12` and can't be driven directly in
    // a unit test (same reason none of this module's other `run_*`
    // functions are tested directly); this mirrors `run_index`'s body
    // exactly, minus the `Variant`/`LPXLOPER12` parsing already covered by
    // `data::parse_dimension`/`parse_optional_dimension`'s own tests
    // (SPEC-0015).
    fn run_index_for_test(
        operand: Expression,
        row: usize,
        col: usize,
        rows: usize,
        cols: usize,
        name: Option<String>,
    ) -> Result<String, CvxError> {
        let operand_shape = crate::analytics::shape::infer_shape(&operand)?;
        crate::analytics::shape::check_index_bounds(operand_shape, row, col, rows, cols)?;
        let expr = Expression::index(operand, row - 1, col - 1, rows, cols);
        Registry::global().insert_expression(name, expr, vec![])
    }

    #[test]
    fn cvx_index_selects_a_single_entry() {
        let var_handle = Registry::global().insert_variable(None, (3, 1)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let handle = run_index_for_test(operand.clone(), 2, 1, 1, 1, None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::index(operand, 1, 0, 1, 1));
    }

    #[test]
    fn cvx_index_defaults_rows_and_cols_to_one() {
        let var_handle = Registry::global().insert_variable(None, (3, 1)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let handle = run_index_for_test(operand.clone(), 1, 1, 1, 1, None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::index(operand, 0, 0, 1, 1));
    }

    #[test]
    fn cvx_index_selects_a_row() {
        let var_handle = Registry::global().insert_variable(None, (2, 3)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let handle = run_index_for_test(operand.clone(), 2, 1, 1, 3, None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::index(operand, 1, 0, 1, 3));
    }

    #[test]
    fn cvx_index_selects_a_column() {
        let var_handle = Registry::global().insert_variable(None, (2, 3)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let handle = run_index_for_test(operand.clone(), 1, 2, 2, 1, None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::index(operand, 0, 1, 2, 1));
    }

    #[test]
    fn cvx_index_selects_a_general_sub_block() {
        let var_handle = Registry::global().insert_variable(None, (3, 3)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let handle = run_index_for_test(operand.clone(), 2, 2, 2, 2, None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::index(operand, 1, 1, 2, 2));
    }

    #[test]
    fn cvx_index_of_an_index_nests_correctly() {
        let var_handle = Registry::global().insert_variable(None, (3, 1)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let inner_handle = run_index_for_test(operand, 2, 1, 2, 1, None).unwrap();
        let inner_expr = resolve_handle_for_test(&inner_handle).unwrap();
        let outer_handle = run_index_for_test(inner_expr.clone(), 1, 1, 1, 1, None).unwrap();
        let (_, uuid) = parse_handle(&outer_handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::index(inner_expr, 0, 0, 1, 1));
    }

    #[test]
    fn cvx_index_out_of_bounds_is_a_descriptive_error() {
        let var_handle = Registry::global().insert_variable(None, (2, 2)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let err = run_index_for_test(operand, 2, 2, 2, 2, None).unwrap_err();
        assert_eq!(
            err,
            CvxError::InvalidExpression(
                "requested rows 2..3 and columns 2..3 are out of bounds for a 2x2 operand"
                    .to_string()
            )
        );
    }

    #[test]
    fn cvx_index_reusing_a_name_overwrites_the_previous_entry() {
        // Expressions intentionally allow same-table name reuse (unlike
        // parameters/variables): `insert_expression` overwrites the old
        // entry so repeated Excel edits/recalculation can keep reusing the
        // same name (`src/core/registry.rs::insert_expression`).
        // `CVX.INDEX` inherits this unchanged.
        let var_handle = Registry::global().insert_variable(None, (2, 1)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        run_index_for_test(
            operand.clone(),
            1,
            1,
            1,
            1,
            Some("reused_index_name".to_string()),
        )
        .unwrap();
        run_index_for_test(
            operand.clone(),
            2,
            1,
            1,
            1,
            Some("reused_index_name".to_string()),
        )
        .unwrap();
        let entry = Registry::global()
            .get_expression_by_name("reused_index_name")
            .unwrap();
        assert_eq!(entry.expression, Expression::index(operand, 1, 0, 1, 1));
    }

    #[test]
    fn cvx_index_name_already_used_by_a_different_object_table_errors() {
        let var_handle = Registry::global()
            .insert_variable(
                Some("index_name_ambiguous_with_variable".to_string()),
                (2, 1),
            )
            .unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let err = run_index_for_test(
            operand,
            1,
            1,
            1,
            1,
            Some("index_name_ambiguous_with_variable".to_string()),
        )
        .unwrap_err();
        assert!(matches!(err, CvxError::AmbiguousIdentifier(_)));
    }

    // --- CVX.MATMUL/CVX.TRANSPOSE tests (SPEC-0018) ---

    /// `run_binary`/`run_unary` themselves take `LPXLOPER12` and can't be
    /// driven directly in a unit test (same reason as `run_index_for_test`
    /// above); these mirror their bodies exactly, minus the
    /// `Variant`/`LPXLOPER12` parsing.
    fn run_matmul_for_test(
        left: Expression,
        right: Expression,
        name: Option<String>,
    ) -> Result<String, CvxError> {
        let expr = Expression::matmul(left, right);
        Registry::global().insert_expression(name, expr, vec![])
    }

    fn run_transpose_for_test(
        operand: Expression,
        name: Option<String>,
    ) -> Result<String, CvxError> {
        let expr = Expression::transpose(operand);
        Registry::global().insert_expression(name, expr, vec![])
    }

    #[test]
    fn cvx_matmul_constructs_the_expected_expression_for_parameter_and_variable_operands() {
        let param_handle = Registry::global()
            .insert_parameter(None, (1, 3), vec![1.0, 2.0, 3.0])
            .unwrap();
        let var_handle = Registry::global().insert_variable(None, (3, 1)).unwrap();
        let left = resolve_handle_for_test(&param_handle).unwrap();
        let right = resolve_handle_for_test(&var_handle).unwrap();

        let handle = run_matmul_for_test(left.clone(), right.clone(), None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::matmul(left, right));
    }

    #[test]
    fn cvx_matmul_constructs_the_expected_expression_for_expression_operands() {
        let var_handle = Registry::global().insert_variable(None, (2, 2)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let left_handle = run_transpose_for_test(operand.clone(), None).unwrap();
        let left = resolve_handle_for_test(&left_handle).unwrap();

        let handle = run_matmul_for_test(left.clone(), operand.clone(), None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::matmul(left, operand));
    }

    #[test]
    fn cvx_matmul_does_not_error_at_call_time_even_for_already_incompatible_shapes() {
        // Lazy shape validation (Non-Objective, SPEC-0018): the handle is
        // built unconditionally; the mismatch only surfaces later via
        // `infer_shape`/a solve.
        let a_handle = Registry::global().insert_variable(None, (2, 3)).unwrap();
        let b_handle = Registry::global().insert_variable(None, (2, 2)).unwrap();
        let a = resolve_handle_for_test(&a_handle).unwrap();
        let b = resolve_handle_for_test(&b_handle).unwrap();

        let handle = run_matmul_for_test(a, b, None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();

        let err = crate::analytics::shape::infer_shape(&entry.expression).unwrap_err();
        assert!(matches!(err, CvxError::InvalidExpression(_)));
    }

    #[test]
    fn cvx_matmul_reusing_a_name_overwrites_the_previous_entry() {
        let var_handle = Registry::global().insert_variable(None, (1, 3)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        run_matmul_for_test(
            operand.clone(),
            Expression::constant(1.0),
            Some("reused_matmul_name".to_string()),
        )
        .unwrap();
        run_matmul_for_test(
            operand.clone(),
            Expression::constant(2.0),
            Some("reused_matmul_name".to_string()),
        )
        .unwrap();
        let entry = Registry::global()
            .get_expression_by_name("reused_matmul_name")
            .unwrap();
        assert_eq!(
            entry.expression,
            Expression::matmul(operand, Expression::constant(2.0))
        );
    }

    #[test]
    fn cvx_matmul_name_already_used_by_a_different_object_table_errors() {
        let var_handle = Registry::global()
            .insert_variable(
                Some("matmul_name_ambiguous_with_variable".to_string()),
                (1, 1),
            )
            .unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let err = run_matmul_for_test(
            operand.clone(),
            Expression::constant(1.0),
            Some("matmul_name_ambiguous_with_variable".to_string()),
        )
        .unwrap_err();
        assert!(matches!(err, CvxError::AmbiguousIdentifier(_)));
    }

    #[test]
    fn cvx_transpose_constructs_the_expected_expression() {
        let var_handle = Registry::global().insert_variable(None, (2, 3)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let handle = run_transpose_for_test(operand.clone(), None).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::transpose(operand));
    }

    #[test]
    fn cvx_transpose_of_a_transpose_nests_correctly() {
        let var_handle = Registry::global().insert_variable(None, (2, 3)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let inner_handle = run_transpose_for_test(operand, None).unwrap();
        let inner = resolve_handle_for_test(&inner_handle).unwrap();
        let outer_handle = run_transpose_for_test(inner.clone(), None).unwrap();
        let (_, uuid) = parse_handle(&outer_handle).unwrap();
        let entry = Registry::global().get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, Expression::transpose(inner));
    }

    #[test]
    fn cvx_transpose_reusing_a_name_overwrites_the_previous_entry() {
        let var_handle = Registry::global().insert_variable(None, (1, 3)).unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        run_transpose_for_test(operand.clone(), Some("reused_transpose_name".to_string())).unwrap();
        run_transpose_for_test(operand.clone(), Some("reused_transpose_name".to_string())).unwrap();
        let entry = Registry::global()
            .get_expression_by_name("reused_transpose_name")
            .unwrap();
        assert_eq!(entry.expression, Expression::transpose(operand));
    }

    #[test]
    fn cvx_transpose_name_already_used_by_a_different_object_table_errors() {
        let var_handle = Registry::global()
            .insert_variable(
                Some("transpose_name_ambiguous_with_variable".to_string()),
                (1, 1),
            )
            .unwrap();
        let operand = resolve_handle_for_test(&var_handle).unwrap();
        let err = run_transpose_for_test(
            operand,
            Some("transpose_name_ambiguous_with_variable".to_string()),
        )
        .unwrap_err();
        assert!(matches!(err, CvxError::AmbiguousIdentifier(_)));
    }
}
