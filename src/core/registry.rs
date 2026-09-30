use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{OnceLock, RwLock};

use uuid::Uuid;

use crate::analytics::ast::Relation;
use crate::core::error::CvxError;
use crate::core::handle::{format_handle, HandleKind};
use crate::core::variable::Variable;
use cvxrust::{Expression, Sense, SolveStatus};

/// A parameter stored in the registry: dense numeric data keyed by UUID and
/// an optional unique name.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    /// `(rows, cols)`.
    pub shape: (usize, usize),
    /// Row-major dense data.
    pub data: Vec<f64>,
    pub content_hash: u64,
}

/// A variable stored in the registry: a `cvxrust` decision variable keyed by
/// UUID and an optional unique name.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    /// `(rows, cols)`.
    pub shape: (usize, usize),
    pub variable: Variable,
}

/// An expression stored in the registry: a lazy `cvxrust::Expression` keyed
/// by UUID and an optional unique name.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub expression: Expression,
    pub dependencies: Vec<String>,
}

/// A constraint stored in the registry: a relation between two lazy
/// `cvxrust::Expression` operands, keyed by UUID and an optional unique
/// name.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub relation: Relation,
    pub lhs: Expression,
    pub rhs: Expression,
    pub dependencies: Vec<String>,
}

/// An ordered set of constraints stored in the registry, keyed by UUID and
/// an optional unique name.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintSetEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    /// Ordered UUIDs of the member constraints, referencing the constraint
    /// table.
    pub constraints: Vec<Uuid>,
}

/// An objective stored in the registry: a sense paired with a lazy
/// `cvxrust::Expression`, keyed by UUID and an optional unique name.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectiveEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    pub sense: Sense,
    pub expression: Expression,
    pub dependencies: Vec<String>,
}

/// A problem stored in the registry: an objective, an ordered list of
/// constraints, and the distinct variables they reference, keyed by UUID
/// and an optional unique name.
#[derive(Debug, Clone, PartialEq)]
pub struct ProblemEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    /// References an `ObjectiveEntry`.
    pub objective: Uuid,
    /// Ordered `ConstraintEntry` references; may be empty.
    pub constraints: Vec<Uuid>,
    /// Distinct variable UUIDs referenced (directly or transitively) by the
    /// objective and constraints, in first-seen order. Positionally aligned
    /// with the `variables` field of the `cvxrust::Problem` passed to
    /// `cvxrust::solve`.
    pub variables: Vec<Uuid>,
}

/// A solver result stored in the registry, keyed by UUID and an optional
/// unique name.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultEntry {
    pub uuid: Uuid,
    pub name: Option<String>,
    /// References a `ProblemEntry`.
    pub problem: Uuid,
    pub status: SolveStatus,
    pub objective_value: Option<f64>,
    /// Solved values keyed by `VariableEntry` UUID, row-major per the
    /// variable's shape. Empty when `status` is not `Optimal`.
    pub variable_values: HashMap<Uuid, Vec<f64>>,
}

/// A UUID- and name-indexed table shared by every kind of registry object
/// (parameters today; variables, expressions, and more in later
/// specifications). Insertion is content-addressed: supplying the same hash
/// again returns the existing UUID instead of creating a duplicate entry.
struct RegistryTable<T> {
    by_uuid: HashMap<Uuid, T>,
    by_name: HashMap<String, Uuid>,
    by_hash: HashMap<u64, Uuid>,
}

impl<T: Clone> RegistryTable<T> {
    fn new() -> Self {
        RegistryTable {
            by_uuid: HashMap::new(),
            by_name: HashMap::new(),
            by_hash: HashMap::new(),
        }
    }

    /// Inserts an entry built by `build`, reusing a cached UUID when
    /// `content_hash` already exists. `build` receives the freshly generated
    /// UUID and must produce the entry to store.
    fn insert_content_addressed(
        &mut self,
        content_hash: u64,
        name: Option<String>,
        build: impl FnOnce(Uuid) -> T,
    ) -> Result<Uuid, CvxError> {
        if let Some(uuid) = self.by_hash.get(&content_hash).copied() {
            return Ok(uuid);
        }

        if let Some(name) = &name {
            if self.by_name.contains_key(name) {
                return Err(CvxError::DuplicateName(name.clone()));
            }
        }

        let uuid = Uuid::new_v4();
        let entry = build(uuid);
        self.by_uuid.insert(uuid, entry);
        self.by_hash.insert(content_hash, uuid);
        if let Some(name) = name {
            self.by_name.insert(name, uuid);
        }

        Ok(uuid)
    }

    fn get_by_uuid(&self, uuid: Uuid) -> Option<T> {
        self.by_uuid.get(&uuid).cloned()
    }

    fn get_by_name(&self, name: &str) -> Option<T> {
        let uuid = *self.by_name.get(name)?;
        self.by_uuid.get(&uuid).cloned()
    }
}

/// Thread-safe registry of `cvxx` objects. Each object kind gets its own
/// lock-protected table.
pub struct Registry {
    parameters: RwLock<RegistryTable<ParameterEntry>>,
    variables: RwLock<RegistryTable<VariableEntry>>,
    expressions: RwLock<RegistryTable<ExpressionEntry>>,
    constraints: RwLock<RegistryTable<ConstraintEntry>>,
    constraint_sets: RwLock<RegistryTable<ConstraintSetEntry>>,
    objectives: RwLock<RegistryTable<ObjectiveEntry>>,
    problems: RwLock<RegistryTable<ProblemEntry>>,
    results: RwLock<RegistryTable<ResultEntry>>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            parameters: RwLock::new(RegistryTable::new()),
            variables: RwLock::new(RegistryTable::new()),
            expressions: RwLock::new(RegistryTable::new()),
            constraints: RwLock::new(RegistryTable::new()),
            constraint_sets: RwLock::new(RegistryTable::new()),
            objectives: RwLock::new(RegistryTable::new()),
            problems: RwLock::new(RegistryTable::new()),
            results: RwLock::new(RegistryTable::new()),
        }
    }

    /// The process-wide registry shared by all Excel calls.
    pub fn global() -> &'static Registry {
        static INSTANCE: OnceLock<Registry> = OnceLock::new();
        INSTANCE.get_or_init(Registry::new)
    }

    /// Inserts a parameter, reusing a cached entry when the same data and
    /// name were already registered. Returns the parameter's handle.
    pub fn insert_parameter(
        &self,
        name: Option<String>,
        shape: (usize, usize),
        data: Vec<f64>,
    ) -> Result<String, CvxError> {
        let content_hash = content_hash(name.as_deref(), shape, &data);
        let entry_name = name.clone();

        let mut table = self
            .parameters
            .write()
            .map_err(|_| CvxError::Registry("registry lock poisoned".to_string()))?;

        let uuid =
            table.insert_content_addressed(content_hash, name, move |uuid| ParameterEntry {
                uuid,
                name: entry_name,
                shape,
                data,
                content_hash,
            })?;

        Ok(format_handle(HandleKind::Param, uuid))
    }

    pub fn get_parameter_by_uuid(&self, uuid: Uuid) -> Option<ParameterEntry> {
        self.parameters.read().ok()?.get_by_uuid(uuid)
    }

    pub fn get_parameter_by_name(&self, name: &str) -> Option<ParameterEntry> {
        self.parameters.read().ok()?.get_by_name(name)
    }

    /// Inserts a variable with the requested shape and optional name. Two
    /// calls with no name always create distinct entries. Reusing a name
    /// overwrites the previous variable so Excel formulas can be edited
    /// and recalculated without manual cleanup.
    pub fn insert_variable(
        &self,
        name: Option<String>,
        shape: (usize, usize),
    ) -> Result<String, CvxError> {
        let entry_name = name.clone();

        let mut table = self
            .variables
            .write()
            .map_err(|_| CvxError::Registry("registry lock poisoned".to_string()))?;

        let uuid = Uuid::new_v4();
        let entry = VariableEntry {
            uuid,
            name: entry_name,
            shape,
            variable: Variable::new(uuid.as_u64_pair().0, shape),
        };

        // Overwrite any existing variable with the same name for
        // convenient Excel editing.
        if let Some(old_name) = &name {
            if let Some(old) = table.by_name.get(old_name).copied() {
                table.by_uuid.remove(&old);
            }
            table.by_name.insert(old_name.clone(), uuid);
        }

        table.by_uuid.insert(uuid, entry);
        Ok(format_handle(HandleKind::Var, uuid))
    }

    pub fn get_variable_by_uuid(&self, uuid: Uuid) -> Option<VariableEntry> {
        self.variables.read().ok()?.get_by_uuid(uuid)
    }

    pub fn get_variable_by_name(&self, name: &str) -> Option<VariableEntry> {
        self.variables.read().ok()?.get_by_name(name)
    }

    /// Inserts an expression. Expressions are distinct objects even when
    /// structurally identical. If a name is reused, the old entry is
    /// overwritten so editing and recalculating Excel formulas is
    /// convenient.
    pub fn insert_expression(
        &self,
        name: Option<String>,
        expression: Expression,
        dependencies: Vec<String>,
    ) -> Result<String, CvxError> {
        let entry_name = name.clone();

        let mut table = self
            .expressions
            .write()
            .map_err(|_| CvxError::Registry("registry lock poisoned".to_string()))?;

        let uuid = Uuid::new_v4();
        let entry = ExpressionEntry {
            uuid,
            name: entry_name,
            expression,
            dependencies,
        };

        // If this name was already used, drop the old entry so repeated
        // Excel edits reuse the same name with new content.
        if let Some(old_name) = &name {
            if let Some(old) = table.by_name.get(old_name).copied() {
                table.by_uuid.remove(&old);
            }
            table.by_name.insert(old_name.clone(), uuid);
        }

        table.by_uuid.insert(uuid, entry);
        Ok(format_handle(HandleKind::Expr, uuid))
    }

    pub fn get_expression_by_uuid(&self, uuid: Uuid) -> Option<ExpressionEntry> {
        self.expressions.read().ok()?.get_by_uuid(uuid)
    }

    pub fn get_expression_by_name(&self, name: &str) -> Option<ExpressionEntry> {
        self.expressions.read().ok()?.get_by_name(name)
    }

    /// Inserts a constraint. Constraints are distinct objects even when
    /// structurally identical. If a name is reused, the old entry is
    /// overwritten so editing and recalculating Excel formulas is
    /// convenient.
    pub fn insert_constraint(
        &self,
        name: Option<String>,
        relation: Relation,
        lhs: Expression,
        rhs: Expression,
        dependencies: Vec<String>,
    ) -> Result<String, CvxError> {
        let entry_name = name.clone();

        let mut table = self
            .constraints
            .write()
            .map_err(|_| CvxError::Registry("registry lock poisoned".to_string()))?;

        let uuid = Uuid::new_v4();
        let entry = ConstraintEntry {
            uuid,
            name: entry_name,
            relation,
            lhs,
            rhs,
            dependencies,
        };

        // If this name was already used, drop the old entry so repeated
        // Excel edits reuse the same name with new content.
        if let Some(old_name) = &name {
            if let Some(old) = table.by_name.get(old_name).copied() {
                table.by_uuid.remove(&old);
            }
            table.by_name.insert(old_name.clone(), uuid);
        }

        table.by_uuid.insert(uuid, entry);
        Ok(format_handle(HandleKind::Constr, uuid))
    }

    pub fn get_constraint_by_uuid(&self, uuid: Uuid) -> Option<ConstraintEntry> {
        self.constraints.read().ok()?.get_by_uuid(uuid)
    }

    pub fn get_constraint_by_name(&self, name: &str) -> Option<ConstraintEntry> {
        self.constraints.read().ok()?.get_by_name(name)
    }

    /// Combines an ordered list of constraint UUIDs into a constraint set.
    /// If a name is reused, the old entry is overwritten so editing and
    /// recalculating Excel formulas is convenient.
    pub fn insert_constraint_set(
        &self,
        name: Option<String>,
        constraints: Vec<Uuid>,
    ) -> Result<String, CvxError> {
        let entry_name = name.clone();

        let mut table = self
            .constraint_sets
            .write()
            .map_err(|_| CvxError::Registry("registry lock poisoned".to_string()))?;

        let uuid = Uuid::new_v4();
        let entry = ConstraintSetEntry {
            uuid,
            name: entry_name,
            constraints,
        };

        if let Some(old_name) = &name {
            if let Some(old) = table.by_name.get(old_name).copied() {
                table.by_uuid.remove(&old);
            }
            table.by_name.insert(old_name.clone(), uuid);
        }

        table.by_uuid.insert(uuid, entry);
        Ok(format_handle(HandleKind::ConstrSet, uuid))
    }

    pub fn get_constraint_set_by_uuid(&self, uuid: Uuid) -> Option<ConstraintSetEntry> {
        self.constraint_sets.read().ok()?.get_by_uuid(uuid)
    }

    pub fn get_constraint_set_by_name(&self, name: &str) -> Option<ConstraintSetEntry> {
        self.constraint_sets.read().ok()?.get_by_name(name)
    }

    /// Inserts an objective. If a name is reused, the old entry is
    /// overwritten so editing and recalculating Excel formulas is
    /// convenient.
    pub fn insert_objective(
        &self,
        name: Option<String>,
        sense: Sense,
        expression: Expression,
        dependencies: Vec<String>,
    ) -> Result<String, CvxError> {
        let entry_name = name.clone();

        let mut table = self
            .objectives
            .write()
            .map_err(|_| CvxError::Registry("registry lock poisoned".to_string()))?;

        let uuid = Uuid::new_v4();
        let entry = ObjectiveEntry {
            uuid,
            name: entry_name,
            sense,
            expression,
            dependencies,
        };

        if let Some(old_name) = &name {
            if let Some(old) = table.by_name.get(old_name).copied() {
                table.by_uuid.remove(&old);
            }
            table.by_name.insert(old_name.clone(), uuid);
        }

        table.by_uuid.insert(uuid, entry);
        Ok(format_handle(HandleKind::Obj, uuid))
    }

    pub fn get_objective_by_uuid(&self, uuid: Uuid) -> Option<ObjectiveEntry> {
        self.objectives.read().ok()?.get_by_uuid(uuid)
    }

    pub fn get_objective_by_name(&self, name: &str) -> Option<ObjectiveEntry> {
        self.objectives.read().ok()?.get_by_name(name)
    }

    /// Inserts a problem. If a name is reused, the old entry is overwritten
    /// so editing and recalculating Excel formulas is convenient.
    pub fn insert_problem(
        &self,
        name: Option<String>,
        objective: Uuid,
        constraints: Vec<Uuid>,
        variables: Vec<Uuid>,
    ) -> Result<String, CvxError> {
        let entry_name = name.clone();

        let mut table = self
            .problems
            .write()
            .map_err(|_| CvxError::Registry("registry lock poisoned".to_string()))?;

        let uuid = Uuid::new_v4();
        let entry = ProblemEntry {
            uuid,
            name: entry_name,
            objective,
            constraints,
            variables,
        };

        if let Some(old_name) = &name {
            if let Some(old) = table.by_name.get(old_name).copied() {
                table.by_uuid.remove(&old);
            }
            table.by_name.insert(old_name.clone(), uuid);
        }

        table.by_uuid.insert(uuid, entry);
        Ok(format_handle(HandleKind::Prob, uuid))
    }

    pub fn get_problem_by_uuid(&self, uuid: Uuid) -> Option<ProblemEntry> {
        self.problems.read().ok()?.get_by_uuid(uuid)
    }

    pub fn get_problem_by_name(&self, name: &str) -> Option<ProblemEntry> {
        self.problems.read().ok()?.get_by_name(name)
    }

    /// Inserts a solver result. If a name is reused, the old entry is
    /// overwritten so editing and recalculating Excel formulas is
    /// convenient.
    pub fn insert_result(
        &self,
        name: Option<String>,
        problem: Uuid,
        status: SolveStatus,
        objective_value: Option<f64>,
        variable_values: HashMap<Uuid, Vec<f64>>,
    ) -> Result<String, CvxError> {
        let entry_name = name.clone();

        let mut table = self
            .results
            .write()
            .map_err(|_| CvxError::Registry("registry lock poisoned".to_string()))?;

        let uuid = Uuid::new_v4();
        let entry = ResultEntry {
            uuid,
            name: entry_name,
            problem,
            status,
            objective_value,
            variable_values,
        };

        if let Some(old_name) = &name {
            if let Some(old) = table.by_name.get(old_name).copied() {
                table.by_uuid.remove(&old);
            }
            table.by_name.insert(old_name.clone(), uuid);
        }

        table.by_uuid.insert(uuid, entry);
        Ok(format_handle(HandleKind::Result, uuid))
    }

    pub fn get_result_by_uuid(&self, uuid: Uuid) -> Option<ResultEntry> {
        self.results.read().ok()?.get_by_uuid(uuid)
    }

    pub fn get_result_by_name(&self, name: &str) -> Option<ResultEntry> {
        self.results.read().ok()?.get_by_name(name)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new()
    }
}

fn content_hash(name: Option<&str>, shape: (usize, usize), data: &[f64]) -> u64 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    shape.hash(&mut hasher);
    for value in data {
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_and_looks_up_by_uuid() {
        let registry = Registry::new();
        let handle = registry.insert_parameter(None, (1, 1), vec![1.0]).unwrap();
        let (_, uuid) = crate::core::handle::parse_handle(&handle).unwrap();
        assert_eq!(
            registry.get_parameter_by_uuid(uuid).unwrap().data,
            vec![1.0]
        );
    }

    #[test]
    fn inserts_and_looks_up_by_name() {
        let registry = Registry::new();
        registry
            .insert_parameter(Some("p".to_string()), (2, 1), vec![1.0, 2.0])
            .unwrap();
        let entry = registry.get_parameter_by_name("p").unwrap();
        assert_eq!(entry.shape, (2, 1));
    }

    #[test]
    fn rejects_duplicate_names() {
        let registry = Registry::new();
        registry
            .insert_parameter(Some("p".to_string()), (1, 1), vec![1.0])
            .unwrap();
        let err = registry
            .insert_parameter(Some("p".to_string()), (1, 1), vec![2.0])
            .unwrap_err();
        assert_eq!(err, CvxError::DuplicateName("p".to_string()));
    }

    #[test]
    fn reuses_cached_entry_for_identical_data_and_name() {
        let registry = Registry::new();
        let first = registry
            .insert_parameter(Some("p".to_string()), (1, 2), vec![1.0, 2.0])
            .unwrap();
        let second = registry
            .insert_parameter(Some("p".to_string()), (1, 2), vec![1.0, 2.0])
            .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn distinguishes_unnamed_parameters_by_content() {
        let registry = Registry::new();
        let first = registry.insert_parameter(None, (1, 1), vec![1.0]).unwrap();
        let second = registry.insert_parameter(None, (1, 1), vec![2.0]).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn inserts_and_looks_up_variable_by_uuid() {
        let registry = Registry::new();
        let handle = registry
            .insert_variable(Some("x".to_string()), (3, 1))
            .unwrap();
        let (kind, uuid) = crate::core::handle::parse_handle(&handle).unwrap();
        assert_eq!(kind, HandleKind::Var);
        let entry = registry.get_variable_by_uuid(uuid).unwrap();
        assert_eq!(entry.shape, (3, 1));
        assert_eq!(entry.variable.shape, (3, 1));
    }

    #[test]
    fn inserts_and_looks_up_variable_by_name() {
        let registry = Registry::new();
        registry
            .insert_variable(Some("x".to_string()), (1, 4))
            .unwrap();
        let entry = registry.get_variable_by_name("x").unwrap();
        assert_eq!(entry.shape, (1, 4));
    }

    #[test]
    fn overwrites_named_variable_with_same_name() {
        let registry = Registry::new();
        let first = registry
            .insert_variable(Some("x".to_string()), (1, 1))
            .unwrap();
        let second = registry
            .insert_variable(Some("x".to_string()), (2, 2))
            .unwrap();
        assert_ne!(first, second);

        let (_, first_uuid) = crate::core::handle::parse_handle(&first).unwrap();
        let entry = registry.get_variable_by_name("x").unwrap();
        assert_eq!(entry.shape, (2, 2));
        assert!(registry.get_variable_by_uuid(first_uuid).is_none());
    }

    #[test]
    fn creates_distinct_variables_for_identical_inputs() {
        let registry = Registry::new();
        let first = registry.insert_variable(None, (2, 2)).unwrap();
        let second = registry.insert_variable(None, (2, 2)).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn inserts_and_looks_up_expression_by_uuid() {
        let registry = Registry::new();
        let expr = Expression::constant(1.0);
        let handle = registry
            .insert_expression(Some("e".to_string()), expr.clone(), vec![])
            .unwrap();
        let (kind, uuid) = crate::core::handle::parse_handle(&handle).unwrap();
        assert_eq!(kind, HandleKind::Expr);
        let entry = registry.get_expression_by_uuid(uuid).unwrap();
        assert_eq!(entry.expression, expr);
    }

    #[test]
    fn inserts_and_looks_up_expression_by_name() {
        let registry = Registry::new();
        registry
            .insert_expression(
                Some("profit".to_string()),
                Expression::constant(100.0),
                vec![],
            )
            .unwrap();
        let entry = registry.get_expression_by_name("profit").unwrap();
        assert_eq!(entry.expression, Expression::constant(100.0));
    }

    #[test]
    fn overwrites_named_expression_with_same_name() {
        let registry = Registry::new();
        let first = registry
            .insert_expression(Some("e".to_string()), Expression::constant(1.0), vec![])
            .unwrap();
        let second = registry
            .insert_expression(Some("e".to_string()), Expression::constant(2.0), vec![])
            .unwrap();
        assert_ne!(first, second);

        let (_, first_uuid) = crate::core::handle::parse_handle(&first).unwrap();
        let entry = registry.get_expression_by_name("e").unwrap();
        assert_eq!(entry.expression, Expression::constant(2.0));
        assert!(registry.get_expression_by_uuid(first_uuid).is_none());
    }

    #[test]
    fn creates_distinct_expressions_for_identical_inputs() {
        let registry = Registry::new();
        let first = registry
            .insert_expression(None, Expression::constant(1.0), vec![])
            .unwrap();
        let second = registry
            .insert_expression(None, Expression::constant(1.0), vec![])
            .unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn inserts_and_looks_up_constraint_by_uuid() {
        let registry = Registry::new();
        let handle = registry
            .insert_constraint(
                Some("c".to_string()),
                Relation::LessEqual,
                Expression::constant(1.0),
                Expression::constant(2.0),
                vec![],
            )
            .unwrap();
        let (kind, uuid) = crate::core::handle::parse_handle(&handle).unwrap();
        assert_eq!(kind, HandleKind::Constr);
        let entry = registry.get_constraint_by_uuid(uuid).unwrap();
        assert_eq!(entry.relation, Relation::LessEqual);
        assert_eq!(entry.lhs, Expression::constant(1.0));
        assert_eq!(entry.rhs, Expression::constant(2.0));
    }

    #[test]
    fn inserts_and_looks_up_constraint_by_name() {
        let registry = Registry::new();
        registry
            .insert_constraint(
                Some("budget".to_string()),
                Relation::Equal,
                Expression::constant(1.0),
                Expression::constant(1.0),
                vec![],
            )
            .unwrap();
        let entry = registry.get_constraint_by_name("budget").unwrap();
        assert_eq!(entry.relation, Relation::Equal);
    }

    #[test]
    fn overwrites_named_constraint_with_same_name() {
        let registry = Registry::new();
        let first = registry
            .insert_constraint(
                Some("c".to_string()),
                Relation::LessEqual,
                Expression::constant(1.0),
                Expression::constant(2.0),
                vec![],
            )
            .unwrap();
        let second = registry
            .insert_constraint(
                Some("c".to_string()),
                Relation::GreaterEqual,
                Expression::constant(3.0),
                Expression::constant(4.0),
                vec![],
            )
            .unwrap();
        assert_ne!(first, second);

        let (_, first_uuid) = crate::core::handle::parse_handle(&first).unwrap();
        let entry = registry.get_constraint_by_name("c").unwrap();
        assert_eq!(entry.relation, Relation::GreaterEqual);
        assert!(registry.get_constraint_by_uuid(first_uuid).is_none());
    }

    #[test]
    fn creates_distinct_constraints_for_identical_inputs() {
        let registry = Registry::new();
        let first = registry
            .insert_constraint(
                None,
                Relation::Equal,
                Expression::constant(1.0),
                Expression::constant(1.0),
                vec![],
            )
            .unwrap();
        let second = registry
            .insert_constraint(
                None,
                Relation::Equal,
                Expression::constant(1.0),
                Expression::constant(1.0),
                vec![],
            )
            .unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn inserts_and_looks_up_constraint_set_by_uuid() {
        let registry = Registry::new();
        let c1 = registry
            .insert_constraint(
                None,
                Relation::LessEqual,
                Expression::constant(1.0),
                Expression::constant(2.0),
                vec![],
            )
            .unwrap();
        let (_, c1_uuid) = crate::core::handle::parse_handle(&c1).unwrap();

        let handle = registry
            .insert_constraint_set(Some("cs".to_string()), vec![c1_uuid])
            .unwrap();
        let (kind, uuid) = crate::core::handle::parse_handle(&handle).unwrap();
        assert_eq!(kind, HandleKind::ConstrSet);
        let entry = registry.get_constraint_set_by_uuid(uuid).unwrap();
        assert_eq!(entry.constraints, vec![c1_uuid]);
    }

    #[test]
    fn inserts_and_looks_up_constraint_set_by_name() {
        let registry = Registry::new();
        registry
            .insert_constraint_set(Some("cs".to_string()), vec![Uuid::new_v4()])
            .unwrap();
        assert!(registry.get_constraint_set_by_name("cs").is_some());
    }

    #[test]
    fn overwrites_named_constraint_set_with_same_name() {
        let registry = Registry::new();
        let first = registry
            .insert_constraint_set(Some("cs".to_string()), vec![Uuid::new_v4()])
            .unwrap();
        let second_uuid = Uuid::new_v4();
        let second = registry
            .insert_constraint_set(Some("cs".to_string()), vec![second_uuid])
            .unwrap();
        assert_ne!(first, second);

        let (_, first_uuid) = crate::core::handle::parse_handle(&first).unwrap();
        let entry = registry.get_constraint_set_by_name("cs").unwrap();
        assert_eq!(entry.constraints, vec![second_uuid]);
        assert!(registry.get_constraint_set_by_uuid(first_uuid).is_none());
    }

    #[test]
    fn inserts_and_looks_up_objective_by_uuid() {
        let registry = Registry::new();
        let handle = registry
            .insert_objective(
                Some("obj".to_string()),
                Sense::Minimize,
                Expression::constant(1.0),
                vec![],
            )
            .unwrap();
        let (kind, uuid) = crate::core::handle::parse_handle(&handle).unwrap();
        assert_eq!(kind, HandleKind::Obj);
        let entry = registry.get_objective_by_uuid(uuid).unwrap();
        assert_eq!(entry.sense, Sense::Minimize);
        assert_eq!(entry.expression, Expression::constant(1.0));
    }

    #[test]
    fn inserts_and_looks_up_objective_by_name() {
        let registry = Registry::new();
        registry
            .insert_objective(
                Some("profit".to_string()),
                Sense::Maximize,
                Expression::constant(2.0),
                vec![],
            )
            .unwrap();
        let entry = registry.get_objective_by_name("profit").unwrap();
        assert_eq!(entry.sense, Sense::Maximize);
    }

    #[test]
    fn overwrites_named_objective_with_same_name() {
        let registry = Registry::new();
        let first = registry
            .insert_objective(
                Some("obj".to_string()),
                Sense::Minimize,
                Expression::constant(1.0),
                vec![],
            )
            .unwrap();
        let second = registry
            .insert_objective(
                Some("obj".to_string()),
                Sense::Maximize,
                Expression::constant(2.0),
                vec![],
            )
            .unwrap();
        assert_ne!(first, second);

        let (_, first_uuid) = crate::core::handle::parse_handle(&first).unwrap();
        let entry = registry.get_objective_by_name("obj").unwrap();
        assert_eq!(entry.sense, Sense::Maximize);
        assert!(registry.get_objective_by_uuid(first_uuid).is_none());
    }

    #[test]
    fn inserts_and_looks_up_problem_by_uuid() {
        let registry = Registry::new();
        let objective = Uuid::new_v4();
        let constraint = Uuid::new_v4();
        let variable = Uuid::new_v4();
        let handle = registry
            .insert_problem(
                Some("p".to_string()),
                objective,
                vec![constraint],
                vec![variable],
            )
            .unwrap();
        let (kind, uuid) = crate::core::handle::parse_handle(&handle).unwrap();
        assert_eq!(kind, HandleKind::Prob);
        let entry = registry.get_problem_by_uuid(uuid).unwrap();
        assert_eq!(entry.objective, objective);
        assert_eq!(entry.constraints, vec![constraint]);
        assert_eq!(entry.variables, vec![variable]);
    }

    #[test]
    fn inserts_and_looks_up_problem_by_name() {
        let registry = Registry::new();
        registry
            .insert_problem(Some("p".to_string()), Uuid::new_v4(), vec![], vec![])
            .unwrap();
        assert!(registry.get_problem_by_name("p").is_some());
    }

    #[test]
    fn overwrites_named_problem_with_same_name() {
        let registry = Registry::new();
        let first = registry
            .insert_problem(Some("p".to_string()), Uuid::new_v4(), vec![], vec![])
            .unwrap();
        let second = registry
            .insert_problem(Some("p".to_string()), Uuid::new_v4(), vec![], vec![])
            .unwrap();
        assert_ne!(first, second);

        let (_, first_uuid) = crate::core::handle::parse_handle(&first).unwrap();
        assert!(registry.get_problem_by_uuid(first_uuid).is_none());
    }

    #[test]
    fn inserts_and_looks_up_result_by_uuid() {
        let registry = Registry::new();
        let problem = Uuid::new_v4();
        let variable = Uuid::new_v4();
        let mut variable_values = HashMap::new();
        variable_values.insert(variable, vec![1.0, 2.0]);

        let handle = registry
            .insert_result(
                Some("r".to_string()),
                problem,
                SolveStatus::Optimal,
                Some(3.0),
                variable_values.clone(),
            )
            .unwrap();
        let (kind, uuid) = crate::core::handle::parse_handle(&handle).unwrap();
        assert_eq!(kind, HandleKind::Result);
        let entry = registry.get_result_by_uuid(uuid).unwrap();
        assert_eq!(entry.problem, problem);
        assert_eq!(entry.status, SolveStatus::Optimal);
        assert_eq!(entry.objective_value, Some(3.0));
        assert_eq!(entry.variable_values, variable_values);
    }

    #[test]
    fn inserts_and_looks_up_result_by_name() {
        let registry = Registry::new();
        registry
            .insert_result(
                Some("r".to_string()),
                Uuid::new_v4(),
                SolveStatus::Infeasible,
                None,
                HashMap::new(),
            )
            .unwrap();
        let entry = registry.get_result_by_name("r").unwrap();
        assert_eq!(entry.status, SolveStatus::Infeasible);
    }

    #[test]
    fn overwrites_named_result_with_same_name() {
        let registry = Registry::new();
        let first = registry
            .insert_result(
                Some("r".to_string()),
                Uuid::new_v4(),
                SolveStatus::Optimal,
                Some(1.0),
                HashMap::new(),
            )
            .unwrap();
        let second = registry
            .insert_result(
                Some("r".to_string()),
                Uuid::new_v4(),
                SolveStatus::Unbounded,
                None,
                HashMap::new(),
            )
            .unwrap();
        assert_ne!(first, second);

        let (_, first_uuid) = crate::core::handle::parse_handle(&first).unwrap();
        let entry = registry.get_result_by_name("r").unwrap();
        assert_eq!(entry.status, SolveStatus::Unbounded);
        assert!(registry.get_result_by_uuid(first_uuid).is_none());
    }
}
