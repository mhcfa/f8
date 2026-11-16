use crate::functions::common::{require_args, scalar_bool, scalar_integer};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    BranchPolicy, EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Positional parameter definitions for `IF`.
static IF_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "logical_test",
        "A boolean value or expression that evaluates to TRUE or FALSE.",
        ParamKind::Scalar,
        ParamType::Boolean,
    ),
    ParameterDef::required(
        "value_if_true",
        "The value to return if logical_test evaluates to TRUE.",
        ParamKind::Lazy,
        ParamType::Any,
    ),
    ParameterDef::required(
        "value_if_false",
        "The value to return if logical_test evaluates to FALSE.",
        ParamKind::Lazy,
        ParamType::Any,
    ),
];

/// `IF(logical_test, value_if_true, value_if_false)`: Specifies a logical test to perform.
#[derive(Debug, Default, Clone, Copy)]
pub struct IfFunction;

impl ExcelFunction for IfFunction {
    fn name(&self) -> &'static str {
        "IF"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        IF_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Branch(BranchPolicy::If)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let [cond_arg, true_arg, false_arg] = require_args(self, args)?;

        let (selected_arg, param, position) = match scalar_bool(cond_arg) {
            Ok(true) => (true_arg, "value_if_true", 2),
            Ok(false) => (false_arg, "value_if_false", 3),
            Err(err) => return Ok(CellValue::Error(err)),
        };

        selected_arg.eval_scalar(self.name(), param, position, ctx)
    }
}

/// Positional parameter definitions for `CHOOSE`.
static CHOOSE_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "index_num",
        "Specifies which value argument is selected (1 to 254).",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "value1",
        "Value or expression from which CHOOSE selects.",
        ParamKind::Lazy,
        ParamType::Any,
    ),
];

/// `CHOOSE(index_num, value1, [value2], ...)`: Selects a value from a list of values based on an index.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChooseFunction;

impl ExcelFunction for ChooseFunction {
    fn name(&self) -> &'static str {
        "CHOOSE"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        CHOOSE_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Branch(BranchPolicy::Choose)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let [idx_arg, choices @ ..] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        };
        if choices.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }

        let idx = match scalar_integer(idx_arg) {
            Ok(i) => i,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        if idx < 1 {
            return Ok(CellValue::Error(ExcelError::Value));
        }

        let Ok(zero_based) = usize::try_from(idx - 1) else {
            return Ok(CellValue::Error(ExcelError::Value));
        };

        let Some(choice) = choices.get(zero_based) else {
            return Ok(CellValue::Error(ExcelError::Value));
        };

        choice.eval_scalar(self.name(), "value1", zero_based + 2, ctx)
    }
}
