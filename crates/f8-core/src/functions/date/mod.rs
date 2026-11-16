//! Date and time functions suite (`DATE`, `EDATE`, `EOMONTH`, `YEARFRAC`, `YEAR`, `MONTH`, `DAY`).

pub mod constructors;
pub mod parts;
pub mod yearfrac;

use std::sync::Arc;

pub use constructors::{DateFunction, EdateFunction, EomonthFunction};
pub use parts::{DayFunction, MonthFunction, YearFunction};
pub use yearfrac::{DayCountBasis, YearfracFunction, calculate_yearfrac};

use crate::functions::registry::FunctionRegistry;

/// Registers all standard Date & Time functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(DateFunction));
    registry.register(Arc::new(EdateFunction));
    registry.register(Arc::new(EomonthFunction));
    registry.register(Arc::new(YearfracFunction));
    registry.register(Arc::new(YearFunction));
    registry.register(Arc::new(MonthFunction));
    registry.register(Arc::new(DayFunction));
}
