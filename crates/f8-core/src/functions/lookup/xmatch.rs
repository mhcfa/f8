//! `XMATCH` function implementation.

use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{MatchMode, SearchMode, extract_1d_slice, find_match_index};
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Positional parameter definitions for `XMATCH`.
static XMATCH_PARAMS: &[ParameterDef] = &[
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

/// `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`:
/// Returns the relative position of an item in an array or range.
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 2 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }
        if args.len() > 4 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 4,
                found: args.len(),
            });
        }

        let lookup_val = args[0].eval_scalar(self.name(), ctx)?;
        if let CellValue::Error(err) = lookup_val {
            return Ok(CellValue::Error(err));
        }

        let slice = match extract_1d_slice(&args[1]) {
            Ok(s) => s,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        let match_mode = if args.len() > 2 && !matches!(args[2], FunctionArg::Omitted) {
            let val = args[2].eval_scalar(self.name(), ctx)?;
            match MatchMode::parse(&val) {
                Ok(m) => m,
                Err(err) => return Ok(CellValue::Error(err)),
            }
        } else {
            MatchMode::Exact
        };

        let search_mode = if args.len() > 3 && !matches!(args[3], FunctionArg::Omitted) {
            let val = args[3].eval_scalar(self.name(), ctx)?;
            match SearchMode::parse(&val) {
                Ok(s) => s,
                Err(err) => return Ok(CellValue::Error(err)),
            }
        } else {
            SearchMode::FirstToLast
        };

        match find_match_index(&lookup_val, slice, match_mode, search_mode) {
            Ok(Some(idx)) => Ok(CellValue::Number((idx + 1) as f64)),
            Ok(None) => Ok(CellValue::Error(ExcelError::Na)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
