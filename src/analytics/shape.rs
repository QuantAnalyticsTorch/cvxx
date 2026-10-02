//! Diagnostic-only shape inference and expression rendering for
//! `CVX.DESCRIBE`/`CVX.SHAPE` (SPEC-0007). Read-only and side-effect-free;
//! never called during expression construction (SPEC-0004) or solving
//! (SPEC-0010).

use crate::core::error::CvxError;
use crate::core::registry::Registry;
use cvxrust::Expression;

/// Infers the `(rows, cols)` shape of an `Expression` for display purposes
/// only.
pub fn infer_shape(expr: &Expression) -> Result<(usize, usize), CvxError> {
    match expr {
        Expression::Constant(_) => Ok((1, 1)),
        Expression::Variable(v) => Ok(v.shape),
        Expression::Parameter { shape, .. } => Ok(*shape),
        Expression::Add(l, r)
        | Expression::Sub(l, r)
        | Expression::Mul(l, r)
        | Expression::Div(l, r) => broadcast_shape(infer_shape(l)?, infer_shape(r)?),
        Expression::Neg(e) => infer_shape(e),
        Expression::Scale { expr, .. } => infer_shape(expr),
        Expression::Sum(_) => Ok((1, 1)),
    }
}

/// Scalar-broadcasts two operand shapes, or requires them to be equal.
fn broadcast_shape(a: (usize, usize), b: (usize, usize)) -> Result<(usize, usize), CvxError> {
    if a == (1, 1) {
        Ok(b)
    } else if b == (1, 1) || a == b {
        Ok(a)
    } else {
        Err(CvxError::InvalidExpression(format!(
            "shape mismatch: {}x{} vs {}x{}",
            a.0, a.1, b.0, b.1
        )))
    }
}

/// Renders an `Expression` as a fully-parenthesized diagnostic string for
/// `CVX.DESCRIBE`. Not guaranteed to round-trip through `CVX.EXPRESSION`'s
/// parser. A `Variable`/`Parameter` leaf shows its current registered name
/// (quoted), when `registry` has one, else the structural placeholder
/// (SPEC-0013).
pub fn render_expression(expr: &Expression, registry: &Registry) -> String {
    match expr {
        Expression::Constant(c) => format!("{c}"),
        Expression::Variable(v) => registry
            .get_variable_by_variable_id(v.id)
            .and_then(|e| e.name)
            .map(|name| format!("\"{name}\""))
            .unwrap_or_else(|| format!("var#{}", v.id)),
        Expression::Parameter { id, shape, .. } => registry
            .get_parameter_by_parameter_id(*id)
            .and_then(|e| e.name)
            .map(|name| format!("\"{name}\""))
            .unwrap_or_else(|| format!("param({}x{})", shape.0, shape.1)),
        Expression::Add(l, r) => format!(
            "{} + {}",
            render_expression(l, registry),
            render_expression(r, registry)
        ),
        Expression::Sub(l, r) => format!(
            "{} - ({})",
            render_expression(l, registry),
            render_expression(r, registry)
        ),
        Expression::Mul(l, r) => format!(
            "({}) * ({})",
            render_expression(l, registry),
            render_expression(r, registry)
        ),
        Expression::Div(l, r) => format!(
            "({}) / ({})",
            render_expression(l, registry),
            render_expression(r, registry)
        ),
        Expression::Neg(e) => format!("-({})", render_expression(e, registry)),
        Expression::Scale { scalar, expr } => {
            format!("{scalar} * ({})", render_expression(expr, registry))
        }
        Expression::Sum(e) => format!("sum({})", render_expression(e, registry)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cvxrust::Variable;

    fn var(id: u64, shape: (usize, usize)) -> Expression {
        Expression::from_variable(Variable::new(id, shape))
    }

    #[test]
    fn infers_scalar_constant_shape() {
        assert_eq!(infer_shape(&Expression::constant(1.0)).unwrap(), (1, 1));
    }

    #[test]
    fn infers_bare_variable_shape() {
        assert_eq!(infer_shape(&var(1, (2, 3))).unwrap(), (2, 3));
    }

    #[test]
    fn infers_bare_parameter_shape() {
        let param = Expression::from_parameter(1, (3, 1), vec![1.0, 2.0, 3.0]);
        assert_eq!(infer_shape(&param).unwrap(), (3, 1));
    }

    #[test]
    fn broadcasts_scalar_against_non_scalar_operand() {
        let expr = Expression::add(Expression::constant(1.0), var(1, (2, 2)));
        assert_eq!(infer_shape(&expr).unwrap(), (2, 2));

        let expr = Expression::mul(var(1, (2, 2)), Expression::constant(2.0));
        assert_eq!(infer_shape(&expr).unwrap(), (2, 2));
    }

    #[test]
    fn requires_equal_shapes_for_non_scalar_operands() {
        let expr = Expression::add(var(1, (2, 2)), var(2, (2, 2)));
        assert_eq!(infer_shape(&expr).unwrap(), (2, 2));
    }

    #[test]
    fn rejects_mismatched_non_scalar_shapes() {
        let expr = Expression::add(var(1, (2, 2)), var(2, (3, 3)));
        let err = infer_shape(&expr).unwrap_err();
        assert_eq!(
            err,
            CvxError::InvalidExpression("shape mismatch: 2x2 vs 3x3".to_string())
        );
    }

    #[test]
    fn passes_through_neg_scale_sub_div() {
        let base = var(1, (2, 1));
        assert_eq!(infer_shape(&Expression::neg(base.clone())).unwrap(), (2, 1));
        assert_eq!(
            infer_shape(&Expression::scale(2.0, base.clone())).unwrap(),
            (2, 1)
        );
        assert_eq!(
            infer_shape(&Expression::sub(base.clone(), Expression::constant(1.0))).unwrap(),
            (2, 1)
        );
        assert_eq!(
            infer_shape(&Expression::div(base, Expression::constant(2.0))).unwrap(),
            (2, 1)
        );
    }

    #[test]
    fn sum_always_infers_a_scalar_shape() {
        assert_eq!(
            infer_shape(&Expression::sum(var(1, (3, 1)))).unwrap(),
            (1, 1)
        );
        assert_eq!(
            infer_shape(&Expression::sum(Expression::constant(1.0))).unwrap(),
            (1, 1)
        );
    }

    #[test]
    fn renders_each_node_kind() {
        let registry = Registry::new();
        assert_eq!(
            render_expression(&Expression::constant(1.5), &registry),
            "1.5"
        );
        assert_eq!(render_expression(&var(3, (1, 1)), &registry), "var#3");
        assert_eq!(
            render_expression(
                &Expression::from_parameter(2, (2, 2), vec![0.0; 4]),
                &registry
            ),
            "param(2x2)"
        );
        assert_eq!(
            render_expression(&Expression::neg(Expression::constant(1.0)), &registry),
            "-(1)"
        );
        assert_eq!(
            render_expression(
                &Expression::scale(2.0, Expression::constant(3.0)),
                &registry
            ),
            "2 * (3)"
        );
        assert_eq!(
            render_expression(&Expression::sum(var(3, (3, 1))), &registry),
            "sum(var#3)"
        );
    }

    #[test]
    fn renders_nested_combination() {
        let registry = Registry::new();
        let expr = Expression::add(
            Expression::mul(Expression::constant(2.0), var(1, (1, 1))),
            Expression::neg(Expression::constant(1.0)),
        );
        assert_eq!(render_expression(&expr, &registry), "(2) * (var#1) + -(1)");
    }

    #[test]
    fn renders_registered_variable_and_parameter_by_name() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("x".to_string()), (1, 1))
            .unwrap();
        let variable = registry.get_variable_by_name("x").unwrap().variable;
        let param_handle = registry
            .insert_parameter(Some("budget".to_string()), (1, 1), vec![1.0])
            .unwrap();
        let (_, param_uuid) = crate::core::handle::parse_handle(&param_handle).unwrap();

        let expr = Expression::from_variable(variable);
        assert_eq!(render_expression(&expr, &registry), "\"x\"");

        let param_expr = Expression::from_parameter(
            crate::core::registry::parameter_id(param_uuid),
            (1, 1),
            vec![1.0],
        );
        assert_eq!(render_expression(&param_expr, &registry), "\"budget\"");
    }

    #[test]
    fn renders_same_reference_consistently_when_repeated() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("x".to_string()), (1, 1))
            .unwrap();
        let variable = registry.get_variable_by_name("x").unwrap().variable;
        let expr = Expression::mul(
            Expression::from_variable(variable),
            Expression::from_variable(variable),
        );
        assert_eq!(render_expression(&expr, &registry), "(\"x\") * (\"x\")");
    }
}
