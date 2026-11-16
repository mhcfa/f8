pub mod boolean;
pub mod branching;
pub mod error_handling;

pub use boolean::{AndFunction, NotFunction, OrFunction};
pub use branching::{ChooseFunction, IfFunction, IfsFunction};
pub use error_handling::{IferrorFunction, IfnaFunction};

use std::sync::Arc;

use crate::functions::registry::FunctionRegistry;

/// Registers all standard logical functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(IfFunction));
    registry.register(Arc::new(IfsFunction));
    registry.register(Arc::new(IferrorFunction));
    registry.register(Arc::new(IfnaFunction));
    registry.register(Arc::new(AndFunction));
    registry.register(Arc::new(OrFunction));
    registry.register(Arc::new(NotFunction));
    registry.register(Arc::new(ChooseFunction));
}
