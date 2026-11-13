//! Excel conditional aggregation functions (*IFS family).
//!
//! Provides conditional aggregation implementations including `SUMIFS`, `AVERAGEIFS`,
//! `COUNTIFS`, `MAXIFS`, and `MINIFS` conforming to Excel calculation semantics.

use crate::functions::error::FunctionError;
use crate::functions::math::criteria::Criterion;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ShapePolicy, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

// ============================================================================
// Parameter Definitions
// ============================================================================

static SUMIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "sum_range",
        "The range of cells to sum if criteria are met.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria_range1",
        "The range of cells to evaluate against criteria1.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that determines which cells in criteria_range1 to sum.",
        ParamKind::Scalar,
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
        ParamKind::Scalar,
        ParamType::Any,
    ),
];

static AVERAGEIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "average_range",
        "The range of cells to average if criteria are met.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria_range1",
        "The range of cells to evaluate against criteria1.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that determines which cells to average.",
        ParamKind::Scalar,
        ParamType::Any,
    ),
];

static MINIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "min_range",
        "The range of cells to evaluate for the minimum value.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria_range1",
        "The range of cells to evaluate against criteria1.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that determines which cells to evaluate.",
        ParamKind::Scalar,
        ParamType::Any,
    ),
];

static MAXIFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "max_range",
        "The range of cells to evaluate for the maximum value.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria_range1",
        "The range of cells to evaluate against criteria1.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "criteria1",
        "The condition that determines which cells to evaluate.",
        ParamKind::Scalar,
        ParamType::Any,
    ),
];

// ============================================================================
// Shared Evaluation Helper
// ============================================================================

/// Pair of criteria range cell values and compiled criterion.
type CriteriaPair<'a> = (&'a [CellValue], Criterion);

fn extract_range_slot<'a>(
    func_name: &'static str,
    arg: &'a FunctionArg<'a>,
) -> Result<(usize, usize, &'a [CellValue]), FunctionError> {
    match arg {
        FunctionArg::Array {
            rows, cols, values, ..
        } => Ok((*rows, *cols, values.as_slice())),
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
        FunctionArg::Omitted => Err(FunctionError::InvalidArgumentKind {
            function: func_name,
            expected: "range",
            found: "omitted",
        }),
    }
}

/// Extracts and validates paired criteria ranges and criteria values, enforcing identical `(rows, cols)`.
fn collect_criteria_pairs<'a>(
    func_name: &'static str,
    args: &'a [FunctionArg<'a>],
    start_arg_idx: usize,
    expected_rows: usize,
    expected_cols: usize,
    ctx: &dyn EvalContext,
) -> Result<Vec<CriteriaPair<'a>>, FunctionError> {
    let remaining = args.len() - start_arg_idx;
    let num_pairs = remaining / 2;
    let mut pairs = Vec::with_capacity(num_pairs);

    for p in 0..num_pairs {
        let cr_idx = start_arg_idx + 2 * p;
        let crit_idx = cr_idx + 1;

        let (cr_rows, cr_cols, cr_values) = extract_range_slot(func_name, &args[cr_idx])?;

        if cr_rows != expected_rows || cr_cols != expected_cols {
            return Err(FunctionError::DimensionMismatch {
                function: func_name,
                expected_rows,
                expected_cols,
                found_rows: cr_rows,
                found_cols: cr_cols,
            });
        }

        let crit_val = args[crit_idx].eval_scalar(func_name, ctx)?;
        let criterion = Criterion::parse(&crit_val);
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (target_rows, target_cols, target_values) = extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self.name(), args, 1, target_rows, target_cols, ctx)?;

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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (cr1_rows, cr1_cols, _) = extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self.name(), args, 0, cr1_rows, cr1_cols, ctx)?;

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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (target_rows, target_cols, target_values) = extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self.name(), args, 1, target_rows, target_cols, ctx)?;

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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (target_rows, target_cols, target_values) = extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self.name(), args, 1, target_rows, target_cols, ctx)?;

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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let (target_rows, target_cols, target_values) = extract_range_slot(self.name(), &args[0])?;
        let pairs = collect_criteria_pairs(self.name(), args, 1, target_rows, target_cols, ctx)?;

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
