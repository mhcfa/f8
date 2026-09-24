//! Date and time functions suite (`DATE`, `EDATE`, `EOMONTH`, `YEARFRAC`, `YEAR`, `MONTH`, `DAY`, `DAYS`).

pub mod constructors;
pub mod parts;
pub mod serial;
pub mod yearfrac;

pub use constructors::{DateFunction, EdateFunction, EomonthFunction};
pub use parts::{DayFunction, DaysFunction, MonthFunction, YearFunction};
pub use serial::{
    DateSerial, DateTuple, EXCEL_1900_LEAP_DAY_SERIAL, MAX_EXCEL_SERIAL, MIN_EXCEL_SERIAL,
    UNIX_EPOCH_SERIAL, validate_date_serial,
};
pub use yearfrac::{DayCountBasis, YearfracFunction, calculate_yearfrac};

use crate::functions::registry::FunctionRegistry;
use std::sync::Arc;

/// Registers all standard Date & Time functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(DateFunction));
    registry.register(Arc::new(EdateFunction));
    registry.register(Arc::new(EomonthFunction));
    registry.register(Arc::new(YearfracFunction));
    registry.register(Arc::new(YearFunction));
    registry.register(Arc::new(MonthFunction));
    registry.register(Arc::new(DayFunction));
    registry.register(Arc::new(DaysFunction));
}
