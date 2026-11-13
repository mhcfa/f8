use crate::functions::common::{LogicalCollector, extract_scalar_bool};
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ValueType};

static AND_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "logical1",
    "The first condition or range to evaluate.",
    ParamKind::RangeOrArray,
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let collector = match LogicalCollector::collect(self.name(), "logical1", args, ctx)? {
            Ok(c) => c,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        Ok(CellValue::Boolean(collector.all_true))
    }
}

static OR_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "logical1",
    "The first condition or range to evaluate.",
    ParamKind::RangeOrArray,
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let collector = match LogicalCollector::collect(self.name(), "logical1", args, ctx)? {
            Ok(c) => c,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        Ok(CellValue::Boolean(collector.any_true))
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        match extract_scalar_bool(&args[0], self.name(), "logical", 1, ctx)? {
            Ok(b) => Ok(CellValue::Boolean(!b)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
