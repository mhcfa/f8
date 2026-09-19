use crate::functions::common::normalize_zero;
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Positional parameter definitions for `SUMPRODUCT`.
static SUMPRODUCT_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "array1",
    "The first array argument whose components you want to multiply and then add.",
    ParamKind::RangeOrArray,
)];

/// `SUMPRODUCT(array1, [array2], ...)`: Multiplies corresponding components in the given arrays, and returns the sum of those products.
#[derive(Debug, Default, Clone, Copy)]
pub struct SumproductFunction;

impl ExcelFunction for SumproductFunction {
    fn name(&self) -> &'static str {
        "SUMPRODUCT"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        SUMPRODUCT_PARAMS
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }

        // Collect (rows, cols, &[CellValue]) for each argument
        let mut arrays = Vec::with_capacity(args.len());
        let mut target_dims: Option<(usize, usize)> = None;

        for arg in args {
            let (rows, cols, slice) = match arg {
                FunctionArg::Array {
                    rows, cols, values, ..
                } => (*rows, *cols, values.as_slice()),
                FunctionArg::Scalar(val) => (1, 1, std::slice::from_ref(val)),
                FunctionArg::Lazy(_) => {
                    return Err(FunctionError::EvaluationFailure {
                        function: self.name(),
                        message: "unexpected lazy argument in SUMPRODUCT".into(),
                    });
                }
                FunctionArg::Omitted => {
                    return Ok(CellValue::Error(ExcelError::Value));
                }
            };

            if slice.len() != rows * cols {
                return Err(FunctionError::EvaluationFailure {
                    function: self.name(),
                    message: format!(
                        "array element count ({}) does not match dimensions ({rows}x{cols})",
                        slice.len()
                    ),
                });
            }

            if let Some((target_rows, target_cols)) = target_dims {
                if rows != target_rows || cols != target_cols {
                    return Ok(CellValue::Error(ExcelError::Value));
                }
            } else {
                target_dims = Some((rows, cols));
            }

            arrays.push(slice);
        }

        let (target_rows, target_cols) = target_dims.unwrap_or((0, 0));
        let num_elements = target_rows * target_cols;

        // Check for error values in any array - cell errors bubble up immediately
        for slice in &arrays {
            for v in *slice {
                if let CellValue::Error(err) = v {
                    return Ok(CellValue::Error(*err));
                }
            }
        }

        let mut total_sum = 0.0;

        for k in 0..num_elements {
            let mut product = 1.0;
            for slice in &arrays {
                let val = match slice.get(k) {
                    Some(CellValue::Number(n)) => *n,
                    _ => 0.0, // Non-numeric entries (text, booleans, empty) are treated as 0.0
                };
                product *= val;
            }
            total_sum += product;
        }

        Ok(CellValue::Number(normalize_zero(total_sum)))
    }
}
