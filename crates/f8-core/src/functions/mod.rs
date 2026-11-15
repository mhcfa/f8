//! Excel function registry, parameter contracts, and standard catalog.
//!
//! Provides the `ExcelFunction` trait, parameter definitions, and built-in
//! Excel functions.

pub mod binder;
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
pub mod typecheck;

pub use error::FunctionError;
pub use registry::FunctionRegistry;
pub use traits::{
    BoundCriterion, BranchPolicy, EmptyPolicy, EvalContext, ExcelFunction, FunctionArg,
    FunctionCategory, LookupPolicy, ParamKind, ParamRequirement, ParamType, ParameterDef,
    RangeGrid, ReferenceKind, RolePolicy, ShapePolicy, ValidationPolicy, VariadicRule,
};
pub use typecheck::{StageAError, StaticRef};
