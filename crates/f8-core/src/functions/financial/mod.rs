pub mod annuity;
pub mod discounting;
pub mod irr;

pub use annuity::PmtFunction;
pub use discounting::{NpvFunction, XnpvFunction};
pub use irr::{IrrFunction, XirrFunction};

use std::sync::Arc;

use crate::functions::registry::FunctionRegistry;

/// Registers all standard financial functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(NpvFunction));
    registry.register(Arc::new(XnpvFunction));
    registry.register(Arc::new(IrrFunction));
    registry.register(Arc::new(XirrFunction));
    registry.register(Arc::new(PmtFunction));
}
