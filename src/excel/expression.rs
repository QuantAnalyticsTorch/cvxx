//! Excel-facing expression builders: `CVX.EXPRESSION` and functional helpers.

use xladd::variant::Variant;
use xladd::xlcall::LPXLOPER12;

use crate::analytics::parser;
use crate::analytics::resolve::resolve_expr;
use crate::core::error::CvxError;
use crate::core::handle::{parse_handle, HandleKind};
use crate::core::registry::Registry;
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
                .map(|e| Expression::from_parameter(e.shape, e.data))
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
            return Ok(Expression::from_parameter(entry.shape, entry.data));
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
        assert_eq!(expr, Expression::from_parameter((1, 1), vec![5.0]));
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
                .map(|e| Expression::from_parameter(e.shape, e.data))
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
}
