//! Excel-facing result inspection and universal handle description:
//! `CVX.VALUE`, `CVX.STATUS`, `CVX.OBJECTIVE_VALUE`, `CVX.DESCRIBE`,
//! `CVX.SHAPE`, `CVX.TYPE` (SPEC-0007).

use uuid::Uuid;
use xladd::variant::Variant;
use xladd::xlcall::{xlerrValue, LPXLOPER12};

use crate::analytics::ast::Relation;
use crate::analytics::{infer_shape, render_expression};
use crate::core::error::CvxError;
use crate::core::handle::{format_handle, parse_handle, HandleKind};
use crate::core::registry::{
    ConstraintEntry, ConstraintSetEntry, ExpressionEntry, ObjectiveEntry, ParameterEntry,
    ProblemEntry, Registry, ResultEntry, VariableEntry,
};
use crate::data;
use cvxrust::{Sense, SolveStatus};

/// One entry from any registry table, used only by
/// `CVX.DESCRIBE`/`CVX.SHAPE`/`CVX.TYPE`, which must work uniformly across
/// every `HandleKind`.
#[derive(Debug, Clone, PartialEq)]
enum RegistryObject {
    Parameter(ParameterEntry),
    Variable(VariableEntry),
    Expression(ExpressionEntry),
    Constraint(ConstraintEntry),
    ConstraintSet(ConstraintSetEntry),
    Objective(ObjectiveEntry),
    Problem(ProblemEntry),
    Result(ResultEntry),
}

impl RegistryObject {
    fn kind(&self) -> HandleKind {
        match self {
            RegistryObject::Parameter(_) => HandleKind::Param,
            RegistryObject::Variable(_) => HandleKind::Var,
            RegistryObject::Expression(_) => HandleKind::Expr,
            RegistryObject::Constraint(_) => HandleKind::Constr,
            RegistryObject::ConstraintSet(_) => HandleKind::ConstrSet,
            RegistryObject::Objective(_) => HandleKind::Obj,
            RegistryObject::Problem(_) => HandleKind::Prob,
            RegistryObject::Result(_) => HandleKind::Result,
        }
    }

    fn uuid(&self) -> Uuid {
        match self {
            RegistryObject::Parameter(e) => e.uuid,
            RegistryObject::Variable(e) => e.uuid,
            RegistryObject::Expression(e) => e.uuid,
            RegistryObject::Constraint(e) => e.uuid,
            RegistryObject::ConstraintSet(e) => e.uuid,
            RegistryObject::Objective(e) => e.uuid,
            RegistryObject::Problem(e) => e.uuid,
            RegistryObject::Result(e) => e.uuid,
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            RegistryObject::Parameter(e) => e.name.as_deref(),
            RegistryObject::Variable(e) => e.name.as_deref(),
            RegistryObject::Expression(e) => e.name.as_deref(),
            RegistryObject::Constraint(e) => e.name.as_deref(),
            RegistryObject::ConstraintSet(e) => e.name.as_deref(),
            RegistryObject::Objective(e) => e.name.as_deref(),
            RegistryObject::Problem(e) => e.name.as_deref(),
            RegistryObject::Result(e) => e.name.as_deref(),
        }
    }

    fn type_str(&self) -> &'static str {
        match self {
            RegistryObject::Parameter(_) => "parameter",
            RegistryObject::Variable(_) => "variable",
            RegistryObject::Expression(_) => "expression",
            RegistryObject::Constraint(_) => "constraint",
            RegistryObject::ConstraintSet(_) => "constraint_set",
            RegistryObject::Objective(_) => "objective",
            RegistryObject::Problem(_) => "problem",
            RegistryObject::Result(_) => "result",
        }
    }
}

/// Resolves `text` (a `cvx:<kind>:<uuid>` handle or a registered name)
/// against every registry table.
fn resolve_any(text: &str) -> Result<RegistryObject, CvxError> {
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text)?;
        let found = match kind {
            HandleKind::Param => registry
                .get_parameter_by_uuid(uuid)
                .map(RegistryObject::Parameter),
            HandleKind::Var => registry
                .get_variable_by_uuid(uuid)
                .map(RegistryObject::Variable),
            HandleKind::Expr => registry
                .get_expression_by_uuid(uuid)
                .map(RegistryObject::Expression),
            HandleKind::Constr => registry
                .get_constraint_by_uuid(uuid)
                .map(RegistryObject::Constraint),
            HandleKind::ConstrSet => registry
                .get_constraint_set_by_uuid(uuid)
                .map(RegistryObject::ConstraintSet),
            HandleKind::Obj => registry
                .get_objective_by_uuid(uuid)
                .map(RegistryObject::Objective),
            HandleKind::Prob => registry
                .get_problem_by_uuid(uuid)
                .map(RegistryObject::Problem),
            HandleKind::Result => registry
                .get_result_by_uuid(uuid)
                .map(RegistryObject::Result),
        };
        return found.ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()));
    }

    let mut matches = Vec::new();
    if let Some(e) = registry.get_parameter_by_name(text) {
        matches.push(RegistryObject::Parameter(e));
    }
    if let Some(e) = registry.get_variable_by_name(text) {
        matches.push(RegistryObject::Variable(e));
    }
    if let Some(e) = registry.get_expression_by_name(text) {
        matches.push(RegistryObject::Expression(e));
    }
    if let Some(e) = registry.get_constraint_by_name(text) {
        matches.push(RegistryObject::Constraint(e));
    }
    if let Some(e) = registry.get_constraint_set_by_name(text) {
        matches.push(RegistryObject::ConstraintSet(e));
    }
    if let Some(e) = registry.get_objective_by_name(text) {
        matches.push(RegistryObject::Objective(e));
    }
    if let Some(e) = registry.get_problem_by_name(text) {
        matches.push(RegistryObject::Problem(e));
    }
    if let Some(e) = registry.get_result_by_name(text) {
        matches.push(RegistryObject::Result(e));
    }

    resolve_from_matches(text, matches)
}

/// Picks the single match out of every table that matched `text` by name.
/// Split out of [`resolve_any`] so the ambiguity/not-found decision can be
/// unit-tested without needing to violate the insertion-time cross-table
/// name uniqueness invariant (SPEC-0002) that normally prevents more than
/// one match from ever occurring.
fn resolve_from_matches(
    text: &str,
    matches: Vec<RegistryObject>,
) -> Result<RegistryObject, CvxError> {
    let mut iter = matches.into_iter();
    match (iter.next(), iter.next()) {
        (None, _) => Err(CvxError::UnknownIdentifier(text.to_string())),
        (Some(only), None) => Ok(only),
        (Some(_), Some(_)) => Err(CvxError::AmbiguousIdentifier(text.to_string())),
    }
}

fn status_str(status: &SolveStatus) -> &'static str {
    match status {
        SolveStatus::Optimal => "optimal",
        SolveStatus::Infeasible => "infeasible",
        SolveStatus::Unbounded => "unbounded",
        SolveStatus::Error(_) => "error",
    }
}

const MAX_DESCRIBE_LEN: usize = 500;

/// Formats `obj`'s primary, human-readable identifier: its registered name
/// if it has one, else its handle. Used both for the leading identifier in
/// `describe()` and, via `display_ref`, for references to other objects
/// inside `describe_body`.
fn primary_identifier(obj: &RegistryObject) -> String {
    obj.name()
        .map(|name| format!("\"{name}\""))
        .unwrap_or_else(|| format_handle(obj.kind(), obj.uuid()))
}

/// Resolves `uuid` (known to be of kind `kind`) to its registry entry and
/// returns its name (quoted, as in `primary_identifier`) if it has one,
/// else its handle. Falls back to the handle if the entry has since been
/// removed from the registry (should not normally occur, since referenced
/// objects are not independently deletable — see Error Handling).
fn display_ref(kind: HandleKind, uuid: Uuid) -> String {
    let name = match kind {
        HandleKind::Constr => Registry::global()
            .get_constraint_by_uuid(uuid)
            .and_then(|e| e.name),
        HandleKind::Var => Registry::global()
            .get_variable_by_uuid(uuid)
            .and_then(|e| e.name),
        HandleKind::Obj => Registry::global()
            .get_objective_by_uuid(uuid)
            .and_then(|e| e.name),
        _ => None, // only the three referenced kinds above are ever passed in
    };
    match name {
        Some(name) => format!("\"{name}\""),
        None => format_handle(kind, uuid),
    }
}

fn describe(obj: &RegistryObject) -> String {
    let primary = primary_identifier(obj);
    let handle = format_handle(obj.kind(), obj.uuid());
    // Only show the handle a second time when it isn't already the primary
    // identifier (i.e. only when the object has a name).
    let handle_suffix = if obj.name().is_some() {
        format!(" ({handle})")
    } else {
        String::new()
    };
    let body = describe_body(obj);
    truncate_describe(format!("{primary}{handle_suffix}: {body}"))
}

fn truncate_describe(text: String) -> String {
    if text.chars().count() <= MAX_DESCRIBE_LEN {
        return text;
    }
    let mut truncated: String = text.chars().take(MAX_DESCRIBE_LEN).collect();
    truncated.push_str("... (truncated)");
    truncated
}

fn describe_body(obj: &RegistryObject) -> String {
    match obj {
        RegistryObject::Parameter(e) => {
            let csv = e
                .data
                .iter()
                .map(f64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("parameter {}x{}, data=[{csv}]", e.shape.0, e.shape.1)
        }
        RegistryObject::Variable(e) => format!("variable {}x{}", e.shape.0, e.shape.1),
        RegistryObject::Expression(e) => {
            let shape_part = match infer_shape(&e.expression) {
                Ok((r, c)) => format!("{r}x{c}"),
                Err(err) => format!("(shape unavailable: {err})"),
            };
            format!(
                "expression {shape_part} = {}",
                render_expression(&e.expression, Registry::global())
            )
        }
        RegistryObject::Constraint(e) => {
            let op = relation_str(e.relation);
            format!(
                "constraint: {} {op} {}",
                render_expression(&e.lhs, Registry::global()),
                render_expression(&e.rhs, Registry::global())
            )
        }
        RegistryObject::ConstraintSet(e) => {
            let handles = e
                .constraints
                .iter()
                .map(|uuid| display_ref(HandleKind::Constr, *uuid))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "constraint_set: [{} constraint(s)]: {handles}",
                e.constraints.len()
            )
        }
        RegistryObject::Objective(e) => {
            let sense = match e.sense {
                Sense::Minimize => "minimize",
                Sense::Maximize => "maximize",
            };
            format!(
                "objective {sense}: {}",
                render_expression(&e.expression, Registry::global())
            )
        }
        RegistryObject::Problem(e) => {
            let obj_handle = display_ref(HandleKind::Obj, e.objective);
            let c_handles = e
                .constraints
                .iter()
                .map(|uuid| display_ref(HandleKind::Constr, *uuid))
                .collect::<Vec<_>>()
                .join(", ");
            let v_handles = e
                .variables
                .iter()
                .map(|uuid| display_ref(HandleKind::Var, *uuid))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "problem: objective={obj_handle}, constraints=[{}]: {c_handles}, variables=[{}]: {v_handles}",
                e.constraints.len(),
                e.variables.len()
            )
        }
        RegistryObject::Result(e) => {
            let ov = e
                .objective_value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "n/a".to_string());
            format!(
                "result: status={}, objective_value={ov}, variables=[{}]",
                status_str(&e.status),
                e.variable_values.len()
            )
        }
    }
}

fn relation_str(relation: Relation) -> &'static str {
    match relation {
        Relation::LessEqual => "<=",
        Relation::GreaterEqual => ">=",
        Relation::Equal => "=",
    }
}

fn shape_str(obj: &RegistryObject) -> Result<String, CvxError> {
    match obj {
        RegistryObject::Parameter(e) => Ok(format!("{}x{}", e.shape.0, e.shape.1)),
        RegistryObject::Variable(e) => Ok(format!("{}x{}", e.shape.0, e.shape.1)),
        RegistryObject::Expression(e) => {
            let (r, c) = infer_shape(&e.expression)?;
            Ok(format!("{r}x{c}"))
        }
        _ => Err(CvxError::InvalidExpression(format!(
            "object of type '{}' has no shape",
            obj.type_str()
        ))),
    }
}

/// `CVX.DESCRIBE(handle)` — describes any registry object as a diagnostic
/// string.
#[export_name = "CVX.DESCRIBE"]
pub extern "system" fn cvx_describe(handle: LPXLOPER12) -> LPXLOPER12 {
    let result = run_handle(handle, |obj| Ok(describe(obj)));
    to_xloper_err_string_result(result, "CVX.DESCRIBE")
}

/// `CVX.SHAPE(handle)` — returns the `"<rows>x<cols>"` shape of a
/// parameter, variable, or expression.
#[export_name = "CVX.SHAPE"]
pub extern "system" fn cvx_shape(handle: LPXLOPER12) -> LPXLOPER12 {
    let result = run_handle(handle, shape_str);
    to_xloper_err_string_result(result, "CVX.SHAPE")
}

/// `CVX.TYPE(handle)` — returns the type name of any registry object.
#[export_name = "CVX.TYPE"]
pub extern "system" fn cvx_type(handle: LPXLOPER12) -> LPXLOPER12 {
    let result = run_handle(handle, |obj| Ok(obj.type_str().to_string()));
    to_xloper_err_string_result(result, "CVX.TYPE")
}

fn run_handle(
    handle: LPXLOPER12,
    f: impl FnOnce(&RegistryObject) -> Result<String, CvxError>,
) -> Result<String, CvxError> {
    let text = data::parse_string(&Variant::from_xloper(handle))?;
    let obj = resolve_any(text.trim())?;
    f(&obj)
}

fn to_xloper_err_string_result(result: Result<String, CvxError>, context: &str) -> LPXLOPER12 {
    let variant = match result {
        Ok(s) => Variant::from_str(&s),
        Err(err) => {
            tracing::error!(error = %err, "{context} failed");
            Variant::from_err(xlerrValue)
        }
    };
    Box::into_raw(Box::new(variant)) as LPXLOPER12
}

/// `CVX.STATUS(result)` — returns the solve status of a result as a string.
#[export_name = "CVX.STATUS"]
pub extern "system" fn cvx_status(result: LPXLOPER12) -> LPXLOPER12 {
    to_xloper_err_string_result(run_status(result), "CVX.STATUS")
}

fn run_status(result: LPXLOPER12) -> Result<String, CvxError> {
    let uuid = resolve_result_uuid(result)?;
    status_for(uuid)
}

fn status_for(result_uuid: Uuid) -> Result<String, CvxError> {
    let entry = Registry::global()
        .get_result_by_uuid(result_uuid)
        .ok_or_else(|| CvxError::Registry("result disappeared".to_string()))?;
    Ok(status_str(&entry.status).to_string())
}

/// `CVX.OBJECTIVE_VALUE(result)` — returns the objective value of an
/// optimal result.
#[export_name = "CVX.OBJECTIVE_VALUE"]
pub extern "system" fn cvx_objective_value(result: LPXLOPER12) -> LPXLOPER12 {
    let variant = match run_objective_value(result) {
        Ok(value) => Variant::from_float(value),
        Err(err) => {
            tracing::error!(error = %err, "CVX.OBJECTIVE_VALUE failed");
            Variant::from_err(xlerrValue)
        }
    };
    Box::into_raw(Box::new(variant)) as LPXLOPER12
}

fn run_objective_value(result: LPXLOPER12) -> Result<f64, CvxError> {
    let uuid = resolve_result_uuid(result)?;
    objective_value_for(uuid)
}

fn objective_value_for(result_uuid: Uuid) -> Result<f64, CvxError> {
    let entry = Registry::global()
        .get_result_by_uuid(result_uuid)
        .ok_or_else(|| CvxError::Registry("result disappeared".to_string()))?;
    entry.objective_value.ok_or_else(|| {
        CvxError::InvalidExpression(format!(
            "result status is {}; no objective value is available",
            status_str(&entry.status)
        ))
    })
}

/// `CVX.VALUE(result, variable)` — returns a solved variable's value(s):
/// a scalar for a `(1, 1)` variable, an array otherwise.
#[export_name = "CVX.VALUE"]
pub extern "system" fn cvx_value(result: LPXLOPER12, variable: LPXLOPER12) -> LPXLOPER12 {
    to_xloper_value_result(run_value(result, variable), "CVX.VALUE")
}

fn run_value(
    result: LPXLOPER12,
    variable: LPXLOPER12,
) -> Result<((usize, usize), Vec<f64>), CvxError> {
    let result_uuid = resolve_result_uuid(result)?;
    let (variable_uuid, variable_text) = resolve_variable_uuid_arg(variable)?;
    value_for(result_uuid, variable_uuid, &variable_text)
}

fn value_for(
    result_uuid: Uuid,
    variable_uuid: Uuid,
    variable_text: &str,
) -> Result<((usize, usize), Vec<f64>), CvxError> {
    let registry = Registry::global();
    let result_entry = registry
        .get_result_by_uuid(result_uuid)
        .ok_or_else(|| CvxError::Registry("result disappeared".to_string()))?;
    let variable_entry = registry
        .get_variable_by_uuid(variable_uuid)
        .ok_or_else(|| CvxError::Registry("variable disappeared".to_string()))?;

    if result_entry.status != SolveStatus::Optimal {
        return Err(CvxError::InvalidExpression(format!(
            "result status is {}; no variable values are available",
            status_str(&result_entry.status)
        )));
    }

    let values = result_entry
        .variable_values
        .get(&variable_uuid)
        .ok_or_else(|| CvxError::UnknownIdentifier(variable_text.to_string()))?;

    Ok((variable_entry.shape, values.clone()))
}

/// Converts a solved variable's values into a scalar or row-major array
/// `Variant` matching its shape.
fn to_xloper_value_result(
    result: Result<((usize, usize), Vec<f64>), CvxError>,
    context: &str,
) -> LPXLOPER12 {
    let variant = match result {
        Ok(((1, 1), values)) => Variant::from_float(values[0]),
        Ok(((rows, cols), values)) => {
            let cells: Vec<Variant> = values.into_iter().map(Variant::from_float).collect();
            Variant::from_array(cols, rows, &cells)
        }
        Err(err) => {
            tracing::error!(error = %err, "{context} failed");
            Variant::from_err(xlerrValue)
        }
    };
    Box::into_raw(Box::new(variant)) as LPXLOPER12
}

fn resolve_result_uuid(arg: LPXLOPER12) -> Result<Uuid, CvxError> {
    let text = data::parse_string(&Variant::from_xloper(arg))?;
    let text = text.trim();
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(text)?;
        if kind != HandleKind::Result {
            return Err(CvxError::UnknownIdentifier(text.to_string()));
        }
        return registry
            .get_result_by_uuid(uuid)
            .map(|_| uuid)
            .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()));
    }

    registry
        .get_result_by_name(text)
        .map(|entry| entry.uuid)
        .ok_or_else(|| CvxError::UnknownIdentifier(text.to_string()))
}

fn resolve_variable_uuid_arg(arg: LPXLOPER12) -> Result<(Uuid, String), CvxError> {
    let text = data::parse_string(&Variant::from_xloper(arg))?;
    let text = text.trim().to_string();
    let registry = Registry::global();

    if text.starts_with("cvx:") {
        let (kind, uuid) = parse_handle(&text)?;
        if kind != HandleKind::Var {
            return Err(CvxError::UnknownIdentifier(text));
        }
        return registry
            .get_variable_by_uuid(uuid)
            .map(|_| (uuid, text.clone()))
            .ok_or(CvxError::UnknownIdentifier(text));
    }

    registry
        .get_variable_by_name(&text)
        .map(|entry| (entry.uuid, text.clone()))
        .ok_or(CvxError::UnknownIdentifier(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::ast::Relation as AstRelation;
    use cvxrust::{Expression, Variable};
    use std::collections::HashMap;

    #[test]
    fn resolves_every_kind_by_handle_and_name() {
        let registry = Registry::global();

        let param = registry
            .insert_parameter(Some("inspect_param".to_string()), (1, 1), vec![1.0])
            .unwrap();
        let var = registry
            .insert_variable(Some("inspect_var".to_string()), (1, 1))
            .unwrap();
        let variable_entry = registry.get_variable_by_name("inspect_var").unwrap();
        let expr = registry
            .insert_expression(
                Some("inspect_expr".to_string()),
                Expression::from_variable(variable_entry.variable),
                vec![var.clone()],
            )
            .unwrap();
        let constr = registry
            .insert_constraint(
                Some("inspect_constr".to_string()),
                AstRelation::LessEqual,
                Expression::from_variable(variable_entry.variable),
                Expression::constant(10.0),
                vec![var.clone()],
            )
            .unwrap();
        let (_, constr_uuid) = parse_handle(&constr).unwrap();
        let constrset = registry
            .insert_constraint_set(Some("inspect_constrset".to_string()), vec![constr_uuid])
            .unwrap();
        let objective = registry
            .insert_objective(
                Some("inspect_obj".to_string()),
                Sense::Minimize,
                Expression::from_variable(variable_entry.variable),
                vec![var.clone()],
            )
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective).unwrap();
        let problem = registry
            .insert_problem(
                Some("inspect_prob".to_string()),
                objective_uuid,
                vec![constr_uuid],
                vec![variable_entry.uuid],
            )
            .unwrap();
        let (_, problem_uuid) = parse_handle(&problem).unwrap();
        let result = registry
            .insert_result(
                Some("inspect_result".to_string()),
                problem_uuid,
                SolveStatus::Optimal,
                Some(1.0),
                HashMap::new(),
            )
            .unwrap();

        for (handle, name) in [
            (&param, "inspect_param"),
            (&var, "inspect_var"),
            (&expr, "inspect_expr"),
            (&constr, "inspect_constr"),
            (&constrset, "inspect_constrset"),
            (&objective, "inspect_obj"),
            (&problem, "inspect_prob"),
            (&result, "inspect_result"),
        ] {
            let by_handle = resolve_any(handle).unwrap();
            let by_name = resolve_any(name).unwrap();
            assert_eq!(by_handle, by_name);
        }
    }

    #[test]
    fn resolve_any_reports_unknown_identifier() {
        let err = resolve_any("does_not_exist_inspect").unwrap_err();
        assert_eq!(
            err,
            CvxError::UnknownIdentifier("does_not_exist_inspect".to_string())
        );
    }

    #[test]
    fn resolve_from_matches_reports_ambiguity_when_more_than_one_table_matches() {
        // Under normal insertion, SPEC-0002's cross-table uniqueness check
        // guarantees at most one match; this exercises resolve_any's
        // defensive fallback directly without needing to violate that
        // invariant.
        let a = RegistryObject::Parameter(ParameterEntry {
            uuid: Uuid::new_v4(),
            name: Some("dup".to_string()),
            shape: (1, 1),
            data: vec![1.0],
            content_hash: 0,
        });
        let b = RegistryObject::Variable(VariableEntry {
            uuid: Uuid::new_v4(),
            name: Some("dup".to_string()),
            shape: (1, 1),
            variable: Variable::new(1, (1, 1)),
        });
        let err = resolve_from_matches("dup", vec![a, b]).unwrap_err();
        assert_eq!(err, CvxError::AmbiguousIdentifier("dup".to_string()));
    }

    #[test]
    fn describes_parameter_with_data() {
        let handle = Registry::global()
            .insert_parameter(Some("describe_param".to_string()), (1, 2), vec![1.0, 2.0])
            .unwrap();
        let obj = resolve_any(&handle).unwrap();
        assert_eq!(
            describe(&obj),
            format!("\"describe_param\" ({handle}): parameter 1x2, data=[1, 2]")
        );
    }

    #[test]
    fn describes_variable_without_name() {
        let handle = Registry::global().insert_variable(None, (2, 3)).unwrap();
        let obj = resolve_any(&handle).unwrap();
        assert_eq!(describe(&obj), format!("{handle}: variable 2x3"));
    }

    #[test]
    fn describes_expression_with_inferable_shape() {
        let var = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let variable_entry = Registry::global()
            .get_variable_by_uuid(parse_handle(&var).unwrap().1)
            .unwrap();
        let handle = Registry::global()
            .insert_expression(
                Some("describe_expr".to_string()),
                Expression::from_variable(variable_entry.variable),
                vec![var],
            )
            .unwrap();
        let obj = resolve_any(&handle).unwrap();
        assert_eq!(
            describe(&obj),
            format!(
                "\"describe_expr\" ({handle}): expression 1x1 = var#{}",
                variable_entry.variable.id
            )
        );
    }

    #[test]
    fn primary_identifier_prefers_name_over_handle() {
        let handle = Registry::global()
            .insert_variable(Some("primary_named".to_string()), (1, 1))
            .unwrap();
        let obj = resolve_any(&handle).unwrap();
        assert_eq!(primary_identifier(&obj), "\"primary_named\"");
    }

    #[test]
    fn primary_identifier_falls_back_to_handle_without_name() {
        let handle = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let obj = resolve_any(&handle).unwrap();
        assert_eq!(primary_identifier(&obj), handle);
    }

    #[test]
    fn display_ref_prefers_name_over_handle() {
        let handle = Registry::global()
            .insert_variable(Some("display_ref_named".to_string()), (1, 1))
            .unwrap();
        let (kind, uuid) = parse_handle(&handle).unwrap();
        assert_eq!(display_ref(kind, uuid), "\"display_ref_named\"");
    }

    #[test]
    fn display_ref_falls_back_to_handle_without_name() {
        let handle = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let (kind, uuid) = parse_handle(&handle).unwrap();
        assert_eq!(display_ref(kind, uuid), handle);
    }

    #[test]
    fn constraint_set_describe_mixes_named_and_unnamed_members() {
        let named_var = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let named_var_entry = Registry::global()
            .get_variable_by_uuid(parse_handle(&named_var).unwrap().1)
            .unwrap();
        let named_constr = Registry::global()
            .insert_constraint(
                Some("named_constr".to_string()),
                AstRelation::LessEqual,
                Expression::from_variable(named_var_entry.variable),
                Expression::constant(1.0),
                vec![named_var.clone()],
            )
            .unwrap();
        let unnamed_constr = Registry::global()
            .insert_constraint(
                None,
                AstRelation::LessEqual,
                Expression::from_variable(named_var_entry.variable),
                Expression::constant(2.0),
                vec![named_var],
            )
            .unwrap();
        let (_, named_uuid) = parse_handle(&named_constr).unwrap();
        let (_, unnamed_uuid) = parse_handle(&unnamed_constr).unwrap();
        let set_handle = Registry::global()
            .insert_constraint_set(None, vec![named_uuid, unnamed_uuid])
            .unwrap();
        let obj = resolve_any(&set_handle).unwrap();
        assert_eq!(
            describe(&obj),
            format!(
                "{set_handle}: constraint_set: [2 constraint(s)]: \"named_constr\", {unnamed_constr}"
            )
        );
    }

    #[test]
    fn problem_describe_mixes_named_and_unnamed_references() {
        let objective = Registry::global()
            .insert_objective(
                Some("named_objective".to_string()),
                Sense::Minimize,
                Expression::constant(1.0),
                vec![],
            )
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective).unwrap();
        let var = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let variable_entry = Registry::global()
            .get_variable_by_uuid(parse_handle(&var).unwrap().1)
            .unwrap();
        let constr = Registry::global()
            .insert_constraint(
                None,
                AstRelation::LessEqual,
                Expression::from_variable(variable_entry.variable),
                Expression::constant(1.0),
                vec![var.clone()],
            )
            .unwrap();
        let (_, constr_uuid) = parse_handle(&constr).unwrap();
        let problem = Registry::global()
            .insert_problem(
                None,
                objective_uuid,
                vec![constr_uuid],
                vec![variable_entry.uuid],
            )
            .unwrap();
        let obj = resolve_any(&problem).unwrap();
        assert_eq!(
            describe(&obj),
            format!(
                "{problem}: problem: objective=\"named_objective\", constraints=[1]: {constr}, variables=[1]: {var}"
            )
        );
    }

    #[test]
    fn expression_body_prefers_names_for_variables_and_parameters() {
        let x = Registry::global()
            .insert_variable(Some("x".to_string()), (1, 1))
            .unwrap();
        let x_entry = Registry::global()
            .get_variable_by_uuid(parse_handle(&x).unwrap().1)
            .unwrap();
        let y = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let y_entry = Registry::global()
            .get_variable_by_uuid(parse_handle(&y).unwrap().1)
            .unwrap();

        // "x" * "x" + var#<y>
        let expr = Expression::add(
            Expression::mul(
                Expression::from_variable(x_entry.variable),
                Expression::from_variable(x_entry.variable),
            ),
            Expression::from_variable(y_entry.variable),
        );
        let handle = Registry::global()
            .insert_expression(Some("total".to_string()), expr, vec![x, y])
            .unwrap();
        let obj = resolve_any(&handle).unwrap();
        assert_eq!(
            describe(&obj),
            format!(
                "\"total\" ({handle}): expression 1x1 = (\"x\") * (\"x\") + var#{}",
                y_entry.variable.id
            )
        );
    }

    #[test]
    fn constraint_body_prefers_names_for_both_operands() {
        let var = Registry::global()
            .insert_variable(Some("x".to_string()), (1, 1))
            .unwrap();
        let var_entry = Registry::global()
            .get_variable_by_uuid(parse_handle(&var).unwrap().1)
            .unwrap();
        let param_handle = Registry::global()
            .insert_parameter(Some("budget".to_string()), (1, 1), vec![10.0])
            .unwrap();
        let (_, param_uuid) = parse_handle(&param_handle).unwrap();

        let handle = Registry::global()
            .insert_constraint(
                None,
                AstRelation::LessEqual,
                Expression::from_variable(var_entry.variable),
                Expression::from_parameter(
                    crate::core::registry::parameter_id(param_uuid),
                    (1, 1),
                    vec![10.0],
                ),
                vec![var, param_handle],
            )
            .unwrap();
        let obj = resolve_any(&handle).unwrap();
        assert_eq!(
            describe(&obj),
            format!("{handle}: constraint: \"x\" <= \"budget\"")
        );
    }

    #[test]
    fn describe_truncates_long_output() {
        let data = vec![1.0; 200];
        let handle = Registry::global()
            .insert_parameter(None, (1, 200), data)
            .unwrap();
        let obj = resolve_any(&handle).unwrap();
        let described = describe(&obj);
        assert!(described.ends_with("... (truncated)"));
        assert_eq!(
            described.chars().count(),
            MAX_DESCRIBE_LEN + "... (truncated)".chars().count()
        );
    }

    #[test]
    fn shape_of_parameter_variable_and_expression() {
        let param = Registry::global()
            .insert_parameter(None, (2, 2), vec![0.0; 4])
            .unwrap();
        assert_eq!(shape_str(&resolve_any(&param).unwrap()).unwrap(), "2x2");

        let var = Registry::global().insert_variable(None, (3, 1)).unwrap();
        assert_eq!(shape_str(&resolve_any(&var).unwrap()).unwrap(), "3x1");

        let variable_entry = Registry::global()
            .get_variable_by_uuid(parse_handle(&var).unwrap().1)
            .unwrap();
        let expr = Registry::global()
            .insert_expression(
                None,
                Expression::from_variable(variable_entry.variable),
                vec![var],
            )
            .unwrap();
        assert_eq!(shape_str(&resolve_any(&expr).unwrap()).unwrap(), "3x1");
    }

    #[test]
    fn shape_rejects_non_shaped_kinds() {
        let objective = Registry::global()
            .insert_objective(None, Sense::Minimize, Expression::constant(1.0), vec![])
            .unwrap();
        let err = shape_str(&resolve_any(&objective).unwrap()).unwrap_err();
        assert_eq!(
            err,
            CvxError::InvalidExpression("object of type 'objective' has no shape".to_string())
        );
    }

    #[test]
    fn type_of_each_kind() {
        let param = Registry::global()
            .insert_parameter(None, (1, 1), vec![1.0])
            .unwrap();
        assert_eq!(resolve_any(&param).unwrap().type_str(), "parameter");

        let var = Registry::global().insert_variable(None, (1, 1)).unwrap();
        assert_eq!(resolve_any(&var).unwrap().type_str(), "variable");
    }

    #[test]
    fn status_and_objective_value_for_optimal_result() {
        let objective = Registry::global()
            .insert_objective(None, Sense::Minimize, Expression::constant(1.0), vec![])
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective).unwrap();
        let problem = Registry::global()
            .insert_problem(None, objective_uuid, vec![], vec![])
            .unwrap();
        let (_, problem_uuid) = parse_handle(&problem).unwrap();
        let result = Registry::global()
            .insert_result(
                None,
                problem_uuid,
                SolveStatus::Optimal,
                Some(3.5),
                HashMap::new(),
            )
            .unwrap();
        let (_, result_uuid) = parse_handle(&result).unwrap();
        assert_eq!(status_for(result_uuid).unwrap(), "optimal");
        assert_eq!(objective_value_for(result_uuid).unwrap(), 3.5);
    }

    #[test]
    fn objective_value_missing_for_non_optimal_result() {
        let objective = Registry::global()
            .insert_objective(None, Sense::Minimize, Expression::constant(1.0), vec![])
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective).unwrap();
        let problem = Registry::global()
            .insert_problem(None, objective_uuid, vec![], vec![])
            .unwrap();
        let (_, problem_uuid) = parse_handle(&problem).unwrap();
        let result = Registry::global()
            .insert_result(
                None,
                problem_uuid,
                SolveStatus::Infeasible,
                None,
                HashMap::new(),
            )
            .unwrap();
        let (_, result_uuid) = parse_handle(&result).unwrap();
        assert_eq!(status_for(result_uuid).unwrap(), "infeasible");
        let err = objective_value_for(result_uuid).unwrap_err();
        assert_eq!(
            err,
            CvxError::InvalidExpression(
                "result status is infeasible; no objective value is available".to_string()
            )
        );
    }

    #[test]
    fn value_reads_scalar_and_array_variables() {
        let var_scalar = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let (_, var_scalar_uuid) = parse_handle(&var_scalar).unwrap();
        let var_array = Registry::global().insert_variable(None, (2, 1)).unwrap();
        let (_, var_array_uuid) = parse_handle(&var_array).unwrap();

        let objective = Registry::global()
            .insert_objective(None, Sense::Minimize, Expression::constant(1.0), vec![])
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective).unwrap();
        let problem = Registry::global()
            .insert_problem(
                None,
                objective_uuid,
                vec![],
                vec![var_scalar_uuid, var_array_uuid],
            )
            .unwrap();
        let (_, problem_uuid) = parse_handle(&problem).unwrap();

        let mut variable_values = HashMap::new();
        variable_values.insert(var_scalar_uuid, vec![7.0]);
        variable_values.insert(var_array_uuid, vec![1.0, 2.0]);

        let result = Registry::global()
            .insert_result(
                None,
                problem_uuid,
                SolveStatus::Optimal,
                Some(0.0),
                variable_values,
            )
            .unwrap();
        let (_, result_uuid) = parse_handle(&result).unwrap();

        assert_eq!(
            value_for(result_uuid, var_scalar_uuid, "scalar").unwrap(),
            ((1, 1), vec![7.0])
        );
        assert_eq!(
            value_for(result_uuid, var_array_uuid, "array").unwrap(),
            ((2, 1), vec![1.0, 2.0])
        );
    }

    #[test]
    fn value_missing_for_variable_not_part_of_solved_problem() {
        let solved_var = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let (_, solved_var_uuid) = parse_handle(&solved_var).unwrap();
        let other_var = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let (_, other_var_uuid) = parse_handle(&other_var).unwrap();

        let objective = Registry::global()
            .insert_objective(None, Sense::Minimize, Expression::constant(1.0), vec![])
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective).unwrap();
        let problem = Registry::global()
            .insert_problem(None, objective_uuid, vec![], vec![solved_var_uuid])
            .unwrap();
        let (_, problem_uuid) = parse_handle(&problem).unwrap();

        let mut variable_values = HashMap::new();
        variable_values.insert(solved_var_uuid, vec![1.0]);

        let result = Registry::global()
            .insert_result(
                None,
                problem_uuid,
                SolveStatus::Optimal,
                Some(0.0),
                variable_values,
            )
            .unwrap();
        let (_, result_uuid) = parse_handle(&result).unwrap();

        let err = value_for(result_uuid, other_var_uuid, "other_var").unwrap_err();
        assert_eq!(err, CvxError::UnknownIdentifier("other_var".to_string()));
    }

    #[test]
    fn value_errors_for_non_optimal_result() {
        let var = Registry::global().insert_variable(None, (1, 1)).unwrap();
        let (_, var_uuid) = parse_handle(&var).unwrap();

        let objective = Registry::global()
            .insert_objective(None, Sense::Minimize, Expression::constant(1.0), vec![])
            .unwrap();
        let (_, objective_uuid) = parse_handle(&objective).unwrap();
        let problem = Registry::global()
            .insert_problem(None, objective_uuid, vec![], vec![var_uuid])
            .unwrap();
        let (_, problem_uuid) = parse_handle(&problem).unwrap();

        let result = Registry::global()
            .insert_result(
                None,
                problem_uuid,
                SolveStatus::Unbounded,
                None,
                HashMap::new(),
            )
            .unwrap();
        let (_, result_uuid) = parse_handle(&result).unwrap();

        let err = value_for(result_uuid, var_uuid, "x").unwrap_err();
        assert_eq!(
            err,
            CvxError::InvalidExpression(
                "result status is unbounded; no variable values are available".to_string()
            )
        );
    }
}
