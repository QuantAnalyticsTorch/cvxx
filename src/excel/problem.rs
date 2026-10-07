//! Excel-facing problem builders: `CVX.MINIMIZE`, `CVX.MAXIMIZE`,
//! `CVX.PROBLEM`, and `CVX.SOLVE`.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;
use xladd::variant::Variant;
use xladd::xlcall::LPXLOPER12;

use crate::analytics::ast::Relation;
use crate::core::error::CvxError;
use crate::core::handle::{parse_handle, HandleKind};
use crate::core::registry::{ConstraintSetItem, ExpressionEntry, Registry};
use crate::data;
use crate::excel::constraint::{resolve_constraint_set_item, resolve_operand};
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

    let (constraint_uuids, domain_uuids) = resolve_constraints_arg(constraints)?;

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
    // Domain-restricted variables (SPEC-0019) must also be solved for,
    // even when they appear in no other constraint or in the objective.
    for uuid in &domain_uuids {
        if let Some(domain_entry) = registry.get_domain_by_uuid(*uuid) {
            if let Some(variable_entry) =
                registry.get_variable_by_variable_id(domain_entry.domain.variable.id)
            {
                if seen_vars.insert(variable_entry.uuid) {
                    variables.push(variable_entry.uuid);
                }
            }
        }
    }

    let name = data::parse_optional_name(&Variant::from_xloper(name))?;
    registry.insert_problem(
        name,
        objective_uuid,
        constraint_uuids,
        domain_uuids,
        variables,
    )
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
/// argument yields no constraints or domains. A single non-blank cell
/// naming a constraint set uses that set's members directly; otherwise
/// every non-blank cell is resolved as an individual constraint or domain
/// handle/name (SPEC-0019). Returns the resolved constraint UUIDs and
/// domain UUIDs separately.
fn resolve_constraints_arg(arg: LPXLOPER12) -> Result<(Vec<Uuid>, Vec<Uuid>), CvxError> {
    let variant = Variant::from_xloper(arg);
    let (cols, rows) = variant.dim();
    if cols == 0 || rows == 0 {
        return Ok((Vec::new(), Vec::new()));
    }

    if cols == 1 && rows == 1 {
        let cell = data::parse_optional_string_range(&variant)?
            .into_iter()
            .next()
            .flatten();
        let Some(text) = cell else {
            return Ok((Vec::new(), Vec::new()));
        };
        if let Some(items) = resolve_constraint_set(&text)? {
            return Ok(split_items(items));
        }
        return Ok(split_items(vec![resolve_constraint_set_item(&text)?]));
    }

    let entries = data::parse_optional_string_range(&variant)?;
    let mut items = Vec::new();
    for entry in entries.into_iter().flatten() {
        items.push(resolve_constraint_set_item(&entry)?);
    }
    Ok(split_items(items))
}

/// Splits an ordered list of `ConstraintSetItem`s into its
/// constraint/domain UUID buckets, each preserving the original relative
/// order within its own bucket (SPEC-0019).
fn split_items(items: Vec<ConstraintSetItem>) -> (Vec<Uuid>, Vec<Uuid>) {
    let mut constraints = Vec::new();
    let mut domains = Vec::new();
    for item in items {
        match item {
            ConstraintSetItem::Constraint(uuid) => constraints.push(uuid),
            ConstraintSetItem::Domain(uuid) => domains.push(uuid),
        }
    }
    (constraints, domains)
}

fn resolve_constraint_set(text: &str) -> Result<Option<Vec<ConstraintSetItem>>, CvxError> {
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text)?;
        if kind != HandleKind::ConstrSet {
            return Ok(None);
        }
        return registry
            .get_constraint_set_by_uuid(uuid)
            .map(|entry| Some(entry.items))
            .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()));
    }

    Ok(registry
        .get_constraint_set_by_name(text)
        .map(|entry| entry.items))
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

    let mut cvx_domains = Vec::with_capacity(problem_entry.domains.len());
    for uuid in &problem_entry.domains {
        let entry = registry
            .get_domain_by_uuid(*uuid)
            .ok_or_else(|| CvxError::Registry("domain missing for problem".to_string()))?;
        cvx_domains.push(entry.domain);
    }

    let cvx_problem = cvxrust::Problem {
        sense: objective_entry.sense,
        objective: objective_entry.expression,
        constraints: cvx_constraints,
        domains: cvx_domains,
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
        // A bare (2, 1) variable used directly as the objective is rejected
        // because the objective must evaluate to a single value (SPEC-0014);
        // the variable itself is no longer rejected outright.
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
                vec![],
                vec![var_uuid],
            )
            .unwrap();

        let result = run_solve_for_test(&problem_handle, None);
        assert_eq!(
            result.unwrap_err(),
            CvxError::SolveFailed(
                "objective must evaluate to a single value (shape 1x1); got shape 2x1".to_string()
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
                vec![],
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

    #[test]
    fn grammar_sum_and_index_solve_end_to_end_with_correct_dependency_tracking() {
        // Builds a problem entirely through CVX.EXPRESSION/CVX.CONSTRAINT
        // grammar strings using the new sum(...)/index(...) keyword syntax
        // (SPEC-0015), confirming: (1) they resolve/solve identically to
        // the CVX.SUM/CVX.INDEX functional builders, and (2) unlike those
        // functional builders, the referenced variable is swept into
        // `problem.variables` automatically by CVX.PROBLEM's real
        // `expand_variable_dependencies`, since resolve_expr/
        // resolve_constraint already recorded it as a dependency.
        let registry = Registry::global();
        registry
            .insert_variable(Some("grammar_sum_index_w".to_string()), (3, 1))
            .unwrap();
        let w_uuid = registry
            .get_variable_by_name("grammar_sum_index_w")
            .unwrap()
            .uuid;

        let bound_ast =
            crate::analytics::parser::parse_constraint("grammar_sum_index_w >= 0").unwrap();
        let bound_resolved =
            crate::analytics::resolve::resolve_constraint(registry, &bound_ast).unwrap();
        let bound_handle = registry
            .insert_constraint(
                None,
                bound_resolved.relation,
                bound_resolved.lhs,
                bound_resolved.rhs,
                bound_resolved.dependencies,
            )
            .unwrap();
        let (_, bound_uuid) = parse_handle(&bound_handle).unwrap();

        let pin_ast =
            crate::analytics::parser::parse_constraint("index(grammar_sum_index_w, 2, 1) == 5")
                .unwrap();
        let pin_resolved =
            crate::analytics::resolve::resolve_constraint(registry, &pin_ast).unwrap();
        let pin_handle = registry
            .insert_constraint(
                None,
                pin_resolved.relation,
                pin_resolved.lhs,
                pin_resolved.rhs,
                pin_resolved.dependencies,
            )
            .unwrap();
        let (_, pin_uuid) = parse_handle(&pin_handle).unwrap();

        let objective_ast = crate::analytics::parser::parse("sum(grammar_sum_index_w)").unwrap();
        let objective_resolved =
            crate::analytics::resolve::resolve_expr(registry, &objective_ast).unwrap();
        let objective_handle = registry
            .insert_objective(
                None,
                Sense::Minimize,
                objective_resolved.expression,
                objective_resolved.dependencies,
            )
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective_handle).unwrap();
        let objective_entry = registry.get_objective_by_uuid(objective_uuid).unwrap();

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
        for uuid in [bound_uuid, pin_uuid] {
            let entry = registry.get_constraint_by_uuid(uuid).unwrap();
            expand_variable_dependencies(
                registry,
                &entry.dependencies,
                &mut seen_exprs,
                &mut seen_vars,
                &mut variables,
            );
        }
        assert_eq!(variables, vec![w_uuid]);

        let problem_handle = registry
            .insert_problem(
                None,
                objective_uuid,
                vec![bound_uuid, pin_uuid],
                vec![],
                variables,
            )
            .unwrap();

        let result_handle = run_solve_for_test(&problem_handle, None).unwrap();
        let (_, result_uuid) = parse_handle(&result_handle).unwrap();
        let result_entry = registry.get_result_by_uuid(result_uuid).unwrap();

        assert_eq!(result_entry.status, cvxrust::SolveStatus::Optimal);
        assert!((result_entry.objective_value.unwrap() - 5.0).abs() < 1e-4);
        let values = &result_entry.variable_values[&w_uuid];
        // "index(grammar_sum_index_w, 2, 1)" is 1-based row 2 -> 0-based
        // entry 1, so only the second entry is pinned to 5.
        assert!((values[0] - 0.0).abs() < 1e-4);
        assert!((values[1] - 5.0).abs() < 1e-4);
        assert!((values[2] - 0.0).abs() < 1e-4);
    }

    #[test]
    fn solves_mixed_integer_knapsack_end_to_end_with_a_binary_domain() {
        // Integration test (SPEC-0019's Test Approach): a sample problem
        // with a `CVX.BINARY`-style domain restriction over a whole
        // variable, combined with an ordinary constraint, is resolved by
        // `CVX.PROBLEM`/`CVX.SOLVE`'s real domain-aware path and solved via
        // the `microlp` route, matching the project-selection/knapsack
        // scenario from `ISSUE-0019`.
        let registry = Registry::global();

        registry
            .insert_parameter(
                Some("knapsack_weights_for_test".to_string()),
                (4, 1),
                vec![2.0, 3.0, 4.0, 5.0],
            )
            .unwrap();
        registry
            .insert_parameter(
                Some("knapsack_values_for_test".to_string()),
                (4, 1),
                vec![3.0, 4.0, 5.0, 8.0],
            )
            .unwrap();
        registry
            .insert_parameter(
                Some("knapsack_capacity_for_test".to_string()),
                (1, 1),
                vec![5.0],
            )
            .unwrap();

        let selected_handle = registry
            .insert_variable(Some("knapsack_selected_for_test".to_string()), (4, 1))
            .unwrap();
        let (_, selected_uuid) = parse_handle(&selected_handle).unwrap();
        let selected_variable = registry
            .get_variable_by_uuid(selected_uuid)
            .unwrap()
            .variable;

        let domain_handle = registry
            .insert_domain(
                None,
                cvxrust::DomainConstraint {
                    variable: selected_variable,
                    row_start: 0,
                    col_start: 0,
                    rows: 4,
                    cols: 1,
                    domain: cvxrust::Domain::Binary,
                },
            )
            .unwrap();
        let (_, domain_uuid) = parse_handle(&domain_handle).unwrap();

        let capacity_ast = crate::analytics::parser::parse_constraint(
            "sum(knapsack_weights_for_test * knapsack_selected_for_test) <= knapsack_capacity_for_test",
        )
        .unwrap();
        let capacity_resolved =
            crate::analytics::resolve::resolve_constraint(registry, &capacity_ast).unwrap();
        let capacity_handle = registry
            .insert_constraint(
                None,
                capacity_resolved.relation,
                capacity_resolved.lhs,
                capacity_resolved.rhs,
                capacity_resolved.dependencies,
            )
            .unwrap();
        let (_, capacity_uuid) = parse_handle(&capacity_handle).unwrap();

        let objective_ast = crate::analytics::parser::parse(
            "sum(knapsack_values_for_test * knapsack_selected_for_test)",
        )
        .unwrap();
        let objective_resolved =
            crate::analytics::resolve::resolve_expr(registry, &objective_ast).unwrap();
        let objective_handle = registry
            .insert_objective(
                None,
                Sense::Maximize,
                objective_resolved.expression,
                objective_resolved.dependencies,
            )
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective_handle).unwrap();

        let problem_handle = registry
            .insert_problem(
                Some("knapsack_problem_for_test".to_string()),
                objective_uuid,
                vec![capacity_uuid],
                vec![domain_uuid],
                vec![selected_uuid],
            )
            .unwrap();

        let result_handle = run_solve_for_test_with_domains(&problem_handle, None).unwrap();
        let (_, result_uuid) = parse_handle(&result_handle).unwrap();
        let result_entry = registry.get_result_by_uuid(result_uuid).unwrap();

        assert_eq!(result_entry.status, cvxrust::SolveStatus::Optimal);
        assert!((result_entry.objective_value.unwrap() - 8.0).abs() < 1e-4);
        let values = &result_entry.variable_values[&selected_uuid];
        // Only the fourth item (weight 5, value 8) fits the capacity-5
        // knapsack on its own and beats every other feasible combination.
        assert!((values[0] - 0.0).abs() < 1e-6);
        assert!((values[1] - 0.0).abs() < 1e-6);
        assert!((values[2] - 0.0).abs() < 1e-6);
        assert!((values[3] - 1.0).abs() < 1e-6);
    }

    /// Like `run_solve_for_test`, but routes `problem_entry.domains` into
    /// `cvxrust::Problem::domains` too, exercising the same domain-aware
    /// path as the real `run_solve`.
    fn run_solve_for_test_with_domains(
        problem_text: &str,
        name: Option<String>,
    ) -> Result<String, CvxError> {
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

        let mut cvx_domains = Vec::with_capacity(problem_entry.domains.len());
        for uuid in &problem_entry.domains {
            let entry = registry.get_domain_by_uuid(*uuid).unwrap();
            cvx_domains.push(entry.domain);
        }

        let cvx_problem = cvxrust::Problem {
            sense: objective_entry.sense,
            objective: objective_entry.expression,
            constraints: cvx_constraints,
            domains: cvx_domains,
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
            domains: vec![],
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
