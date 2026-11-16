//! `XMATCH` function implementation.

use crate::functions::common::{optional_scalar_number, require_args};
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{MatchMode, SearchMode, find_match_index};
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, LookupPolicy, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType, f64_to_strict_integer};

/// Positional parameter definitions for `XMATCH`.
static XMATCH_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "lookup_value",
        "The value you want to search for.",
        ParamKind::Scalar,
        ParamType::Any,
    ),
    ParameterDef::required(
        "lookup_array",
        "The 1D range to search.",
        ParamKind::Range,
        ParamType::Any,
    )
    .allow_empty_in_slice(),
    ParameterDef::with_default(
        "match_mode",
        "The match type: 0 (exact), -1 (exact or next smaller), 1 (exact or next larger).",
        ParamKind::Scalar,
        ParamType::Number,
        CellValue::Number(0.0),
    ),
    ParameterDef::with_default(
        "search_mode",
        "The search type: 1 (first-to-last), -1 (last-to-first), 2 (binary ascending), -2 (binary descending).",
        ParamKind::Scalar,
        ParamType::Number,
        CellValue::Number(1.0),
    ),
];

/// `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`:
/// Returns the relative position of an item in a range.
#[derive(Debug, Default, Clone, Copy)]
pub struct XmatchFunction;

impl ExcelFunction for XmatchFunction {
    fn name(&self) -> &'static str {
        "XMATCH"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Lookup
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        XMATCH_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Lookup(LookupPolicy::Xmatch)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let [lookup_arg, lookup_arr_arg] = require_args(self, args)?;

        let lookup_val = lookup_arg.eval_scalar(self.name(), "lookup_value", 1, ctx)?;
        let (_, _, slice) = lookup_arr_arg.as_1d_range(self.name(), "lookup_array")?;

        let mm_res = optional_scalar_number(args, 2, 0.0);
        let sm_res = optional_scalar_number(args, 3, 1.0);

        if let CellValue::Error(err) = lookup_val {
            return Ok(CellValue::Error(err));
        }
        let mm_f = match mm_res {
            Ok(v) => v,
            Err(err) => return Ok(CellValue::Error(err)),
        };
        let sm_f = match sm_res {
            Ok(v) => v,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        let match_mode = match f64_to_strict_integer(mm_f).and_then(MatchMode::from_mode_int) {
            Ok(mode) => mode,
            Err(err) => return Ok(CellValue::Error(err)),
        };
        let search_mode = match f64_to_strict_integer(sm_f).and_then(SearchMode::from_mode_int) {
            Ok(mode) => mode,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        match find_match_index(&lookup_val, slice, match_mode, search_mode) {
            Ok(Some(idx)) => Ok(CellValue::number((idx + 1) as f64)),
            Ok(None) => Ok(CellValue::Error(ExcelError::Na)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
