use crate::functions::common::eval_branch_arg;
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Positional parameter definitions for `IF`.
static IF_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "logical_test",
        "Any value or expression that can be evaluated to TRUE or FALSE.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "value_if_true",
        "The value to return if logical_test evaluates to TRUE.",
        ParamKind::Lazy,
    ),
    ParameterDef {
        name: "value_if_false",
        description: "The value to return if logical_test evaluates to FALSE.",
        kind: ParamKind::Lazy,
        optional: true,
        default_value: Some(CellValue::Boolean(false)),
    },
];

/// `IF(logical_test, value_if_true, [value_if_false])`: Specifies a logical test to perform.
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

        let cond_val = args[0].eval_scalar(self.name(), ctx)?;
        if let CellValue::Error(err) = cond_val {
            return Ok(CellValue::Error(err));
        }

        let cond = match cond_val.coerce_to_bool() {
            Ok(b) => b,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        if cond {
            eval_branch_arg(&args[1], self.name(), ctx)
        } else if args.len() < 3 {
            Ok(CellValue::Boolean(false))
        } else {
            match &args[2] {
                FunctionArg::Omitted => Ok(CellValue::Boolean(false)),
                other => eval_branch_arg(other, self.name(), ctx),
            }
        }
    }
}

/// Positional parameter definitions for `IFS`.
static IFS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "logical_test1",
        "Any value or expression that can be evaluated to TRUE or FALSE.",
        ParamKind::Lazy,
    ),
    ParameterDef::required(
        "value_if_true1",
        "The value to return if logical_test1 evaluates to TRUE.",
        ParamKind::Lazy,
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Pairs
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
        if !args.len().is_multiple_of(2) {
            return Err(FunctionError::InvalidVariadicPairs {
                function: self.name(),
                count: args.len(),
            });
        }

        for [test_arg, val_arg] in args.as_chunks::<2>().0 {
            let cond_val = eval_branch_arg(test_arg, self.name(), ctx)?;
            if let CellValue::Error(err) = cond_val {
                return Ok(CellValue::Error(err));
            }

            let cond = match cond_val.coerce_to_bool() {
                Ok(b) => b,
                Err(err) => return Ok(CellValue::Error(err)),
            };

            if cond {
                return eval_branch_arg(val_arg, self.name(), ctx);
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
    ),
    ParameterDef::required(
        "value1",
        "Value or expression from which CHOOSE selects.",
        ParamKind::Lazy,
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
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

        let idx_val = args[0].eval_scalar(self.name(), ctx)?;
        if let CellValue::Error(err) = idx_val {
            return Ok(CellValue::Error(err));
        }

        let num = match idx_val.coerce_to_number() {
            Ok(n) => n,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        if !num.is_finite() {
            return Ok(CellValue::Error(ExcelError::Value));
        }

        let idx = num.trunc() as i64;
        let choices = &args[1..];

        if idx < 1 || (idx as usize) > choices.len() {
            return Ok(CellValue::Error(ExcelError::Value));
        }

        eval_branch_arg(&choices[(idx - 1) as usize], self.name(), ctx)
    }
}

/// Positional parameter definitions for `SWITCH`.
static SWITCH_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "expression",
        "The value that will be compared against value1...valueN.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "value1",
        "The value that will be compared against expression.",
        ParamKind::Lazy,
    ),
    ParameterDef::required(
        "result1",
        "The value to be returned if value1 matches expression.",
        ParamKind::Lazy,
    ),
];

/// `SWITCH(expression, val1, result1, [default_or_val2, result2], ...)`: Evaluates an expression against a list of values.
#[derive(Debug, Default, Clone, Copy)]
pub struct SwitchFunction;

impl ExcelFunction for SwitchFunction {
    fn name(&self) -> &'static str {
        "SWITCH"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        SWITCH_PARAMS
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 3 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        }

        let target = args[0].eval_scalar(self.name(), ctx)?;
        if let CellValue::Error(err) = target {
            return Ok(CellValue::Error(err));
        }

        let rem = &args[1..];
        let pair_count = rem.len() / 2;
        let has_default = rem.len() % 2 == 1;

        for i in 0..pair_count {
            let val_arg = &rem[2 * i];
            let res_arg = &rem[2 * i + 1];

            let case_val = eval_branch_arg(val_arg, self.name(), ctx)?;
            if let CellValue::Error(err) = case_val {
                return Ok(CellValue::Error(err));
            }

            if target.excel_eq(&case_val) {
                return eval_branch_arg(res_arg, self.name(), ctx);
            }
        }

        if has_default {
            let default_arg = &rem[rem.len() - 1];
            eval_branch_arg(default_arg, self.name(), ctx)
        } else {
            Ok(CellValue::Error(ExcelError::Na))
        }
    }
}
