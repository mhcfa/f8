use crate::functions::common::{scalar_bool, scalar_integer};
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    BranchPolicy, EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ExpectedType, TypeError, ValueType};

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
        validate_bound_args(self, args)?;

        let [cond_arg, true_arg, false_arg] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        };

        match scalar_bool(cond_arg) {
            Ok(true) => true_arg.eval_scalar(self.name(), ctx),
            Ok(false) => false_arg.eval_scalar(self.name(), ctx),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `IFS`.
static IFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "logical_test1",
        "A boolean value or expression that evaluates to TRUE or FALSE.",
        ParamKind::Lazy,
        ParamType::Boolean,
    ),
    ParameterDef::required(
        "value_if_true1",
        "The value to return if logical_test1 evaluates to TRUE.",
        ParamKind::Lazy,
        ParamType::Any,
    ),
];

/// `IFS(logical_test1, value_if_true1, [logical_test2, value_if_true2], ...)`: Evaluates multiple conditions.
#[derive(Debug, Default, Clone, Copy)]
pub struct IfsFunction;

impl ExcelFunction for IfsFunction {
    fn name(&self) -> &'static str {
        "IFS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        IFS_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Pairs
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Branch(BranchPolicy::Ifs)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        for (pair_idx, [test_arg, val_arg]) in args.as_chunks::<2>().0.iter().enumerate() {
            let cond_pos = 2 * pair_idx + 1;
            let cond_val = test_arg.eval_scalar(self.name(), ctx)?;
            match cond_val.strict_bool() {
                Ok(Ok(true)) => return val_arg.eval_scalar(self.name(), ctx),
                Ok(Ok(false)) => {}
                Ok(Err(err)) => return Ok(CellValue::Error(err)),
                Err(found) => {
                    return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                        function: self.name(),
                        param: "logical_test1",
                        position: cond_pos,
                        expected: ExpectedType::Boolean,
                        found,
                    }));
                }
            }
        }

        // If no conditions evaluate to TRUE, Excel returns #N/A
        Ok(CellValue::Error(ExcelError::Na))
    }
}

/// Positional parameter definitions for `CHOOSE`.
static CHOOSE_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "index_num",
        "Specifies which value argument is selected (1 to 254).",
        ParamKind::Scalar,
        ParamType::Integer,
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
        validate_bound_args(self, args)?;

        let Some((idx_arg, choices)) = args.split_first() else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        };

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

        choice.eval_scalar(self.name(), ctx)
    }
}
