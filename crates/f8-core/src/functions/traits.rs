use std::fmt;

use serde::{Deserialize, Serialize};

use crate::expr::ast::{Expr, TableRef};
use crate::functions::error::FunctionError;
use crate::value::{CellValue, TypeError, ValueType};

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
    /// Metadata, cell introspection, and error inspection (e.g. `ISERROR`, `NA`).
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
    /// Pre-evaluated range reference (`Expr::Reference` required).
    Range,
    /// Pre-evaluated scalar expression or range reference.
    ScalarOrRange,
    /// Unevaluated lazy expression evaluated conditionally on demand.
    Lazy,
}

/// Syntactic reference classification for `FunctionArg::Range` bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReferenceKind {
    /// Single-cell coordinate reference (no `start..end` slice on either axis).
    SingleCell,
    /// Explicit range slice (`start..end` on row and/or column axis, including `1x1` slices).
    Slice,
}

/// Declarative value-type contract for a function parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParamType {
    /// Accepts any value type (`Number`, `Text`, `Boolean`, `Empty`, `Error`).
    Any,
    /// Requires a numeric scalar (`ValueType::Number`).
    Number,
    /// Requires a boolean value (`ValueType::Boolean`).
    Boolean,
}

impl ParamType {
    /// Returns the required [`ValueType`] for this parameter, or `None` for [`Self::Any`].
    #[must_use]
    pub const fn expected_value_type(self) -> Option<ValueType> {
        match self {
            Self::Any => None,
            Self::Number => Some(ValueType::Number),
            Self::Boolean => Some(ValueType::Boolean),
        }
    }
}

/// Declarative column-role constraint for reference arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum RolePolicy {
    /// Accepts both `AxisRole::Measure` and `AxisRole::Descriptor` columns.
    #[default]
    AnyRole,
    /// Rejects references whose resolved column has `AxisRole::Descriptor`.
    MeasureOnly,
}

/// Declarative empty-cell handling policy for function parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EmptyPolicy {
    /// Rejects `CellValue::Empty` across `Scalar`, `ReferenceKind::SingleCell`, and `ReferenceKind::Slice`.
    #[default]
    Forbid,
    /// Rejects `CellValue::Empty` on `Scalar` and `ReferenceKind::SingleCell`, while permitting sparse `Empty` inside `ReferenceKind::Slice`.
    AllowInSlice,
}

/// Variadic tail behavior for function signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VariadicRule {
    /// Fixed arity; no variadic tail permitted.
    None,
    /// Arbitrary number of trailing arguments matching the final parameter definition (e.g. `SUM`).
    Single,
}

/// Requirement and default policy for a positional function parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamRequirement {
    /// Parameter must be supplied explicitly and cannot use `_`.
    Required,
    /// Optional parameter without a static scalar default value.
    Optional,
    /// Optional parameter with a static fallback value substituted when skipped via intermediate `_`.
    OptionalWithDefault(CellValue),
}

impl ParamRequirement {
    /// Returns `true` if this parameter is optional.
    #[must_use]
    pub const fn is_optional(&self) -> bool {
        matches!(self, Self::Optional | Self::OptionalWithDefault(_))
    }

    /// Returns the static default value if configured via [`Self::OptionalWithDefault`].
    #[must_use]
    pub const fn default_value(&self) -> Option<&CellValue> {
        match self {
            Self::OptionalWithDefault(val) => Some(val),
            Self::Required | Self::Optional => None,
        }
    }
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
    /// Declarative value-type constraint.
    pub param_type: ParamType,
    /// Column axis-role policy for reference arguments.
    pub role_policy: RolePolicy,
    /// Empty-cell policy for this parameter.
    pub empty_policy: EmptyPolicy,
    /// Requirement and default policy for this parameter.
    pub requirement: ParamRequirement,
}

impl ParameterDef {
    /// Constructs a required parameter with no default value.
    #[must_use]
    pub const fn required(
        name: &'static str,
        description: &'static str,
        kind: ParamKind,
        param_type: ParamType,
    ) -> Self {
        Self {
            name,
            description,
            kind,
            param_type,
            role_policy: RolePolicy::AnyRole,
            empty_policy: EmptyPolicy::Forbid,
            requirement: ParamRequirement::Required,
        }
    }

    /// Constructs an optional parameter without an explicit default value.
    #[must_use]
    pub const fn optional(
        name: &'static str,
        description: &'static str,
        kind: ParamKind,
        param_type: ParamType,
    ) -> Self {
        Self {
            name,
            description,
            kind,
            param_type,
            role_policy: RolePolicy::AnyRole,
            empty_policy: EmptyPolicy::Forbid,
            requirement: ParamRequirement::Optional,
        }
    }

    /// Constructs an optional parameter with an explicit default fallback value.
    #[must_use]
    pub const fn with_default(
        name: &'static str,
        description: &'static str,
        kind: ParamKind,
        param_type: ParamType,
        default_value: CellValue,
    ) -> Self {
        Self {
            name,
            description,
            kind,
            param_type,
            role_policy: RolePolicy::AnyRole,
            empty_policy: EmptyPolicy::Forbid,
            requirement: ParamRequirement::OptionalWithDefault(default_value),
        }
    }

    /// Restricts reference arguments on this parameter to `AxisRole::Measure` columns.
    #[must_use]
    pub const fn measure_only(mut self) -> Self {
        self.role_policy = RolePolicy::MeasureOnly;
        self
    }

    /// Permits sparse `CellValue::Empty` elements inside syntactic `ReferenceKind::Slice` ranges.
    #[must_use]
    pub const fn allow_empty_in_slice(mut self) -> Self {
        self.empty_policy = EmptyPolicy::AllowInSlice;
        self
    }
}

/// Bound runtime argument supplied to an `ExcelFunction::evaluate` call.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionArg<'a> {
    /// Eagerly resolved scalar value.
    Scalar(CellValue),
    /// Evaluated range reference (`SingleCell` or `Slice`).
    Range {
        /// Number of rows.
        rows: usize,
        /// Number of columns.
        cols: usize,
        /// Flattened row-major cell values.
        values: Vec<CellValue>,
        /// Syntactic reference classification (`SingleCell` vs `Slice`).
        ref_kind: ReferenceKind,
    },
    /// Unevaluated AST expression for lazy conditional branches.
    Lazy(&'a Expr),
    /// Intermediate omitted optional-parameter placeholder (`_`) without a static default.
    Omitted,
}

impl<'a> FunctionArg<'a> {
    /// Returns the human-readable kind name of this argument for diagnostic messages.
    #[must_use]
    pub(crate) const fn kind_name(&self) -> &'static str {
        match self {
            Self::Scalar(_) => "scalar",
            Self::Range { .. } => "range",
            Self::Lazy(_) => "lazy",
            Self::Omitted => "omitted",
        }
    }

    /// Constructs a [`Self::Range`].
    #[must_use]
    pub fn range(
        rows: usize,
        cols: usize,
        values: Vec<CellValue>,
        ref_kind: ReferenceKind,
    ) -> Self {
        Self::Range {
            rows,
            cols,
            values,
            ref_kind,
        }
    }

    /// Extracts the borrowed cell-value slice from a bound range (`Self::Range`).
    pub fn as_range_slice(&self, function: &'static str) -> Result<&[CellValue], FunctionError> {
        match self {
            Self::Range { values, .. } => Ok(values.as_slice()),
            Self::Scalar(_) | Self::Lazy(_) | Self::Omitted => {
                Err(FunctionError::InvalidArgumentKind {
                    function,
                    expected: "range",
                    found: self.kind_name(),
                })
            }
        }
    }

    /// Extracts `(rows, cols, values)` from a 1D bound range, rejecting 2D ranges (`rows > 1 && cols > 1`).
    pub fn as_1d_range(
        &self,
        function: &'static str,
        param: &'static str,
    ) -> Result<(usize, usize, &[CellValue]), FunctionError> {
        match self {
            Self::Range {
                rows, cols, values, ..
            } => {
                if *rows > 1 && *cols > 1 {
                    return Err(FunctionError::ExpectedOneDimensionalSlice {
                        function,
                        param,
                        rows: *rows,
                        cols: *cols,
                    });
                }
                Ok((*rows, *cols, values.as_slice()))
            }
            Self::Scalar(_) | Self::Lazy(_) | Self::Omitted => {
                Err(FunctionError::InvalidArgumentKind {
                    function,
                    expected: "range",
                    found: self.kind_name(),
                })
            }
        }
    }

    /// Resolves this argument to a single non-empty scalar `CellValue`, evaluating lazy expressions if necessary
    /// and returning `TypeError::EmptyArgument` when the resolved value is `CellValue::Empty`.
    pub fn eval_scalar(
        &self,
        function: &'static str,
        param: &'static str,
        position: usize,
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let val = match self {
            Self::Scalar(val) => val.clone(),
            Self::Lazy(expr) => ctx.eval_expr(expr)?,
            Self::Range {
                rows,
                cols,
                ref_kind: ReferenceKind::Slice,
                ..
            } => {
                return Err(FunctionError::SliceInScalarContext {
                    rows: *rows,
                    cols: *cols,
                });
            }
            Self::Range {
                ref_kind: ReferenceKind::SingleCell,
                ..
            } => {
                return Err(FunctionError::InvalidArgumentKind {
                    function,
                    expected: "scalar",
                    found: "range",
                });
            }
            Self::Omitted => {
                return Err(FunctionError::InvalidArgumentKind {
                    function,
                    expected: "scalar",
                    found: "omitted",
                });
            }
        };
        if val.is_empty() {
            return Err(FunctionError::Type(TypeError::EmptyArgument {
                function,
                param,
                position,
            }));
        }
        Ok(val)
    }
}

/// Declarative AST validation policy for short-circuiting conditional branching functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchPolicy {
    /// `IF(logical_test, value_if_true, value_if_false)`.
    If,
    /// `CHOOSE(index_num, value1, ...)`.
    Choose,
    /// `IFERROR(value, value_if_error)`.
    ErrorTrap,
}

/// Declarative AST validation policy for coordinate and vector lookup functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupPolicy {
    /// `INDEX(array, row_num, [col_num])`.
    Index,
    /// `XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])`.
    Xlookup,
    /// `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`.
    Xmatch,
}

/// Declarative slice-shape coupling policy across range parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapePolicy {
    /// All range arguments must have identical `(rows, cols)` dimensions (`SUMPRODUCT`).
    EqualAllRanges,
    /// Two designated range arguments must each be 1D slices of equal element count (`XNPV`, `XIRR`).
    OneDimEqualLength {
        /// Zero-based argument index of the first 1D slice.
        first_idx: usize,
        /// Parameter name of the first 1D slice.
        first_param: &'static str,
        /// Zero-based argument index of the second 1D slice.
        second_idx: usize,
        /// Parameter name of the second 1D slice.
        second_param: &'static str,
    },
}

/// Declarative Stage A / Stage B AST validation policy for an [`ExcelFunction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidationPolicy {
    /// Standard positional `ParameterDef` type and kind validation.
    #[default]
    Standard,
    /// Positional parameter validation plus coupled range-shape invariants.
    ShapeCoupled(ShapePolicy),
    /// Short-circuiting conditional branch unification and selector validation.
    Branch(BranchPolicy),
    /// Coordinate/vector lookup shape and slice homogeneity validation.
    Lookup(LookupPolicy),
}

/// Evaluated 2D range grid carrying dimensions and cell values.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeGrid {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Flattened row-major cell values.
    pub values: Vec<CellValue>,
}

/// Contextual evaluation interface passed to function evaluation routines.
pub trait EvalContext {
    /// Evaluates a single formula AST expression to a scalar `CellValue`.
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError>;

    /// Evaluates a resolved coordinate or range reference to a [`RangeGrid`].
    ///
    /// The default implementation delegates to `eval_expr` returning a 1x1 grid.
    fn eval_range(&self, table_ref: &TableRef) -> Result<RangeGrid, FunctionError> {
        let val = self.eval_expr(&Expr::Reference(table_ref.clone()))?;
        Ok(RangeGrid {
            rows: 1,
            cols: 1,
            values: vec![val],
        })
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

    /// Statically guaranteed scalar return [`ValueType`], or `None` for polymorphic/error-bottom functions.
    fn return_type(&self) -> Option<ValueType>;

    /// Variadic tail repetition rule.
    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    /// Declarative AST validation policy for Stage A and Stage B expression validation.
    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Standard
    }

    /// Minimum number of required positional arguments.
    fn min_args(&self) -> usize {
        self.parameters()
            .iter()
            .take_while(|p| !p.requirement.is_optional())
            .count()
    }

    /// Maximum number of accepted arguments, or `None` if variadic.
    fn max_args(&self) -> Option<usize> {
        match self.variadic_rule() {
            VariadicRule::None => Some(self.parameters().len()),
            VariadicRule::Single => None,
        }
    }

    /// Executes the function logic given bound arguments and evaluation context.
    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError>;
}
