use crate::expr::ast::Expr;
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, ParamKind, ParameterDef, VariadicRule,
};
use indexmap::IndexMap;
use std::sync::Arc;

/// Thread-safe registry and dispatcher of built-in and user-defined Excel functions.
#[derive(Clone, Default)]
pub struct FunctionRegistry {
    functions: IndexMap<String, Arc<dyn ExcelFunction>>,
}

impl FunctionRegistry {
    /// Constructs an empty function registry with no functions registered.
    #[must_use]
    pub fn new() -> Self {
        Self {
            functions: IndexMap::new(),
        }
    }

    /// Constructs a standard function registry populated with core canonical archetypes (`ABS`, `SUM`, `IF`).
    #[must_use]
    pub fn standard() -> Self {
        let mut registry = Self::new();
        crate::functions::math::register(&mut registry);
        crate::functions::logical::register(&mut registry);
        crate::functions::financial::register(&mut registry);
        registry
    }

    /// Registers a function implementation, storing it by canonical uppercase name.
    pub fn register(&mut self, func: Arc<dyn ExcelFunction>) {
        self.functions
            .insert(func.name().to_ascii_uppercase(), func);
    }

    /// Looks up a function by name (case-insensitive).
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&dyn ExcelFunction> {
        self.functions
            .get(&name.to_ascii_uppercase())
            .map(std::convert::AsRef::as_ref)
    }

    /// Looks up a shared reference to a function by name (case-insensitive).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn ExcelFunction>> {
        self.functions.get(&name.to_ascii_uppercase()).cloned()
    }

    /// Returns an iterator over all registered functions in deterministic insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<dyn ExcelFunction>)> {
        self.functions.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Returns the number of registered functions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Returns `true` if no functions are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Validates an invocation signature against registered arity bounds, variadic rules, and required parameters.
    pub fn validate_call(
        &self,
        name: &str,
        args: &[Expr],
    ) -> Result<&dyn ExcelFunction, FunctionError> {
        let func = self
            .lookup(name)
            .ok_or_else(|| FunctionError::UnknownFunction(name.to_string()))?;

        if args.len() < func.min_args() {
            return Err(FunctionError::TooFewArguments {
                function: func.name(),
                expected: func.min_args(),
                found: args.len(),
            });
        }

        if let Some(max) = func.max_args()
            && args.len() > max
        {
            return Err(FunctionError::TooManyArguments {
                function: func.name(),
                expected: max,
                found: args.len(),
            });
        }

        if func.variadic_rule() == VariadicRule::Pairs {
            let fixed_count = func.parameters().len();
            if args.len() > fixed_count && !(args.len() - fixed_count).is_multiple_of(2) {
                return Err(FunctionError::InvalidVariadicPairs {
                    function: func.name(),
                    count: args.len(),
                });
            }
        }

        for (i, arg) in args.iter().enumerate() {
            if matches!(arg, Expr::Omitted) {
                let param = resolve_parameter(func, i)?;
                if !param.optional {
                    return Err(FunctionError::CannotOmitRequiredArgument {
                        function: func.name(),
                        param: param.name,
                        position: i + 1,
                    });
                }
            }
        }

        Ok(func)
    }

    /// Binds parsed formula arguments into runtime `FunctionArg` values, evaluating scalars or delegating lazy branches.
    pub fn bind_arguments<'a>(
        &self,
        func: &dyn ExcelFunction,
        expr_args: &'a [Expr],
        ctx: &dyn EvalContext,
    ) -> Result<Vec<FunctionArg<'a>>, FunctionError> {
        self.validate_call(func.name(), expr_args)?;

        let target_len = std::cmp::max(expr_args.len(), func.parameters().len());
        let mut bound = Vec::with_capacity(target_len);

        for (i, arg_expr) in expr_args.iter().enumerate() {
            let param = resolve_parameter(func, i)?;

            if matches!(arg_expr, Expr::Omitted) {
                if let Some(default_val) = &param.default_value {
                    bound.push(FunctionArg::Scalar(default_val.clone()));
                } else {
                    bound.push(FunctionArg::Omitted);
                }
                continue;
            }

            match param.kind {
                ParamKind::Lazy => {
                    bound.push(FunctionArg::Lazy(arg_expr));
                }
                ParamKind::Scalar => {
                    let val = ctx.eval_expr(arg_expr)?;
                    bound.push(FunctionArg::Scalar(val));
                }
                ParamKind::RangeOrArray => {
                    if matches!(arg_expr, Expr::Reference(_)) {
                        let (rows, cols, values) = ctx.eval_range(arg_expr)?;
                        bound.push(FunctionArg::Array {
                            rows,
                            cols,
                            values,
                            is_range_ref: true,
                        });
                    } else {
                        let val = ctx.eval_expr(arg_expr)?;
                        bound.push(FunctionArg::Scalar(val));
                    }
                }
            }
        }

        // Fill in missing trailing optional parameters with defaults or Omitted
        if expr_args.len() < func.parameters().len() {
            for param in &func.parameters()[expr_args.len()..] {
                if let Some(default_val) = &param.default_value {
                    bound.push(FunctionArg::Scalar(default_val.clone()));
                } else {
                    bound.push(FunctionArg::Omitted);
                }
            }
        }

        Ok(bound)
    }
}

/// Resolves the expected parameter definition for a given 0-indexed argument position,
/// applying cyclical pairs alternation when `VariadicRule::Pairs` is enabled.
fn resolve_parameter(
    func: &dyn ExcelFunction,
    index: usize,
) -> Result<&ParameterDef, FunctionError> {
    if index < func.parameters().len() {
        Ok(&func.parameters()[index])
    } else if func.variadic_rule() == VariadicRule::Pairs {
        let fixed_count = func.parameters().len();
        if fixed_count < 2 {
            return Err(FunctionError::EvaluationFailure {
                function: func.name(),
                message: "variadic pairs rule requires at least two parameter definitions".into(),
            });
        }
        let offset = (index - fixed_count) % 2;
        Ok(&func.parameters()[fixed_count - 2 + offset])
    } else {
        func.parameters()
            .last()
            .ok_or_else(|| FunctionError::EvaluationFailure {
                function: func.name(),
                message: "variadic argument without parameter definition".into(),
            })
    }
}
