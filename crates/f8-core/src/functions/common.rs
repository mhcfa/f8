use crate::functions::traits::{FunctionArg, ReferenceKind};
use crate::value::{CellValue, ExcelError, ExpectedType, TypeError, ValueType, normalize_zero};

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
                FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
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

    /// Collects boolean observations across arguments for `AND` and `OR` after `validate_bound_args`,
    /// skipping sparse `Empty` cells in `ReferenceKind::Slice` arguments and returning `ExcelError::Value`
    /// when `count == 0` (all-empty slice).
    pub fn collect(args: &[FunctionArg<'_>]) -> Result<Self, ExcelError> {
        let mut collector = Self::new();

        for arg in args {
            match arg {
                FunctionArg::Scalar(val) => match val.strict_bool() {
                    Ok(Ok(b)) => collector.record(b),
                    Ok(Err(err)) => return Err(err),
                    Err(_) => return Err(ExcelError::Value),
                },
                FunctionArg::Array {
                    values, ref_kind, ..
                } => {
                    for v in values {
                        match v {
                            CellValue::Boolean(b) => collector.record(*b),
                            CellValue::Error(e) => return Err(*e),
                            CellValue::Empty => match ref_kind {
                                ReferenceKind::Slice => {}
                                ReferenceKind::SingleCell => return Err(ExcelError::Value),
                            },
                            CellValue::Number(_) | CellValue::Text(_) => {
                                return Err(ExcelError::Value);
                            }
                        }
                    }
                }
                FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
            }
        }

        if collector.count == 0 {
            return Err(ExcelError::Value);
        }

        Ok(collector)
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
            FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
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
            FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
        }
    }

    count
}

/// Extracts a finite numeric scalar after `validate_bound_args`, bubbling `CellValue::Error(e)` as `Err(e)`.
pub fn scalar_number(arg: &FunctionArg<'_>) -> Result<f64, ExcelError> {
    match arg {
        FunctionArg::Scalar(val) => match val.strict_number() {
            Ok(res) => res,
            Err(_) => Err(ExcelError::Value),
        },
        FunctionArg::Array { .. } | FunctionArg::Lazy(_) | FunctionArg::Omitted => {
            Err(ExcelError::Value)
        }
    }
}

/// Extracts an optional finite numeric scalar at `idx`, returning `Ok(default)` when omitted or out of bounds.
pub fn optional_scalar_number(
    args: &[FunctionArg<'_>],
    idx: usize,
    default: f64,
) -> Result<f64, ExcelError> {
    match args.get(idx) {
        None | Some(FunctionArg::Omitted) => Ok(default),
        Some(arg) => scalar_number(arg),
    }
}

/// Extracts a safe integer scalar after `validate_bound_args`, bubbling `CellValue::Error(e)` as `Err(e)`.
pub fn scalar_integer(arg: &FunctionArg<'_>) -> Result<i64, ExcelError> {
    match arg {
        FunctionArg::Scalar(val) => match val.strict_integer() {
            Ok(res) => res,
            Err(_) => Err(ExcelError::Value),
        },
        FunctionArg::Array { .. } | FunctionArg::Lazy(_) | FunctionArg::Omitted => {
            Err(ExcelError::Value)
        }
    }
}

/// Extracts an optional safe integer scalar at `idx`, returning `Ok(default)` when omitted or out of bounds.
pub fn optional_scalar_integer(
    args: &[FunctionArg<'_>],
    idx: usize,
    default: i64,
) -> Result<i64, ExcelError> {
    match args.get(idx) {
        None | Some(FunctionArg::Omitted) => Ok(default),
        Some(arg) => scalar_integer(arg),
    }
}

/// Extracts a boolean scalar after `validate_bound_args`, bubbling `CellValue::Error(e)` as `Err(e)`.
pub fn scalar_bool(arg: &FunctionArg<'_>) -> Result<bool, ExcelError> {
    match arg {
        FunctionArg::Scalar(val) => match val.strict_bool() {
            Ok(res) => res,
            Err(_) => Err(ExcelError::Value),
        },
        FunctionArg::Array { .. } | FunctionArg::Lazy(_) | FunctionArg::Omitted => {
            Err(ExcelError::Value)
        }
    }
}
