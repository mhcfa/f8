//! `INDEX` function implementation.

use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Positional parameter definitions for `INDEX`.
static INDEX_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "array",
        "The range of cells or an array constant.",
        ParamKind::RangeOrArray,
    ),
    ParameterDef::required(
        "row_num",
        "The row position in the array or range to return a value from.",
        ParamKind::Scalar,
    ),
    ParameterDef::optional(
        "col_num",
        "The column position in the array or range to return a value from.",
        ParamKind::Scalar,
    ),
];

/// `INDEX(array, row_num, [col_num])`:
/// Returns the value of a cell in a table or array selected by row and column position.
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
        if args.len() > 3 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        }

        // Extract array dimensions and flattened elements
        let (rows, cols, values) = match &args[0] {
            FunctionArg::Array {
                rows, cols, values, ..
            } => (*rows, *cols, values.as_slice()),
            FunctionArg::Scalar(val) => {
                if let CellValue::Error(err) = val {
                    return Ok(CellValue::Error(*err));
                }
                (1, 1, std::slice::from_ref(val))
            }
            _ => return Ok(CellValue::Error(ExcelError::Value)),
        };

        if rows == 0 || cols == 0 || values.is_empty() {
            return Ok(CellValue::Error(ExcelError::Ref));
        }

        // Evaluate row_num
        let row_arg = args[1].eval_scalar(self.name(), ctx)?;
        let row_num = match row_arg.coerce_to_number() {
            Ok(n) => {
                if !n.is_finite() {
                    return Ok(CellValue::Error(ExcelError::Value));
                }
                n.trunc() as i64
            }
            Err(err) => return Ok(CellValue::Error(err)),
        };

        // Resolve 1D vs 2D row and column indices
        let (target_row, target_col) = if args.len() > 2 && !matches!(args[2], FunctionArg::Omitted)
        {
            let col_arg = args[2].eval_scalar(self.name(), ctx)?;
            let col_num = match col_arg.coerce_to_number() {
                Ok(n) => {
                    if !n.is_finite() {
                        return Ok(CellValue::Error(ExcelError::Value));
                    }
                    n.trunc() as i64
                }
                Err(err) => return Ok(CellValue::Error(err)),
            };
            (row_num, col_num)
        } else if rows == 1 {
            // Horizontal 1D vector: row_num is interpreted as col_num
            (1, row_num)
        } else if cols == 1 {
            // Vertical 1D vector: row_num is row_num
            (row_num, 1)
        } else {
            // 2D matrix without col_num is invalid in scalar context
            return Ok(CellValue::Error(ExcelError::Value));
        };

        // Validate 1-based bounds
        if target_row < 1 || target_row > rows as i64 || target_col < 1 || target_col > cols as i64
        {
            return Ok(CellValue::Error(ExcelError::Ref));
        }

        let idx = (target_row - 1) as usize * cols + (target_col - 1) as usize;
        Ok(values[idx].clone())
    }
}
