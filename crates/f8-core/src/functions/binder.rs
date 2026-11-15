//! Runtime argument binding (`bind_arguments`) and active value-domain validation (`validate_bound_args`).

use crate::expr::ast::{CallArg, Expr};
use crate::functions::error::FunctionError;
use crate::functions::registry::FunctionRegistry;
use crate::functions::traits::{
    EmptyPolicy, EvalContext, ExcelFunction, FunctionArg, ParamKind, ParamType, ParameterDef,
    RangeGrid, ReferenceKind, ValidationPolicy, VariadicRule,
};
use crate::value::{
    CellValue, DateSerial, ExpectedType, StrictIntError, TypeError, ValueType,
    is_valid_input_date_serial,
};

impl FunctionRegistry {
    /// Binds parsed formula arguments 1-to-1 into runtime [`FunctionArg`] values, substituting defaults
    /// for intermediate omitted optional parameters.
    pub fn bind_arguments<'a>(
        &self,
        func: &dyn ExcelFunction,
        expr_args: &'a [CallArg],
        ctx: &dyn EvalContext,
    ) -> Result<Vec<FunctionArg<'a>>, FunctionError> {
        let mut bound = Vec::with_capacity(expr_args.len());

        for (i, call_arg) in expr_args.iter().enumerate() {
            let param = resolve_parameter(func, i)?;
            let arg_expr = match call_arg {
                CallArg::Expr(expr) => expr,
                CallArg::Omitted => {
                    bound.push(bind_omitted_arg(param));
                    continue;
                }
            };

            match (param.kind, arg_expr) {
                (ParamKind::Lazy, _) => {
                    bound.push(FunctionArg::Lazy(arg_expr));
                }
                (ParamKind::Range | ParamKind::RangeOrArray, Expr::Reference(table_ref)) => {
                    let RangeGrid { rows, cols, values } = ctx.eval_range(arg_expr)?;
                    let ref_kind = if table_ref.is_slice() {
                        ReferenceKind::Slice
                    } else {
                        ReferenceKind::SingleCell
                    };
                    bound.push(FunctionArg::array(rows, cols, values, ref_kind));
                }
                (
                    ParamKind::Range,
                    Expr::Literal(_)
                    | Expr::Unary { .. }
                    | Expr::Binary { .. }
                    | Expr::FunctionCall { .. },
                ) => {
                    return Err(FunctionError::InvalidArgumentKind {
                        function: func.name(),
                        expected: "range",
                        found: "scalar",
                    });
                }
                (
                    ParamKind::RangeOrArray,
                    Expr::Literal(_)
                    | Expr::Unary { .. }
                    | Expr::Binary { .. }
                    | Expr::FunctionCall { .. },
                )
                | (ParamKind::Scalar, _) => {
                    bound.push(FunctionArg::Scalar(ctx.eval_expr(arg_expr)?));
                }
            }
        }

        Ok(bound)
    }
}

fn bind_omitted_arg<'a>(param: &ParameterDef) -> FunctionArg<'a> {
    if let Some(default_val) = param.requirement.default_value() {
        FunctionArg::Scalar(default_val.clone())
    } else {
        FunctionArg::Omitted
    }
}

fn extract_bound_calendar_int(arg: Option<&FunctionArg<'_>>) -> Option<i64> {
    match arg {
        Some(FunctionArg::Scalar(val)) => match val.strict_integer() {
            Ok(Ok(iv)) => Some(iv),
            Ok(Err(_)) | Err(_) => None,
        },
        Some(FunctionArg::Array { .. } | FunctionArg::Lazy(_) | FunctionArg::Omitted) | None => {
            None
        }
    }
}

pub(crate) fn validate_calendar_date_components(
    year: Option<i64>,
    month: Option<i64>,
    day: Option<i64>,
) -> Result<(), FunctionError> {
    if let Some(y) = year
        && !(1900..=9999).contains(&y)
    {
        return Err(FunctionError::InvalidDateYear { year: y });
    }
    if let Some(m) = month
        && !(1..=12).contains(&m)
    {
        return Err(FunctionError::InvalidDateMonth { month: m });
    }
    if let Some(d) = day {
        let max_day = match (year, month) {
            (Some(y), Some(m)) => DateSerial::days_in_month(y as i32, m as u32),
            (None, Some(m)) => DateSerial::days_in_month(2000, m as u32),
            (_, None) => 31,
        };
        if d < 1 || d > i64::from(max_day) {
            return Err(FunctionError::InvalidDateDay { day: d, max_day });
        }
    }
    Ok(())
}

/// Validates active runtime `ParamType`, `EmptyPolicy`, and `CalendarDate` constraints across bound arguments.
pub(crate) fn validate_bound_args(
    func: &dyn ExcelFunction,
    args: &[FunctionArg<'_>],
) -> Result<(), FunctionError> {
    let min_args = func.min_args();
    if args.len() < min_args {
        return Err(FunctionError::TooFewArguments {
            function: func.name(),
            expected: min_args,
            found: args.len(),
        });
    }
    if let Some(max_args) = func.max_args()
        && args.len() > max_args
    {
        return Err(FunctionError::TooManyArguments {
            function: func.name(),
            expected: max_args,
            found: args.len(),
        });
    }
    if func.variadic_rule() == VariadicRule::Pairs
        && args.len() > func.parameters().len()
        && !(args.len() - func.parameters().len()).is_multiple_of(2)
    {
        return Err(FunctionError::InvalidVariadicPairs {
            function: func.name(),
            count: args.len(),
        });
    }

    for (i, arg) in args.iter().enumerate() {
        let param = resolve_parameter(func, i)?;
        let position = i + 1;

        match arg {
            FunctionArg::Scalar(val) => {
                validate_value_against_param_type(
                    val,
                    func.name(),
                    param.name,
                    position,
                    param.param_type,
                    EmptyValidation::RejectEmpty,
                )?;
            }
            FunctionArg::Array {
                values, ref_kind, ..
            } => {
                let empty_validation = match (param.empty_policy, ref_kind) {
                    (EmptyPolicy::AllowInSlice, ReferenceKind::Slice) => EmptyValidation::SkipEmpty,
                    (EmptyPolicy::AllowInSlice, ReferenceKind::SingleCell)
                    | (EmptyPolicy::ForbidAll, ReferenceKind::SingleCell | ReferenceKind::Slice) => {
                        EmptyValidation::RejectEmpty
                    }
                };
                for val in values {
                    if matches!(val, CellValue::Error(_)) {
                        continue;
                    }
                    validate_value_against_param_type(
                        val,
                        func.name(),
                        param.name,
                        position,
                        param.param_type,
                        empty_validation,
                    )?;
                }
            }
            FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
        }
    }

    if func.validation_policy() == ValidationPolicy::CalendarDate {
        let year = extract_bound_calendar_int(args.first());
        let month = extract_bound_calendar_int(args.get(1));
        let day = extract_bound_calendar_int(args.get(2));
        validate_calendar_date_components(year, month, day)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmptyValidation {
    RejectEmpty,
    SkipEmpty,
}

fn validate_value_against_param_type(
    val: &CellValue,
    function: &'static str,
    param: &'static str,
    position: usize,
    param_type: ParamType,
    empty_policy: EmptyValidation,
) -> Result<(), FunctionError> {
    if param_type == ParamType::Any {
        return Ok(());
    }

    match empty_policy {
        EmptyValidation::SkipEmpty => {
            if matches!(val, CellValue::Empty) {
                return Ok(());
            }
        }
        EmptyValidation::RejectEmpty => {}
    }

    match param_type {
        ParamType::Any => Ok(()),
        ParamType::NonEmpty => match val {
            CellValue::Number(_)
            | CellValue::Text(_)
            | CellValue::Boolean(_)
            | CellValue::Error(_) => Ok(()),
            CellValue::Empty => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::NonEmpty,
                found: ValueType::Empty,
            })),
        },
        ParamType::Number => match val.strict_number() {
            Ok(_) => Ok(()),
            Err(found) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::Number,
                found,
            })),
        },
        ParamType::DateSerial => match val.strict_number() {
            Ok(Ok(serial)) => {
                if is_valid_input_date_serial(serial) {
                    Ok(())
                } else {
                    Err(FunctionError::InvalidDateSerial {
                        function,
                        param,
                        position,
                        serial: serial.to_string(),
                    })
                }
            }
            Ok(Err(_)) => Ok(()),
            Err(found) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::Number,
                found,
            })),
        },
        ParamType::Integer => match val.strict_integer() {
            Ok(_) => Ok(()),
            Err(StrictIntError::WrongType(found)) => {
                Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found,
                }))
            }
            Err(StrictIntError::NonInteger(value)) => {
                Err(FunctionError::Type(TypeError::NonIntegerArgument {
                    function,
                    param,
                    position,
                    value,
                }))
            }
        },
        ParamType::AnnuityType => match val.strict_integer() {
            Ok(Ok(iv)) => {
                if (0..=1).contains(&iv) {
                    Ok(())
                } else {
                    Err(FunctionError::InvalidAnnuityType {
                        function,
                        found: iv,
                    })
                }
            }
            Ok(Err(_)) => Ok(()),
            Err(StrictIntError::WrongType(found)) => {
                Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found,
                }))
            }
            Err(StrictIntError::NonInteger(value)) => {
                Err(FunctionError::Type(TypeError::NonIntegerArgument {
                    function,
                    param,
                    position,
                    value,
                }))
            }
        },
        ParamType::DayCountBasis => match val.strict_integer() {
            Ok(Ok(iv)) => {
                if (0..=4).contains(&iv) {
                    Ok(())
                } else {
                    Err(FunctionError::InvalidDayCountBasis { found: iv })
                }
            }
            Ok(Err(_)) => Ok(()),
            Err(StrictIntError::WrongType(found)) => {
                Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found,
                }))
            }
            Err(StrictIntError::NonInteger(value)) => {
                Err(FunctionError::Type(TypeError::NonIntegerArgument {
                    function,
                    param,
                    position,
                    value,
                }))
            }
        },
        ParamType::Boolean => match val.strict_bool() {
            Ok(_) => Ok(()),
            Err(found) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::Boolean,
                found,
            })),
        },
    }
}

/// Resolves the expected parameter definition for a given 0-indexed argument position,
/// applying cyclical pairs alternation when `VariadicRule::Pairs` is enabled.
pub(crate) fn resolve_parameter(
    func: &dyn ExcelFunction,
    index: usize,
) -> Result<&ParameterDef, FunctionError> {
    if index < func.parameters().len() {
        Ok(&func.parameters()[index])
    } else if func.variadic_rule() == VariadicRule::Pairs {
        let fixed_count = func.parameters().len();
        if fixed_count < 2 {
            return Err(FunctionError::EvaluationFailure {
                function: func.name(),
                message: "variadic pairs rule requires at least two parameter definitions".into(),
            });
        }
        let offset = (index - fixed_count) % 2;
        Ok(&func.parameters()[fixed_count - 2 + offset])
    } else {
        func.parameters()
            .last()
            .ok_or_else(|| FunctionError::EvaluationFailure {
                function: func.name(),
                message: "variadic argument without parameter definition".into(),
            })
    }
}
