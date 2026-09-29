use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{OnceLock, RwLock};

use uuid::Uuid;

use crate::core::error::CvxError;
use crate::core::handle::{format_handle, HandleKind};
use crate::core::variable::Variable;
use cvxrust::Expression;

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
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            parameters: RwLock::new(RegistryTable::new()),
            variables: RwLock::new(RegistryTable::new()),
            expressions: RwLock::new(RegistryTable::new()),
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
            variable: Variable::new(shape),
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
}
