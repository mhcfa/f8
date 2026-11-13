//! `XLOOKUP` function implementation.

use crate::functions::common::{BranchTypeUnifier, eval_taken_branch_arm, validate_dormant_arm};
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

        let (lookup_val, lookup_vt) = lookup_arg.eval_scalar_with_type(self.name(), ctx)?;
        let (l_rows, l_cols, lookup_slice, lookup_err_types) =
            extract_1d_range_slice(lookup_arr_arg, self.name(), "lookup_array")?;
        let (r_rows, r_cols, return_slice, return_err_types) =
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

        let match_mode = match args.get(4) {
            None | Some(FunctionArg::Omitted) => MatchMode::Exact,
            Some(
                match_arg @ (FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Array { .. }
                | FunctionArg::Lazy(_)),
            ) => {
                let val = match_arg.eval_scalar(self.name(), ctx)?;
                MatchMode::parse(&val, self.name(), "match_mode", 5)?
            }
        };

        let search_mode = match args.get(5) {
            None | Some(FunctionArg::Omitted) => SearchMode::FirstToLast,
            Some(
                search_arg @ (FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Array { .. }
                | FunctionArg::Lazy(_)),
            ) => {
                let val = search_arg.eval_scalar(self.name(), ctx)?;
                SearchMode::parse(&val, self.name(), "search_mode", 6)?
            }
        };

        validate_lookup_array(
            lookup_slice,
            lookup_err_types,
            lookup_vt,
            search_mode,
            self.name(),
        )?;
        let ret_vt = validate_homogeneous_slice(
            return_slice,
            return_err_types,
            self.name(),
            "return_array",
            3,
        )?;
        let mut unifier = BranchTypeUnifier::with_initial(self.name(), ret_vt);

        if let CellValue::Error(err) = lookup_val {
            if let Some(nf_arg) = args.get(3) {
                let nf_vt = validate_dormant_arm(nf_arg, self.name(), None, ctx)?;
                unifier.record(4, nf_vt).map_err(FunctionError::Type)?;
            }
            return Ok(CellValue::Error(err));
        }

        match find_match_index(&lookup_val, lookup_slice, match_mode, search_mode) {
            Ok(Some(idx)) => {
                if let Some(nf_arg) = args.get(3) {
                    let nf_vt = validate_dormant_arm(nf_arg, self.name(), None, ctx)?;
                    unifier.record(4, nf_vt).map_err(FunctionError::Type)?;
                }
                Ok(return_slice
                    .get(idx)
                    .cloned()
                    .unwrap_or(CellValue::Error(ExcelError::Ref)))
            }
            Ok(None) => match args.get(3) {
                None | Some(FunctionArg::Omitted) => Ok(CellValue::Error(ExcelError::Na)),
                Some(
                    not_found_arg @ (FunctionArg::Scalar(_)
                    | FunctionArg::TypedErrorScalar { .. }
                    | FunctionArg::Array { .. }
                    | FunctionArg::Lazy(_)),
                ) => {
                    let (nf_val, nf_vt) = eval_taken_branch_arm(not_found_arg, self.name(), ctx)?;
                    unifier.record(4, nf_vt).map_err(FunctionError::Type)?;
                    Ok(nf_val)
                }
            },
            Err(err) => {
                if let Some(nf_arg) = args.get(3) {
                    let nf_vt = validate_dormant_arm(nf_arg, self.name(), None, ctx)?;
                    unifier.record(4, nf_vt).map_err(FunctionError::Type)?;
                }
                Ok(CellValue::Error(err))
            }
        }
    }
}
