use crate::expr::ast::Expr;
use crate::functions::error::FunctionError;
use crate::value::CellValue;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Functional taxonomy categories for Excel built-in functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FunctionCategory {
    /// Mathematical and trigonometric calculations (e.g. `ABS`, `SUM`, `ROUND`).
    Math,
    /// Logical branching and boolean operators (e.g. `IF`, `AND`, `OR`).
    Logical,
    /// Financial and investment algorithms (e.g. `XIRR`, `NPV`, `PMT`).
    Financial,
    /// Coordinate and reference lookup functions (e.g. `XLOOKUP`, `INDEX`).
    Lookup,
    /// Date and time operations (e.g. `YEARFRAC`, `DATE`).
    DateTime,
    /// Text manipulation and string formatting (e.g. `CONCAT`, `TEXT`).
    Text,
    /// Metadata, cell introspection, and error inspection (e.g. `ISERROR`, `TYPE`).
    Information,
}

impl FunctionCategory {
    /// Returns the human-readable display title for this category.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Math => "Math",
            Self::Logical => "Logical",
            Self::Financial => "Financial",
            Self::Lookup => "Lookup",
            Self::DateTime => "Date & Time",
            Self::Text => "Text",
            Self::Information => "Information",
        }
    }
}

impl fmt::Display for FunctionCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Evaluation and argument binding behavior for a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParamKind {
    /// Pre-evaluated eager scalar cell value.
    Scalar,
    /// Pre-evaluated range slice or 2D array of cell values.
    RangeOrArray,
    /// Unevaluated lazy expression evaluated conditionally on demand.
    Lazy,
}

/// Variadic tail behavior for function signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VariadicRule {
    /// Fixed arity; no variadic tail permitted.
    None,
    /// Arbitrary number of trailing arguments matching the final parameter definition (e.g. `SUM`).
    Single,
    /// Trailing arguments must occur in balanced pairs (e.g. `criteria_range1, criteria1` in `SUMIFS`).
    Pairs,
}

/// Metadata definition describing an individual function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDef {
    /// Parameter identifier name (e.g. `"logical_test"`, `"number"`).
    pub name: &'static str,
    /// Human-readable description of the parameter.
    pub description: &'static str,
    /// Argument consumption kind.
    pub kind: ParamKind,
    /// Whether this parameter can be omitted by callers.
    pub optional: bool,
    /// Default cell value assigned when omitted or unspecified.
    pub default_value: Option<CellValue>,
}

impl ParameterDef {
    /// Constructs a required parameter with no default value.
    pub const fn required(name: &'static str, description: &'static str, kind: ParamKind) -> Self {
        Self {
            name,
            description,
            kind,
            optional: false,
            default_value: None,
        }
    }

    /// Constructs an optional parameter without an explicit default value.
    pub const fn optional(name: &'static str, description: &'static str, kind: ParamKind) -> Self {
        Self {
            name,
            description,
            kind,
            optional: true,
            default_value: None,
        }
    }

    /// Constructs an optional parameter with an explicit default fallback value.
    pub fn with_default(
        name: &'static str,
        description: &'static str,
        kind: ParamKind,
        default_value: CellValue,
    ) -> Self {
        Self {
            name,
            description,
            kind,
            optional: true,
            default_value: Some(default_value),
        }
    }
}

/// Bound runtime argument supplied to an `ExcelFunction::evaluate` call.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionArg<'a> {
    /// Eagerly resolved scalar value.
    Scalar(CellValue),
    /// Multi-dimensional array or evaluated range slice.
    Array {
        /// Number of rows.
        rows: usize,
        /// Number of columns.
        cols: usize,
        /// Flattened row-major cell values.
        values: Vec<CellValue>,
        /// Whether this array originated from an explicit range reference (e.g. `Table[...]`).
        is_range_ref: bool,
    },
    /// Unevaluated AST expression for lazy conditional branches.
    Lazy(&'a Expr),
    /// Explicitly or implicitly omitted argument placeholder.
    Omitted,
}

impl<'a> FunctionArg<'a> {
    /// Resolves this argument to a single scalar `CellValue`, evaluating lazy expressions if necessary.
    pub fn eval_scalar(
        &self,
        function: &'static str,
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match self {
            Self::Scalar(val) => Ok(val.clone()),
            Self::Lazy(expr) => ctx.eval_expr(expr),
            Self::Array { .. } => Err(FunctionError::InvalidArgumentKind {
                function,
                expected: "scalar",
                found: "array",
            }),
            Self::Omitted => Err(FunctionError::InvalidArgumentKind {
                function,
                expected: "scalar",
                found: "omitted",
            }),
        }
    }

    /// Resolves this argument to a 2D array `(rows, cols, values)`, evaluating lazy expressions if necessary.
    pub fn eval_array(
        &self,
        function: &'static str,
        ctx: &dyn EvalContext,
    ) -> Result<(usize, usize, Vec<CellValue>), FunctionError> {
        match self {
            Self::Array {
                rows, cols, values, ..
            } => Ok((*rows, *cols, values.clone())),
            Self::Scalar(val) => Ok((1, 1, vec![val.clone()])),
            Self::Lazy(expr) => {
                let val = ctx.eval_expr(expr)?;
                Ok((1, 1, vec![val]))
            }
            Self::Omitted => Err(FunctionError::InvalidArgumentKind {
                function,
                expected: "array",
                found: "omitted",
            }),
        }
    }
}

/// Contextual evaluation interface passed to function evaluation routines.
pub trait EvalContext {
    /// Evaluates a single formula AST expression to a scalar `CellValue`.
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError>;

    /// Evaluates a range or multi-cell expression to a 2D grid `(rows, cols, values)`.
    ///
    /// The default implementation delegates to `eval_expr` returning a $1 \times 1$ grid.
    fn eval_range(&self, expr: &Expr) -> Result<(usize, usize, Vec<CellValue>), FunctionError> {
        let val = self.eval_expr(expr)?;
        Ok((1, 1, vec![val]))
    }
}

/// Extensible trait implemented by all built-in and user-defined Excel functions.
pub trait ExcelFunction: Send + Sync + fmt::Debug {
    /// Canonical uppercase name of the function (e.g. `"SUM"`, `"IF"`).
    fn name(&self) -> &'static str;

    /// Functional taxonomy category.
    fn category(&self) -> FunctionCategory;

    /// Static slice of positional parameter definitions.
    fn parameters(&self) -> &'static [ParameterDef];

    /// Variadic tail repetition rule.
    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    /// Minimum number of required positional arguments.
    fn min_args(&self) -> usize {
        self.parameters().iter().take_while(|p| !p.optional).count()
    }

    /// Maximum number of accepted arguments, or `None` if variadic.
    fn max_args(&self) -> Option<usize> {
        match self.variadic_rule() {
            VariadicRule::None => Some(self.parameters().len()),
            VariadicRule::Single | VariadicRule::Pairs => None,
        }
    }

    /// Executes the function logic given bound arguments and evaluation context.
    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError>;
}
