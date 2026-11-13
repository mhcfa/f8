use crate::functions::common::{
    BranchTypeUnifier, eval_branch_arg, eval_taken_branch_arm, extract_scalar_bool,
    extract_scalar_integer, validate_dormant_arm,
};
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    BranchPolicy, EvalContext, ExcelFunction, ExpectationOrigin, FunctionArg, FunctionCategory,
    ParamKind, ParamType, ParameterDef, TypeExpectation, ValidationPolicy, VariadicRule,
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

        let mut unifier = BranchTypeUnifier::new(self.name());

        match extract_scalar_bool(cond_arg, self.name(), "logical_test", 1, ctx)? {
            Ok(true) => {
                let (val, vt1) = eval_taken_branch_arm(true_arg, self.name(), ctx)?;
                let vt2 = validate_dormant_arm(false_arg, self.name(), None, ctx)?;
                unifier.record(2, vt1).map_err(FunctionError::Type)?;
                unifier.record(3, vt2).map_err(FunctionError::Type)?;
                Ok(val)
            }
            Ok(false) => {
                let vt1 = validate_dormant_arm(true_arg, self.name(), None, ctx)?;
                let (val, vt2) = eval_taken_branch_arm(false_arg, self.name(), ctx)?;
                unifier.record(2, vt1).map_err(FunctionError::Type)?;
                unifier.record(3, vt2).map_err(FunctionError::Type)?;
                Ok(val)
            }
            Err(err) => {
                let vt1 = validate_dormant_arm(true_arg, self.name(), None, ctx)?;
                let vt2 = validate_dormant_arm(false_arg, self.name(), None, ctx)?;
                unifier.record(2, vt1).map_err(FunctionError::Type)?;
                unifier.record(3, vt2).map_err(FunctionError::Type)?;
                Ok(CellValue::Error(err))
            }
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

        let mut unifier = BranchTypeUnifier::new(self.name());
        let mut selected: Option<CellValue> = None;

        for (pair_idx, [test_arg, val_arg]) in args.as_chunks::<2>().0.iter().enumerate() {
            let cond_pos = 2 * pair_idx + 1;
            let val_pos = 2 * pair_idx + 2;
            let cond_exp = TypeExpectation {
                expected: ExpectedType::Boolean,
                origin: ExpectationOrigin::FunctionParam {
                    function: self.name(),
                    param: "logical_test1",
                    position: cond_pos,
                },
            };

            if selected.is_some() {
                validate_dormant_arm(test_arg, self.name(), Some(cond_exp), ctx)?;
                let val_vt = validate_dormant_arm(val_arg, self.name(), None, ctx)?;
                unifier
                    .record(val_pos, val_vt)
                    .map_err(FunctionError::Type)?;
                continue;
            }

            let cond_val = eval_branch_arg(test_arg, self.name(), ctx)?;
            match cond_val.strict_bool() {
                Ok(Ok(true)) => {
                    let (val, val_vt) = eval_taken_branch_arm(val_arg, self.name(), ctx)?;
                    unifier
                        .record(val_pos, val_vt)
                        .map_err(FunctionError::Type)?;
                    selected = Some(val);
                }
                Ok(Ok(false)) => {
                    let val_vt = validate_dormant_arm(val_arg, self.name(), None, ctx)?;
                    unifier
                        .record(val_pos, val_vt)
                        .map_err(FunctionError::Type)?;
                }
                Ok(Err(err)) => {
                    validate_dormant_arm(test_arg, self.name(), Some(cond_exp), ctx)?;
                    let val_vt = validate_dormant_arm(val_arg, self.name(), None, ctx)?;
                    unifier
                        .record(val_pos, val_vt)
                        .map_err(FunctionError::Type)?;
                    selected = Some(CellValue::Error(err));
                }
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
        Ok(selected.unwrap_or(CellValue::Error(ExcelError::Na)))
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

        let idx_res = extract_scalar_integer(idx_arg, self.name(), "index_num", 1, ctx)?;
        let selected_idx: Result<usize, ExcelError> = match idx_res {
            Err(err) => Err(err),
            Ok(idx) => {
                if idx < 1 {
                    Err(ExcelError::Value)
                } else if let Ok(zero_based) = usize::try_from(idx - 1)
                    && zero_based < choices.len()
                {
                    Ok(zero_based)
                } else {
                    Err(ExcelError::Value)
                }
            }
        };

        let mut unifier = BranchTypeUnifier::new(self.name());
        let mut selected_val: Option<CellValue> = None;

        for (k, choice) in choices.iter().enumerate() {
            let pos = k + 2;
            if selected_idx == Ok(k) {
                let (val, vt) = eval_taken_branch_arm(choice, self.name(), ctx)?;
                unifier.record(pos, vt).map_err(FunctionError::Type)?;
                selected_val = Some(val);
            } else {
                let vt = validate_dormant_arm(choice, self.name(), None, ctx)?;
                unifier.record(pos, vt).map_err(FunctionError::Type)?;
            }
        }

        match (selected_idx, selected_val) {
            (Ok(_), Some(val)) => Ok(val),
            (Err(err), _) => Ok(CellValue::Error(err)),
            (Ok(_), None) => Ok(CellValue::Error(ExcelError::Value)),
        }
    }
}
