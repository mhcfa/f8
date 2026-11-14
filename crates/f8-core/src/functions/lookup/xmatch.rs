//! `XMATCH` function implementation.

use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{
    MatchMode, SearchMode, extract_1d_range_slice, find_match_index, validate_lookup_array,
};
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, LookupPolicy, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Positional parameter definitions for `XMATCH`.
static XMATCH_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "lookup_value",
        "The value you want to search for.",
        ParamKind::Scalar,
        ParamType::NonEmpty,
    ),
    ParameterDef::required(
        "lookup_array",
        "The 1D range to search.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::with_default(
        "match_mode",
        "The match type: 0 (exact), -1 (exact or next smaller), 1 (exact or next larger).",
        ParamKind::Scalar,
        ParamType::Integer,
        CellValue::Number(0.0),
    ),
    ParameterDef::with_default(
        "search_mode",
        "The search type: 1 (first-to-last), -1 (last-to-first), 2 (binary ascending), -2 (binary descending).",
        ParamKind::Scalar,
        ParamType::Integer,
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
        validate_bound_args(self, args)?;

        let [lookup_arg, lookup_arr_arg, ..] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        };

        let (lookup_val, lookup_vt) = lookup_arg.eval_scalar_with_type(self.name(), ctx)?;
        let (_rows, _cols, slice, slice_err_types) =
            extract_1d_range_slice(lookup_arr_arg, self.name(), "lookup_array")?;

        let match_mode = match args.get(2) {
            None | Some(FunctionArg::Omitted) => MatchMode::Exact,
            Some(
                match_arg @ (FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Array { .. }
                | FunctionArg::Lazy(_)
                | FunctionArg::Criterion(_)),
            ) => {
                let val = match_arg.eval_scalar(self.name(), ctx)?;
                MatchMode::parse(&val, self.name(), "match_mode", 3)?
            }
        };

        let search_mode = match args.get(3) {
            None | Some(FunctionArg::Omitted) => SearchMode::FirstToLast,
            Some(
                search_arg @ (FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Array { .. }
                | FunctionArg::Lazy(_)
                | FunctionArg::Criterion(_)),
            ) => {
                let val = search_arg.eval_scalar(self.name(), ctx)?;
                SearchMode::parse(&val, self.name(), "search_mode", 4)?
            }
        };

        validate_lookup_array(slice, slice_err_types, lookup_vt, search_mode, self.name())?;

        if let CellValue::Error(err) = lookup_val {
            return Ok(CellValue::Error(err));
        }

        match find_match_index(&lookup_val, slice, match_mode, search_mode) {
            Ok(Some(idx)) => Ok(CellValue::number((idx + 1) as f64)),
            Ok(None) => Ok(CellValue::Error(ExcelError::Na)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
