//! Error predicate and error generation functions (`ISERROR`, `NA`).

use std::sync::Arc;

use crate::functions::error::FunctionError;
use crate::functions::registry::FunctionRegistry;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Positional parameter definitions for `ISERROR`.
static ISERROR_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "value",
    "The value or expression you want to test for any error.",
    ParamKind::Scalar,
    ParamType::Any,
)];

/// `ISERROR(value)`: Returns `TRUE` if `value` evaluates to any Excel error value, and `FALSE` otherwise.
#[derive(Debug, Default, Clone, Copy)]
pub struct IserrorFunction;

impl ExcelFunction for IserrorFunction {
    fn name(&self) -> &'static str {
        "ISERROR"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Information
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        ISERROR_PARAMS
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
        let val = args[0].eval_scalar(self.name(), "value", 1, ctx)?;
        Ok(CellValue::Boolean(matches!(val, CellValue::Error(_))))
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
        _args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        Ok(CellValue::Error(ExcelError::Na))
    }
}

/// Registers all standard information functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(IserrorFunction));
    registry.register(Arc::new(NaFunction));
}
