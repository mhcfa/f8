//! `XLOOKUP` function implementation.

use crate::functions::common::eval_branch_arg;
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{MatchMode, SearchMode, extract_1d_slice, find_match_index};
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Positional parameter definitions for `XLOOKUP`.
static XLOOKUP_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "lookup_value",
        "The value you want to search for.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "lookup_array",
        "The 1D range or array to search.",
        ParamKind::RangeOrArray,
    ),
    ParameterDef::required(
        "return_array",
        "The 1D range or array to return matching items from.",
        ParamKind::RangeOrArray,
    ),
    ParameterDef::optional(
        "if_not_found",
        "The value to return if no match is found.",
        ParamKind::Lazy,
    ),
    ParameterDef {
        name: "match_mode",
        description: "The match type: 0 (exact), -1 (exact or next smaller), 1 (exact or next larger), 2 (wildcard).",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(0.0)),
    },
    ParameterDef {
        name: "search_mode",
        description: "The search type: 1 (first-to-last), -1 (last-to-first), 2 (binary ascending), -2 (binary descending).",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(1.0)),
    },
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 3 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        }
        if args.len() > 6 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 6,
                found: args.len(),
            });
        }

        let lookup_val = args[0].eval_scalar(self.name(), ctx)?;
        if let CellValue::Error(err) = lookup_val {
            return Ok(CellValue::Error(err));
        }

        let lookup_slice = match extract_1d_slice(&args[1]) {
            Ok(s) => s,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        let return_slice = match extract_1d_slice(&args[2]) {
            Ok(s) => s,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        if lookup_slice.len() != return_slice.len() {
            return Ok(CellValue::Error(ExcelError::Value));
        }

        let match_mode = if args.len() > 4 && !matches!(args[4], FunctionArg::Omitted) {
            let val = args[4].eval_scalar(self.name(), ctx)?;
            match MatchMode::parse(&val) {
                Ok(m) => m,
                Err(err) => return Ok(CellValue::Error(err)),
            }
        } else {
            MatchMode::Exact
        };

        let search_mode = if args.len() > 5 && !matches!(args[5], FunctionArg::Omitted) {
            let val = args[5].eval_scalar(self.name(), ctx)?;
            match SearchMode::parse(&val) {
                Ok(s) => s,
                Err(err) => return Ok(CellValue::Error(err)),
            }
        } else {
            SearchMode::FirstToLast
        };

        match find_match_index(&lookup_val, lookup_slice, match_mode, search_mode) {
            Ok(Some(idx)) => Ok(return_slice[idx].clone()),
            Ok(None) => {
                if args.len() > 3 && !matches!(args[3], FunctionArg::Omitted) {
                    eval_branch_arg(&args[3], self.name(), ctx)
                } else {
                    Ok(CellValue::Error(ExcelError::Na))
                }
            }
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
