//! Name resolution: convert the user AST into a `cvxrust::Expression` by
//! looking up identifiers in the registry.

use std::collections::HashSet;

use crate::analytics::ast::{Constraint, Expr, ExprNode, Relation};
use crate::core::error::CvxError;
use crate::core::handle::{parse_handle, HandleKind};
use crate::core::registry::{parameter_id, Registry};
use cvxrust::Expression;

/// A resolved expression together with the registry handles it depends on.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedExpr {
    pub expression: Expression,
    pub dependencies: Vec<String>,
}

/// A resolved constraint together with the registry handles it depends on.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedConstraint {
    pub relation: Relation,
    pub lhs: Expression,
    pub rhs: Expression,
    pub dependencies: Vec<String>,
}

/// Resolves an AST against the registry. Identifiers may be:
/// - names of registered parameters, variables, or expressions;
/// - literal handles of the form `cvx:<kind>:<uuid>`.
///
/// Numeric constants are left as constants. Named expressions referenced by
/// name are inlined (their AST is resolved recursively), but their handles
/// are recorded as dependencies.
pub fn resolve_expr(registry: &Registry, ast: &ExprNode) -> Result<ResolvedExpr, CvxError> {
    let mut resolver = Resolver::new(registry);
    let expression = resolver.resolve(ast)?;
    let dependencies = resolver.dependencies.into_iter().collect();
    Ok(ResolvedExpr {
        expression,
        dependencies,
    })
}

/// Resolves both operands of a parsed constraint against the registry,
/// using the same identifier resolution rules as [`resolve_expr`].
pub fn resolve_constraint(
    registry: &Registry,
    constraint: &Constraint,
) -> Result<ResolvedConstraint, CvxError> {
    let mut resolver = Resolver::new(registry);
    let lhs = resolver.resolve(&constraint.lhs)?;
    let rhs = resolver.resolve(&constraint.rhs)?;
    let dependencies = resolver.dependencies.into_iter().collect();
    Ok(ResolvedConstraint {
        relation: constraint.relation,
        lhs,
        rhs,
        dependencies,
    })
}

struct Resolver<'a> {
    registry: &'a Registry,
    dependencies: HashSet<String>,
}

impl<'a> Resolver<'a> {
    fn new(registry: &'a Registry) -> Self {
        Resolver {
            registry,
            dependencies: HashSet::new(),
        }
    }

    fn resolve(&mut self, node: &ExprNode) -> Result<Expression, CvxError> {
        match node.as_ref() {
            Expr::Constant(value) => Ok(Expression::constant(*value)),
            Expr::Identifier(name) => self.resolve_identifier(name),
            Expr::Add(left, right) => {
                let l = self.resolve(left)?;
                let r = self.resolve(right)?;
                Ok(Expression::add(l, r))
            }
            Expr::Sub(left, right) => {
                let l = self.resolve(left)?;
                let r = self.resolve(right)?;
                Ok(Expression::sub(l, r))
            }
            Expr::Mul(left, right) => {
                let l = self.resolve(left)?;
                let r = self.resolve(right)?;
                Ok(Expression::mul(l, r))
            }
            Expr::Div(left, right) => {
                let l = self.resolve(left)?;
                let r = self.resolve(right)?;
                Ok(Expression::div(l, r))
            }
            Expr::Neg(operand) => {
                let expr = self.resolve(operand)?;
                Ok(Expression::neg(expr))
            }
        }
    }

    fn resolve_identifier(&mut self, name: &str) -> Result<Expression, CvxError> {
        // If the identifier looks like a handle, try to parse it first.
        if name.starts_with("cvx:") {
            return self.resolve_handle(name);
        }

        // Otherwise look up by name in parameter, variable, then expression
        // tables.
        if let Some(entry) = self.registry.get_parameter_by_name(name) {
            self.dependencies.insert(name.to_string());
            return Ok(Expression::from_parameter(
                parameter_id(entry.uuid),
                entry.shape,
                entry.data,
            ));
        }

        if let Some(entry) = self.registry.get_variable_by_name(name) {
            self.dependencies.insert(name.to_string());
            return Ok(Expression::from_variable(entry.variable));
        }

        if let Some(entry) = self.registry.get_expression_by_name(name) {
            self.dependencies.insert(name.to_string());
            return Ok(entry.expression);
        }

        Err(CvxError::UnknownIdentifier(name.to_string()))
    }

    fn resolve_handle(&mut self, handle: &str) -> Result<Expression, CvxError> {
        let (kind, uuid) = parse_handle(handle)?;
        match kind {
            HandleKind::Param => {
                let entry = self
                    .registry
                    .get_parameter_by_uuid(uuid)
                    .ok_or_else(|| CvxError::UnknownIdentifier(handle.to_string()))?;
                self.dependencies.insert(handle.to_string());
                Ok(Expression::from_parameter(
                    parameter_id(entry.uuid),
                    entry.shape,
                    entry.data,
                ))
            }
            HandleKind::Var => {
                let entry = self
                    .registry
                    .get_variable_by_uuid(uuid)
                    .ok_or_else(|| CvxError::UnknownIdentifier(handle.to_string()))?;
                self.dependencies.insert(handle.to_string());
                Ok(Expression::from_variable(entry.variable))
            }
            HandleKind::Expr => {
                let entry = self
                    .registry
                    .get_expression_by_uuid(uuid)
                    .ok_or_else(|| CvxError::UnknownIdentifier(handle.to_string()))?;
                self.dependencies.insert(handle.to_string());
                Ok(entry.expression)
            }
            HandleKind::Constr
            | HandleKind::ConstrSet
            | HandleKind::Obj
            | HandleKind::Prob
            | HandleKind::Result => Err(CvxError::UnknownIdentifier(handle.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::parser::parse;

    #[test]
    fn resolves_parameter_by_name() {
        let registry = Registry::new();
        registry
            .insert_parameter(Some("A".to_string()), (1, 1), vec![2.0])
            .unwrap();
        let param_uuid = registry.get_parameter_by_name("A").unwrap().uuid;

        let ast = parse("A").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::from_parameter(parameter_id(param_uuid), (1, 1), vec![2.0])
        );
        assert_eq!(resolved.dependencies, vec!["A".to_string()]);
    }

    #[test]
    fn resolves_variable_by_name() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("x".to_string()), (3, 1))
            .unwrap();
        let variable = registry.get_variable_by_name("x").unwrap().variable;

        let ast = parse("x").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(resolved.expression, Expression::from_variable(variable));
    }

    #[test]
    fn resolves_arithmetic_expression() {
        let registry = Registry::new();
        registry
            .insert_parameter(Some("A".to_string()), (1, 1), vec![1.0])
            .unwrap();
        registry
            .insert_variable(Some("x".to_string()), (1, 1))
            .unwrap();
        let variable = registry.get_variable_by_name("x").unwrap().variable;
        let param_uuid = registry.get_parameter_by_name("A").unwrap().uuid;

        let ast = parse("2.5 * (A - x)").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::mul(
                Expression::constant(2.5),
                Expression::sub(
                    Expression::from_parameter(parameter_id(param_uuid), (1, 1), vec![1.0]),
                    Expression::from_variable(variable)
                )
            )
        );
    }

    #[test]
    fn rejects_unknown_identifier() {
        let registry = Registry::new();
        let ast = parse("foo").unwrap();
        assert_eq!(
            resolve_expr(&registry, &ast).unwrap_err(),
            CvxError::UnknownIdentifier("foo".to_string())
        );
    }

    #[test]
    fn resolves_constraint_operands_and_dependencies() {
        use crate::analytics::parser::parse_constraint;

        let registry = Registry::new();
        registry
            .insert_variable(Some("x".to_string()), (1, 1))
            .unwrap();
        registry
            .insert_parameter(Some("b".to_string()), (1, 1), vec![10.0])
            .unwrap();
        let variable = registry.get_variable_by_name("x").unwrap().variable;
        let param_uuid = registry.get_parameter_by_name("b").unwrap().uuid;

        let constraint = parse_constraint("x <= b").unwrap();
        let resolved = resolve_constraint(&registry, &constraint).unwrap();

        assert_eq!(resolved.relation, Relation::LessEqual);
        assert_eq!(resolved.lhs, Expression::from_variable(variable));
        assert_eq!(
            resolved.rhs,
            Expression::from_parameter(parameter_id(param_uuid), (1, 1), vec![10.0])
        );
        let mut deps = resolved.dependencies;
        deps.sort();
        assert_eq!(deps, vec!["b".to_string(), "x".to_string()]);
    }

    #[test]
    fn rejects_constraint_with_unknown_identifier() {
        use crate::analytics::parser::parse_constraint;

        let registry = Registry::new();
        let constraint = parse_constraint("foo <= 1").unwrap();
        assert_eq!(
            resolve_constraint(&registry, &constraint).unwrap_err(),
            CvxError::UnknownIdentifier("foo".to_string())
        );
    }
}
