use crate::functions::common::{NumericCollector, count_non_empty, count_numbers, normalize_zero};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Positional parameter definitions for `SUM`.
static SUM_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range to sum.",
    ParamKind::RangeOrArray,
)];

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

        match NumericCollector::collect(args) {
            Ok(collector) => Ok(CellValue::Number(collector.sum)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `AVERAGE`.
static AVERAGE_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range for which you want the average.",
    ParamKind::RangeOrArray,
)];

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

        match NumericCollector::collect(args) {
            Ok(collector) => {
                if collector.count == 0 {
                    Ok(CellValue::Error(ExcelError::Div0))
                } else {
                    let avg = collector.sum / collector.count as f64;
                    Ok(CellValue::Number(normalize_zero(avg)))
                }
            }
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `COUNT`.
static COUNT_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "value1",
    "The first item, cell reference, or range within which you want to count numbers.",
    ParamKind::RangeOrArray,
)];

/// `COUNT(value1, [value2], ...)`: Counts the number of cells that contain numbers.
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

        let count = count_numbers(args);
        Ok(CellValue::Number(count as f64))
    }
}

/// Positional parameter definitions for `COUNTA`.
static COUNTA_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "value1",
    "The first item, cell reference, or range within which you want to count non-empty cells.",
    ParamKind::RangeOrArray,
)];

/// `COUNTA(value1, [value2], ...)`: Counts the number of cells that are not empty.
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

        let count = count_non_empty(args);
        Ok(CellValue::Number(count as f64))
    }
}

/// Positional parameter definitions for `MAX`.
static MAX_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range for which you want the maximum.",
    ParamKind::RangeOrArray,
)];

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

        match NumericCollector::collect(args) {
            Ok(collector) => Ok(CellValue::Number(collector.max_or_zero())),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `MIN`.
static MIN_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range for which you want the minimum.",
    ParamKind::RangeOrArray,
)];

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

        match NumericCollector::collect(args) {
            Ok(collector) => Ok(CellValue::Number(collector.min_or_zero())),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `MEDIAN`.
static MEDIAN_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number1",
    "The first number, cell reference, or range for which you want the median.",
    ParamKind::RangeOrArray,
)];

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
                    Ok(CellValue::Number(normalize_zero(median)))
                }
            }
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
