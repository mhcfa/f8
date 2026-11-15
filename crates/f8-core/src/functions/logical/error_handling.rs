use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    BranchPolicy, EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ExpectedType, TypeError, ValueType};

static IFERROR_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "value",
        "The value or expression to check for an error.",
        ParamKind::Lazy,
        ParamType::NonEmpty,
    ),
    ParameterDef::required(
        "value_if_error",
        "The value to return if the formula evaluates to an error.",
        ParamKind::Lazy,
        ParamType::NonEmpty,
    ),
];

/// `IFERROR(value, value_if_error)`: Returns value_if_error if value is an error; otherwise returns value.
#[derive(Debug, Default, Clone, Copy)]
pub struct IferrorFunction;

impl ExcelFunction for IferrorFunction {
    fn name(&self) -> &'static str {
        "IFERROR"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        IFERROR_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Branch(BranchPolicy::ErrorTrap)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let [val_arg, fallback_arg] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        };

        let evaluated_value = val_arg.eval_scalar(self.name(), ctx)?;
        if matches!(evaluated_value, CellValue::Empty) {
            return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: self.name(),
                param: "value",
                position: 1,
                expected: ExpectedType::NonEmpty,
                found: ValueType::Empty,
            }));
        }

        if matches!(evaluated_value, CellValue::Error(_)) {
            let fallback_value = fallback_arg.eval_scalar(self.name(), ctx)?;
            if matches!(fallback_value, CellValue::Empty) {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: self.name(),
                    param: "value_if_error",
                    position: 2,
                    expected: ExpectedType::NonEmpty,
                    found: ValueType::Empty,
                }));
            }
            Ok(fallback_value)
        } else {
            Ok(evaluated_value)
        }
    }
}

static IFNA_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "value",
        "The value or expression to check for an #N/A error.",
        ParamKind::Lazy,
        ParamType::NonEmpty,
    ),
    ParameterDef::required(
        "value_if_na",
        "The value to return if the formula evaluates to #N/A.",
        ParamKind::Lazy,
        ParamType::NonEmpty,
    ),
];

/// `IFNA(value, value_if_na)`: Returns value_if_na if value evaluates to #N/A; otherwise returns value.
#[derive(Debug, Default, Clone, Copy)]
pub struct IfnaFunction;

impl ExcelFunction for IfnaFunction {
    fn name(&self) -> &'static str {
        "IFNA"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        IFNA_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Branch(BranchPolicy::ErrorTrap)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let [val_arg, fallback_arg] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        };

        let evaluated_value = val_arg.eval_scalar(self.name(), ctx)?;
        if matches!(evaluated_value, CellValue::Empty) {
            return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: self.name(),
                param: "value",
                position: 1,
                expected: ExpectedType::NonEmpty,
                found: ValueType::Empty,
            }));
        }

        if matches!(evaluated_value, CellValue::Error(ExcelError::Na)) {
            let fallback_value = fallback_arg.eval_scalar(self.name(), ctx)?;
            if matches!(fallback_value, CellValue::Empty) {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: self.name(),
                    param: "value_if_na",
                    position: 2,
                    expected: ExpectedType::NonEmpty,
                    found: ValueType::Empty,
                }));
            }
            Ok(fallback_value)
        } else {
            Ok(evaluated_value)
        }
    }
}
