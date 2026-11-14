//! Excel conditional aggregation functions (*IFS family).
//!
//! Provides conditional aggregation implementations including `SUMIFS`, `AVERAGEIFS`,
//! `COUNTIFS`, `MAXIFS`, and `MINIFS` conforming to Excel calculation semantics.

use crate::expr::ast::CriteriaComparator;
use crate::functions::common::value_type_satisfies;
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{TypedRangeSlice, validate_homogeneous_slice};
use crate::functions::math::criteria::{EqualityOp, TypedCriterion};
use crate::functions::registry::{resolve_parameter, validate_bound_args};
use crate::functions::traits::{
    BoundCriterion, EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind,
    ParamType, ParameterDef, ShapePolicy, ValidationPolicy, VariadicRule,
};
use crate::value::{
    CellValue, ExcelError, ExpectedType, TypeError, ValueType, normalize_criteria_number,
    normalize_zero, validate_text_criterion,
};

// ============================================================================
// Parameter Definitions
// ============================================================================

static SUMIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "sum_range",
        "The range of cells to sum if criteria are met.",
        ParamKind::Range,
        ParamType::Number,
    )
    .measure_only()
    .allow_empty_in_slice(),
    ParameterDef::required(
        "criteria_range1",
        "The range of cells to evaluate against criteria1.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that determines which cells in criteria_range1 to sum.",
        ParamKind::Criteria,
        ParamType::Any,
    ),
];

static COUNTIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "criteria_range1",
        "The first range of cells to evaluate.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that defines which cells will be counted.",
        ParamKind::Criteria,
        ParamType::Any,
    ),
];

static AVERAGEIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "average_range",
        "The range of cells to average if criteria are met.",
        ParamKind::Range,
        ParamType::Number,
    )
    .measure_only()
    .allow_empty_in_slice(),
    ParameterDef::required(
        "criteria_range1",
        "The range of cells to evaluate against criteria1.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that determines which cells to average.",
        ParamKind::Criteria,
        ParamType::Any,
    ),
];

static MINIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "min_range",
        "The range of cells to evaluate for the minimum value.",
        ParamKind::Range,
        ParamType::Number,
    )
    .measure_only()
    .allow_empty_in_slice(),
    ParameterDef::required(
        "criteria_range1",
        "The range of cells to evaluate against criteria1.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that determines which cells to evaluate.",
        ParamKind::Criteria,
        ParamType::Any,
    ),
];

static MAXIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "max_range",
        "The range of cells to evaluate for the maximum value.",
        ParamKind::Range,
        ParamType::Number,
    )
    .measure_only()
    .allow_empty_in_slice(),
    ParameterDef::required(
        "criteria_range1",
        "The range of cells to evaluate against criteria1.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that determines which cells to evaluate.",
        ParamKind::Criteria,
        ParamType::Any,
    ),
];

// ============================================================================
// Shared Evaluation Helper
// ============================================================================

/// Pair of criteria range cell values and compiled typed criterion.
type CriteriaPair<'a> = (&'a [CellValue], TypedCriterion);

fn extract_range_slot<'a>(
    func_name: &'static str,
    arg: &'a FunctionArg<'a>,
) -> Result<TypedRangeSlice<'a>, FunctionError> {
    match arg {
        FunctionArg::Array {
            rows,
            cols,
            values,
            error_types,
            ..
        } => Ok((*rows, *cols, values.as_slice(), error_types.as_slice())),
        FunctionArg::Scalar(_) | FunctionArg::TypedErrorScalar { .. } => {
            Err(FunctionError::InvalidArgumentKind {
                function: func_name,
                expected: "range",
                found: "scalar",
            })
        }
        FunctionArg::Lazy(_) => Err(FunctionError::InvalidArgumentKind {
            function: func_name,
            expected: "range",
            found: "lazy",
        }),
        FunctionArg::Criterion(_) => Err(FunctionError::InvalidArgumentKind {
            function: func_name,
            expected: "range",
            found: "criterion",
        }),
        FunctionArg::Omitted => Err(FunctionError::InvalidArgumentKind {
            function: func_name,
            expected: "range",
            found: "omitted",
        }),
    }
}

fn compile_equality_criterion(
    func_name: &'static str,
    crit_param: &'static str,
    crit_pos: usize,
    range_vt: Option<ValueType>,
    eq_op: EqualityOp,
    value: &CellValue,
) -> Result<TypedCriterion, FunctionError> {
    let prefix_utf16_len = match eq_op {
        EqualityOp::Exact => 0,
        EqualityOp::NotEq => 2,
    };
    let expected_ty = range_vt
        .and_then(ExpectedType::from_value_type)
        .unwrap_or(ExpectedType::NonEmpty);

    match value {
        CellValue::Empty => Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: func_name,
            param: crit_param,
            position: crit_pos,
            expected: expected_ty,
            found: ValueType::Empty,
        })),
        CellValue::Error(error) => Err(FunctionError::ErrorInCriteria {
            function: func_name,
            param: crit_param,
            position: crit_pos,
            error: *error,
        }),
        CellValue::Number(n) => {
            if !value_type_satisfies(ValueType::Number, expected_ty) {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    expected: expected_ty,
                    found: ValueType::Number,
                }));
            }
            if !n.is_finite() {
                return Err(FunctionError::ErrorInCriteria {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    error: ExcelError::Num,
                });
            }
            Ok(match eq_op {
                EqualityOp::Exact => TypedCriterion::NumberExact(normalize_zero(*n)),
                EqualityOp::NotEq => TypedCriterion::NumberCmp {
                    op: CriteriaComparator::NotEq,
                    threshold_15sig: normalize_criteria_number(*n),
                },
            })
        }
        CellValue::Boolean(b) => {
            if !value_type_satisfies(ValueType::Boolean, expected_ty) {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    expected: expected_ty,
                    found: ValueType::Boolean,
                }));
            }
            Ok(TypedCriterion::Boolean {
                op: eq_op,
                value: *b,
            })
        }
        CellValue::Text(s) => {
            if !value_type_satisfies(ValueType::Text, expected_ty) {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    expected: expected_ty,
                    found: ValueType::Text,
                }));
            }
            validate_text_criterion(s, prefix_utf16_len).map_err(|reason| {
                FunctionError::InvalidTextCriterion {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    reason,
                }
            })?;
            Ok(TypedCriterion::Text {
                op: eq_op,
                value: s.clone(),
            })
        }
    }
}

fn compile_bound_criterion(
    func_name: &'static str,
    cr_param: &'static str,
    cr_pos: usize,
    crit_param: &'static str,
    crit_pos: usize,
    range_vt: Option<ValueType>,
    bound: &BoundCriterion,
) -> Result<TypedCriterion, FunctionError> {
    match bound {
        BoundCriterion::BlankCheck(check) => Ok(TypedCriterion::Blank(*check)),

        BoundCriterion::Comparator {
            op:
                op @ (CriteriaComparator::Lt
                | CriteriaComparator::Lte
                | CriteriaComparator::Gt
                | CriteriaComparator::Gte),
            value,
        } => {
            if let Some(r_vt) = range_vt
                && r_vt != ValueType::Number
            {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: cr_param,
                    position: cr_pos,
                    expected: ExpectedType::Number,
                    found: r_vt,
                }));
            }
            match value {
                CellValue::Number(n) => {
                    if !n.is_finite() {
                        return Err(FunctionError::ErrorInCriteria {
                            function: func_name,
                            param: crit_param,
                            position: crit_pos,
                            error: ExcelError::Num,
                        });
                    }
                    Ok(TypedCriterion::NumberCmp {
                        op: *op,
                        threshold_15sig: normalize_criteria_number(*n),
                    })
                }
                CellValue::Error(error) => Err(FunctionError::ErrorInCriteria {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    error: *error,
                }),
                CellValue::Text(_) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    expected: ExpectedType::Number,
                    found: ValueType::Text,
                })),
                CellValue::Boolean(_) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    expected: ExpectedType::Number,
                    found: ValueType::Boolean,
                })),
                CellValue::Empty => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    expected: ExpectedType::Number,
                    found: ValueType::Empty,
                })),
            }
        }

        BoundCriterion::Errored {
            op:
                Some(
                    CriteriaComparator::Lt
                    | CriteriaComparator::Lte
                    | CriteriaComparator::Gt
                    | CriteriaComparator::Gte,
                ),
            error,
            inferred_type,
        } => {
            if let Some(r_vt) = range_vt
                && r_vt != ValueType::Number
            {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: cr_param,
                    position: cr_pos,
                    expected: ExpectedType::Number,
                    found: r_vt,
                }));
            }
            if let Some(c_vt) = *inferred_type
                && c_vt != ValueType::Number
            {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    expected: ExpectedType::Number,
                    found: c_vt,
                }));
            }
            Err(FunctionError::ErrorInCriteria {
                function: func_name,
                param: crit_param,
                position: crit_pos,
                error: *error,
            })
        }

        BoundCriterion::Errored {
            op: None | Some(CriteriaComparator::NotEq),
            error,
            inferred_type,
        } => {
            let expected_ty = range_vt
                .and_then(ExpectedType::from_value_type)
                .unwrap_or(ExpectedType::NonEmpty);
            if let Some(c_vt) = *inferred_type
                && !value_type_satisfies(c_vt, expected_ty)
            {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: func_name,
                    param: crit_param,
                    position: crit_pos,
                    expected: expected_ty,
                    found: c_vt,
                }));
            }
            Err(FunctionError::ErrorInCriteria {
                function: func_name,
                param: crit_param,
                position: crit_pos,
                error: *error,
            })
        }

        BoundCriterion::Exact(value) => compile_equality_criterion(
            func_name,
            crit_param,
            crit_pos,
            range_vt,
            EqualityOp::Exact,
            value,
        ),

        BoundCriterion::Comparator {
            op: CriteriaComparator::NotEq,
            value,
        } => compile_equality_criterion(
            func_name,
            crit_param,
            crit_pos,
            range_vt,
            EqualityOp::NotEq,
            value,
        ),
    }
}

/// Extracts and validates paired criteria ranges and bound criteria across all pairs.
fn collect_criteria_pairs<'a>(
    func: &dyn ExcelFunction,
    args: &'a [FunctionArg<'a>],
    start_arg_idx: usize,
    expected_rows: usize,
    expected_cols: usize,
) -> Result<Vec<CriteriaPair<'a>>, FunctionError> {
    let func_name = func.name();
    let remaining = args.len() - start_arg_idx;
    let num_pairs = remaining / 2;
    let mut pairs = Vec::with_capacity(num_pairs);

    for p in 0..num_pairs {
        let cr_idx = start_arg_idx + 2 * p;
        let crit_idx = cr_idx + 1;
        let cr_param = resolve_parameter(func, cr_idx)?.name;
        let crit_param = resolve_parameter(func, crit_idx)?.name;

        let (cr_rows, cr_cols, cr_values, cr_error_types) =
            extract_range_slot(func_name, &args[cr_idx])?;

        if cr_rows != expected_rows || cr_cols != expected_cols {
            return Err(FunctionError::DimensionMismatch {
                function: func_name,
                expected_rows,
                expected_cols,
                found_rows: cr_rows,
                found_cols: cr_cols,
            });
        }

        let bound_crit = match &args[crit_idx] {
            FunctionArg::Criterion(crit) => crit,
            FunctionArg::Scalar(_) | FunctionArg::TypedErrorScalar { .. } => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func_name,
                    expected: "criterion",
                    found: "scalar",
                });
            }
            FunctionArg::Array { .. } => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func_name,
                    expected: "criterion",
                    found: "array",
                });
            }
            FunctionArg::Lazy(_) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func_name,
                    expected: "criterion",
                    found: "lazy",
                });
            }
            FunctionArg::Omitted => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func_name,
                    expected: "criterion",
                    found: "omitted",
                });
            }
        };

        let range_vt =
            validate_homogeneous_slice(cr_values, cr_error_types, func_name, cr_param, cr_idx + 1)?;
        let criterion = compile_bound_criterion(
            func_name,
            cr_param,
            cr_idx + 1,
            crit_param,
            crit_idx + 1,
            range_vt,
            bound_crit,
        )?;
        pairs.push((cr_values, criterion));
    }

    Ok(pairs)
}

/// Evaluates whether all criteria match for a given cell position index.
fn all_criteria_match(pairs: &[CriteriaPair<'_>], index: usize) -> bool {
    for (cr_values, criterion) in pairs {
        let Some(val) = cr_values.get(index) else {
            return false;
        };
        if !criterion.matches(val) {
            return false;
        }
    }
    true
}

// ============================================================================
// SUMIFS
// ============================================================================

/// Built-in `SUMIFS` conditional summation function.
#[derive(Debug, Default)]
pub struct SumifsFunction;

impl ExcelFunction for SumifsFunction {
    fn name(&self) -> &'static str {
        "SUMIFS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        SUMIFS_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Pairs
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::ShapeCoupled(ShapePolicy::EqualTargetAndOddPairRanges)
    }

    fn evaluate<'a>(
        &self,
        args: &[FunctionArg<'a>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (target_rows, target_cols, target_values, _) =
            extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self, args, 1, target_rows, target_cols)?;

        let mut sum = 0.0;
        for (i, cell) in target_values.iter().enumerate() {
            if all_criteria_match(&pairs, i) {
                match cell {
                    CellValue::Error(e) => return Ok(CellValue::Error(*e)),
                    CellValue::Number(n) => {
                        if !n.is_finite() {
                            return Ok(CellValue::Error(ExcelError::Num));
                        }
                        sum += *n;
                    }
                    CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {}
                }
            }
        }

        Ok(CellValue::number(sum))
    }
}

// ============================================================================
// COUNTIFS
// ============================================================================

/// Built-in `COUNTIFS` conditional counting function.
#[derive(Debug, Default)]
pub struct CountifsFunction;

impl ExcelFunction for CountifsFunction {
    fn name(&self) -> &'static str {
        "COUNTIFS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        COUNTIFS_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Pairs
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::ShapeCoupled(ShapePolicy::EqualEvenPairRanges)
    }

    fn evaluate<'a>(
        &self,
        args: &[FunctionArg<'a>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (cr1_rows, cr1_cols, _, _) = extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self, args, 0, cr1_rows, cr1_cols)?;

        let total_cells = cr1_rows * cr1_cols;
        let mut count = 0.0;
        for i in 0..total_cells {
            if all_criteria_match(&pairs, i) {
                count += 1.0;
            }
        }

        Ok(CellValue::number(count))
    }
}

// ============================================================================
// AVERAGEIFS
// ============================================================================

/// Built-in `AVERAGEIFS` conditional arithmetic mean function.
#[derive(Debug, Default)]
pub struct AverageifsFunction;

impl ExcelFunction for AverageifsFunction {
    fn name(&self) -> &'static str {
        "AVERAGEIFS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        AVERAGEIFS_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Pairs
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::ShapeCoupled(ShapePolicy::EqualTargetAndOddPairRanges)
    }

    fn evaluate<'a>(
        &self,
        args: &[FunctionArg<'a>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (target_rows, target_cols, target_values, _) =
            extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self, args, 1, target_rows, target_cols)?;

        let mut sum = 0.0;
        let mut count = 0usize;

        for (i, cell) in target_values.iter().enumerate() {
            if all_criteria_match(&pairs, i) {
                match cell {
                    CellValue::Error(e) => return Ok(CellValue::Error(*e)),
                    CellValue::Number(n) => {
                        if !n.is_finite() {
                            return Ok(CellValue::Error(ExcelError::Num));
                        }
                        sum += *n;
                        count += 1;
                    }
                    CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {}
                }
            }
        }

        if count == 0 {
            Ok(CellValue::Error(ExcelError::Div0))
        } else {
            Ok(CellValue::number(sum / count as f64))
        }
    }
}

// ============================================================================
// MINIFS
// ============================================================================

/// Built-in `MINIFS` conditional minimum value function.
#[derive(Debug, Default)]
pub struct MinifsFunction;

impl ExcelFunction for MinifsFunction {
    fn name(&self) -> &'static str {
        "MINIFS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        MINIFS_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Pairs
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::ShapeCoupled(ShapePolicy::EqualTargetAndOddPairRanges)
    }

    fn evaluate<'a>(
        &self,
        args: &[FunctionArg<'a>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (target_rows, target_cols, target_values, _) =
            extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self, args, 1, target_rows, target_cols)?;

        let mut min_val: Option<f64> = None;

        for (i, cell) in target_values.iter().enumerate() {
            if all_criteria_match(&pairs, i) {
                match cell {
                    CellValue::Error(e) => return Ok(CellValue::Error(*e)),
                    CellValue::Number(n) => {
                        if !n.is_finite() {
                            return Ok(CellValue::Error(ExcelError::Num));
                        }
                        min_val = Some(min_val.map_or(*n, |m| f64::min(m, *n)));
                    }
                    CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {}
                }
            }
        }

        Ok(CellValue::number(min_val.unwrap_or(0.0)))
    }
}

// ============================================================================
// MAXIFS
// ============================================================================

/// Built-in `MAXIFS` conditional maximum value function.
#[derive(Debug, Default)]
pub struct MaxifsFunction;

impl ExcelFunction for MaxifsFunction {
    fn name(&self) -> &'static str {
        "MAXIFS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        MAXIFS_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Pairs
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::ShapeCoupled(ShapePolicy::EqualTargetAndOddPairRanges)
    }

    fn evaluate<'a>(
        &self,
        args: &[FunctionArg<'a>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (target_rows, target_cols, target_values, _) =
            extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self, args, 1, target_rows, target_cols)?;

        let mut max_val: Option<f64> = None;

        for (i, cell) in target_values.iter().enumerate() {
            if all_criteria_match(&pairs, i) {
                match cell {
                    CellValue::Error(e) => return Ok(CellValue::Error(*e)),
                    CellValue::Number(n) => {
                        if !n.is_finite() {
                            return Ok(CellValue::Error(ExcelError::Num));
                        }
                        max_val = Some(max_val.map_or(*n, |m| f64::max(m, *n)));
                    }
                    CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {}
                }
            }
        }

        Ok(CellValue::number(max_val.unwrap_or(0.0)))
    }
}
