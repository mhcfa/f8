//! Cell introspection and error generation functions (`ISBLANK`, `NA`).

use std::sync::Arc;

use crate::functions::error::FunctionError;
use crate::functions::registry::{FunctionRegistry, validate_bound_args};
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Positional parameter definitions for `ISBLANK`.
static ISBLANK_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "value",
    "The value or cell reference you want to test for blankness.",
    ParamKind::Scalar,
    ParamType::Any,
)];

/// `ISBLANK(value)`: Returns `TRUE` if `value` refers to an empty cell, and `FALSE` otherwise.
#[derive(Debug, Default, Clone, Copy)]
pub struct IsblankFunction;

impl ExcelFunction for IsblankFunction {
    fn name(&self) -> &'static str {
        "ISBLANK"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Information
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        ISBLANK_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Boolean)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;
        let val = args[0].eval_scalar(self.name(), ctx)?;
        Ok(CellValue::Boolean(matches!(val, CellValue::Empty)))
    }
}

/// `NA()`: Returns the error value `#N/A` (value not available).
#[derive(Debug, Default, Clone, Copy)]
pub struct NaFunction;

impl ExcelFunction for NaFunction {
    fn name(&self) -> &'static str {
        "NA"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Information
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        &[]
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;
        Ok(CellValue::Error(ExcelError::Na))
    }
}

/// Registers all standard information functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(IsblankFunction));
    registry.register(Arc::new(NaFunction));
}
