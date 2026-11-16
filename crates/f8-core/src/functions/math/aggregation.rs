use crate::functions::common::NumericCollector;
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ReferenceKind, VariadicRule,
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
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }
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
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }

        let mut count = 0usize;
        for arg in args {
            let FunctionArg::Range {
                values, ref_kind, ..
            } = arg
            else {
                return Err(FunctionError::InvalidArgumentKind {
                    function: self.name(),
                    expected: "range",
                    found: arg.kind_name(),
                });
            };
            for v in values {
                match v {
                    CellValue::Number(n) => {
                        if !n.is_finite() {
                            return Ok(CellValue::Error(ExcelError::Num));
                        }
                        count += 1;
                    }
                    CellValue::Error(_) => {}
                    CellValue::Empty => match ref_kind {
                        ReferenceKind::Slice => {}
                        ReferenceKind::SingleCell => {
                            return Ok(CellValue::Error(ExcelError::Value));
                        }
                    },
                    CellValue::Text(_) | CellValue::Boolean(_) => {
                        return Ok(CellValue::Error(ExcelError::Value));
                    }
                }
            }
        }

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
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }
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
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }
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
                    Ok(CellValue::number(median))
                }
            }
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
