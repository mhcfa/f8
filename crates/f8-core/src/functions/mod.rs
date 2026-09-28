//! Excel function registry, parameter contracts, and standard catalog.
//!
//! Provides the `ExcelFunction` trait, parameter definitions, and built-in
//! Excel functions.

pub mod common;
pub mod date;
pub mod error;
pub mod financial;
pub mod logical;
pub mod lookup;
pub mod math;
pub mod registry;
pub mod traits;

pub use error::FunctionError;
pub use registry::FunctionRegistry;
pub use traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
