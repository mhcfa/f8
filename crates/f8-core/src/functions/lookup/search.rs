//! Shared 1D vector search and matching engine for lookup functions (`XLOOKUP`, `XMATCH`).
//!
//! Implements strict mode parsing, slice homogeneity and binary-search monotonicity checks,
//! linear forward/reverse scans, and binary ascending/descending bisection.

use std::cmp::Ordering;

use crate::functions::error::FunctionError;
use crate::functions::traits::FunctionArg;
use crate::value::{CellValue, ExcelError, ExpectedType, StrictIntError, TypeError, ValueType};

/// Match mode configuration for `XLOOKUP` and `XMATCH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchMode {
    /// Exact match (default: `0`).
    #[default]
    Exact,
    /// Exact match or next smaller item (`-1`).
    ExactOrNextSmaller,
    /// Exact match or next larger item (`1`).
    ExactOrNextLarger,
}

impl MatchMode {
    /// Converts a validated mode integer (`{0, -1, 1}`) into [`MatchMode`].
    pub fn from_mode_int(
        mode: i64,
        function: &'static str,
        param: &'static str,
        position: usize,
    ) -> Result<Self, FunctionError> {
        match mode {
            0 => Ok(Self::Exact),
            -1 => Ok(Self::ExactOrNextSmaller),
            1 => Ok(Self::ExactOrNextLarger),
            _ => Err(FunctionError::InvalidLookupMode {
                function,
                param,
                position,
                mode,
            }),
        }
    }

    /// Parses a `CellValue` into a [`MatchMode`] using strict integer extraction followed by mode-set validation.
    pub fn parse(
        val: &CellValue,
        function: &'static str,
        param: &'static str,
        position: usize,
    ) -> Result<Self, FunctionError> {
        let int_val = match val.strict_integer() {
            Ok(Ok(i)) => i,
            Ok(Err(_)) => {
                return Err(FunctionError::NonLiteralLookupMode {
                    function,
                    param,
                    position,
                });
            }
            Err(StrictIntError::WrongType(found)) => {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found,
                }));
            }
            Err(StrictIntError::NonInteger(value)) => {
                return Err(FunctionError::Type(TypeError::NonIntegerArgument {
                    function,
                    param,
                    position,
                    value,
                }));
            }
        };

        Self::from_mode_int(int_val, function, param, position)
    }
}

/// Search mode configuration for `XLOOKUP` and `XMATCH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Linear search from first item to last item (default: `1`).
    #[default]
    FirstToLast,
    /// Linear search from last item to first item (`-1`).
    LastToFirst,
    /// Binary search requiring monotonically non-decreasing keys (`2`).
    BinaryAscending,
    /// Binary search requiring monotonically non-increasing keys (`-2`).
    BinaryDescending,
}

impl SearchMode {
    /// Returns the integer mode code corresponding to this search mode.
    #[must_use]
    pub const fn as_mode_int(self) -> i64 {
        match self {
            Self::FirstToLast => 1,
            Self::LastToFirst => -1,
            Self::BinaryAscending => 2,
            Self::BinaryDescending => -2,
        }
    }

    /// Converts a validated mode integer (`{1, -1, 2, -2}`) into [`SearchMode`].
    pub fn from_mode_int(
        mode: i64,
        function: &'static str,
        param: &'static str,
        position: usize,
    ) -> Result<Self, FunctionError> {
        match mode {
            1 => Ok(Self::FirstToLast),
            -1 => Ok(Self::LastToFirst),
            2 => Ok(Self::BinaryAscending),
            -2 => Ok(Self::BinaryDescending),
            _ => Err(FunctionError::InvalidLookupMode {
                function,
                param,
                position,
                mode,
            }),
        }
    }

    /// Parses a `CellValue` into a [`SearchMode`] using strict integer extraction followed by mode-set validation.
    pub fn parse(
        val: &CellValue,
        function: &'static str,
        param: &'static str,
        position: usize,
    ) -> Result<Self, FunctionError> {
        let int_val = match val.strict_integer() {
            Ok(Ok(i)) => i,
            Ok(Err(_)) => {
                return Err(FunctionError::NonLiteralLookupMode {
                    function,
                    param,
                    position,
                });
            }
            Err(StrictIntError::WrongType(found)) => {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found,
                }));
            }
            Err(StrictIntError::NonInteger(value)) => {
                return Err(FunctionError::Type(TypeError::NonIntegerArgument {
                    function,
                    param,
                    position,
                    value,
                }));
            }
        };

        Self::from_mode_int(int_val, function, param, position)
    }
}

/// Validates that all non-`Empty` elements (including `CellValue::Error(_)` cells carrying concrete
/// inferred `ValueType` provenance in `error_types`) in `slice` share a single `ValueType`,
/// returning that inferred `ValueType` (or `None` if all elements are `Empty` or untyped `Error`).
pub fn validate_homogeneous_slice(
    slice: &[CellValue],
    error_types: &[Option<ValueType>],
    function: &'static str,
    param: &'static str,
    position: usize,
) -> Result<Option<ValueType>, FunctionError> {
    let mut first_vt: Option<ValueType> = None;
    for (idx, cell) in slice.iter().enumerate() {
        let cell_vt = match cell {
            CellValue::Number(_) => ValueType::Number,
            CellValue::Text(_) => ValueType::Text,
            CellValue::Boolean(_) => ValueType::Boolean,
            CellValue::Empty => continue,
            CellValue::Error(_) => match error_types.get(idx).copied().flatten() {
                Some(vt @ (ValueType::Number | ValueType::Text | ValueType::Boolean)) => vt,
                Some(ValueType::Empty) | None => continue,
            },
        };
        match first_vt {
            None => first_vt = Some(cell_vt),
            Some(expected_vt) => {
                if cell_vt != expected_vt {
                    return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                        function,
                        param,
                        position,
                        expected: ExpectedType::from_value_type(expected_vt)
                            .unwrap_or(ExpectedType::NonEmpty),
                        found: cell_vt,
                    }));
                }
            }
        }
    }
    Ok(first_vt)
}

/// Validates `lookup_array` (position `2`) for `XLOOKUP` and `XMATCH` under linear or binary `search_mode`.
///
/// - Linear search (`1` / `-1`): All non-`Empty` elements (including `CellValue::Error(_)` cells with
///   concrete `error_types` provenance; polymorphic `None` errors are skipped) must be mutually homogeneous
///   and match `lookup_value_type` when `Some`.
/// - Binary search (`2` / `-2`): Every element must be non-`Empty` (`ExpectedType::NonEmpty`),
///   mutually homogeneous and compatible with `lookup_value_type` (checked prior to `ErrorInBinaryLookupArray`),
///   non-`Error` (`ErrorInBinaryLookupArray`), and monotonically sorted (`UnsortedBinaryLookupArray`)
///   under `CellValue::compare_relational`.
pub fn validate_lookup_array(
    slice: &[CellValue],
    error_types: &[Option<ValueType>],
    lookup_value_type: Option<ValueType>,
    search_mode: SearchMode,
    function: &'static str,
) -> Result<(), FunctionError> {
    let mut expected_vt = lookup_value_type.filter(|vt| *vt != ValueType::Empty);

    match search_mode {
        SearchMode::FirstToLast | SearchMode::LastToFirst => {
            for (idx, cell) in slice.iter().enumerate() {
                let cell_vt = match cell {
                    CellValue::Number(_) => ValueType::Number,
                    CellValue::Text(_) => ValueType::Text,
                    CellValue::Boolean(_) => ValueType::Boolean,
                    CellValue::Empty => continue,
                    CellValue::Error(_) => match error_types.get(idx).copied().flatten() {
                        Some(vt @ (ValueType::Number | ValueType::Text | ValueType::Boolean)) => vt,
                        Some(ValueType::Empty) | None => continue,
                    },
                };
                match expected_vt {
                    None => expected_vt = Some(cell_vt),
                    Some(exp_vt) => {
                        if cell_vt != exp_vt {
                            return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                                function,
                                param: "lookup_array",
                                position: 2,
                                expected: ExpectedType::from_value_type(exp_vt)
                                    .unwrap_or(ExpectedType::NonEmpty),
                                found: cell_vt,
                            }));
                        }
                    }
                }
            }
            Ok(())
        }
        SearchMode::BinaryAscending | SearchMode::BinaryDescending => {
            let mode_int = search_mode.as_mode_int();
            for (idx, cell) in slice.iter().enumerate() {
                let cell_vt = match cell {
                    CellValue::Empty => {
                        return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                            function,
                            param: "lookup_array",
                            position: 2,
                            expected: ExpectedType::NonEmpty,
                            found: ValueType::Empty,
                        }));
                    }
                    CellValue::Error(_) => match error_types.get(idx).copied().flatten() {
                        Some(vt @ (ValueType::Number | ValueType::Text | ValueType::Boolean)) => vt,
                        Some(ValueType::Empty) | None => continue,
                    },
                    CellValue::Number(_) => ValueType::Number,
                    CellValue::Text(_) => ValueType::Text,
                    CellValue::Boolean(_) => ValueType::Boolean,
                };
                match expected_vt {
                    None => expected_vt = Some(cell_vt),
                    Some(exp_vt) => {
                        if cell_vt != exp_vt {
                            return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                                function,
                                param: "lookup_array",
                                position: 2,
                                expected: ExpectedType::from_value_type(exp_vt)
                                    .unwrap_or(ExpectedType::NonEmpty),
                                found: cell_vt,
                            }));
                        }
                    }
                }
            }

            for cell in slice {
                match cell {
                    CellValue::Error(_) => {
                        return Err(FunctionError::ErrorInBinaryLookupArray {
                            function,
                            search_mode: mode_int,
                        });
                    }
                    CellValue::Number(n) if !n.is_finite() => {
                        return Err(FunctionError::ErrorInBinaryLookupArray {
                            function,
                            search_mode: mode_int,
                        });
                    }
                    CellValue::Number(_)
                    | CellValue::Text(_)
                    | CellValue::Boolean(_)
                    | CellValue::Empty => {}
                }
            }

            for pair in slice.windows(2) {
                let [prev, curr] = pair else {
                    continue;
                };
                match prev.compare_relational(curr) {
                    Ok(Ok(ord)) => {
                        let out_of_order = match search_mode {
                            SearchMode::BinaryAscending => ord == Ordering::Greater,
                            SearchMode::BinaryDescending => ord == Ordering::Less,
                            SearchMode::FirstToLast | SearchMode::LastToFirst => false,
                        };
                        if out_of_order {
                            return Err(FunctionError::UnsortedBinaryLookupArray {
                                function,
                                search_mode: mode_int,
                            });
                        }
                    }
                    Ok(Err(_)) => {
                        return Err(FunctionError::ErrorInBinaryLookupArray {
                            function,
                            search_mode: mode_int,
                        });
                    }
                    Err((left_vt, right_vt)) => {
                        return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                            function,
                            param: "lookup_array",
                            position: 2,
                            expected: ExpectedType::from_value_type(left_vt)
                                .unwrap_or(ExpectedType::NonEmpty),
                            found: right_vt,
                        }));
                    }
                }
            }

            Ok(())
        }
    }
}

/// Borrowed 1D range slice `(rows, cols, values, error_types)`.
pub type TypedRangeSlice<'a> = (usize, usize, &'a [CellValue], &'a [Option<ValueType>]);

/// Extracts a 1D range slice `(rows, cols, values, error_types)` from `arg`, rejecting non-range arguments with
/// `InvalidArgumentKind` and 2D matrices (`rows > 1 && cols > 1`) with `ExpectedOneDimensionalSlice`.
pub fn extract_1d_range_slice<'a>(
    arg: &'a FunctionArg<'_>,
    function: &'static str,
    param: &'static str,
) -> Result<TypedRangeSlice<'a>, FunctionError> {
    match arg {
        FunctionArg::Array {
            rows,
            cols,
            values,
            error_types,
            ..
        } => {
            if *rows > 1 && *cols > 1 {
                return Err(FunctionError::ExpectedOneDimensionalSlice {
                    function,
                    param,
                    rows: *rows,
                    cols: *cols,
                });
            }
            Ok((*rows, *cols, values.as_slice(), error_types.as_slice()))
        }
        FunctionArg::Scalar(_) | FunctionArg::TypedErrorScalar { .. } => {
            Err(FunctionError::InvalidArgumentKind {
                function,
                expected: "range",
                found: "scalar",
            })
        }
        FunctionArg::Lazy(_) => Err(FunctionError::InvalidArgumentKind {
            function,
            expected: "range",
            found: "lazy",
        }),
        FunctionArg::Omitted => Err(FunctionError::InvalidArgumentKind {
            function,
            expected: "range",
            found: "omitted",
        }),
    }
}

/// Searches `slice` for `lookup_val` using the specified `match_mode` and `search_mode`.
///
/// Returns `Ok(Some(index))` with 0-based index if found, `Ok(None)` if not found,
/// or `Err(ExcelError)` on domain error inputs.
pub fn find_match_index(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
    search_mode: SearchMode,
) -> Result<Option<usize>, ExcelError> {
    if let CellValue::Error(err) = lookup_val {
        return Err(*err);
    }

    if slice.is_empty() {
        return Ok(None);
    }

    match search_mode {
        SearchMode::FirstToLast => find_linear_forward(lookup_val, slice, match_mode),
        SearchMode::LastToFirst => find_linear_reverse(lookup_val, slice, match_mode),
        SearchMode::BinaryAscending => find_binary_ascending(lookup_val, slice, match_mode),
        SearchMode::BinaryDescending => find_binary_descending(lookup_val, slice, match_mode),
    }
}

/// Performs linear forward search (indices `0..slice.len()`), skipping `Empty` and `Error` cells.
fn find_linear_forward(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    match match_mode {
        MatchMode::Exact => {
            for (i, cell) in slice.iter().enumerate() {
                if matches!(cell, CellValue::Empty | CellValue::Error(_)) {
                    continue;
                }
                if let Ok(Ok(Ordering::Equal)) = cell.compare_relational(lookup_val) {
                    return Ok(Some(i));
                }
            }
            Ok(None)
        }
        MatchMode::ExactOrNextSmaller => {
            let mut exact_match = None;
            let mut best_smaller: Option<(usize, &CellValue)> = None;

            for (i, cell) in slice.iter().enumerate() {
                if matches!(cell, CellValue::Empty | CellValue::Error(_)) {
                    continue;
                }
                match cell.compare_relational(lookup_val) {
                    Ok(Ok(Ordering::Equal)) => {
                        exact_match = Some(i);
                        break;
                    }
                    Ok(Ok(Ordering::Less)) => match best_smaller {
                        None => best_smaller = Some((i, cell)),
                        Some((_, best_val)) => {
                            if matches!(
                                cell.compare_relational(best_val),
                                Ok(Ok(Ordering::Greater))
                            ) {
                                best_smaller = Some((i, cell));
                            }
                        }
                    },
                    Ok(Ok(Ordering::Greater)) | Ok(Err(_)) | Err(_) => {}
                }
            }

            if exact_match.is_some() {
                return Ok(exact_match);
            }
            Ok(best_smaller.map(|(idx, _)| idx))
        }
        MatchMode::ExactOrNextLarger => {
            let mut exact_match = None;
            let mut best_larger: Option<(usize, &CellValue)> = None;

            for (i, cell) in slice.iter().enumerate() {
                if matches!(cell, CellValue::Empty | CellValue::Error(_)) {
                    continue;
                }
                match cell.compare_relational(lookup_val) {
                    Ok(Ok(Ordering::Equal)) => {
                        exact_match = Some(i);
                        break;
                    }
                    Ok(Ok(Ordering::Greater)) => match best_larger {
                        None => best_larger = Some((i, cell)),
                        Some((_, best_val)) => {
                            if matches!(cell.compare_relational(best_val), Ok(Ok(Ordering::Less))) {
                                best_larger = Some((i, cell));
                            }
                        }
                    },
                    Ok(Ok(Ordering::Less)) | Ok(Err(_)) | Err(_) => {}
                }
            }

            if exact_match.is_some() {
                return Ok(exact_match);
            }
            Ok(best_larger.map(|(idx, _)| idx))
        }
    }
}

/// Performs linear reverse search (indices `slice.len() - 1` down to `0`), skipping `Empty` and `Error` cells.
fn find_linear_reverse(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    match match_mode {
        MatchMode::Exact => {
            for (i, cell) in slice.iter().enumerate().rev() {
                if matches!(cell, CellValue::Empty | CellValue::Error(_)) {
                    continue;
                }
                if let Ok(Ok(Ordering::Equal)) = cell.compare_relational(lookup_val) {
                    return Ok(Some(i));
                }
            }
            Ok(None)
        }
        MatchMode::ExactOrNextSmaller => {
            let mut exact_match = None;
            let mut best_smaller: Option<(usize, &CellValue)> = None;

            for (i, cell) in slice.iter().enumerate().rev() {
                if matches!(cell, CellValue::Empty | CellValue::Error(_)) {
                    continue;
                }
                match cell.compare_relational(lookup_val) {
                    Ok(Ok(Ordering::Equal)) => {
                        exact_match = Some(i);
                        break;
                    }
                    Ok(Ok(Ordering::Less)) => match best_smaller {
                        None => best_smaller = Some((i, cell)),
                        Some((_, best_val)) => {
                            if matches!(
                                cell.compare_relational(best_val),
                                Ok(Ok(Ordering::Greater))
                            ) {
                                best_smaller = Some((i, cell));
                            }
                        }
                    },
                    Ok(Ok(Ordering::Greater)) | Ok(Err(_)) | Err(_) => {}
                }
            }

            if exact_match.is_some() {
                return Ok(exact_match);
            }
            Ok(best_smaller.map(|(idx, _)| idx))
        }
        MatchMode::ExactOrNextLarger => {
            let mut exact_match = None;
            let mut best_larger: Option<(usize, &CellValue)> = None;

            for (i, cell) in slice.iter().enumerate().rev() {
                if matches!(cell, CellValue::Empty | CellValue::Error(_)) {
                    continue;
                }
                match cell.compare_relational(lookup_val) {
                    Ok(Ok(Ordering::Equal)) => {
                        exact_match = Some(i);
                        break;
                    }
                    Ok(Ok(Ordering::Greater)) => match best_larger {
                        None => best_larger = Some((i, cell)),
                        Some((_, best_val)) => {
                            if matches!(cell.compare_relational(best_val), Ok(Ok(Ordering::Less))) {
                                best_larger = Some((i, cell));
                            }
                        }
                    },
                    Ok(Ok(Ordering::Less)) | Ok(Err(_)) | Err(_) => {}
                }
            }

            if exact_match.is_some() {
                return Ok(exact_match);
            }
            Ok(best_larger.map(|(idx, _)| idx))
        }
    }
}

fn compare_binary_cell(cell: &CellValue, lookup_val: &CellValue) -> Result<Ordering, ExcelError> {
    match cell.compare_relational(lookup_val) {
        Ok(Ok(ord)) => Ok(ord),
        Ok(Err(err)) => Err(err),
        Err(_) => Err(ExcelError::Value),
    }
}

/// Performs binary search on validated monotonically non-decreasing `slice`.
fn find_binary_ascending(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    let mut low = 0;
    let mut high = slice.len();
    let mut best = None;

    while low < high {
        let mid = low + (high - low) / 2;
        let Some(cell) = slice.get(mid) else {
            break;
        };
        let ord = compare_binary_cell(cell, lookup_val)?;
        match match_mode {
            MatchMode::Exact => match ord {
                Ordering::Equal => return Ok(Some(mid)),
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
            },
            MatchMode::ExactOrNextSmaller => match ord {
                Ordering::Equal => return Ok(Some(mid)),
                Ordering::Less => {
                    best = Some(mid);
                    low = mid + 1;
                }
                Ordering::Greater => high = mid,
            },
            MatchMode::ExactOrNextLarger => match ord {
                Ordering::Equal => return Ok(Some(mid)),
                Ordering::Less => low = mid + 1,
                Ordering::Greater => {
                    best = Some(mid);
                    high = mid;
                }
            },
        }
    }

    Ok(best)
}

/// Performs binary search on validated monotonically non-increasing `slice`.
fn find_binary_descending(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    let mut low = 0;
    let mut high = slice.len();
    let mut best = None;

    while low < high {
        let mid = low + (high - low) / 2;
        let Some(cell) = slice.get(mid) else {
            break;
        };
        let ord = compare_binary_cell(cell, lookup_val)?;
        match match_mode {
            MatchMode::Exact => match ord {
                Ordering::Equal => return Ok(Some(mid)),
                Ordering::Greater => low = mid + 1,
                Ordering::Less => high = mid,
            },
            MatchMode::ExactOrNextSmaller => match ord {
                Ordering::Equal => return Ok(Some(mid)),
                Ordering::Greater => low = mid + 1,
                Ordering::Less => {
                    best = Some(mid);
                    high = mid;
                }
            },
            MatchMode::ExactOrNextLarger => match ord {
                Ordering::Equal => return Ok(Some(mid)),
                Ordering::Greater => {
                    best = Some(mid);
                    low = mid + 1;
                }
                Ordering::Less => high = mid,
            },
        }
    }

    Ok(best)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test code")]
mod tests {
    use super::*;
    use crate::functions::traits::ReferenceKind;

    #[test]
    fn test_exact_match_linear() {
        let items = vec![
            CellValue::Text("apple".into()),
            CellValue::Text("banana".into()),
            CellValue::Text("cherry".into()),
        ];
        let res = find_match_index(
            &CellValue::Text("BANANA".into()),
            &items,
            MatchMode::Exact,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res, Some(1));
    }

    #[test]
    fn test_reverse_linear_search() {
        let items = vec![
            CellValue::Text("A".into()),
            CellValue::Text("B".into()),
            CellValue::Text("A".into()),
        ];
        let fwd = find_match_index(
            &CellValue::Text("A".into()),
            &items,
            MatchMode::Exact,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(fwd, Some(0));

        let rev = find_match_index(
            &CellValue::Text("A".into()),
            &items,
            MatchMode::Exact,
            SearchMode::LastToFirst,
        )
        .unwrap();
        assert_eq!(rev, Some(2));
    }

    #[test]
    fn test_approximate_smaller_and_larger() {
        let items = vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ];

        let smaller = find_match_index(
            &CellValue::Number(25.0),
            &items,
            MatchMode::ExactOrNextSmaller,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(smaller, Some(1));

        let larger = find_match_index(
            &CellValue::Number(25.0),
            &items,
            MatchMode::ExactOrNextLarger,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(larger, Some(2));

        let none_smaller = find_match_index(
            &CellValue::Number(5.0),
            &items,
            MatchMode::ExactOrNextSmaller,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(none_smaller, None);
    }

    #[test]
    fn test_binary_search_ascending_and_descending() {
        let asc = vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
            CellValue::Number(40.0),
        ];
        validate_lookup_array(
            &asc,
            &[],
            Some(ValueType::Number),
            SearchMode::BinaryAscending,
            "XMATCH",
        )
        .unwrap();
        let res_asc = find_match_index(
            &CellValue::Number(30.0),
            &asc,
            MatchMode::Exact,
            SearchMode::BinaryAscending,
        )
        .unwrap();
        assert_eq!(res_asc, Some(2));

        let desc = vec![
            CellValue::Number(40.0),
            CellValue::Number(30.0),
            CellValue::Number(20.0),
            CellValue::Number(10.0),
        ];
        validate_lookup_array(
            &desc,
            &[],
            Some(ValueType::Number),
            SearchMode::BinaryDescending,
            "XMATCH",
        )
        .unwrap();
        let res_desc = find_match_index(
            &CellValue::Number(30.0),
            &desc,
            MatchMode::Exact,
            SearchMode::BinaryDescending,
        )
        .unwrap();
        assert_eq!(res_desc, Some(1));
    }

    #[test]
    fn test_linear_skips_sparse_empty_and_errors() {
        let items = vec![
            CellValue::Empty,
            CellValue::Error(ExcelError::Div0),
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Number(30.0),
        ];
        validate_lookup_array(
            &items,
            &[],
            Some(ValueType::Number),
            SearchMode::FirstToLast,
            "XLOOKUP",
        )
        .unwrap();

        let res_smaller = find_match_index(
            &CellValue::Number(25.0),
            &items,
            MatchMode::ExactOrNextSmaller,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res_smaller, Some(2));

        let res_larger = find_match_index(
            &CellValue::Number(25.0),
            &items,
            MatchMode::ExactOrNextLarger,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res_larger, Some(4));
    }

    #[test]
    fn test_validate_lookup_array_and_homogeneous_slice_strictness() {
        // Mixed-type slice fails even when lookup_value_type is None (e.g. #DIV/0! lookup_value)
        let mixed = vec![CellValue::Number(10.0), CellValue::Text("20".into())];
        assert_eq!(
            validate_lookup_array(&mixed, &[], None, SearchMode::FirstToLast, "XLOOKUP"),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "XLOOKUP",
                param: "lookup_array",
                position: 2,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            }))
        );

        // Typed error provenance in slice participates in homogeneity and binary search checks
        let num_and_err_txt = vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Na)];
        let err_types_txt = vec![None, Some(ValueType::Text)];
        assert_eq!(
            validate_lookup_array(
                &num_and_err_txt,
                &err_types_txt,
                Some(ValueType::Number),
                SearchMode::FirstToLast,
                "XLOOKUP",
            ),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "XLOOKUP",
                param: "lookup_array",
                position: 2,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            }))
        );
        assert_eq!(
            validate_lookup_array(
                &num_and_err_txt,
                &err_types_txt,
                Some(ValueType::Number),
                SearchMode::BinaryAscending,
                "XMATCH",
            ),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "XMATCH",
                param: "lookup_array",
                position: 2,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            }))
        );
        assert_eq!(
            validate_homogeneous_slice(&num_and_err_txt, &err_types_txt, "INDEX", "array", 1),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "INDEX",
                param: "array",
                position: 1,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            }))
        );
        let all_div0 = vec![
            CellValue::Error(ExcelError::Div0),
            CellValue::Error(ExcelError::Div0),
        ];
        let all_div0_types = vec![Some(ValueType::Number), Some(ValueType::Number)];
        assert_eq!(
            validate_homogeneous_slice(&all_div0, &all_div0_types, "INDEX", "array", 1),
            Ok(Some(ValueType::Number))
        );

        // Binary search rejects Empty, Error, and unsorted slices
        let with_empty = vec![
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Number(30.0),
        ];
        assert_eq!(
            validate_lookup_array(
                &with_empty,
                &[],
                Some(ValueType::Number),
                SearchMode::BinaryAscending,
                "XLOOKUP"
            ),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "XLOOKUP",
                param: "lookup_array",
                position: 2,
                expected: ExpectedType::NonEmpty,
                found: ValueType::Empty,
            }))
        );

        let with_err = vec![
            CellValue::Number(10.0),
            CellValue::Error(ExcelError::Div0),
            CellValue::Number(30.0),
        ];
        assert_eq!(
            validate_lookup_array(
                &with_err,
                &[None, Some(ValueType::Number), None],
                Some(ValueType::Number),
                SearchMode::BinaryAscending,
                "XLOOKUP"
            ),
            Err(FunctionError::ErrorInBinaryLookupArray {
                function: "XLOOKUP",
                search_mode: 2,
            })
        );

        let unsorted_asc = vec![
            CellValue::Number(10.0),
            CellValue::Number(30.0),
            CellValue::Number(20.0),
        ];
        assert_eq!(
            validate_lookup_array(
                &unsorted_asc,
                &[],
                Some(ValueType::Number),
                SearchMode::BinaryAscending,
                "XLOOKUP"
            ),
            Err(FunctionError::UnsortedBinaryLookupArray {
                function: "XLOOKUP",
                search_mode: 2,
            })
        );

        let unsorted_desc = vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ];
        assert_eq!(
            validate_lookup_array(
                &unsorted_desc,
                &[],
                Some(ValueType::Number),
                SearchMode::BinaryDescending,
                "XMATCH"
            ),
            Err(FunctionError::UnsortedBinaryLookupArray {
                function: "XMATCH",
                search_mode: -2,
            })
        );

        assert_eq!(
            validate_homogeneous_slice(&mixed, &[], "INDEX", "array", 1),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "INDEX",
                param: "array",
                position: 1,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            }))
        );
    }

    #[test]
    fn test_extract_1d_range_slice_and_strict_modes() {
        let scalar = FunctionArg::Scalar(CellValue::Number(10.0));
        assert_eq!(
            extract_1d_range_slice(&scalar, "XMATCH", "lookup_array"),
            Err(FunctionError::InvalidArgumentKind {
                function: "XMATCH",
                expected: "range",
                found: "scalar",
            })
        );

        let mat = FunctionArg::array(2, 2, vec![CellValue::Number(1.0); 4], ReferenceKind::Slice);
        assert_eq!(
            extract_1d_range_slice(&mat, "XMATCH", "lookup_array"),
            Err(FunctionError::ExpectedOneDimensionalSlice {
                function: "XMATCH",
                param: "lookup_array",
                rows: 2,
                cols: 2,
            })
        );

        assert_eq!(
            MatchMode::parse(&CellValue::Number(1.5), "XMATCH", "match_mode", 3),
            Err(FunctionError::Type(TypeError::NonIntegerArgument {
                function: "XMATCH",
                param: "match_mode",
                position: 3,
                value: "1.5".into(),
            }))
        );
        assert_eq!(
            MatchMode::parse(&CellValue::Number(2.0), "XLOOKUP", "match_mode", 5),
            Err(FunctionError::InvalidLookupMode {
                function: "XLOOKUP",
                param: "match_mode",
                position: 5,
                mode: 2,
            })
        );
        assert_eq!(
            SearchMode::parse(&CellValue::Boolean(true), "XMATCH", "search_mode", 4),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "XMATCH",
                param: "search_mode",
                position: 4,
                expected: ExpectedType::Integer,
                found: ValueType::Boolean,
            }))
        );
        assert_eq!(
            SearchMode::parse(&CellValue::Number(99.0), "XMATCH", "search_mode", 4),
            Err(FunctionError::InvalidLookupMode {
                function: "XMATCH",
                param: "search_mode",
                position: 4,
                mode: 99,
            })
        );
    }
}
