//! Runtime argument binding (`bind_arguments`) and bound argument validation (`validate_bound_args`).

use crate::expr::ast::{CallArg, Expr};
use crate::functions::error::FunctionError;
use crate::functions::registry::FunctionRegistry;
use crate::functions::traits::{
    BoundCriterion, EmptyPolicy, EvalContext, ExcelFunction, FunctionArg, ParamKind, ParamType,
    ParameterDef, RangeGrid, ReferenceKind, RolePolicy, ShapePolicy, ValidationPolicy,
    VariadicRule,
};
use crate::model::table::AxisRole;
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
        crate::functions::typecheck::validate_signature(func, expr_args)?;

        let mut bound = Vec::with_capacity(expr_args.len());

        for (i, call_arg) in expr_args.iter().enumerate() {
            let param = resolve_parameter(func, i)?;
            let position = i + 1;

            match param.kind {
                ParamKind::Criteria => {
                    bound.push(bind_criteria_arg(param, call_arg, ctx)?);
                }
                ParamKind::Lazy => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        bound.push(bind_omitted_non_criteria(param));
                        continue;
                    };
                    bound.push(FunctionArg::Lazy(arg_expr));
                }
                ParamKind::Scalar => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        bound.push(bind_omitted_non_criteria(param));
                        continue;
                    };
                    bound.push(FunctionArg::Scalar(ctx.eval_expr(arg_expr)?));
                }
                ParamKind::Range => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        bound.push(bind_omitted_non_criteria(param));
                        continue;
                    };
                    match arg_expr {
                        Expr::Reference(table_ref) => {
                            let RangeGrid {
                                rows,
                                cols,
                                values,
                                axis_role,
                            } = ctx.eval_range(arg_expr)?;
                            let ref_kind = if table_ref.is_slice() {
                                ReferenceKind::Slice
                            } else {
                                ReferenceKind::SingleCell
                            };
                            bound.push(FunctionArg::Array {
                                rows,
                                cols,
                                values,
                                ref_kind,
                                axis_role,
                            });
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            return Err(FunctionError::InvalidArgumentKind {
                                function: func.name(),
                                expected: "range",
                                found: "scalar",
                            });
                        }
                    }
                }
                ParamKind::RangeOrArray => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        bound.push(bind_omitted_non_criteria(param));
                        continue;
                    };
                    match arg_expr {
                        Expr::Reference(table_ref) => {
                            let RangeGrid {
                                rows,
                                cols,
                                values,
                                axis_role,
                            } = ctx.eval_range(arg_expr)?;
                            let ref_kind = if table_ref.is_slice() {
                                ReferenceKind::Slice
                            } else {
                                ReferenceKind::SingleCell
                            };
                            bound.push(FunctionArg::Array {
                                rows,
                                cols,
                                values,
                                ref_kind,
                                axis_role,
                            });
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            bound.push(FunctionArg::Scalar(ctx.eval_expr(arg_expr)?));
                        }
                    }
                }
            }
        }

        Ok(bound)
    }
}

pub(crate) fn extract_non_criteria_expr<'a>(
    func: &dyn ExcelFunction,
    param: &ParameterDef,
    position: usize,
    call_arg: &'a CallArg,
) -> Result<Option<&'a Expr>, FunctionError> {
    match call_arg {
        CallArg::Expr(expr) => Ok(Some(expr)),
        CallArg::Omitted => Ok(None),
        CallArg::Comparator { .. } | CallArg::BlankCheck(_) => {
            Err(FunctionError::UnexpectedCriteriaSyntax {
                function: func.name(),
                param: param.name,
                position,
            })
        }
    }
}

fn bind_omitted_non_criteria<'a>(param: &ParameterDef) -> FunctionArg<'a> {
    if let Some(default_val) = param.requirement.default_value() {
        FunctionArg::Scalar(default_val.clone())
    } else {
        FunctionArg::Omitted
    }
}

fn bind_criteria_arg<'a>(
    param: &ParameterDef,
    call_arg: &CallArg,
    ctx: &dyn EvalContext,
) -> Result<FunctionArg<'a>, FunctionError> {
    match call_arg {
        CallArg::Omitted => {
            if let Some(default_val) = param.requirement.default_value() {
                Ok(FunctionArg::Criterion(BoundCriterion::Exact(
                    default_val.clone(),
                )))
            } else {
                Ok(FunctionArg::Omitted)
            }
        }
        CallArg::BlankCheck(check) => {
            Ok(FunctionArg::Criterion(BoundCriterion::BlankCheck(*check)))
        }
        CallArg::Expr(arg_expr) => Ok(FunctionArg::Criterion(BoundCriterion::Exact(
            ctx.eval_expr(arg_expr)?,
        ))),
        CallArg::Comparator { op, expr: arg_expr } => {
            Ok(FunctionArg::Criterion(BoundCriterion::Comparator {
                op: *op,
                value: ctx.eval_expr(arg_expr)?,
            }))
        }
    }
}

fn extract_bound_calendar_int(arg: Option<&FunctionArg<'_>>) -> Option<i64> {
    match arg {
        Some(FunctionArg::Scalar(val)) => match val.strict_integer() {
            Ok(Ok(iv)) => Some(iv),
            Ok(Err(_)) | Err(_) => None,
        },
        Some(
            FunctionArg::Array { .. }
            | FunctionArg::Lazy(_)
            | FunctionArg::Criterion(_)
            | FunctionArg::Omitted,
        )
        | None => None,
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

pub(crate) fn require_1d_slice(
    function: &'static str,
    param: &'static str,
    shape: Option<(usize, usize)>,
) -> Result<(), FunctionError> {
    if let Some((rows, cols)) = shape
        && rows > 1
        && cols > 1
    {
        return Err(FunctionError::ExpectedOneDimensionalSlice {
            function,
            param,
            rows,
            cols,
        });
    }
    Ok(())
}

pub(crate) fn validate_shape_policy(
    function: &'static str,
    policy: ShapePolicy,
    shapes: &[Option<(usize, usize)>],
) -> Result<(), FunctionError> {
    match policy {
        ShapePolicy::EqualAllRanges => {
            if let Some((exp_r, exp_c)) = shapes.first().copied().flatten() {
                for shape in shapes.iter().skip(1) {
                    if let Some((r, c)) = *shape
                        && (r, c) != (exp_r, exp_c)
                    {
                        return Err(FunctionError::DimensionMismatch {
                            function,
                            expected_rows: exp_r,
                            expected_cols: exp_c,
                            found_rows: r,
                            found_cols: c,
                        });
                    }
                }
            }
        }
        ShapePolicy::EqualTargetAndOddPairRanges => {
            if let Some((exp_r, exp_c)) = shapes.first().copied().flatten() {
                for idx in (1..shapes.len()).step_by(2) {
                    if let Some((r, c)) = shapes[idx]
                        && (r, c) != (exp_r, exp_c)
                    {
                        return Err(FunctionError::DimensionMismatch {
                            function,
                            expected_rows: exp_r,
                            expected_cols: exp_c,
                            found_rows: r,
                            found_cols: c,
                        });
                    }
                }
            }
        }
        ShapePolicy::EqualEvenPairRanges => {
            if let Some((exp_r, exp_c)) = shapes.first().copied().flatten() {
                for idx in (2..shapes.len()).step_by(2) {
                    if let Some((r, c)) = shapes[idx]
                        && (r, c) != (exp_r, exp_c)
                    {
                        return Err(FunctionError::DimensionMismatch {
                            function,
                            expected_rows: exp_r,
                            expected_cols: exp_c,
                            found_rows: r,
                            found_cols: c,
                        });
                    }
                }
            }
        }
        ShapePolicy::OneDimEqualLength {
            first_idx,
            first_param,
            second_idx,
            second_param,
        } => {
            let first_shape = shapes.get(first_idx).copied().flatten();
            let second_shape = shapes.get(second_idx).copied().flatten();
            require_1d_slice(function, first_param, first_shape)?;
            require_1d_slice(function, second_param, second_shape)?;
            if let (Some((r1, c1)), Some((r2, c2))) = (first_shape, second_shape) {
                let len1 = r1 * c1;
                let len2 = r2 * c2;
                if len1 != len2 {
                    return Err(FunctionError::LengthMismatch {
                        function,
                        expected: len1,
                        found: len2,
                    });
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgPresence {
    Present,
    Omitted,
}

pub(crate) trait ArgSlot {
    fn presence(&self) -> ArgPresence;
}

impl ArgSlot for CallArg {
    fn presence(&self) -> ArgPresence {
        match self {
            Self::Expr(_) | Self::Comparator { .. } | Self::BlankCheck(_) => ArgPresence::Present,
            Self::Omitted => ArgPresence::Omitted,
        }
    }
}

impl ArgSlot for FunctionArg<'_> {
    fn presence(&self) -> ArgPresence {
        match self {
            Self::Scalar(_) | Self::Array { .. } | Self::Lazy(_) | Self::Criterion(_) => {
                ArgPresence::Present
            }
            Self::Omitted => ArgPresence::Omitted,
        }
    }
}

/// Validates runtime bound argument count, variadic pair parity, omitted parameter eligibility,
/// argument kinds (`Range` vs scalar/slice), coupled slice shapes, column `RolePolicy`, `EmptyPolicy`,
/// and declarative `ParamType` / `CalendarDate` constraints across all arguments.
pub(crate) fn validate_bound_args(
    func: &dyn ExcelFunction,
    args: &[FunctionArg<'_>],
) -> Result<(), FunctionError> {
    validate_arg_slots(func, args)?;

    let mut shapes: Vec<Option<(usize, usize)>> = Vec::with_capacity(args.len());

    // Pass 1: Validate `ParamKind` vs bound `FunctionArg` variant and collect array shapes.
    for (i, arg) in args.iter().enumerate() {
        let param = resolve_parameter(func, i)?;

        match (param.kind, arg) {
            (_, FunctionArg::Omitted) => shapes.push(None),
            (ParamKind::Range, FunctionArg::Array { rows, cols, .. }) => {
                shapes.push(Some((*rows, *cols)));
            }
            (ParamKind::Range, FunctionArg::Scalar(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "range",
                    found: "scalar",
                });
            }
            (ParamKind::Range, FunctionArg::Lazy(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "range",
                    found: "lazy",
                });
            }
            (ParamKind::Range, FunctionArg::Criterion(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "range",
                    found: "criterion",
                });
            }
            (
                ParamKind::Scalar | ParamKind::Lazy,
                FunctionArg::Array {
                    rows,
                    cols,
                    ref_kind: ReferenceKind::Slice,
                    ..
                },
            ) => {
                return Err(FunctionError::SliceInScalarContext {
                    rows: *rows,
                    cols: *cols,
                });
            }
            (
                ParamKind::Scalar | ParamKind::Lazy,
                FunctionArg::Array {
                    ref_kind: ReferenceKind::SingleCell,
                    ..
                },
            ) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "scalar",
                    found: "array",
                });
            }
            (ParamKind::Scalar | ParamKind::Lazy, FunctionArg::Criterion(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "scalar",
                    found: "criterion",
                });
            }
            (ParamKind::Scalar, FunctionArg::Lazy(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "scalar",
                    found: "lazy",
                });
            }
            (ParamKind::Scalar, FunctionArg::Scalar(_))
            | (ParamKind::Lazy, FunctionArg::Scalar(_) | FunctionArg::Lazy(_)) => {
                shapes.push(Some((1, 1)));
            }
            (ParamKind::RangeOrArray, FunctionArg::Array { rows, cols, .. }) => {
                shapes.push(Some((*rows, *cols)));
            }
            (ParamKind::RangeOrArray, FunctionArg::Scalar(_) | FunctionArg::Lazy(_)) => {
                shapes.push(Some((1, 1)));
            }
            (ParamKind::RangeOrArray, FunctionArg::Criterion(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "range or scalar",
                    found: "criterion",
                });
            }
            (ParamKind::Criteria, FunctionArg::Criterion(_)) => {
                shapes.push(Some((1, 1)));
            }
            (
                ParamKind::Criteria,
                FunctionArg::Array {
                    rows,
                    cols,
                    ref_kind: ReferenceKind::Slice,
                    ..
                },
            ) => {
                return Err(FunctionError::SliceInScalarContext {
                    rows: *rows,
                    cols: *cols,
                });
            }
            (
                ParamKind::Criteria,
                FunctionArg::Array {
                    ref_kind: ReferenceKind::SingleCell,
                    ..
                },
            ) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "criterion",
                    found: "array",
                });
            }
            (ParamKind::Criteria, FunctionArg::Scalar(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "criterion",
                    found: "scalar",
                });
            }
            (ParamKind::Criteria, FunctionArg::Lazy(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "criterion",
                    found: "lazy",
                });
            }
        }
    }

    // Pass 2: Coupled slice-shape invariants run before role and element `ParamType` checks.
    if let ValidationPolicy::ShapeCoupled(shape_policy) = func.validation_policy() {
        validate_shape_policy(func.name(), shape_policy, &shapes)?;
    }

    // Pass 3: Declarative `RolePolicy::MeasureOnly` runs before element `ParamType` checks.
    for (i, arg) in args.iter().enumerate() {
        let param = resolve_parameter(func, i)?;
        if param.role_policy == RolePolicy::MeasureOnly
            && let FunctionArg::Array {
                axis_role: AxisRole::Descriptor,
                ..
            } = arg
        {
            return Err(FunctionError::DescriptorReferenceNotAllowed {
                function: func.name(),
                param: param.name,
                position: i + 1,
            });
        }
    }

    // Pass 4: Per-argument `ParamType` and `EmptyPolicy` validation.
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
            FunctionArg::Lazy(_) | FunctionArg::Criterion(_) | FunctionArg::Omitted => {}
        }
    }

    // Pass 5: Calendar `(year, month, day)` domain validation after `ParamType::Integer` checks.
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

pub(crate) fn validate_arg_slots<T: ArgSlot>(
    func: &dyn ExcelFunction,
    args: &[T],
) -> Result<(), FunctionError> {
    let min = func.min_args();
    if args.len() < min {
        return Err(FunctionError::TooFewArguments {
            function: func.name(),
            expected: min,
            found: args.len(),
        });
    }

    if let Some(max) = func.max_args()
        && args.len() > max
    {
        return Err(FunctionError::TooManyArguments {
            function: func.name(),
            expected: max,
            found: args.len(),
        });
    }

    if func.variadic_rule() == VariadicRule::Pairs {
        let fixed_count = func.parameters().len();
        if args.len() > fixed_count && !(args.len() - fixed_count).is_multiple_of(2) {
            return Err(FunctionError::InvalidVariadicPairs {
                function: func.name(),
                count: args.len(),
            });
        }
    }

    let last_present = args
        .iter()
        .rposition(|arg| arg.presence() == ArgPresence::Present);

    for (i, arg) in args.iter().enumerate() {
        match arg.presence() {
            ArgPresence::Present => {}
            ArgPresence::Omitted => {
                let param = resolve_parameter(func, i)?;
                if !param.requirement.is_optional() {
                    return Err(FunctionError::CannotOmitRequiredArgument {
                        function: func.name(),
                        param: param.name,
                        position: i + 1,
                    });
                }
                if last_present.is_none_or(|last| i >= last) {
                    return Err(FunctionError::CannotOmitTrailingArgument {
                        function: func.name(),
                        param: param.name,
                        position: i + 1,
                    });
                }
            }
        }
    }

    Ok(())
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
