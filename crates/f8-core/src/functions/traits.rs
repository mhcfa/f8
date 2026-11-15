use std::fmt;

use serde::{Deserialize, Serialize};

use crate::expr::ast::{BlankCheck, CriteriaComparator, Expr};
use crate::functions::error::FunctionError;
use crate::model::table::AxisRole;
use crate::value::{CellValue, ExpectedType, ValueType};

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
    /// Pre-evaluated range reference (`Expr::Reference` required).
    Range,
    /// Pre-evaluated range slice or scalar argument.
    RangeOrArray,
    /// Unevaluated lazy expression evaluated conditionally on demand.
    Lazy,
    /// Typed conditional-aggregation criterion (`CallArg::{Expr, Comparator, BlankCheck}`).
    Criteria,
}

/// Syntactic reference classification for `FunctionArg::Array` bindings.
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
    /// Requires a non-empty scalar (`ExpectedType::NonEmpty`: `Number`, `Text`, or `Boolean`).
    NonEmpty,
    /// Requires a numeric scalar (`ExpectedType::Number`).
    Number,
    /// Requires an exact safe integer (`ExpectedType::Integer`).
    Integer,
    /// Requires a finite calendar date serial in `1.0..=2_958_465.0`.
    DateSerial,
    /// Requires an exact annuity timing integer in `{0, 1}`.
    AnnuityType,
    /// Requires an exact `YEARFRAC` day-count basis integer in `0..=4`.
    DayCountBasis,
    /// Requires a boolean value (`ExpectedType::Boolean`).
    Boolean,
}

impl ParamType {
    /// Converts this parameter type into its corresponding [`ExpectedType`], or `None` for [`Self::Any`].
    #[must_use]
    pub const fn to_expected_type(self) -> Option<ExpectedType> {
        match self {
            Self::Any => None,
            Self::NonEmpty => Some(ExpectedType::NonEmpty),
            Self::Number | Self::DateSerial => Some(ExpectedType::Number),
            Self::Integer | Self::AnnuityType | Self::DayCountBasis => Some(ExpectedType::Integer),
            Self::Boolean => Some(ExpectedType::Boolean),
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

/// Declarative empty-cell handling policy for non-`ParamType::Any` parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EmptyPolicy {
    /// Rejects `CellValue::Empty` across `Scalar`, `ReferenceKind::SingleCell`, and `ReferenceKind::Slice`.
    #[default]
    ForbidAll,
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
    /// Trailing arguments must occur in balanced pairs (e.g. `criteria_range1, criteria1` in `SUMIFS`).
    Pairs,
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
    /// Empty-cell policy for non-`ParamType::Any` parameters.
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
            empty_policy: EmptyPolicy::ForbidAll,
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
            empty_policy: EmptyPolicy::ForbidAll,
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
            empty_policy: EmptyPolicy::ForbidAll,
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

/// Bound runtime criterion argument supplied to `ParamKind::Criteria` slots.
#[derive(Debug, Clone, PartialEq)]
pub enum BoundCriterion {
    /// Exact-equality criterion (`CallArg::Expr`).
    Exact(CellValue),
    /// Scoped comparator criterion (`CallArg::Comparator`).
    Comparator {
        /// Scoped comparison operator (`<>`, `<`, `<=`, `>`, `>=`).
        op: CriteriaComparator,
        /// Evaluated threshold or comparison value.
        value: CellValue,
    },
    /// Scoped blank-status keyword (`BLANK` or `NOT_BLANK`).
    BlankCheck(BlankCheck),
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
        /// Syntactic reference classification (`SingleCell` vs `Slice`).
        ref_kind: ReferenceKind,
        /// Uniform column axis role (`Measure` or `Descriptor`) of the resolved reference.
        axis_role: AxisRole,
    },
    /// Unevaluated AST expression for lazy conditional branches.
    Lazy(&'a Expr),
    /// Bound conditional-aggregation criterion.
    Criterion(BoundCriterion),
    /// Intermediate omitted optional-parameter placeholder (`_`) without a static default.
    Omitted,
}

impl<'a> FunctionArg<'a> {
    /// Returns the human-readable kind name of this argument for diagnostic messages.
    #[must_use]
    pub(crate) const fn kind_name(&self) -> &'static str {
        match self {
            Self::Scalar(_) => "scalar",
            Self::Array { .. } => "array",
            Self::Lazy(_) => "lazy",
            Self::Criterion(_) => "criterion",
            Self::Omitted => "omitted",
        }
    }

    /// Constructs an [`Self::Array`].
    #[must_use]
    pub fn array(
        rows: usize,
        cols: usize,
        values: Vec<CellValue>,
        ref_kind: ReferenceKind,
        axis_role: AxisRole,
    ) -> Self {
        Self::Array {
            rows,
            cols,
            values,
            ref_kind,
            axis_role,
        }
    }

    /// Resolves this argument to a single scalar `CellValue`, evaluating lazy expressions if necessary.
    pub fn eval_scalar(
        &self,
        function: &'static str,
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match self {
            Self::Scalar(val) => Ok(val.clone()),
            Self::Lazy(expr) => ctx.eval_expr(expr),
            Self::Array {
                rows,
                cols,
                ref_kind: ReferenceKind::Slice,
                ..
            } => Err(FunctionError::SliceInScalarContext {
                rows: *rows,
                cols: *cols,
            }),
            Self::Array {
                ref_kind: ReferenceKind::SingleCell,
                ..
            } => Err(FunctionError::InvalidArgumentKind {
                function,
                expected: "scalar",
                found: "array",
            }),
            Self::Criterion(_) => Err(FunctionError::InvalidArgumentKind {
                function,
                expected: "scalar",
                found: "criterion",
            }),
            Self::Omitted => Err(FunctionError::InvalidArgumentKind {
                function,
                expected: "scalar",
                found: "omitted",
            }),
        }
    }
}

/// Declarative AST validation policy for short-circuiting conditional branching functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchPolicy {
    /// `IF(logical_test, value_if_true, value_if_false)`.
    If,
    /// `IFS(logical_test1, value_if_true1, ...)`.
    Ifs,
    /// `CHOOSE(index_num, value1, ...)`.
    Choose,
    /// `IFERROR(value, value_if_error)` and `IFNA(value, value_if_na)`.
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
    /// Odd-indexed criteria ranges (`1, 3, ...`) must match target range `0` (`SUMIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS`).
    EqualTargetAndOddPairRanges,
    /// Even-indexed criteria ranges (`2, 4, ...`) must match first criteria range `0` (`COUNTIFS`).
    EqualEvenPairRanges,
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
    /// Coordinate/vector lookup shape, mode-literal, and slice homogeneity validation.
    Lookup(LookupPolicy),
    /// Calendar `(year, month, day)` integer domain validation (`DATE`).
    CalendarDate,
}

/// Evaluated 2D range grid carrying dimensions, cell values, and uniform column [`AxisRole`].
#[derive(Debug, Clone, PartialEq)]
pub struct RangeGrid {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Flattened row-major cell values.
    pub values: Vec<CellValue>,
    /// Uniform column axis role (`Measure` or `Descriptor`) of the resolved reference.
    pub axis_role: AxisRole,
}

/// Contextual evaluation interface passed to function evaluation routines.
pub trait EvalContext {
    /// Evaluates a single formula AST expression to a scalar `CellValue`.
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError>;

    /// Evaluates a range or multi-cell expression to a [`RangeGrid`].
    ///
    /// The default implementation delegates to `eval_expr` returning a 1x1 `Measure` grid.
    fn eval_range(&self, expr: &Expr) -> Result<RangeGrid, FunctionError> {
        let val = self.eval_expr(expr)?;
        Ok(RangeGrid {
            rows: 1,
            cols: 1,
            values: vec![val],
            axis_role: AxisRole::Measure,
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
