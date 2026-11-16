use crate::functions::common::{LogicalCollector, scalar_bool};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ValueType};

static AND_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "logical1",
    "The first condition or range to evaluate.",
    ParamKind::ScalarOrRange,
    ParamType::Boolean,
)
.allow_empty_in_slice()];

/// `AND(logical1, [logical2], ...)`: Returns TRUE if all of its arguments are TRUE.
#[derive(Debug, Default, Clone, Copy)]
pub struct AndFunction;

impl ExcelFunction for AndFunction {
    fn name(&self) -> &'static str {
        "AND"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        AND_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Boolean)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match LogicalCollector::collect(args) {
            Ok(c) => Ok(CellValue::Boolean(c.all_true)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

static OR_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "logical1",
    "The first condition or range to evaluate.",
    ParamKind::ScalarOrRange,
    ParamType::Boolean,
)
.allow_empty_in_slice()];

/// `OR(logical1, [logical2], ...)`: Returns TRUE if any argument is TRUE.
#[derive(Debug, Default, Clone, Copy)]
pub struct OrFunction;

impl ExcelFunction for OrFunction {
    fn name(&self) -> &'static str {
        "OR"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        OR_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Boolean)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match LogicalCollector::collect(args) {
            Ok(c) => Ok(CellValue::Boolean(c.any_true)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

static NOT_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "logical",
    "A boolean value or expression that evaluates to TRUE or FALSE.",
    ParamKind::Scalar,
    ParamType::Boolean,
)];

/// `NOT(logical)`: Reverses the logic of its argument.
#[derive(Debug, Default, Clone, Copy)]
pub struct NotFunction;

impl ExcelFunction for NotFunction {
    fn name(&self) -> &'static str {
        "NOT"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        NOT_PARAMS
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
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match scalar_bool(&args[0]) {
            Ok(b) => Ok(CellValue::Boolean(!b)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
