pub mod aggregation;
pub mod basic;
pub mod product;
pub mod rounding;

pub use aggregation::{
    AverageFunction, CountFunction, MaxFunction, MedianFunction, MinFunction, SumFunction,
};
pub use basic::AbsFunction;
pub use product::SumproductFunction;
pub use rounding::{RoundFunction, RounddownFunction, RoundupFunction};

use std::sync::Arc;

use crate::functions::registry::FunctionRegistry;

/// Registers all standard mathematical and aggregation functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(AbsFunction));
    registry.register(Arc::new(SumFunction));
    registry.register(Arc::new(AverageFunction));
    registry.register(Arc::new(CountFunction));
    registry.register(Arc::new(MaxFunction));
    registry.register(Arc::new(MinFunction));
    registry.register(Arc::new(MedianFunction));
    registry.register(Arc::new(RoundFunction));
    registry.register(Arc::new(RoundupFunction));
    registry.register(Arc::new(RounddownFunction));
    registry.register(Arc::new(SumproductFunction));
}
