//! Cell introspection, error predicates, and error generation functions (`ISBLANK`, `ISERROR`, `ISNA`, `ERROR.TYPE`, `NA`).

use std::sync::Arc;

use crate::functions::error::FunctionError;
use crate::functions::registry::FunctionRegistry;
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
)
.allow_empty()];

/// Positional parameter definitions for `ISERROR`.
static ISERROR_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "value",
    "The value or expression you want to test for any error.",
    ParamKind::Scalar,
    ParamType::Any,
)
.allow_empty()];

/// Positional parameter definitions for `ISNA`.
static ISNA_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "value",
    "The value or expression you want to test for the #N/A error value.",
    ParamKind::Scalar,
    ParamType::Any,
)
.allow_empty()];

/// Positional parameter definitions for `ERROR.TYPE`.
static ERRORTYPE_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "error_val",
    "The error value whose numeric Excel code you want to find.",
    ParamKind::Scalar,
    ParamType::Any,
)
.allow_empty()];

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
        let val = args[0].eval_scalar(self.name(), ctx)?;
        Ok(CellValue::Boolean(matches!(val, CellValue::Empty)))
    }
}

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
        let val = args[0].eval_scalar(self.name(), ctx)?;
        Ok(CellValue::Boolean(matches!(val, CellValue::Error(_))))
    }
}

/// `ISNA(value)`: Returns `TRUE` if `value` evaluates to `#N/A`, and `FALSE` otherwise.
#[derive(Debug, Default, Clone, Copy)]
pub struct IsnaFunction;

impl ExcelFunction for IsnaFunction {
    fn name(&self) -> &'static str {
        "ISNA"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Information
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        ISNA_PARAMS
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
        let val = args[0].eval_scalar(self.name(), ctx)?;
        Ok(CellValue::Boolean(matches!(
            val,
            CellValue::Error(ExcelError::Na)
        )))
    }
}

/// `ERROR.TYPE(error_val)`: Returns the numeric code corresponding to an Excel error value, or `#N/A` otherwise.
#[derive(Debug, Default, Clone, Copy)]
pub struct ErrortypeFunction;

impl ExcelFunction for ErrortypeFunction {
    fn name(&self) -> &'static str {
        "ERROR.TYPE"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Information
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        ERRORTYPE_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let val = args[0].eval_scalar(self.name(), ctx)?;
        match val {
            CellValue::Error(err) => Ok(CellValue::Number(err.error_type_code())),
            CellValue::Number(_)
            | CellValue::Text(_)
            | CellValue::Boolean(_)
            | CellValue::Empty => Ok(CellValue::Error(ExcelError::Na)),
        }
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
    registry.register(Arc::new(IsblankFunction));
    registry.register(Arc::new(IserrorFunction));
    registry.register(Arc::new(IsnaFunction));
    registry.register(Arc::new(ErrortypeFunction));
    registry.register(Arc::new(NaFunction));
}
