//! `CVX.INTEGER`/`CVX.BINARY` — declares an integer/binary domain
//! restriction over a variable, or a `CVX.INDEX` selection directly over a
//! variable, and returns a `cvx:dom:<uuid>` handle (SPEC-0019).

use uuid::Uuid;
use xladd::variant::Variant;
use xladd::xlcall::LPXLOPER12;

use crate::core::error::CvxError;
use crate::core::handle::{parse_handle, HandleKind};
use crate::core::registry::Registry;
use crate::data;
use crate::excel::expression::{resolve_handle_text, to_xloper_result};
use cvxrust::{Domain, DomainConstraint, Expression, Variable};

/// `CVX.INTEGER(variable, [name])` — restricts `variable`'s scalar
/// entries to integer values and returns a `cvx:dom:<uuid>` handle.
#[export_name = "CVX.INTEGER"]
pub extern "system" fn cvx_integer(variable: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_domain(variable, name, Domain::Integer);
    to_xloper_result(result, "CVX.INTEGER")
}

/// `CVX.BINARY(variable, [name])` — restricts `variable`'s scalar entries
/// to `0`/`1` and returns a `cvx:dom:<uuid>` handle.
#[export_name = "CVX.BINARY"]
pub extern "system" fn cvx_binary(variable: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_domain(variable, name, Domain::Binary);
    to_xloper_result(result, "CVX.BINARY")
}

fn run_domain(variable: LPXLOPER12, name: LPXLOPER12, domain: Domain) -> Result<String, CvxError> {
    let text = data::parse_string(&Variant::from_xloper(variable))?
        .trim()
        .to_string();
    let name = data::parse_optional_name(&Variant::from_xloper(name))?;

    let constraint = resolve_domain_operand(&text)?;
    Registry::global().insert_domain(
        name,
        DomainConstraint {
            domain,
            ..constraint
        },
    )
}

/// Resolves a `CVX.INTEGER`/`CVX.BINARY` `variable` operand to the
/// rectangular sub-block it restricts. Accepts a bare `cvx:var:`/name (the
/// whole variable) or a `cvx:expr:`/name whose stored expression is
/// exactly a plain `CVX.INDEX` selection directly over a bare variable, or
/// a bare variable expression; rejects numeric literals and every other
/// handle kind/expression shape.
fn resolve_domain_operand(text: &str) -> Result<DomainConstraint, CvxError> {
    // Reject numeric literals explicitly: `resolve_handle_text` would
    // otherwise reject them too (via `UnknownIdentifier`), but spelling it
    // out here keeps the "a domain restriction is meaningless without a
    // variable" rule obvious at the call site.
    if text.parse::<f64>().is_ok() {
        return Err(unsupported_operand_error());
    }

    let expr = resolve_handle_text(text)?;
    domain_constraint_from_expression(expr)
}

fn domain_constraint_from_expression(expr: Expression) -> Result<DomainConstraint, CvxError> {
    match expr {
        Expression::Variable(v) => Ok(whole_variable_domain(v)),
        Expression::Index {
            expr: inner,
            row_start,
            col_start,
            rows,
            cols,
        } => match *inner {
            Expression::Variable(v) => Ok(DomainConstraint {
                variable: v,
                row_start,
                col_start,
                rows,
                cols,
                domain: Domain::Integer, // overwritten by the caller
            }),
            _ => Err(unsupported_operand_error()),
        },
        _ => Err(unsupported_operand_error()),
    }
}

fn whole_variable_domain(variable: Variable) -> DomainConstraint {
    let (rows, cols) = variable.shape;
    DomainConstraint {
        variable,
        row_start: 0,
        col_start: 0,
        rows,
        cols,
        domain: Domain::Integer, // overwritten by the caller
    }
}

fn unsupported_operand_error() -> CvxError {
    CvxError::InvalidExpression(
        "CVX.INTEGER/CVX.BINARY can only restrict a variable or a CVX.INDEX selection directly over a variable"
            .to_string(),
    )
}

/// Resolves a `CVX.CONSTRAINTS`/`CVX.PROBLEM` member reference to a domain
/// UUID, mirroring `resolve_constraint_uuid` in `excel::constraint`.
pub(crate) fn resolve_domain_uuid(text: &str) -> Result<Uuid, CvxError> {
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text)?;
        if kind != HandleKind::Dom {
            return Err(CvxError::UnknownIdentifier(text.to_string()));
        }
        return registry
            .get_domain_by_uuid(uuid)
            .map(|_| uuid)
            .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()));
    }

    registry
        .get_domain_by_name(text)
        .map(|entry| entry.uuid)
        .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::handle::parse_handle;

    fn fresh_variable(shape: (usize, usize)) -> (String, Variable) {
        let handle = Registry::global().insert_variable(None, shape).unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        let entry = Registry::global().get_variable_by_uuid(uuid).unwrap();
        (handle, entry.variable)
    }

    #[test]
    fn builds_integer_domain_over_whole_variable() {
        let (handle, variable) = fresh_variable((2, 3));
        let dom_expr = resolve_handle_text(&handle);
        assert!(dom_expr.is_ok());

        let constraint = domain_constraint_from_expression(dom_expr.unwrap()).unwrap();
        assert_eq!(constraint.variable, variable);
        assert_eq!((constraint.row_start, constraint.col_start), (0, 0));
        assert_eq!((constraint.rows, constraint.cols), (2, 3));
    }

    #[test]
    fn rejects_numeric_literal_operand() {
        assert!(resolve_domain_operand("1.5").is_err());
    }

    #[test]
    fn rejects_unsupported_expression_shape() {
        let (handle, _) = fresh_variable((1, 1));
        let expr_handle = Registry::global()
            .insert_expression(
                None,
                Expression::sum(resolve_handle_text(&handle).unwrap()),
                vec![],
            )
            .unwrap();
        assert!(resolve_domain_operand(&expr_handle).is_err());
    }

    #[test]
    fn resolves_domain_uuid_by_handle() {
        let (handle, _) = fresh_variable((1, 1));
        let dom_handle = Registry::global()
            .insert_domain(
                None,
                domain_constraint_from_expression(resolve_handle_text(&handle).unwrap())
                    .map(|c| DomainConstraint {
                        domain: Domain::Integer,
                        ..c
                    })
                    .unwrap(),
            )
            .unwrap();
        let (_, uuid) = parse_handle(&dom_handle).unwrap();
        assert_eq!(resolve_domain_uuid(&dom_handle).unwrap(), uuid);
    }

    #[test]
    fn rejects_non_domain_handle() {
        let handle = Registry::global()
            .insert_parameter(None, (1, 1), vec![1.0])
            .unwrap();
        assert!(resolve_domain_uuid(&handle).is_err());
    }
}
