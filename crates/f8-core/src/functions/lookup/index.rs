//! `INDEX` function implementation.

use crate::functions::common::{require_args, scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, LookupPolicy, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, TypeError, ValueType, f64_to_strict_integer};

/// Positional parameter definitions for `INDEX`.
static INDEX_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "array",
        "The range of cells to index into.",
        ParamKind::Range,
        ParamType::Any,
    )
    .allow_empty_in_slice(),
    ParameterDef::required(
        "row_num",
        "The 1-based row (or 1D element) position in the range.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::optional(
        "col_num",
        "The 1-based column position in the range.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
];

/// `INDEX(array, row_num, [col_num])`:
/// Returns the value of a cell in a range selected by row and column position.
#[derive(Debug, Default, Clone, Copy)]
pub struct IndexFunction;

impl ExcelFunction for IndexFunction {
    fn name(&self) -> &'static str {
        "INDEX"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Lookup
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        INDEX_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Lookup(LookupPolicy::Index)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let [array_arg, row_arg] = require_args(self, args)?;
        let FunctionArg::Range {
            rows, cols, values, ..
        } = array_arg
        else {
            return Err(FunctionError::InvalidArgumentKind {
                function: self.name(),
                expected: "range",
                found: array_arg.kind_name(),
            });
        };
        let (rows, cols, values) = (*rows, *cols, values.as_slice());

        let col_slot = match args.get(2) {
            None | Some(FunctionArg::Omitted) => None,
            Some(slot) => Some(slot),
        };
        if col_slot.is_none() && rows > 1 && cols > 1 {
            return Err(FunctionError::MissingColumnIndexFor2DSlice {
                function: self.name(),
                rows,
                cols,
            });
        }

        let row_f_res = scalar_number(row_arg);
        let col_f_res = col_slot.map(scalar_number);

        if rows == 0 || cols == 0 || values.is_empty() {
            return Ok(CellValue::Error(ExcelError::Ref));
        }

        let row_f = match row_f_res {
            Ok(r) => r,
            Err(err) => return Ok(CellValue::Error(err)),
        };
        let col_f_opt = match col_f_res {
            Some(Ok(c)) => Some(c),
            Some(Err(err)) => return Ok(CellValue::Error(err)),
            None => None,
        };

        let row_num = match f64_to_strict_integer(row_f) {
            Ok(r) => r,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        let (target_row, target_col) = if let Some(col_f) = col_f_opt {
            let col_num = match f64_to_strict_integer(col_f) {
                Ok(c) => c,
                Err(err) => return Ok(CellValue::Error(err)),
            };
            (row_num, col_num)
        } else if rows == 1 {
            // Horizontal 1D vector: row_num indexes along the single row
            (1, row_num)
        } else {
            // Vertical 1D vector (cols == 1)
            (row_num, 1)
        };

        if target_row < 1 || target_row > rows as i64 || target_col < 1 || target_col > cols as i64
        {
            return Ok(CellValue::Error(ExcelError::Ref));
        }

        let idx = (target_row - 1) as usize * cols + (target_col - 1) as usize;
        let val = values
            .get(idx)
            .cloned()
            .unwrap_or(CellValue::Error(ExcelError::Ref));
        if val.is_empty() {
            return Err(FunctionError::Type(TypeError::EmptyArgument {
                function: self.name(),
                param: "array",
                position: 1,
            }));
        }
        Ok(val)
    }
}
