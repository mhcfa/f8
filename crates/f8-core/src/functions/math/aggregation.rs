use crate::functions::common::{NumericCollector, count_non_empty, count_numbers};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Positional parameter definitions for `SUM`.
static SUM_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range to sum.",
    ParamKind::ScalarOrRange,
    ParamType::Number,
)
.measure_only()
.allow_empty_in_slice()];

/// `SUM(number1, [number2], ...)`: Adds all numbers in a range of cells or scalar arguments.
#[derive(Debug, Default, Clone, Copy)]
pub struct SumFunction;

impl ExcelFunction for SumFunction {
    fn name(&self) -> &'static str {
        "SUM"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        SUM_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match NumericCollector::collect(args) {
            Ok(collector) => Ok(CellValue::number(collector.sum)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `AVERAGE`.
static AVERAGE_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range for which you want the average.",
    ParamKind::ScalarOrRange,
    ParamType::Number,
)
.measure_only()
.allow_empty_in_slice()];

/// `AVERAGE(number1, [number2], ...)`: Computes the arithmetic mean of arguments.
#[derive(Debug, Default, Clone, Copy)]
pub struct AverageFunction;

impl ExcelFunction for AverageFunction {
    fn name(&self) -> &'static str {
        "AVERAGE"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        AVERAGE_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match NumericCollector::collect(args) {
            Ok(collector) => {
                if collector.count == 0 {
                    Ok(CellValue::Error(ExcelError::Div0))
                } else {
                    let avg = collector.sum / collector.count as f64;
                    Ok(CellValue::number(avg))
                }
            }
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `COUNT`.
static COUNT_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "value1",
    "The first cell reference or range within which you want to count numbers.",
    ParamKind::Range,
    ParamType::Number,
)
.measure_only()
.allow_empty_in_slice()];

/// `COUNT(value1, [value2], ...)`: Counts the number of cells in ranges that contain numbers.
#[derive(Debug, Default, Clone, Copy)]
pub struct CountFunction;

impl ExcelFunction for CountFunction {
    fn name(&self) -> &'static str {
        "COUNT"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        COUNT_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let count = count_numbers(args);
        Ok(CellValue::number(count as f64))
    }
}

/// Positional parameter definitions for `COUNTA`.
static COUNTA_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "value1",
    "The first cell reference or range within which you want to count non-empty cells.",
    ParamKind::Range,
    ParamType::Any,
)
.allow_empty()];

/// `COUNTA(value1, [value2], ...)`: Counts the number of cells in ranges that are not empty.
#[derive(Debug, Default, Clone, Copy)]
pub struct CountaFunction;

impl ExcelFunction for CountaFunction {
    fn name(&self) -> &'static str {
        "COUNTA"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        COUNTA_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let count = count_non_empty(args);
        Ok(CellValue::number(count as f64))
    }
}

/// Positional parameter definitions for `MAX`.
static MAX_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range for which you want the maximum.",
    ParamKind::ScalarOrRange,
    ParamType::Number,
)
.measure_only()
.allow_empty_in_slice()];

/// `MAX(number1, [number2], ...)`: Returns the largest numeric value in a set of values.
#[derive(Debug, Default, Clone, Copy)]
pub struct MaxFunction;

impl ExcelFunction for MaxFunction {
    fn name(&self) -> &'static str {
        "MAX"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        MAX_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match NumericCollector::collect(args) {
            Ok(collector) => Ok(CellValue::number(collector.max_or_zero())),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `MIN`.
static MIN_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range for which you want the minimum.",
    ParamKind::ScalarOrRange,
    ParamType::Number,
)
.measure_only()
.allow_empty_in_slice()];

/// `MIN(number1, [number2], ...)`: Returns the smallest numeric value in a set of values.
#[derive(Debug, Default, Clone, Copy)]
pub struct MinFunction;

impl ExcelFunction for MinFunction {
    fn name(&self) -> &'static str {
        "MIN"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        MIN_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match NumericCollector::collect(args) {
            Ok(collector) => Ok(CellValue::number(collector.min_or_zero())),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `MEDIAN`.
static MEDIAN_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range for which you want the median.",
    ParamKind::ScalarOrRange,
    ParamType::Number,
)
.measure_only()
.allow_empty_in_slice()];

/// `MEDIAN(number1, [number2], ...)`: Returns the median (middle number) of the given numbers.
#[derive(Debug, Default, Clone, Copy)]
pub struct MedianFunction;

impl ExcelFunction for MedianFunction {
    fn name(&self) -> &'static str {
        "MEDIAN"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        MEDIAN_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match NumericCollector::collect_values(args) {
            Ok(mut values) => {
                if values.is_empty() {
                    Ok(CellValue::Error(ExcelError::Num))
                } else {
                    values.sort_by(|a, b| a.total_cmp(b));
                    let n = values.len();
                    let median = if n % 2 == 1 {
                        values[n / 2]
                    } else {
                        (values[n / 2 - 1] + values[n / 2]) / 2.0
                    };
                    Ok(CellValue::number(median))
                }
            }
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
