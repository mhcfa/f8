//! `XLOOKUP` function implementation.

use crate::functions::common::optional_scalar_integer;
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{
    MatchMode, SearchMode, find_match_index, validate_lookup_array,
};
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
        ParamType::Any,
    ),
    ParameterDef::required(
        "lookup_array",
        "The 1D range to search.",
        ParamKind::Range,
        ParamType::Any,
    )
    .allow_empty(),
    ParameterDef::required(
        "return_array",
        "The 1D range to return matching items from.",
        ParamKind::Range,
        ParamType::Any,
    )
    .allow_empty(),
    ParameterDef::optional(
        "if_not_found",
        "The value to return if no match is found.",
        ParamKind::Lazy,
        ParamType::Any,
    )
    .allow_empty(),
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
        let [lookup_arg, lookup_arr_arg, return_arr_arg, ..] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        };

        let lookup_val = lookup_arg.eval_scalar(self.name(), ctx)?;
        let lookup_slice = lookup_arr_arg.as_range_slice(self.name())?;
        let return_slice = return_arr_arg.as_range_slice(self.name())?;

        let match_mode =
            match optional_scalar_integer(args, 4, 0).and_then(MatchMode::from_mode_int) {
                Ok(mode) => mode,
                Err(err) => return Ok(CellValue::Error(err)),
            };

        let search_mode =
            match optional_scalar_integer(args, 5, 1).and_then(SearchMode::from_mode_int) {
                Ok(mode) => mode,
                Err(err) => return Ok(CellValue::Error(err)),
            };

        if let Err(err) = validate_lookup_array(lookup_slice, search_mode) {
            return Ok(CellValue::Error(err));
        }

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
