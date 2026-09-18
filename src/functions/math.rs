use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Positional parameter definitions for `ABS`.
static ABS_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number",
    "The real number for which you want the absolute value.",
    ParamKind::Scalar,
)];

/// `ABS(number)`: Returns the absolute value of a number.
#[derive(Debug, Default, Clone, Copy)]
pub struct AbsFunction;

impl ExcelFunction for AbsFunction {
    fn name(&self) -> &'static str {
        "ABS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        ABS_PARAMS
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() != 1 {
            return if args.is_empty() {
                Err(FunctionError::TooFewArguments {
                    function: self.name(),
                    expected: 1,
                    found: 0,
                })
            } else {
                Err(FunctionError::TooManyArguments {
                    function: self.name(),
                    expected: 1,
                    found: args.len(),
                })
            };
        }

        let val = args[0].eval_scalar(self.name(), ctx)?;
        match val.coerce_to_number() {
            Ok(n) => {
                let abs = n.abs();
                let normalized = if abs == 0.0 { 0.0 } else { abs };
                Ok(CellValue::Number(normalized))
            }
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }

        let mut total = 0.0;

        for arg in args {
            match arg {
                FunctionArg::Scalar(val) => match val {
                    CellValue::Number(n) => total += n,
                    CellValue::Boolean(true) => total += 1.0,
                    CellValue::Boolean(false) | CellValue::Empty => total += 0.0,
                    CellValue::Error(e) => return Ok(CellValue::Error(*e)),
                    CellValue::Text(_) => return Ok(CellValue::Error(ExcelError::Value)),
                },
                FunctionArg::Array {
                    values,
                    is_range_ref,
                    ..
                } => {
                    if *is_range_ref {
                        for v in values {
                            match v {
                                CellValue::Number(n) => total += n,
                                CellValue::Error(e) => return Ok(CellValue::Error(*e)),
                                CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {
                                    // Range references silently ignore text, booleans, and empty cells
                                }
                            }
                        }
                    } else {
                        // Explicit array literal: text generates #VALUE!
                        for v in values {
                            match v {
                                CellValue::Number(n) => total += n,
                                CellValue::Boolean(true) => total += 1.0,
                                CellValue::Boolean(false) | CellValue::Empty => total += 0.0,
                                CellValue::Error(e) => return Ok(CellValue::Error(*e)),
                                CellValue::Text(_) => {
                                    return Ok(CellValue::Error(ExcelError::Value));
                                }
                            }
                        }
                    }
                }
                FunctionArg::Lazy(expr) => {
                    let val = ctx.eval_expr(expr)?;
                    match val {
                        CellValue::Number(n) => total += n,
                        CellValue::Boolean(true) => total += 1.0,
                        CellValue::Boolean(false) | CellValue::Empty => total += 0.0,
                        CellValue::Error(e) => return Ok(CellValue::Error(e)),
                        CellValue::Text(_) => return Ok(CellValue::Error(ExcelError::Value)),
                    }
                }
                FunctionArg::Omitted => {
                    total += 0.0;
                }
            }
        }

        let normalized = if total == 0.0 { 0.0 } else { total };
        Ok(CellValue::Number(normalized))
    }
}
