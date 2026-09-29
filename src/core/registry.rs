use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{OnceLock, RwLock};

use uuid::Uuid;

use crate::core::error::CvxError;
use crate::core::handle::{format_handle, HandleKind};

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
    fn insert(
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
/// lock-protected table; only parameters exist so far.
pub struct Registry {
    parameters: RwLock<RegistryTable<ParameterEntry>>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            parameters: RwLock::new(RegistryTable::new()),
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

        let uuid = table.insert(content_hash, name, move |uuid| ParameterEntry {
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
}
