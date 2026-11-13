//! `INDEX` function implementation.

use crate::functions::common::extract_scalar_integer;
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::validate_homogeneous_slice;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, LookupPolicy, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Positional parameter definitions for `INDEX`.
static INDEX_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "array",
        "The range of cells to index into.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "row_num",
        "The 1-based row (or 1D element) position in the range.",
        ParamKind::Scalar,
        ParamType::Integer,
    ),
    ParameterDef::optional(
        "col_num",
        "The 1-based column position in the range.",
        ParamKind::Scalar,
        ParamType::Integer,
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let [arr_arg, row_arg, ..] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        };

        let (rows, cols, values, error_types) = match arr_arg {
            FunctionArg::Array {
                rows,
                cols,
                values,
                error_types,
                ..
            } => (*rows, *cols, values.as_slice(), error_types.as_slice()),
            FunctionArg::Scalar(_) | FunctionArg::TypedErrorScalar { .. } => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: self.name(),
                    expected: "range",
                    found: "scalar",
                });
            }
            FunctionArg::Lazy(_) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: self.name(),
                    expected: "range",
                    found: "lazy",
                });
            }
            FunctionArg::Omitted => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: self.name(),
                    expected: "range",
                    found: "omitted",
                });
            }
        };

        if args.len() == 2 && rows > 1 && cols > 1 {
            return Err(FunctionError::MissingColumnIndexFor2DSlice {
                function: self.name(),
                rows,
                cols,
            });
        }

        validate_homogeneous_slice(values, error_types, self.name(), "array", 1)?;

        let row_res = extract_scalar_integer(row_arg, self.name(), "row_num", 2, ctx)?;
        let col_res = match args.get(2) {
            Some(col_slot) => Some(extract_scalar_integer(
                col_slot,
                self.name(),
                "col_num",
                3,
                ctx,
            )?),
            None => None,
        };

        if rows == 0 || cols == 0 || values.is_empty() {
            return Ok(CellValue::Error(ExcelError::Ref));
        }

        let row_num = match row_res {
            Ok(r) => r,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        let (target_row, target_col) = if let Some(col_res) = col_res {
            let col_num = match col_res {
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
        Ok(values
            .get(idx)
            .cloned()
            .unwrap_or(CellValue::Error(ExcelError::Ref)))
    }
}
