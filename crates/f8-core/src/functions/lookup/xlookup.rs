//! `XLOOKUP` function implementation.

use crate::functions::common::{optional_scalar_number, require_args};
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{MatchMode, SearchMode, find_match_index};
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, LookupPolicy, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, TypeError, ValueType, f64_to_strict_integer};

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
    .allow_empty_in_slice(),
    ParameterDef::required(
        "return_array",
        "The 1D range to return matching items from.",
        ParamKind::Range,
        ParamType::Any,
    )
    .allow_empty_in_slice(),
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
        let [lookup_arg, lookup_arr_arg, return_arr_arg] = require_args(self, args)?;

        let lookup_val = lookup_arg.eval_scalar(self.name(), "lookup_value", 1, ctx)?;
        let (lr, lc, lookup_slice) = lookup_arr_arg.as_1d_range(self.name(), "lookup_array")?;
        let (rr, rc, return_slice) = return_arr_arg.as_1d_range(self.name(), "return_array")?;
        if (lr, lc) != (rr, rc) {
            return Err(FunctionError::DimensionMismatch {
                function: self.name(),
                expected_rows: lr,
                expected_cols: lc,
                found_rows: rr,
                found_cols: rc,
            });
        }

        let mm_res = optional_scalar_number(args, 4, 0.0);
        let sm_res = optional_scalar_number(args, 5, 1.0);

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

        match find_match_index(&lookup_val, lookup_slice, match_mode, search_mode) {
            Ok(Some(idx)) => {
                let val = return_slice
                    .get(idx)
                    .cloned()
                    .unwrap_or(CellValue::Error(ExcelError::Ref));
                if val.is_empty() {
                    return Err(FunctionError::Type(TypeError::EmptyArgument {
                        function: self.name(),
                        param: "return_array",
                        position: 3,
                    }));
                }
                Ok(val)
            }
            Ok(None) => match args.get(3) {
                None | Some(FunctionArg::Omitted) => Ok(CellValue::Error(ExcelError::Na)),
                Some(not_found_arg) => {
                    not_found_arg.eval_scalar(self.name(), "if_not_found", 4, ctx)
                }
            },
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
