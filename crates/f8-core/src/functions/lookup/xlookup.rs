//! `XLOOKUP` function implementation.

use crate::functions::common::optional_scalar_integer;
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{
    MatchMode, SearchMode, extract_1d_range_slice, find_match_index, validate_homogeneous_slice,
    validate_lookup_array,
};
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, LookupPolicy, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Positional parameter definitions for `XLOOKUP`.
static XLOOKUP_PARAMS: &[ParameterDef] = &[
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
    ParameterDef::required(
        "return_array",
        "The 1D range to return matching items from.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::optional(
        "if_not_found",
        "The value to return if no match is found.",
        ParamKind::Lazy,
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

/// `XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])`:
/// Modern lookup returning a value from matching return range.
#[derive(Debug, Default, Clone, Copy)]
pub struct XlookupFunction;

impl ExcelFunction for XlookupFunction {
    fn name(&self) -> &'static str {
        "XLOOKUP"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Lookup
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        XLOOKUP_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Lookup(LookupPolicy::Xlookup)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let [lookup_arg, lookup_arr_arg, return_arr_arg, ..] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        };

        let lookup_val = lookup_arg.eval_scalar(self.name(), ctx)?;
        let lookup_vt = lookup_val.value_type().filter(|vt| *vt != ValueType::Empty);
        let (l_rows, l_cols, lookup_slice) =
            extract_1d_range_slice(lookup_arr_arg, self.name(), "lookup_array")?;
        let (r_rows, r_cols, return_slice) =
            extract_1d_range_slice(return_arr_arg, self.name(), "return_array")?;

        if (l_rows, l_cols) != (r_rows, r_cols) {
            return Err(FunctionError::DimensionMismatch {
                function: self.name(),
                expected_rows: l_rows,
                expected_cols: l_cols,
                found_rows: r_rows,
                found_cols: r_cols,
            });
        }

        let match_mode_int = optional_scalar_integer(args, 4, 0).map_err(|_| {
            FunctionError::NonLiteralLookupMode {
                function: self.name(),
                param: "match_mode",
                position: 5,
            }
        })?;
        let match_mode = MatchMode::from_mode_int(match_mode_int, self.name(), "match_mode", 5)?;

        let search_mode_int = optional_scalar_integer(args, 5, 1).map_err(|_| {
            FunctionError::NonLiteralLookupMode {
                function: self.name(),
                param: "search_mode",
                position: 6,
            }
        })?;
        let search_mode =
            SearchMode::from_mode_int(search_mode_int, self.name(), "search_mode", 6)?;

        validate_lookup_array(lookup_slice, lookup_vt, search_mode, self.name())?;
        validate_homogeneous_slice(return_slice, self.name(), "return_array", 3)?;

        if let CellValue::Error(err) = lookup_val {
            return Ok(CellValue::Error(err));
        }

        match find_match_index(&lookup_val, lookup_slice, match_mode, search_mode) {
            Ok(Some(idx)) => Ok(return_slice
                .get(idx)
                .cloned()
                .unwrap_or(CellValue::Error(ExcelError::Ref))),
            Ok(None) => match args.get(3) {
                None | Some(FunctionArg::Omitted) => Ok(CellValue::Error(ExcelError::Na)),
                Some(not_found_arg) => not_found_arg.eval_scalar(self.name(), ctx),
            },
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
