//! Excel conditional aggregation functions (*IFS family).
//!
//! Provides conditional aggregation implementations including `SUMIFS`, `AVERAGEIFS`,
//! `COUNTIFS`, `MAXIFS`, and `MINIFS` conforming to Excel calculation semantics.

use crate::expr::ast::CriteriaComparator;
use crate::functions::binder::resolve_parameter;
use crate::functions::common::{NumericCollector, value_type_satisfies};
use crate::functions::error::FunctionError;
use crate::functions::math::criteria::{EqualityOp, TypedCriterion};
use crate::functions::registry::validate_bound_args;
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

/// Extracts and validates paired criteria ranges and bound criteria across all pairs after `validate_bound_args`.
fn collect_criteria_pairs<'a>(
    func: &dyn ExcelFunction,
    args: &'a [FunctionArg<'a>],
    start_arg_idx: usize,
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

        let cr_values = args[cr_idx].as_range_slice(func_name)?;
        let FunctionArg::Criterion(bound_crit) = &args[crit_idx] else {
            return Err(FunctionError::InvalidArgumentKind {
                function: func_name,
                expected: "criterion",
                found: "scalar",
            });
        };

        let range_vt = cr_values
            .iter()
            .find_map(|c| c.value_type().filter(|vt| *vt != ValueType::Empty));
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

/// Accumulates numeric observations from `args[0]` across cells matching all criteria pairs (`SUMIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS`).
fn collect_conditional_numbers<'a>(
    func: &dyn ExcelFunction,
    args: &'a [FunctionArg<'a>],
) -> Result<Result<NumericCollector, ExcelError>, FunctionError> {
    let target_values = args[0].as_range_slice(func.name())?;
    let pairs = collect_criteria_pairs(func, args, 1)?;

    let mut collector = NumericCollector::new();
    for (i, cell) in target_values.iter().enumerate() {
        if all_criteria_match(&pairs, i) {
            match cell {
                CellValue::Error(e) => return Ok(Err(*e)),
                CellValue::Number(n) => {
                    if !n.is_finite() {
                        return Ok(Err(ExcelError::Num));
                    }
                    collector.record(normalize_zero(*n));
                }
                CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {}
            }
        }
    }
    collector.sum = normalize_zero(collector.sum);
    Ok(Ok(collector))
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

        match collect_conditional_numbers(self, args)? {
            Ok(c) => Ok(CellValue::number(c.sum)),
            Err(e) => Ok(CellValue::Error(e)),
        }
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

        let cr1_values = args[0].as_range_slice(self.name())?;
        let pairs = collect_criteria_pairs(self, args, 0)?;

        let total_cells = cr1_values.len();
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

        match collect_conditional_numbers(self, args)? {
            Ok(c) if c.count == 0 => Ok(CellValue::Error(ExcelError::Div0)),
            Ok(c) => Ok(CellValue::number(c.sum / c.count as f64)),
            Err(e) => Ok(CellValue::Error(e)),
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

        match collect_conditional_numbers(self, args)? {
            Ok(c) => Ok(CellValue::number(c.min_or_zero())),
            Err(e) => Ok(CellValue::Error(e)),
        }
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

        match collect_conditional_numbers(self, args)? {
            Ok(c) => Ok(CellValue::number(c.max_or_zero())),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}
