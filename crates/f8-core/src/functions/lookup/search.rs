//! Shared 1D vector search and matching engine for lookup functions (`XLOOKUP`, `XMATCH`).
//!
//! Implements strict mode parsing, binary-search monotonicity checks,
//! linear forward/reverse scans, and binary ascending/descending bisection.

use std::cmp::Ordering;

use crate::functions::error::FunctionError;
use crate::value::{CellValue, ExcelError, ExpectedType, TypeError, ValueType};

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
}

/// Validates `lookup_array` (position `2`) for `XLOOKUP` and `XMATCH` under binary `search_mode` (`2` / `-2`).
///
/// Binary search requires every element to be non-`Empty` (`ExpectedType::NonEmpty`),
/// finite non-`Error` (`ErrorInBinaryLookupArray`), and monotonically sorted (`UnsortedBinaryLookupArray`)
/// under `CellValue::compare_relational`. Slice homogeneity and `lookup_value` compatibility are enforced
/// statically in Pass 4a.
pub fn validate_lookup_array(
    slice: &[CellValue],
    search_mode: SearchMode,
    function: &'static str,
) -> Result<(), FunctionError> {
    if !matches!(
        search_mode,
        SearchMode::BinaryAscending | SearchMode::BinaryDescending
    ) {
        return Ok(());
    }

    for cell in slice {
        if matches!(cell, CellValue::Empty) {
            return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param: "lookup_array",
                position: 2,
                expected: ExpectedType::NonEmpty,
                found: ValueType::Empty,
            }));
        }
    }

    let mode_int = search_mode.as_mode_int();
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
        SearchMode::FirstToLast => find_linear(lookup_val, slice, 0..slice.len(), match_mode),
        SearchMode::LastToFirst => {
            find_linear(lookup_val, slice, (0..slice.len()).rev(), match_mode)
        }
        SearchMode::BinaryAscending => find_binary_ascending(lookup_val, slice, match_mode),
        SearchMode::BinaryDescending => find_binary_descending(lookup_val, slice, match_mode),
    }
}

/// Performs linear search over `indices`, skipping `Empty` and `Error` cells.
fn find_linear<I>(
    lookup_val: &CellValue,
    slice: &[CellValue],
    indices: I,
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError>
where
    I: Iterator<Item = usize>,
{
    let target_ord = match match_mode {
        MatchMode::Exact => None,
        MatchMode::ExactOrNextSmaller => Some(Ordering::Less),
        MatchMode::ExactOrNextLarger => Some(Ordering::Greater),
    };
    let mut best_approx: Option<(usize, &CellValue)> = None;

    for i in indices {
        let Some(cell) = slice.get(i) else {
            continue;
        };
        if matches!(cell, CellValue::Empty | CellValue::Error(_)) {
            continue;
        }
        match cell.compare_relational(lookup_val) {
            Ok(Ok(Ordering::Equal)) => return Ok(Some(i)),
            Ok(Ok(ord)) if Some(ord) == target_ord => match best_approx {
                None => best_approx = Some((i, cell)),
                Some((_, best_val)) => {
                    if cell.compare_relational(best_val) == Ok(Ok(ord.reverse())) {
                        best_approx = Some((i, cell));
                    }
                }
            },
            Ok(Ok(_)) | Ok(Err(_)) | Err(_) => {}
        }
    }

    Ok(best_approx.map(|(idx, _)| idx))
}

fn compare_binary_cell(cell: &CellValue, lookup_val: &CellValue) -> Result<Ordering, ExcelError> {
    match cell.compare_relational(lookup_val) {
        Ok(Ok(ord)) => Ok(ord),
        Ok(Err(err)) => Err(err),
        Err(_) => Err(ExcelError::Value),
    }
}

fn is_exact_match_at(
    slice: &[CellValue],
    idx: Option<usize>,
    lookup_val: &CellValue,
) -> Result<bool, ExcelError> {
    match idx.and_then(|i| slice.get(i)) {
        Some(cell) => Ok(compare_binary_cell(cell, lookup_val)? == Ordering::Equal),
        None => Ok(false),
    }
}

/// Performs lower-bound binary search on validated monotonically non-decreasing `slice`.
///
/// Bisects `slice` at `low = min { i | slice[i] >= lookup_val }` (`0..=slice.len()`):
/// - `Exact`: first index equal to `lookup_val`, else `None`.
/// - `ExactOrNextSmaller`: first index equal to `lookup_val`, else `low - 1` (last `< lookup_val`).
/// - `ExactOrNextLarger`: `low` when `low < slice.len()` (first `>= lookup_val`), else `None`.
fn find_binary_ascending(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    let mut low = 0;
    let mut high = slice.len();

    while low < high {
        let mid = low + (high - low) / 2;
        let Some(cell) = slice.get(mid) else {
            break;
        };
        match compare_binary_cell(cell, lookup_val)? {
            Ordering::Less => low = mid + 1,
            Ordering::Equal | Ordering::Greater => high = mid,
        }
    }

    match match_mode {
        MatchMode::Exact => Ok(is_exact_match_at(slice, Some(low), lookup_val)?.then_some(low)),
        MatchMode::ExactOrNextSmaller => {
            if is_exact_match_at(slice, Some(low), lookup_val)? {
                Ok(Some(low))
            } else {
                Ok(low.checked_sub(1))
            }
        }
        MatchMode::ExactOrNextLarger => Ok((low < slice.len()).then_some(low)),
    }
}

/// Performs reverse-mirror binary search on validated monotonically non-increasing `slice`.
///
/// Bisects `slice` at `low = min { i | slice[i] < lookup_val }` (`0..=slice.len()`):
/// - `Exact`: last index equal to `lookup_val` (`low - 1`), else `None`.
/// - `ExactOrNextSmaller`: last index equal to `lookup_val` (`low - 1`), else `low` (first `< lookup_val`).
/// - `ExactOrNextLarger`: `low - 1` when `low > 0` (last `>= lookup_val`), else `None`.
fn find_binary_descending(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    let mut low = 0;
    let mut high = slice.len();

    while low < high {
        let mid = low + (high - low) / 2;
        let Some(cell) = slice.get(mid) else {
            break;
        };
        match compare_binary_cell(cell, lookup_val)? {
            Ordering::Greater | Ordering::Equal => low = mid + 1,
            Ordering::Less => high = mid,
        }
    }

    let prev = low.checked_sub(1);
    match match_mode {
        MatchMode::Exact => {
            let exact = is_exact_match_at(slice, prev, lookup_val)?;
            Ok(prev.filter(|_| exact))
        }
        MatchMode::ExactOrNextSmaller => {
            if is_exact_match_at(slice, prev, lookup_val)? {
                Ok(prev)
            } else {
                Ok((low < slice.len()).then_some(low))
            }
        }
        MatchMode::ExactOrNextLarger => Ok(prev),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test code")]
mod tests {
    use super::*;

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
        validate_lookup_array(&asc, SearchMode::BinaryAscending, "XMATCH").unwrap();
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
        validate_lookup_array(&desc, SearchMode::BinaryDescending, "XMATCH").unwrap();
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
    fn test_binary_search_duplicate_keys_and_boundaries() {
        let all_modes = [
            MatchMode::Exact,
            MatchMode::ExactOrNextSmaller,
            MatchMode::ExactOrNextLarger,
        ];

        let even_asc = vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ];
        for mode in all_modes {
            assert_eq!(
                find_match_index(
                    &CellValue::Number(20.0),
                    &even_asc,
                    mode,
                    SearchMode::BinaryAscending
                )
                .unwrap(),
                Some(1),
                "even_asc mode={mode:?}"
            );
        }

        let odd_asc = vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(20.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ];
        let odd_desc: Vec<CellValue> = odd_asc.iter().cloned().rev().collect();
        for mode in all_modes {
            assert_eq!(
                find_match_index(
                    &CellValue::Number(20.0),
                    &odd_asc,
                    mode,
                    SearchMode::BinaryAscending
                )
                .unwrap(),
                Some(1),
                "odd_asc mode={mode:?}"
            );
            assert_eq!(
                find_match_index(
                    &CellValue::Number(20.0),
                    &odd_desc,
                    mode,
                    SearchMode::BinaryDescending
                )
                .unwrap(),
                Some(3),
                "odd_desc mode={mode:?}"
            );
        }

        let blocks_asc = vec![
            CellValue::Number(10.0),
            CellValue::Number(10.0),
            CellValue::Number(30.0),
            CellValue::Number(30.0),
        ];
        let blocks_desc: Vec<CellValue> = blocks_asc.iter().cloned().rev().collect();

        let expected_asc = [
            (5.0, None, None, Some(0)),
            (10.0, Some(0), Some(0), Some(0)),
            (20.0, None, Some(1), Some(2)),
            (30.0, Some(2), Some(2), Some(2)),
            (35.0, None, Some(3), None),
        ];
        let n = blocks_asc.len();
        for (q, exp_exact, exp_smaller, exp_larger) in expected_asc {
            let qv = CellValue::Number(q);
            let cases = [
                (MatchMode::Exact, exp_exact),
                (MatchMode::ExactOrNextSmaller, exp_smaller),
                (MatchMode::ExactOrNextLarger, exp_larger),
            ];
            for (mode, exp) in cases {
                let got_asc =
                    find_match_index(&qv, &blocks_asc, mode, SearchMode::BinaryAscending).unwrap();
                assert_eq!(got_asc, exp, "blocks_asc q={q} mode={mode:?}");

                let got_desc =
                    find_match_index(&qv, &blocks_desc, mode, SearchMode::BinaryDescending)
                        .unwrap();
                assert_eq!(
                    got_desc,
                    exp.map(|idx| n - 1 - idx),
                    "blocks_desc mirror q={q} mode={mode:?}"
                );
            }
        }

        let all_same = vec![CellValue::Number(20.0); 4];
        for mode in all_modes {
            assert_eq!(
                find_match_index(
                    &CellValue::Number(20.0),
                    &all_same,
                    mode,
                    SearchMode::BinaryAscending
                )
                .unwrap(),
                Some(0)
            );
            assert_eq!(
                find_match_index(
                    &CellValue::Number(20.0),
                    &all_same,
                    mode,
                    SearchMode::BinaryDescending
                )
                .unwrap(),
                Some(3)
            );
            assert_eq!(
                find_match_index(
                    &CellValue::Number(20.0),
                    &[],
                    mode,
                    SearchMode::BinaryAscending
                )
                .unwrap(),
                None
            );
            assert_eq!(
                find_match_index(
                    &CellValue::Number(20.0),
                    &[],
                    mode,
                    SearchMode::BinaryDescending
                )
                .unwrap(),
                None
            );
        }

        let text_asc = vec![
            CellValue::Text("alpha".into()),
            CellValue::Text("BETA".into()),
            CellValue::Text("beta".into()),
            CellValue::Text("Beta".into()),
            CellValue::Text("gamma".into()),
        ];
        let text_desc: Vec<CellValue> = text_asc.iter().cloned().rev().collect();
        validate_lookup_array(&text_asc, SearchMode::BinaryAscending, "XLOOKUP").unwrap();
        validate_lookup_array(&text_desc, SearchMode::BinaryDescending, "XLOOKUP").unwrap();
        assert_eq!(
            find_match_index(
                &CellValue::Text("bEtA".into()),
                &text_asc,
                MatchMode::Exact,
                SearchMode::BinaryAscending
            )
            .unwrap(),
            Some(1)
        );
        assert_eq!(
            find_match_index(
                &CellValue::Text("bEtA".into()),
                &text_desc,
                MatchMode::Exact,
                SearchMode::BinaryDescending
            )
            .unwrap(),
            Some(3)
        );

        let bool_asc = vec![
            CellValue::Boolean(false),
            CellValue::Boolean(false),
            CellValue::Boolean(true),
            CellValue::Boolean(true),
        ];
        let bool_desc: Vec<CellValue> = bool_asc.iter().cloned().rev().collect();
        assert_eq!(
            find_match_index(
                &CellValue::Boolean(true),
                &bool_asc,
                MatchMode::Exact,
                SearchMode::BinaryAscending
            )
            .unwrap(),
            Some(2)
        );
        assert_eq!(
            find_match_index(
                &CellValue::Boolean(false),
                &bool_desc,
                MatchMode::Exact,
                SearchMode::BinaryDescending
            )
            .unwrap(),
            Some(3)
        );
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
        validate_lookup_array(&items, SearchMode::FirstToLast, "XLOOKUP").unwrap();

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
    fn test_validate_lookup_array_binary_strictness() {
        let with_empty = vec![
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Number(30.0),
        ];
        assert_eq!(
            validate_lookup_array(&with_empty, SearchMode::BinaryAscending, "XLOOKUP"),
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
            validate_lookup_array(&with_err, SearchMode::BinaryAscending, "XLOOKUP"),
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
            validate_lookup_array(&unsorted_asc, SearchMode::BinaryAscending, "XLOOKUP"),
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
            validate_lookup_array(&unsorted_desc, SearchMode::BinaryDescending, "XMATCH"),
            Err(FunctionError::UnsortedBinaryLookupArray {
                function: "XMATCH",
                search_mode: -2,
            })
        );
    }

    #[test]
    fn test_strict_modes() {
        assert_eq!(
            MatchMode::from_mode_int(2, "XLOOKUP", "match_mode", 5),
            Err(FunctionError::InvalidLookupMode {
                function: "XLOOKUP",
                param: "match_mode",
                position: 5,
                mode: 2,
            })
        );
        assert_eq!(
            SearchMode::from_mode_int(99, "XMATCH", "search_mode", 4),
            Err(FunctionError::InvalidLookupMode {
                function: "XMATCH",
                param: "search_mode",
                position: 4,
                mode: 99,
            })
        );
    }
}
