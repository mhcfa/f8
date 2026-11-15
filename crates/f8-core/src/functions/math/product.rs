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
        let mut num_elements = 0;

        for arg in args {
            let values = arg.as_range_slice(self.name())?;
            num_elements = values.len();
            arrays.push(values);
        }

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
