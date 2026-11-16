use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ReferenceKind, ShapePolicy, ValidationPolicy, VariadicRule,
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
        let [first_arg, ..] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        };
        let FunctionArg::Range {
            rows: exp_rows,
            cols: exp_cols,
            ..
        } = first_arg
        else {
            return Err(FunctionError::InvalidArgumentKind {
                function: self.name(),
                expected: "range",
                found: first_arg.kind_name(),
            });
        };
        let (exp_rows, exp_cols) = (*exp_rows, *exp_cols);

        let mut arrays: Vec<(&[CellValue], ReferenceKind)> = Vec::with_capacity(args.len());
        for arg in args {
            let FunctionArg::Range {
                rows,
                cols,
                values,
                ref_kind,
            } = arg
            else {
                return Err(FunctionError::InvalidArgumentKind {
                    function: self.name(),
                    expected: "range",
                    found: arg.kind_name(),
                });
            };
            if (*rows, *cols) != (exp_rows, exp_cols) {
                return Err(FunctionError::DimensionMismatch {
                    function: self.name(),
                    expected_rows: exp_rows,
                    expected_cols: exp_cols,
                    found_rows: *rows,
                    found_cols: *cols,
                });
            }
            arrays.push((values.as_slice(), *ref_kind));
        }

        let mut numeric_arrays: Vec<Vec<f64>> = Vec::with_capacity(arrays.len());
        for (slice, ref_kind) in &arrays {
            let mut row_nums = Vec::with_capacity(slice.len());
            for v in *slice {
                match v {
                    CellValue::Number(n) => {
                        if !n.is_finite() {
                            return Ok(CellValue::Error(ExcelError::Num));
                        }
                        row_nums.push(*n);
                    }
                    CellValue::Error(err) => return Ok(CellValue::Error(*err)),
                    CellValue::Empty => match ref_kind {
                        ReferenceKind::Slice => row_nums.push(0.0),
                        ReferenceKind::SingleCell => {
                            return Ok(CellValue::Error(ExcelError::Value));
                        }
                    },
                    CellValue::Text(_) | CellValue::Boolean(_) => {
                        return Ok(CellValue::Error(ExcelError::Value));
                    }
                }
            }
            numeric_arrays.push(row_nums);
        }

        let num_elements = numeric_arrays.first().map_or(0, Vec::len);
        let mut total_sum = 0.0;

        for k in 0..num_elements {
            let mut product = 1.0;
            for arr in &numeric_arrays {
                if let Some(&val) = arr.get(k) {
                    product *= val;
                }
            }
            total_sum += product;
        }

        Ok(CellValue::number(total_sum))
    }
}
