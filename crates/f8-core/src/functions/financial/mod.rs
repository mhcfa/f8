pub mod annuity;
pub mod discounting;
pub mod irr;

use std::sync::Arc;

pub use annuity::PmtFunction;
pub use discounting::{NpvFunction, XnpvFunction};
pub use irr::{IrrFunction, XirrFunction};

use crate::functions::registry::FunctionRegistry;

/// Registers all standard financial functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(NpvFunction));
    registry.register(Arc::new(XnpvFunction));
    registry.register(Arc::new(IrrFunction));
    registry.register(Arc::new(XirrFunction));
    registry.register(Arc::new(PmtFunction));
}
