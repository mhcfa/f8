//! Thread-safe registry and catalog of built-in and user-defined Excel functions.

use std::sync::Arc;

use indexmap::IndexMap;

use crate::functions::traits::ExcelFunction;
pub use crate::functions::typecheck::{StageAError, StaticRef};

/// Thread-safe registry and dispatcher of built-in and user-defined Excel functions.
#[derive(Clone, Default)]
pub struct FunctionRegistry {
    functions: IndexMap<String, Arc<dyn ExcelFunction>>,
}

impl FunctionRegistry {
    /// Constructs an empty function registry with no functions registered.
    #[must_use]
    pub fn new() -> Self {
        Self {
            functions: IndexMap::new(),
        }
    }

    /// Constructs a standard function registry populated with the 43 built-in Excel functions.
    #[must_use]
    pub fn standard() -> Self {
        let mut registry = Self::new();
        crate::functions::math::register(&mut registry);
        crate::functions::logical::register(&mut registry);
        crate::functions::financial::register(&mut registry);
        crate::functions::lookup::register(&mut registry);
        crate::functions::date::register(&mut registry);
        crate::functions::information::register(&mut registry);
        registry
    }

    /// Registers a function implementation, storing it by canonical uppercase name.
    pub fn register(&mut self, func: Arc<dyn ExcelFunction>) {
        self.functions
            .insert(func.name().to_ascii_uppercase(), func);
    }

    fn get_entry(&self, name: &str) -> Option<&Arc<dyn ExcelFunction>> {
        if let Some(func) = self.functions.get(name) {
            return Some(func);
        }
        if name.bytes().any(|b| b.is_ascii_lowercase()) {
            return self.functions.get(name.to_ascii_uppercase().as_str());
        }
        None
    }

    /// Looks up a function by name (case-insensitive).
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&dyn ExcelFunction> {
        self.get_entry(name).map(std::convert::AsRef::as_ref)
    }

    /// Looks up a shared reference to a function by name (case-insensitive).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn ExcelFunction>> {
        self.get_entry(name).cloned()
    }

    /// Returns an iterator over all registered functions in deterministic insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<dyn ExcelFunction>)> {
        self.functions.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Returns the number of registered functions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Returns `true` if no functions are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}
