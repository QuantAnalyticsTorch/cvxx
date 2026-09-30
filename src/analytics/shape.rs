//! Diagnostic-only shape inference and expression rendering for
//! `CVX.DESCRIBE`/`CVX.SHAPE` (SPEC-0007). Read-only and side-effect-free;
//! never called during expression construction (SPEC-0004) or solving
//! (SPEC-0010).

use crate::core::error::CvxError;
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
/// parser; original Excel-facing identifier names are not recoverable, so
/// variables/parameters are shown structurally instead.
pub fn render_expression(expr: &Expression) -> String {
    match expr {
        Expression::Constant(c) => format!("{c}"),
        Expression::Variable(v) => format!("var#{}", v.id),
        Expression::Parameter { shape, .. } => format!("param({}x{})", shape.0, shape.1),
        Expression::Add(l, r) => format!("{} + {}", render_expression(l), render_expression(r)),
        Expression::Sub(l, r) => format!("{} - ({})", render_expression(l), render_expression(r)),
        Expression::Mul(l, r) => format!("({}) * ({})", render_expression(l), render_expression(r)),
        Expression::Div(l, r) => format!("({}) / ({})", render_expression(l), render_expression(r)),
        Expression::Neg(e) => format!("-({})", render_expression(e)),
        Expression::Scale { scalar, expr } => format!("{scalar} * ({})", render_expression(expr)),
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
        let param = Expression::from_parameter((3, 1), vec![1.0, 2.0, 3.0]);
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
    fn renders_each_node_kind() {
        assert_eq!(render_expression(&Expression::constant(1.5)), "1.5");
        assert_eq!(render_expression(&var(3, (1, 1))), "var#3");
        assert_eq!(
            render_expression(&Expression::from_parameter((2, 2), vec![0.0; 4])),
            "param(2x2)"
        );
        assert_eq!(
            render_expression(&Expression::neg(Expression::constant(1.0))),
            "-(1)"
        );
        assert_eq!(
            render_expression(&Expression::scale(2.0, Expression::constant(3.0))),
            "2 * (3)"
        );
    }

    #[test]
    fn renders_nested_combination() {
        let expr = Expression::add(
            Expression::mul(Expression::constant(2.0), var(1, (1, 1))),
            Expression::neg(Expression::constant(1.0)),
        );
        assert_eq!(render_expression(&expr), "(2) * (var#1) + -(1)");
    }
}
