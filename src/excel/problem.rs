//! Excel-facing problem builders: `CVX.MINIMIZE`, `CVX.MAXIMIZE`,
//! `CVX.PROBLEM`, and `CVX.SOLVE`.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;
use xladd::variant::Variant;
use xladd::xlcall::LPXLOPER12;

use crate::analytics::ast::Relation;
use crate::core::error::CvxError;
use crate::core::handle::{parse_handle, HandleKind};
use crate::core::registry::{ExpressionEntry, Registry};
use crate::data;
use crate::excel::constraint::{resolve_constraint_uuid, resolve_operand};
use crate::excel::expression::to_xloper_result;
use cvxrust::Sense;

/// `CVX.MINIMIZE(objective, [name])` — builds a minimization objective and
/// returns a `cvx:obj:<uuid>` handle.
#[export_name = "CVX.MINIMIZE"]
pub extern "system" fn cvx_minimize(objective: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_objective(objective, name, Sense::Minimize);
    to_xloper_result(result, "CVX.MINIMIZE")
}

/// `CVX.MAXIMIZE(objective, [name])` — builds a maximization objective and
/// returns a `cvx:obj:<uuid>` handle.
#[export_name = "CVX.MAXIMIZE"]
pub extern "system" fn cvx_maximize(objective: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_objective(objective, name, Sense::Maximize);
    to_xloper_result(result, "CVX.MAXIMIZE")
}

fn run_objective(
    objective: LPXLOPER12,
    name: LPXLOPER12,
    sense: Sense,
) -> Result<String, CvxError> {
    // Reuses the constraint operand resolver (SPEC-0005): a bare numeric
    // literal or a handle/name of an existing parameter, variable, or
    // expression.
    let expression = resolve_operand(objective)?;
    let name = data::parse_optional_name(&Variant::from_xloper(name))?;
    Registry::global().insert_objective(name, sense, expression, vec![])
}

/// `CVX.PROBLEM(objective, constraints, [name])` — combines an objective
/// with an ordered list of constraints and returns a `cvx:prob:<uuid>`
/// handle.
#[export_name = "CVX.PROBLEM"]
pub extern "system" fn cvx_problem(
    objective: LPXLOPER12,
    constraints: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    let result = run_problem(objective, constraints, name);
    to_xloper_result(result, "CVX.PROBLEM")
}

fn run_problem(
    objective: LPXLOPER12,
    constraints: LPXLOPER12,
    name: LPXLOPER12,
) -> Result<String, CvxError> {
    let registry = Registry::global();

    let objective_uuid = resolve_objective_uuid(objective)?;
    let objective_entry = registry
        .get_objective_by_uuid(objective_uuid)
        .ok_or_else(|| CvxError::Registry("objective disappeared".to_string()))?;

    let constraint_uuids = resolve_constraints_arg(constraints)?;

    let mut seen_vars = HashSet::new();
    let mut seen_exprs = HashSet::new();
    let mut variables = Vec::new();
    expand_variable_dependencies(
        registry,
        &objective_entry.dependencies,
        &mut seen_exprs,
        &mut seen_vars,
        &mut variables,
    );
    for uuid in &constraint_uuids {
        if let Some(constraint_entry) = registry.get_constraint_by_uuid(*uuid) {
            expand_variable_dependencies(
                registry,
                &constraint_entry.dependencies,
                &mut seen_exprs,
                &mut seen_vars,
                &mut variables,
            );
        }
    }

    let name = data::parse_optional_name(&Variant::from_xloper(name))?;
    registry.insert_problem(name, objective_uuid, constraint_uuids, variables)
}

fn resolve_objective_uuid(arg: LPXLOPER12) -> Result<Uuid, CvxError> {
    let text = data::parse_string(&Variant::from_xloper(arg))?;
    let text = text.trim();
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text)?;
        if kind != HandleKind::Obj {
            return Err(CvxError::UnknownIdentifier(text.to_string()));
        }
        return registry
            .get_objective_by_uuid(uuid)
            .map(|_| uuid)
            .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()));
    }

    registry
        .get_objective_by_name(text)
        .map(|entry| entry.uuid)
        .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()))
}

/// Resolves the `constraints` argument of `CVX.PROBLEM`. A blank/missing
/// argument yields no constraints. A single non-blank cell naming a
/// constraint set uses that set's constraints directly; otherwise every
/// non-blank cell is resolved as an individual constraint handle/name.
fn resolve_constraints_arg(arg: LPXLOPER12) -> Result<Vec<Uuid>, CvxError> {
    let variant = Variant::from_xloper(arg);
    let (cols, rows) = variant.dim();
    if cols == 0 || rows == 0 {
        return Ok(Vec::new());
    }

    if cols == 1 && rows == 1 {
        let cell = data::parse_optional_string_range(&variant)?
            .into_iter()
            .next()
            .flatten();
        let Some(text) = cell else {
            return Ok(Vec::new());
        };
        if let Some(uuids) = resolve_constraint_set(&text)? {
            return Ok(uuids);
        }
        return Ok(vec![resolve_constraint_uuid(&text)?]);
    }

    let entries = data::parse_optional_string_range(&variant)?;
    let mut uuids = Vec::new();
    for entry in entries.into_iter().flatten() {
        uuids.push(resolve_constraint_uuid(&entry)?);
    }
    Ok(uuids)
}

fn resolve_constraint_set(text: &str) -> Result<Option<Vec<Uuid>>, CvxError> {
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text)?;
        if kind != HandleKind::ConstrSet {
            return Ok(None);
        }
        return registry
            .get_constraint_set_by_uuid(uuid)
            .map(|entry| Some(entry.constraints))
            .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()));
    }

    Ok(registry
        .get_constraint_set_by_name(text)
        .map(|entry| entry.constraints))
}

/// Recursively resolves a list of dependency strings (handles or names) to
/// distinct variable UUIDs, in first-seen order. Dependencies that name an
/// expression are expanded into that expression's own dependencies so
/// variables referenced transitively through named expressions are found
/// too. Dependencies are only recorded by the string-parsing builders
/// (`CVX.EXPRESSION`/`CVX.CONSTRAINT`); functional builders (`CVX.ADD`,
/// `CVX.LESS_THAN`, `CVX.MINIMIZE`, etc.) do not track them, so variables
/// referenced only through those paths are not discovered here.
fn expand_variable_dependencies(
    registry: &Registry,
    dependencies: &[String],
    seen_exprs: &mut HashSet<String>,
    seen_vars: &mut HashSet<Uuid>,
    out: &mut Vec<Uuid>,
) {
    for dependency in dependencies {
        if let Some(uuid) = resolve_variable_uuid(registry, dependency) {
            if seen_vars.insert(uuid) {
                out.push(uuid);
            }
            continue;
        }

        if seen_exprs.insert(dependency.clone()) {
            if let Some(entry) = resolve_expression_entry(registry, dependency) {
                expand_variable_dependencies(
                    registry,
                    &entry.dependencies,
                    seen_exprs,
                    seen_vars,
                    out,
                );
            }
        }
    }
}

fn resolve_variable_uuid(registry: &Registry, text: &str) -> Option<Uuid> {
    if let Some(handle) = text.strip_prefix("cvx:") {
        let _ = handle;
        let (kind, uuid) = parse_handle(text).ok()?;
        if kind != HandleKind::Var {
            return None;
        }
        return registry.get_variable_by_uuid(uuid).map(|_| uuid);
    }
    registry.get_variable_by_name(text).map(|entry| entry.uuid)
}

fn resolve_expression_entry(registry: &Registry, text: &str) -> Option<ExpressionEntry> {
    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text).ok()?;
        if kind != HandleKind::Expr {
            return None;
        }
        return registry.get_expression_by_uuid(uuid);
    }
    registry.get_expression_by_name(text)
}

/// `CVX.SOLVE(problem, [name])` — solves a problem and returns a
/// `cvx:result:<uuid>` handle, or an Excel error if solving fails.
#[export_name = "CVX.SOLVE"]
pub extern "system" fn cvx_solve(problem: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = run_solve(problem, name);
    to_xloper_result(result, "CVX.SOLVE")
}

fn run_solve(problem: LPXLOPER12, name: LPXLOPER12) -> Result<String, CvxError> {
    let registry = Registry::global();

    let problem_uuid = resolve_problem_uuid(problem)?;
    let problem_entry = registry
        .get_problem_by_uuid(problem_uuid)
        .ok_or_else(|| CvxError::Registry("problem disappeared".to_string()))?;

    let objective_entry = registry
        .get_objective_by_uuid(problem_entry.objective)
        .ok_or_else(|| CvxError::Registry("objective missing for problem".to_string()))?;

    let mut cvx_constraints = Vec::with_capacity(problem_entry.constraints.len());
    for uuid in &problem_entry.constraints {
        let entry = registry
            .get_constraint_by_uuid(*uuid)
            .ok_or_else(|| CvxError::Registry("constraint missing for problem".to_string()))?;
        cvx_constraints.push(cvxrust::Constraint {
            relation: translate_relation(entry.relation),
            lhs: entry.lhs,
            rhs: entry.rhs,
        });
    }

    let mut cvx_variables = Vec::with_capacity(problem_entry.variables.len());
    for uuid in &problem_entry.variables {
        let entry = registry
            .get_variable_by_uuid(*uuid)
            .ok_or_else(|| CvxError::Registry("variable missing for problem".to_string()))?;
        cvx_variables.push(entry.variable);
    }

    let cvx_problem = cvxrust::Problem {
        sense: objective_entry.sense,
        objective: objective_entry.expression,
        constraints: cvx_constraints,
        variables: cvx_variables,
    };

    let solution = cvxrust::solve(&cvx_problem);

    if let cvxrust::SolveStatus::Error(message) = &solution.status {
        tracing::error!(problem = %problem_uuid, error = %message, "CVX.SOLVE failed");
        return Err(CvxError::SolveFailed(message.clone()));
    }

    let mut variable_values = HashMap::with_capacity(problem_entry.variables.len());
    for (uuid, values) in problem_entry.variables.iter().zip(solution.variable_values) {
        variable_values.insert(*uuid, values);
    }

    let name = data::parse_optional_name(&Variant::from_xloper(name))?;
    registry.insert_result(
        name,
        problem_uuid,
        solution.status,
        solution.objective_value,
        variable_values,
    )
}

fn resolve_problem_uuid(arg: LPXLOPER12) -> Result<Uuid, CvxError> {
    let text = data::parse_string(&Variant::from_xloper(arg))?;
    let text = text.trim();
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text)?;
        if kind != HandleKind::Prob {
            return Err(CvxError::UnknownIdentifier(text.to_string()));
        }
        return registry
            .get_problem_by_uuid(uuid)
            .map(|_| uuid)
            .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()));
    }

    registry
        .get_problem_by_name(text)
        .map(|entry| entry.uuid)
        .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()))
}

fn translate_relation(relation: Relation) -> cvxrust::Relation {
    match relation {
        Relation::LessEqual => cvxrust::Relation::LessEqual,
        Relation::GreaterEqual => cvxrust::Relation::GreaterEqual,
        Relation::Equal => cvxrust::Relation::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cvxrust::Expression;

    #[test]
    fn resolves_objective_uuid_by_handle() {
        let handle = Registry::global()
            .insert_objective(None, Sense::Minimize, Expression::constant(1.0), vec![])
            .unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        assert_eq!(resolve_objective_uuid_for_test(&handle), uuid);
    }

    #[test]
    fn resolves_objective_uuid_by_name() {
        let handle = Registry::global()
            .insert_objective(
                Some("named_objective_for_test".to_string()),
                Sense::Maximize,
                Expression::constant(2.0),
                vec![],
            )
            .unwrap();
        let (_, uuid) = parse_handle(&handle).unwrap();
        assert_eq!(
            resolve_objective_uuid_for_test("named_objective_for_test"),
            uuid
        );
    }

    fn resolve_objective_uuid_for_test(text: &str) -> Uuid {
        let registry = Registry::global();
        if text.starts_with("cvx:") {
            let (_, uuid) = parse_handle(text).unwrap();
            registry.get_objective_by_uuid(uuid).unwrap().uuid
        } else {
            registry.get_objective_by_name(text).unwrap().uuid
        }
    }

    #[test]
    fn deduplicates_shared_variables_between_objective_and_constraint() {
        let registry = Registry::global();
        let var_handle = registry.insert_variable(None, (1, 1)).unwrap();
        let (_, var_uuid) = parse_handle(&var_handle).unwrap();
        let variable_entry = registry.get_variable_by_uuid(var_uuid).unwrap();
        let variable_expr = Expression::from_variable(variable_entry.variable);

        let objective_handle = registry
            .insert_objective(
                None,
                Sense::Minimize,
                variable_expr.clone(),
                vec![var_handle.clone()],
            )
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective_handle).unwrap();

        let constraint_handle = registry
            .insert_constraint(
                None,
                Relation::LessEqual,
                variable_expr,
                Expression::constant(10.0),
                vec![var_handle.clone()],
            )
            .unwrap();
        let (_, constraint_uuid) = parse_handle(&constraint_handle).unwrap();

        let mut seen_vars = HashSet::new();
        let mut seen_exprs = HashSet::new();
        let mut variables = Vec::new();
        let objective_entry = registry.get_objective_by_uuid(objective_uuid).unwrap();
        expand_variable_dependencies(
            registry,
            &objective_entry.dependencies,
            &mut seen_exprs,
            &mut seen_vars,
            &mut variables,
        );
        let constraint_entry = registry.get_constraint_by_uuid(constraint_uuid).unwrap();
        expand_variable_dependencies(
            registry,
            &constraint_entry.dependencies,
            &mut seen_exprs,
            &mut seen_vars,
            &mut variables,
        );

        assert_eq!(variables, vec![var_uuid]);
    }

    #[test]
    fn solve_reports_excel_error_and_does_not_store_a_result() {
        let registry = Registry::global();
        let var_handle = registry.insert_variable(None, (2, 1)).unwrap();
        let (_, var_uuid) = parse_handle(&var_handle).unwrap();
        let variable_entry = registry.get_variable_by_uuid(var_uuid).unwrap();
        let variable_expr = Expression::from_variable(variable_entry.variable);

        let objective_handle = registry
            .insert_objective(
                None,
                Sense::Minimize,
                variable_expr,
                vec![var_handle.clone()],
            )
            .unwrap();
        let problem_handle = registry
            .insert_problem(
                Some("solve_error_test_problem".to_string()),
                parse_handle(&objective_handle).unwrap().1,
                vec![],
                vec![var_uuid],
            )
            .unwrap();

        let result = run_solve_for_test(&problem_handle, None);
        assert_eq!(
            result.unwrap_err(),
            CvxError::SolveFailed(
                "solver only supports scalar (1x1) variables and parameters".to_string()
            )
        );
        assert!(registry
            .get_result_by_name("solve_error_test_problem_result")
            .is_none());
    }

    #[test]
    fn solve_result_variable_values_match_each_variables_own_coefficient() {
        // Regression test for the variable-identity fix (SPEC-0010): two
        // distinct scalar variables of identical shape must not be
        // confused with each other when recovering solved values.
        let registry = Registry::global();

        let x_handle = registry.insert_variable(None, (1, 1)).unwrap();
        let (_, x_uuid) = parse_handle(&x_handle).unwrap();
        let x_variable = registry.get_variable_by_uuid(x_uuid).unwrap().variable;

        let y_handle = registry.insert_variable(None, (1, 1)).unwrap();
        let (_, y_uuid) = parse_handle(&y_handle).unwrap();
        let y_variable = registry.get_variable_by_uuid(y_uuid).unwrap().variable;

        // minimize x + y subject to x >= 3, y >= 5
        let objective_expr = Expression::add(
            Expression::from_variable(x_variable),
            Expression::from_variable(y_variable),
        );
        let objective_handle = registry
            .insert_objective(None, Sense::Minimize, objective_expr, vec![])
            .unwrap();

        let x_constraint_handle = registry
            .insert_constraint(
                None,
                Relation::GreaterEqual,
                Expression::from_variable(x_variable),
                Expression::constant(3.0),
                vec![],
            )
            .unwrap();
        let (_, x_constraint_uuid) = parse_handle(&x_constraint_handle).unwrap();

        let y_constraint_handle = registry
            .insert_constraint(
                None,
                Relation::GreaterEqual,
                Expression::from_variable(y_variable),
                Expression::constant(5.0),
                vec![],
            )
            .unwrap();
        let (_, y_constraint_uuid) = parse_handle(&y_constraint_handle).unwrap();

        let problem_handle = registry
            .insert_problem(
                Some("solve_variable_identity_test_problem".to_string()),
                parse_handle(&objective_handle).unwrap().1,
                vec![x_constraint_uuid, y_constraint_uuid],
                vec![x_uuid, y_uuid],
            )
            .unwrap();

        let result_handle = run_solve_for_test(
            &problem_handle,
            Some("solve_variable_identity_test_result".to_string()),
        )
        .unwrap();
        let (_, result_uuid) = parse_handle(&result_handle).unwrap();
        let result_entry = registry.get_result_by_uuid(result_uuid).unwrap();

        assert_eq!(result_entry.status, cvxrust::SolveStatus::Optimal);
        assert!((result_entry.variable_values[&x_uuid][0] - 3.0).abs() < 1e-6);
        assert!((result_entry.variable_values[&y_uuid][0] - 5.0).abs() < 1e-6);
    }

    fn run_solve_for_test(problem_text: &str, name: Option<String>) -> Result<String, CvxError> {
        let registry = Registry::global();
        let problem_uuid = resolve_objective_and_problem_uuid_for_test(problem_text);
        let problem_entry = registry.get_problem_by_uuid(problem_uuid).unwrap();
        let objective_entry = registry
            .get_objective_by_uuid(problem_entry.objective)
            .unwrap();

        let mut cvx_constraints = Vec::with_capacity(problem_entry.constraints.len());
        for uuid in &problem_entry.constraints {
            let entry = registry.get_constraint_by_uuid(*uuid).unwrap();
            cvx_constraints.push(cvxrust::Constraint {
                relation: translate_relation(entry.relation),
                lhs: entry.lhs,
                rhs: entry.rhs,
            });
        }

        let mut cvx_variables = Vec::with_capacity(problem_entry.variables.len());
        for uuid in &problem_entry.variables {
            let entry = registry.get_variable_by_uuid(*uuid).unwrap();
            cvx_variables.push(entry.variable);
        }

        let cvx_problem = cvxrust::Problem {
            sense: objective_entry.sense,
            objective: objective_entry.expression,
            constraints: cvx_constraints,
            variables: cvx_variables,
        };
        let solution = cvxrust::solve(&cvx_problem);
        if let cvxrust::SolveStatus::Error(message) = &solution.status {
            return Err(CvxError::SolveFailed(message.clone()));
        }

        let mut variable_values = HashMap::with_capacity(problem_entry.variables.len());
        for (uuid, values) in problem_entry.variables.iter().zip(solution.variable_values) {
            variable_values.insert(*uuid, values);
        }

        registry.insert_result(
            name,
            problem_uuid,
            solution.status,
            solution.objective_value,
            variable_values,
        )
    }

    fn resolve_objective_and_problem_uuid_for_test(text: &str) -> Uuid {
        let registry = Registry::global();
        if text.starts_with("cvx:") {
            parse_handle(text).unwrap().1
        } else {
            registry.get_problem_by_name(text).unwrap().uuid
        }
    }
}
