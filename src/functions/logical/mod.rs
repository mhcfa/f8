pub mod branching;

pub use branching::IfFunction;

use crate::functions::registry::FunctionRegistry;
use std::sync::Arc;

/// Registers all standard logical functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(IfFunction));
}
