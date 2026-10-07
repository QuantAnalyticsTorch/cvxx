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
            Expr::MatMul(left, right) => {
                let l = self.resolve(left)?;
                let r = self.resolve(right)?;
                Ok(Expression::matmul(l, r))
            }
            Expr::Transpose(operand) => {
                let expr = self.resolve(operand)?;
                Ok(Expression::transpose(expr))
            }
            Expr::Call { name, args } => self.resolve_call(name, args),
        }
    }

    /// Resolves a function-call-syntax node. Only `sum`/`index` are
    /// recognized (SPEC-0015); any other name is an error.
    fn resolve_call(&mut self, name: &str, args: &[ExprNode]) -> Result<Expression, CvxError> {
        match name {
            "sum" => {
                if args.len() != 1 {
                    return Err(CvxError::InvalidExpression(format!(
                        "sum() takes exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let operand = self.resolve(&args[0])?;
                Ok(Expression::sum(operand))
            }
            "index" => {
                if args.len() != 3 && args.len() != 5 {
                    return Err(CvxError::InvalidExpression(format!(
                        "index() takes exactly 3 or 5 arguments, got {}",
                        args.len()
                    )));
                }
                let operand = self.resolve(&args[0])?;
                let row = literal_dimension(&args[1])?;
                let col = literal_dimension(&args[2])?;
                let rows = if args.len() == 5 {
                    literal_dimension(&args[3])?
                } else {
                    1
                };
                let cols = if args.len() == 5 {
                    literal_dimension(&args[4])?
                } else {
                    1
                };
                let operand_shape = crate::analytics::shape::infer_shape(&operand)?;
                crate::analytics::shape::check_index_bounds(operand_shape, row, col, rows, cols)?;
                Ok(Expression::index(operand, row - 1, col - 1, rows, cols))
            }
            other => Err(CvxError::InvalidExpression(format!(
                "unknown function '{other}'"
            ))),
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
            | HandleKind::Result
            | HandleKind::Dom => Err(CvxError::UnknownIdentifier(handle.to_string())),
        }
    }
}

/// Reads a bare numeric-literal argument (e.g. `index()`'s `row`/`col`/
/// `rows`/`cols`) as a positive integer. Identifiers, arithmetic, and
/// non-integer or non-positive numbers are all rejected — these arguments
/// are positions/counts, never registry references, mirroring
/// `CVX.INDEX`'s own `row`/`col`/`rows`/`cols` (`data::parse_dimension`,
/// SPEC-0015).
fn literal_dimension(node: &ExprNode) -> Result<usize, CvxError> {
    match node.as_ref() {
        Expr::Constant(value) if value.fract() == 0.0 && *value >= 1.0 => Ok(*value as usize),
        Expr::Constant(_) => Err(CvxError::InvalidExpression(
            "index() row/col/rows/cols arguments must be positive integers".to_string(),
        )),
        _ => Err(CvxError::InvalidExpression(
            "index() row/col/rows/cols arguments must be numeric literals".to_string(),
        )),
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

    // --- sum(...)/index(...) grammar tests (SPEC-0015) ---

    #[test]
    fn resolves_sum_call_to_the_same_expression_as_the_functional_builder() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("v".to_string()), (3, 1))
            .unwrap();
        let variable = registry.get_variable_by_name("v").unwrap().variable;

        let ast = parse("sum(v)").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::sum(Expression::from_variable(variable))
        );
        assert_eq!(resolved.dependencies, vec!["v".to_string()]);
    }

    #[test]
    fn resolves_index_call_with_three_arguments_to_the_same_expression_as_the_functional_builder() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("v".to_string()), (3, 1))
            .unwrap();
        let variable = registry.get_variable_by_name("v").unwrap().variable;

        let ast = parse("index(v, 2, 1)").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::index(Expression::from_variable(variable), 1, 0, 1, 1)
        );
        assert_eq!(resolved.dependencies, vec!["v".to_string()]);
    }

    #[test]
    fn resolves_index_call_with_five_arguments() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("M".to_string()), (3, 3))
            .unwrap();
        let variable = registry.get_variable_by_name("M").unwrap().variable;

        let ast = parse("index(M, 2, 2, 2, 2)").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::index(Expression::from_variable(variable), 1, 1, 2, 2)
        );
    }

    #[test]
    fn resolves_nested_sum_of_index() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("X".to_string()), (3, 3))
            .unwrap();
        let variable = registry.get_variable_by_name("X").unwrap().variable;

        let ast = parse("sum(index(X, 1, 1, 2, 2))").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::sum(Expression::index(
                Expression::from_variable(variable),
                0,
                0,
                2,
                2
            ))
        );
        assert_eq!(resolved.dependencies, vec!["X".to_string()]);
    }

    #[test]
    fn rejects_unknown_function_name() {
        let registry = Registry::new();
        let ast = parse("foo(x)").unwrap();
        assert_eq!(
            resolve_expr(&registry, &ast).unwrap_err(),
            CvxError::InvalidExpression("unknown function 'foo'".to_string())
        );
    }

    #[test]
    fn rejects_sum_with_wrong_arity() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("v".to_string()), (1, 1))
            .unwrap();
        let ast = parse("sum(v, v)").unwrap();
        assert_eq!(
            resolve_expr(&registry, &ast).unwrap_err(),
            CvxError::InvalidExpression("sum() takes exactly 1 argument, got 2".to_string())
        );
    }

    #[test]
    fn rejects_index_with_wrong_arity() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("v".to_string()), (1, 1))
            .unwrap();
        let ast = parse("index(v, 1, 1, 1)").unwrap();
        assert_eq!(
            resolve_expr(&registry, &ast).unwrap_err(),
            CvxError::InvalidExpression(
                "index() takes exactly 3 or 5 arguments, got 4".to_string()
            )
        );
    }

    #[test]
    fn rejects_index_with_non_literal_row_argument() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("v".to_string()), (1, 1))
            .unwrap();
        let ast = parse("index(v, 1 + 1, 1)").unwrap();
        assert_eq!(
            resolve_expr(&registry, &ast).unwrap_err(),
            CvxError::InvalidExpression(
                "index() row/col/rows/cols arguments must be numeric literals".to_string()
            )
        );
    }

    #[test]
    fn rejects_index_with_non_positive_integer_row_argument() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("v".to_string()), (1, 1))
            .unwrap();
        let ast = parse("index(v, 0, 1)").unwrap();
        assert_eq!(
            resolve_expr(&registry, &ast).unwrap_err(),
            CvxError::InvalidExpression(
                "index() row/col/rows/cols arguments must be positive integers".to_string()
            )
        );
    }

    #[test]
    fn rejects_out_of_bounds_index_call() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("v".to_string()), (2, 2))
            .unwrap();
        let ast = parse("index(v, 2, 2, 2, 2)").unwrap();
        assert_eq!(
            resolve_expr(&registry, &ast).unwrap_err(),
            CvxError::InvalidExpression(
                "requested rows 2..3 and columns 2..3 are out of bounds for a 2x2 operand"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_unresolvable_identifier_nested_inside_a_call() {
        let registry = Registry::new();
        let ast = parse("sum(missing_name)").unwrap();
        assert_eq!(
            resolve_expr(&registry, &ast).unwrap_err(),
            CvxError::UnknownIdentifier("missing_name".to_string())
        );
    }

    // --- `@`/`.T` grammar resolution tests (SPEC-0018) ---

    #[test]
    fn resolves_matmul_operator_to_the_same_expression_as_the_functional_builder() {
        let registry = Registry::new();
        registry
            .insert_parameter(Some("W".to_string()), (1, 3), vec![1.0, 2.0, 3.0])
            .unwrap();
        registry
            .insert_variable(Some("x".to_string()), (3, 1))
            .unwrap();
        let param_uuid = registry.get_parameter_by_name("W").unwrap().uuid;
        let variable = registry.get_variable_by_name("x").unwrap().variable;

        let ast = parse("W @ x").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::matmul(
                Expression::from_parameter(parameter_id(param_uuid), (1, 3), vec![1.0, 2.0, 3.0]),
                Expression::from_variable(variable)
            )
        );
        let mut deps = resolved.dependencies;
        deps.sort();
        assert_eq!(deps, vec!["W".to_string(), "x".to_string()]);
    }

    #[test]
    fn resolves_transpose_postfix_to_the_same_expression_as_the_functional_builder() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("x".to_string()), (3, 1))
            .unwrap();
        let variable = registry.get_variable_by_name("x").unwrap().variable;

        let ast = parse("x.T").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::transpose(Expression::from_variable(variable))
        );
        assert_eq!(resolved.dependencies, vec!["x".to_string()]);
    }

    #[test]
    fn resolves_nested_matmul_and_transpose_combination_and_tracks_every_dependency() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("w".to_string()), (2, 1))
            .unwrap();
        registry
            .insert_parameter(Some("Sigma".to_string()), (2, 2), vec![1.0, 0.0, 0.0, 1.0])
            .unwrap();
        let w = registry.get_variable_by_name("w").unwrap().variable;
        let sigma_uuid = registry.get_parameter_by_name("Sigma").unwrap().uuid;

        let ast = parse("w.T @ Sigma @ w").unwrap();
        let resolved = resolve_expr(&registry, &ast).unwrap();

        assert_eq!(
            resolved.expression,
            Expression::matmul(
                Expression::matmul(
                    Expression::transpose(Expression::from_variable(w)),
                    Expression::from_parameter(
                        parameter_id(sigma_uuid),
                        (2, 2),
                        vec![1.0, 0.0, 0.0, 1.0]
                    )
                ),
                Expression::from_variable(w)
            )
        );
        let mut deps = resolved.dependencies;
        deps.sort();
        assert_eq!(deps, vec!["Sigma".to_string(), "w".to_string()]);
    }

    #[test]
    fn rejects_unresolvable_identifier_nested_inside_matmul_or_transpose() {
        let registry = Registry::new();
        assert_eq!(
            resolve_expr(&registry, &parse("missing @ 1").unwrap()).unwrap_err(),
            CvxError::UnknownIdentifier("missing".to_string())
        );
        assert_eq!(
            resolve_expr(&registry, &parse("missing.T").unwrap()).unwrap_err(),
            CvxError::UnknownIdentifier("missing".to_string())
        );
    }
}
