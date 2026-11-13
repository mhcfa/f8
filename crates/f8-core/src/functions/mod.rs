//! Excel function registry, parameter contracts, and standard catalog.
//!
//! Provides the `ExcelFunction` trait, parameter definitions, and built-in
//! Excel functions.

pub mod common;
pub mod date;
pub mod error;
pub mod financial;
pub mod information;
pub mod logical;
pub mod lookup;
pub mod math;
pub mod registry;
pub mod traits;

pub use error::FunctionError;
pub use registry::{FunctionRegistry, StageAError};
pub use traits::{
    BranchPolicy, ComparisonSide, EmptyPolicy, EvalContext, ExcelFunction, ExpectationOrigin,
    FunctionArg, FunctionCategory, LookupPolicy, ParamKind, ParamRequirement, ParamType,
    ParameterDef, ReferenceKind, RolePolicy, ShapePolicy, TypeExpectation, TypedRangeGrid,
    ValidationPolicy, VariadicRule,
};
