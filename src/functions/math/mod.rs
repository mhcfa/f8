pub mod aggregation;
pub mod basic;
pub mod conditional;
pub mod criteria;
pub mod product;
pub mod rounding;

pub use aggregation::{
    AverageFunction, CountFunction, CountaFunction, MaxFunction, MinFunction, SumFunction,
};
pub use basic::AbsFunction;
pub use conditional::{
    AverageifsFunction, CountifsFunction, MaxifsFunction, MinifsFunction, SumifsFunction,
};
pub use criteria::{ComparisonOp, CriteriaTarget, Criterion, WildcardToken};
pub use product::SumproductFunction;
pub use rounding::{RoundFunction, RounddownFunction, RoundupFunction};

use crate::functions::registry::FunctionRegistry;
use std::sync::Arc;

/// Registers all standard mathematical and conditional aggregation functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(AbsFunction));
    registry.register(Arc::new(SumFunction));
    registry.register(Arc::new(AverageFunction));
    registry.register(Arc::new(CountFunction));
    registry.register(Arc::new(CountaFunction));
    registry.register(Arc::new(MaxFunction));
    registry.register(Arc::new(MinFunction));
    registry.register(Arc::new(RoundFunction));
    registry.register(Arc::new(RoundupFunction));
    registry.register(Arc::new(RounddownFunction));
    registry.register(Arc::new(SumproductFunction));
    registry.register(Arc::new(SumifsFunction));
    registry.register(Arc::new(CountifsFunction));
    registry.register(Arc::new(AverageifsFunction));
    registry.register(Arc::new(MinifsFunction));
    registry.register(Arc::new(MaxifsFunction));
}
