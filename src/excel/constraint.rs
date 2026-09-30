//! Excel-facing constraint builders: `CVX.CONSTRAINT`, the relational
//! functional builders, and `CVX.CONSTRAINTS`.

use uuid::Uuid;
use xladd::variant::Variant;
use xladd::xlcall::LPXLOPER12;

use crate::analytics::ast::Relation;
use crate::analytics::parser;
use crate::analytics::resolve::resolve_constraint;
use crate::core::error::CvxError;
use crate::core::handle::{parse_handle, HandleKind};
use crate::core::registry::Registry;
use crate::data;
use crate::excel::expression::{resolve_handle_arg, to_xloper_result};
use cvxrust::Expression;

/// `CVX.CONSTRAINT(constraint_string, [name])` — parses a constraint
/// string, resolves identifiers against the registry, and returns a
/// `cvx:constr:<uuid>` handle.
#[export_name = "CVX.CONSTRAINT"]
pub extern "system" fn cvx_constraint(constraint: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_constraint(constraint, name);
    to_xloper_result(result, "CVX.CONSTRAINT")
}

fn run_constraint(constraint: LPXLOPER12, name: LPXLOPER12) -> Result<String, CvxError> {
    let text = data::parse_string(&Variant::from_xloper(constraint))?;
    let name = data::parse_optional_name(&Variant::from_xloper(name))?;

    let ast = parser::parse_constraint(&text)?;
    let registry = Registry::global();
    let resolved = resolve_constraint(registry, &ast)?;

    registry.insert_constraint(
        name,
        resolved.relation,
        resolved.lhs,
        resolved.rhs,
        resolved.dependencies,
    )
}

/// `CVX.LESS_THAN(left, right, [name])` — builds `left <= right`.
#[export_name = "CVX.LESS_THAN"]
pub extern "system" fn cvx_less_than(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    run_relation(left, right, name, Relation::LessEqual, "CVX.LESS_THAN")
}

/// `CVX.GREATER_THAN(left, right, [name])` — builds `left >= right`.
#[export_name = "CVX.GREATER_THAN"]
pub extern "system" fn cvx_greater_than(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    run_relation(
        left,
        right,
        name,
        Relation::GreaterEqual,
        "CVX.GREATER_THAN",
    )
}

/// `CVX.EQUAL(left, right, [name])` — builds `left == right`.
#[export_name = "CVX.EQUAL"]
pub extern "system" fn cvx_equal(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    run_relation(left, right, name, Relation::Equal, "CVX.EQUAL")
}

fn run_relation(
    left: LPXLOPER12,
    right: LPXLOPER12,
    name: LPXLOPER12,
    relation: Relation,
    context: &str,
) -> LPXLOPER12 {
    let result = (|| {
        let lhs = resolve_operand(left)?;
        let rhs = resolve_operand(right)?;
        let name = data::parse_optional_name(&Variant::from_xloper(name))?;
        Registry::global().insert_constraint(name, relation, lhs, rhs, vec![])
    })();

    to_xloper_result(result, context)
}

/// Resolves a constraint operand, which may be a bare numeric literal or a
/// handle/name of an existing parameter, variable, or expression.
pub(crate) fn resolve_operand(arg: LPXLOPER12) -> Result<Expression, CvxError> {
    let variant = Variant::from_xloper(arg);
    if let Ok(value) = data::parse_scalar(&variant) {
        return Ok(Expression::constant(value));
    }
    resolve_handle_arg(arg)
}

/// `CVX.CONSTRAINTS(constraints, [name])` — flattens a range of constraint
/// handles/names into an ordered constraint set and returns a
/// `cvx:constrset:<uuid>` handle.
#[export_name = "CVX.CONSTRAINTS"]
pub extern "system" fn cvx_constraints(constraints: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_constraints(constraints, name);
    to_xloper_result(result, "CVX.CONSTRAINTS")
}

fn run_constraints(constraints: LPXLOPER12, name: LPXLOPER12) -> Result<String, CvxError> {
    let range = Variant::from_xloper(constraints);
    let entries = data::parse_optional_string_range(&range)?;
    let name = data::parse_optional_name(&Variant::from_xloper(name))?;

    let mut uuids = Vec::new();
    for entry in entries.into_iter().flatten() {
        uuids.push(resolve_constraint_uuid(&entry)?);
    }

    if uuids.is_empty() {
        return Err(CvxError::EmptyRange);
    }

    Registry::global().insert_constraint_set(name, uuids)
}

pub(crate) fn resolve_constraint_uuid(text: &str) -> Result<Uuid, CvxError> {
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text)?;
        if kind != HandleKind::Constr {
            return Err(CvxError::UnknownIdentifier(text.to_string()));
        }
        return registry
            .get_constraint_by_uuid(uuid)
            .map(|_| uuid)
            .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()));
    }

    registry
        .get_constraint_by_name(text)
        .map(|entry| entry.uuid)
        .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_constraint_uuid_by_handle() {
        let handle = Registry::global()
            .insert_constraint(
                None,
                Relation::LessEqual,
                Expression::constant(1.0),
                Expression::constant(2.0),
                vec![],
            )
            .unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        assert_eq!(resolve_constraint_uuid(&handle).unwrap(), uuid);
    }

    #[test]
    fn resolves_constraint_uuid_by_name() {
        let handle = Registry::global()
            .insert_constraint(
                Some("named_constraint_for_test".to_string()),
                Relation::Equal,
                Expression::constant(1.0),
                Expression::constant(1.0),
                vec![],
            )
            .unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        assert_eq!(
            resolve_constraint_uuid("named_constraint_for_test").unwrap(),
            uuid
        );
    }

    #[test]
    fn rejects_unknown_constraint_reference() {
        assert!(resolve_constraint_uuid("no_such_constraint").is_err());
    }

    #[test]
    fn rejects_non_constraint_handle() {
        let handle = Registry::global()
            .insert_parameter(None, (1, 1), vec![1.0])
            .unwrap();
        assert!(resolve_constraint_uuid(&handle).is_err());
    }
}
