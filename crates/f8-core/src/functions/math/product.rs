use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ShapePolicy, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Positional parameter definitions for `SUMPRODUCT`.
static SUMPRODUCT_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "array1",
    "The first range argument whose components you want to multiply and then add.",
    ParamKind::Range,
    ParamType::Number,
)
.measure_only()
.allow_empty_in_slice()];

/// `SUMPRODUCT(array1, [array2], ...)`: Multiplies corresponding components in the given ranges, and returns the sum of those products.
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

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::ShapeCoupled(ShapePolicy::EqualAllRanges)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let mut arrays = Vec::with_capacity(args.len());
        let mut target_dims: Option<(usize, usize)> = None;

        for arg in args {
            let (rows, cols, slice) = match arg {
                FunctionArg::Array {
                    rows, cols, values, ..
                } => (*rows, *cols, values.as_slice()),
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
                    return Err(FunctionError::DimensionMismatch {
                        function: self.name(),
                        expected_rows: target_rows,
                        expected_cols: target_cols,
                        found_rows: rows,
                        found_cols: cols,
                    });
                }
            } else {
                target_dims = Some((rows, cols));
            }

            arrays.push(slice);
        }

        let (target_rows, target_cols) = target_dims.unwrap_or((0, 0));
        let num_elements = target_rows * target_cols;

        // Cell errors bubble after all range shapes have been verified
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
                    Some(CellValue::Number(n)) => {
                        if !n.is_finite() {
                            return Ok(CellValue::Error(ExcelError::Num));
                        }
                        *n
                    }
                    Some(
                        CellValue::Text(_)
                        | CellValue::Boolean(_)
                        | CellValue::Empty
                        | CellValue::Error(_),
                    )
                    | None => 0.0,
                };
                product *= val;
            }
            total_sum += product;
        }

        Ok(CellValue::number(total_sum))
    }
}
