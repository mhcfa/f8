use crate::functions::error::FunctionError;
use crate::functions::traits::{EvalContext, FunctionArg, ReferenceKind, TypeExpectation};
use crate::value::{
    CellValue, ExcelError, ExpectedType, StrictIntError, TypeError, ValueType, normalize_zero,
};

/// Aggregated numeric statistics gathered across arguments for arithmetic functions.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericCollector {
    /// Sum of all numbers and numeric scalars.
    pub sum: f64,
    /// Count of numbers collected.
    pub count: usize,
    /// Minimum numeric value encountered, or `None` if no numbers were collected.
    pub min: Option<f64>,
    /// Maximum numeric value encountered, or `None` if no numbers were collected.
    pub max: Option<f64>,
}

impl NumericCollector {
    /// Constructs a new, empty `NumericCollector`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sum: 0.0,
            count: 0,
            min: None,
            max: None,
        }
    }

    /// Records a single numeric observation, updating sum, count, min, and max.
    pub fn record(&mut self, val: f64) {
        self.sum += val;
        self.count += 1;
        self.min = Some(match self.min {
            Some(curr) => curr.min(val),
            None => val,
        });
        self.max = Some(match self.max {
            Some(curr) => curr.max(val),
            None => val,
        });
    }

    /// Iterates over validated numeric observations across arguments, skipping sparse `Empty` cells in slices.
    fn for_each_numeric<F>(args: &[FunctionArg<'_>], mut f: F) -> Result<(), ExcelError>
    where
        F: FnMut(f64),
    {
        for arg in args {
            match arg {
                FunctionArg::Scalar(val) => match val.strict_number() {
                    Ok(Ok(n)) => f(n),
                    Ok(Err(e)) => return Err(e),
                    Err(_) => return Err(ExcelError::Value),
                },
                FunctionArg::TypedErrorScalar { error, .. } => {
                    return Err(*error);
                }
                FunctionArg::Array {
                    values, ref_kind, ..
                } => {
                    for v in values {
                        match v {
                            CellValue::Number(n) => {
                                if !n.is_finite() {
                                    return Err(ExcelError::Num);
                                }
                                f(normalize_zero(*n));
                            }
                            CellValue::Error(e) => return Err(*e),
                            CellValue::Empty => match ref_kind {
                                ReferenceKind::Slice => {}
                                ReferenceKind::SingleCell => return Err(ExcelError::Value),
                            },
                            CellValue::Text(_) | CellValue::Boolean(_) => {
                                return Err(ExcelError::Value);
                            }
                        }
                    }
                }
                FunctionArg::Lazy(_) | FunctionArg::Criterion(_) | FunctionArg::Omitted => {}
            }
        }
        Ok(())
    }

    /// Collects numbers across arguments for arithmetic aggregators (`SUM`, `AVERAGE`, `MIN`, `MAX`).
    pub fn collect(args: &[FunctionArg<'_>]) -> Result<Self, ExcelError> {
        let mut collector = Self::new();
        Self::for_each_numeric(args, |n| collector.record(n))?;
        collector.sum = normalize_zero(collector.sum);
        Ok(collector)
    }

    /// Returns the minimum value, defaulting to `0.0` if no numbers were collected.
    #[must_use]
    pub fn min_or_zero(&self) -> f64 {
        normalize_zero(self.min.unwrap_or(0.0))
    }

    /// Returns the maximum value, defaulting to `0.0` if no numbers were collected.
    #[must_use]
    pub fn max_or_zero(&self) -> f64 {
        normalize_zero(self.max.unwrap_or(0.0))
    }

    /// Collects all numeric observations across arguments into a vector following Excel parity rules.
    pub fn collect_values(args: &[FunctionArg<'_>]) -> Result<Vec<f64>, ExcelError> {
        let mut values = Vec::new();
        Self::for_each_numeric(args, |n| values.push(n))?;
        Ok(values)
    }
}

impl Default for NumericCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluates an individual branch argument, returning its scalar value or evaluating its lazy expression.
pub fn eval_branch_arg(
    arg: &FunctionArg<'_>,
    function: &'static str,
    ctx: &dyn EvalContext,
) -> Result<CellValue, FunctionError> {
    match arg {
        FunctionArg::Scalar(val) => Ok(val.clone()),
        FunctionArg::TypedErrorScalar { error, .. } => Ok(CellValue::Error(*error)),
        FunctionArg::Lazy(expr) => ctx.eval_expr(expr),
        FunctionArg::Array {
            rows,
            cols,
            ref_kind: ReferenceKind::Slice,
            ..
        } => Err(FunctionError::SliceInScalarContext {
            rows: *rows,
            cols: *cols,
        }),
        FunctionArg::Array {
            ref_kind: ReferenceKind::SingleCell,
            ..
        } => Err(FunctionError::InvalidArgumentKind {
            function,
            expected: "scalar or expression",
            found: "array",
        }),
        FunctionArg::Criterion(_) => Err(FunctionError::InvalidArgumentKind {
            function,
            expected: "scalar or expression",
            found: "criterion",
        }),
        FunctionArg::Omitted => Err(FunctionError::InvalidArgumentKind {
            function,
            expected: "scalar or expression",
            found: "omitted",
        }),
    }
}

/// Unifies concrete non-`Empty`, non-`Error` `ValueType`s left-to-right across branch result arms.
#[derive(Debug, Clone, Copy)]
pub struct BranchTypeUnifier {
    function: &'static str,
    unified: Option<ValueType>,
}

impl BranchTypeUnifier {
    /// Creates a new unifier for `function`.
    #[must_use]
    pub const fn new(function: &'static str) -> Self {
        Self {
            function,
            unified: None,
        }
    }

    /// Creates a new unifier pre-seeded with `initial` (when concrete non-`Empty`).
    #[must_use]
    pub const fn with_initial(function: &'static str, initial: Option<ValueType>) -> Self {
        let seeded = match initial {
            Some(ValueType::Number) => Some(ValueType::Number),
            Some(ValueType::Text) => Some(ValueType::Text),
            Some(ValueType::Boolean) => Some(ValueType::Boolean),
            Some(ValueType::Empty) | None => None,
        };
        Self {
            function,
            unified: seeded,
        }
    }

    /// Returns the unified concrete `ValueType`, if any.
    #[must_use]
    pub const fn unified(&self) -> Option<ValueType> {
        self.unified
    }

    /// Records `candidate` at 1-indexed `position`, returning `TypeError::IncompatibleBranchTypes` on mismatch.
    pub fn record(
        &mut self,
        position: usize,
        candidate: Option<ValueType>,
    ) -> Result<Option<ValueType>, TypeError> {
        let concrete = match candidate {
            Some(vt @ (ValueType::Number | ValueType::Text | ValueType::Boolean)) => vt,
            Some(ValueType::Empty) | None => return Ok(self.unified),
        };
        match self.unified {
            None => {
                self.unified = Some(concrete);
                Ok(self.unified)
            }
            Some(expected) => {
                if concrete == expected {
                    Ok(self.unified)
                } else {
                    Err(TypeError::IncompatibleBranchTypes {
                        function: self.function,
                        position,
                        expected,
                        found: concrete,
                    })
                }
            }
        }
    }
}

/// Evaluates a taken branch result arm and returns `(value, inferred_value_type)`.
///
/// When a lazy arm evaluates to `CellValue::Error(_)`, recursively validates its subtree via
/// `EvalContext::validate_dormant_expr` to validate any short-circuited inner branches and infer
/// its static/slice return `ValueType` (for example on `#REF!`/`#N/A`-producing `INDEX`/`XLOOKUP` calls).
pub fn eval_taken_branch_arm(
    arg: &FunctionArg<'_>,
    function: &'static str,
    ctx: &dyn EvalContext,
) -> Result<(CellValue, Option<ValueType>), FunctionError> {
    let val = eval_branch_arg(arg, function, ctx)?;
    let vt = match &val {
        CellValue::Number(_) => Some(ValueType::Number),
        CellValue::Text(_) => Some(ValueType::Text),
        CellValue::Boolean(_) => Some(ValueType::Boolean),
        CellValue::Empty => None,
        CellValue::Error(_) => match arg {
            FunctionArg::Lazy(expr) => ctx.validate_dormant_expr(expr, None)?,
            FunctionArg::TypedErrorScalar { inferred_type, .. } => Some(*inferred_type),
            FunctionArg::Scalar(_)
            | FunctionArg::Array { .. }
            | FunctionArg::Criterion(_)
            | FunctionArg::Omitted => None,
        },
    };
    Ok((val, vt))
}

/// Validates an untaken (dormant) branch argument and returns its inferred `ValueType`, if any.
pub fn validate_dormant_arm(
    arg: &FunctionArg<'_>,
    function: &'static str,
    expected: Option<TypeExpectation>,
    ctx: &dyn EvalContext,
) -> Result<Option<ValueType>, FunctionError> {
    match arg {
        FunctionArg::Lazy(expr) => ctx.validate_dormant_expr(expr, expected),
        FunctionArg::Scalar(val) => match val {
            CellValue::Number(_) | CellValue::Text(_) | CellValue::Boolean(_) => {
                let vt = match val {
                    CellValue::Number(_) => ValueType::Number,
                    CellValue::Text(_) => ValueType::Text,
                    CellValue::Boolean(_) => ValueType::Boolean,
                    CellValue::Empty | CellValue::Error(_) => return Ok(None),
                };
                if let Some(TypeExpectation {
                    expected: exp_ty,
                    origin,
                }) = expected
                {
                    if exp_ty == ExpectedType::Integer {
                        match val.strict_integer() {
                            Ok(_) => {}
                            Err(StrictIntError::WrongType(found)) => {
                                return Err(FunctionError::Type(
                                    origin.into_invalid_type(exp_ty, found),
                                ));
                            }
                            Err(StrictIntError::NonInteger(value)) => {
                                return Err(FunctionError::Type(origin.into_non_integer(value)));
                            }
                        }
                    } else if !value_type_satisfies(vt, exp_ty) {
                        return Err(FunctionError::Type(origin.into_invalid_type(exp_ty, vt)));
                    }
                }
                Ok(Some(vt))
            }
            CellValue::Empty | CellValue::Error(_) => Ok(None),
        },
        FunctionArg::TypedErrorScalar { inferred_type, .. } => {
            if let Some(TypeExpectation {
                expected: exp_ty,
                origin,
            }) = expected
                && !value_type_satisfies(*inferred_type, exp_ty)
            {
                return Err(FunctionError::Type(
                    origin.into_invalid_type(exp_ty, *inferred_type),
                ));
            }
            Ok(Some(*inferred_type))
        }
        FunctionArg::Array {
            rows,
            cols,
            ref_kind: ReferenceKind::Slice,
            ..
        } => Err(FunctionError::SliceInScalarContext {
            rows: *rows,
            cols: *cols,
        }),
        FunctionArg::Array {
            ref_kind: ReferenceKind::SingleCell,
            ..
        } => Err(FunctionError::InvalidArgumentKind {
            function,
            expected: "scalar or expression",
            found: "array",
        }),
        FunctionArg::Criterion(_) => Err(FunctionError::InvalidArgumentKind {
            function,
            expected: "scalar or expression",
            found: "criterion",
        }),
        FunctionArg::Omitted => Ok(None),
    }
}

pub(crate) const fn value_type_satisfies(found: ValueType, expected: ExpectedType) -> bool {
    match (expected, found) {
        (ExpectedType::NonEmpty, ValueType::Number | ValueType::Text | ValueType::Boolean)
        | (ExpectedType::Number | ExpectedType::Integer, ValueType::Number)
        | (ExpectedType::Text, ValueType::Text)
        | (ExpectedType::Boolean, ValueType::Boolean) => true,
        (ExpectedType::NonEmpty, ValueType::Empty)
        | (
            ExpectedType::Number | ExpectedType::Integer,
            ValueType::Text | ValueType::Boolean | ValueType::Empty,
        )
        | (ExpectedType::Text, ValueType::Number | ValueType::Boolean | ValueType::Empty)
        | (ExpectedType::Boolean, ValueType::Number | ValueType::Text | ValueType::Empty) => false,
    }
}

/// Aggregated logical truth statistics gathered across arguments for boolean functions (`AND`, `OR`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalCollector {
    /// Number of valid logical values collected.
    pub count: usize,
    /// Whether all collected values are true (logical conjunction).
    pub all_true: bool,
    /// Whether any collected value is true (logical disjunction).
    pub any_true: bool,
}

impl LogicalCollector {
    /// Constructs a new, empty `LogicalCollector`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            count: 0,
            all_true: true,
            any_true: false,
        }
    }

    /// Records a single logical truth observation.
    pub fn record(&mut self, b: bool) {
        self.count += 1;
        self.all_true = self.all_true && b;
        self.any_true = self.any_true || b;
    }

    /// Collects strict boolean values across arguments for `AND` and `OR`.
    ///
    /// - Rejects `Number` and `Text` everywhere with `TypeError::InvalidArgumentType`.
    /// - Rejects `Empty` on `Scalar`, `Lazy`, and `ReferenceKind::SingleCell` arguments.
    /// - Skips sparse `Empty` cells strictly in `ReferenceKind::Slice` arguments.
    /// - Validates types across all arguments before bubbling the first `ExcelError` or returning
    ///   `ExcelError::Value` when `count == 0` (all-empty slice).
    pub fn collect(
        function: &'static str,
        param: &'static str,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<Result<Self, ExcelError>, FunctionError> {
        let mut collector = Self::new();
        let mut first_error: Option<ExcelError> = None;

        for (idx, arg) in args.iter().enumerate() {
            let position = idx + 1;
            match arg {
                FunctionArg::Scalar(val) => match val.strict_bool() {
                    Ok(Ok(b)) => collector.record(b),
                    Ok(Err(err)) => {
                        if first_error.is_none() {
                            first_error = Some(err);
                        }
                    }
                    Err(found) => {
                        return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                            function,
                            param,
                            position,
                            expected: ExpectedType::Boolean,
                            found,
                        }));
                    }
                },
                FunctionArg::TypedErrorScalar {
                    error,
                    inferred_type,
                } => {
                    if *inferred_type != ValueType::Boolean {
                        return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                            function,
                            param,
                            position,
                            expected: ExpectedType::Boolean,
                            found: *inferred_type,
                        }));
                    }
                    if first_error.is_none() {
                        first_error = Some(*error);
                    }
                }
                FunctionArg::Lazy(_) => {
                    let (val, inferred_vt) = arg.eval_scalar_with_type(function, ctx)?;
                    match val.strict_bool() {
                        Ok(Ok(b)) => collector.record(b),
                        Ok(Err(err)) => {
                            if let Some(vt @ (ValueType::Number | ValueType::Text)) = inferred_vt {
                                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                                    function,
                                    param,
                                    position,
                                    expected: ExpectedType::Boolean,
                                    found: vt,
                                }));
                            }
                            if first_error.is_none() {
                                first_error = Some(err);
                            }
                        }
                        Err(found) => {
                            return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                                function,
                                param,
                                position,
                                expected: ExpectedType::Boolean,
                                found,
                            }));
                        }
                    }
                }
                FunctionArg::Array {
                    values,
                    error_types,
                    ref_kind,
                    ..
                } => {
                    let skip_empty = match ref_kind {
                        ReferenceKind::Slice => true,
                        ReferenceKind::SingleCell => false,
                    };
                    for (elem_idx, v) in values.iter().enumerate() {
                        if skip_empty && matches!(v, CellValue::Empty) {
                            continue;
                        }
                        match v.strict_bool() {
                            Ok(Ok(b)) => collector.record(b),
                            Ok(Err(err)) => {
                                if let Some(vt @ (ValueType::Number | ValueType::Text)) =
                                    error_types.get(elem_idx).copied().flatten()
                                {
                                    return Err(FunctionError::Type(
                                        TypeError::InvalidArgumentType {
                                            function,
                                            param,
                                            position,
                                            expected: ExpectedType::Boolean,
                                            found: vt,
                                        },
                                    ));
                                }
                                if first_error.is_none() {
                                    first_error = Some(err);
                                }
                            }
                            Err(found) => {
                                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                                    function,
                                    param,
                                    position,
                                    expected: ExpectedType::Boolean,
                                    found,
                                }));
                            }
                        }
                    }
                }
                FunctionArg::Criterion(_) => {
                    return Err(FunctionError::InvalidArgumentKind {
                        function,
                        expected: "scalar or array",
                        found: "criterion",
                    });
                }
                FunctionArg::Omitted => {}
            }
        }

        if let Some(err) = first_error {
            return Ok(Err(err));
        }

        if collector.count == 0 {
            return Ok(Err(ExcelError::Value));
        }

        Ok(Ok(collector))
    }
}

impl Default for LogicalCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Counts finite numeric cells across range arguments according to Excel `COUNT` semantics
/// (skipping sparse `Empty` and `Error` cells).
#[must_use]
pub fn count_numbers(args: &[FunctionArg<'_>]) -> usize {
    let mut count = 0;

    for arg in args {
        match arg {
            FunctionArg::Array { values, .. } => {
                for v in values {
                    if matches!(v, CellValue::Number(n) if n.is_finite()) {
                        count += 1;
                    }
                }
            }
            FunctionArg::Scalar(val) => {
                if matches!(val, CellValue::Number(n) if n.is_finite()) {
                    count += 1;
                }
            }
            FunctionArg::TypedErrorScalar { .. }
            | FunctionArg::Lazy(_)
            | FunctionArg::Criterion(_)
            | FunctionArg::Omitted => {}
        }
    }

    count
}

/// Counts non-empty cells across range arguments according to Excel `COUNTA` semantics.
#[must_use]
pub fn count_non_empty(args: &[FunctionArg<'_>]) -> usize {
    let mut count = 0;

    for arg in args {
        match arg {
            FunctionArg::Array { values, .. } => {
                for v in values {
                    if !matches!(v, CellValue::Empty) {
                        count += 1;
                    }
                }
            }
            FunctionArg::Scalar(val) => {
                if !matches!(val, CellValue::Empty) {
                    count += 1;
                }
            }
            FunctionArg::TypedErrorScalar { .. } => {
                count += 1;
            }
            FunctionArg::Lazy(_) | FunctionArg::Criterion(_) | FunctionArg::Omitted => {}
        }
    }

    count
}

/// Extracts a strict numeric scalar from `arg`, rejecting `Text`, `Boolean`, and `Empty` with `TypeError::InvalidArgumentType`
/// while bubbling `CellValue::Error(e)` as `Ok(Err(e))` when its inferred type is compatible.
pub fn extract_scalar_number(
    arg: &FunctionArg<'_>,
    function: &'static str,
    param: &'static str,
    position: usize,
    ctx: &dyn EvalContext,
) -> Result<Result<f64, ExcelError>, FunctionError> {
    let (val, inferred_vt) = arg.eval_scalar_with_type(function, ctx)?;
    match val.strict_number() {
        Ok(Ok(n)) => Ok(Ok(n)),
        Ok(Err(err)) => {
            if let Some(vt @ (ValueType::Text | ValueType::Boolean)) = inferred_vt {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Number,
                    found: vt,
                }));
            }
            Ok(Err(err))
        }
        Err(found) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function,
            param,
            position,
            expected: ExpectedType::Number,
            found,
        })),
    }
}

/// Extracts a strict safe integer from `arg`, rejecting wrong value types with `TypeError::InvalidArgumentType`
/// and fractional/out-of-safe-range numbers with `TypeError::NonIntegerArgument`, while bubbling `CellValue::Error(e)`
/// when its inferred type is compatible.
pub fn extract_scalar_integer(
    arg: &FunctionArg<'_>,
    function: &'static str,
    param: &'static str,
    position: usize,
    ctx: &dyn EvalContext,
) -> Result<Result<i64, ExcelError>, FunctionError> {
    let (val, inferred_vt) = arg.eval_scalar_with_type(function, ctx)?;
    match val.strict_integer() {
        Ok(Ok(i)) => Ok(Ok(i)),
        Ok(Err(err)) => {
            if let Some(vt @ (ValueType::Text | ValueType::Boolean)) = inferred_vt {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found: vt,
                }));
            }
            Ok(Err(err))
        }
        Err(StrictIntError::WrongType(found)) => {
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::Integer,
                found,
            }))
        }
        Err(StrictIntError::NonInteger(value)) => {
            Err(FunctionError::Type(TypeError::NonIntegerArgument {
                function,
                param,
                position,
                value,
            }))
        }
    }
}

/// Extracts a strict boolean scalar from `arg`, rejecting `Number`, `Text`, and `Empty` with `TypeError::InvalidArgumentType`
/// while bubbling `CellValue::Error(e)` as `Ok(Err(e))` when its inferred type is compatible.
pub fn extract_scalar_bool(
    arg: &FunctionArg<'_>,
    function: &'static str,
    param: &'static str,
    position: usize,
    ctx: &dyn EvalContext,
) -> Result<Result<bool, ExcelError>, FunctionError> {
    let (val, inferred_vt) = arg.eval_scalar_with_type(function, ctx)?;
    match val.strict_bool() {
        Ok(Ok(b)) => Ok(Ok(b)),
        Ok(Err(err)) => {
            if let Some(vt @ (ValueType::Number | ValueType::Text)) = inferred_vt {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Boolean,
                    found: vt,
                }));
            }
            Ok(Err(err))
        }
        Err(found) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function,
            param,
            position,
            expected: ExpectedType::Boolean,
            found,
        })),
    }
}
