use crate::functions::common::LogicalCollector;
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::CellValue;

static AND_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "logical1",
    "The first condition or range to evaluate.",
    ParamKind::RangeOrArray,
)];

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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }

        let collector = match LogicalCollector::collect(args) {
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
)];

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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }

        let collector = match LogicalCollector::collect(args) {
            Ok(c) => c,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        Ok(CellValue::Boolean(collector.any_true))
    }
}

static NOT_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "logical",
    "A value or expression that can be evaluated to TRUE or FALSE.",
    ParamKind::Scalar,
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }
        if args.len() > 1 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 1,
                found: args.len(),
            });
        }

        let val = args[0].eval_scalar(self.name(), ctx)?;
        if let CellValue::Error(err) = val {
            return Ok(CellValue::Error(err));
        }

        let b = match val.coerce_to_bool() {
            Ok(b) => b,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        Ok(CellValue::Boolean(!b))
    }
}
