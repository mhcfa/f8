use crate::functions::common::eval_branch_arg;
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

static IFERROR_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "value",
        "The value or expression to check for an error.",
        ParamKind::Lazy,
    ),
    ParameterDef::required(
        "value_if_error",
        "The value to return if the formula evaluates to an error.",
        ParamKind::Lazy,
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
        if args.len() > 2 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }

        let evaluated_value = eval_branch_arg(&args[0], self.name(), ctx);

        let is_err = match &evaluated_value {
            Ok(CellValue::Error(_)) | Err(_) => true,
            Ok(_) => false,
        };

        if is_err {
            eval_branch_arg(&args[1], self.name(), ctx)
        } else {
            evaluated_value
        }
    }
}

static IFNA_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "value",
        "The value or expression to check for an #N/A error.",
        ParamKind::Lazy,
    ),
    ParameterDef::required(
        "value_if_na",
        "The value to return if the formula evaluates to #N/A.",
        ParamKind::Lazy,
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
        if args.len() > 2 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }

        let evaluated_value = eval_branch_arg(&args[0], self.name(), ctx);

        if let Ok(CellValue::Error(ExcelError::Na)) = &evaluated_value {
            eval_branch_arg(&args[1], self.name(), ctx)
        } else {
            evaluated_value
        }
    }
}
