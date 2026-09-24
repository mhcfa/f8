//! Shared 1D vector search and matching engine for lookup functions (`XLOOKUP`, `XMATCH`).
//!
//! Implements linear scan, reverse scan, binary search, approximate matching bounds,
//! and wildcard compilation conforming to Microsoft Excel 365 / 2021 parity.

use crate::functions::common::{matches_wildcard, parse_wildcard_tokens};
use crate::functions::traits::FunctionArg;
use crate::value::{CellValue, ExcelError};

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
    /// Wildcard match with `*`, `?`, and `~` (`2`).
    Wildcard,
}

impl MatchMode {
    /// Parses a `CellValue` into a `MatchMode`.
    pub fn parse(val: &CellValue) -> Result<Self, ExcelError> {
        match val {
            CellValue::Empty => Ok(Self::Exact),
            _ => {
                let n = val.coerce_to_number()?;
                if !n.is_finite() {
                    return Err(ExcelError::Value);
                }
                let int_val = n.trunc() as i64;
                match int_val {
                    0 => Ok(Self::Exact),
                    -1 => Ok(Self::ExactOrNextSmaller),
                    1 => Ok(Self::ExactOrNextLarger),
                    2 => Ok(Self::Wildcard),
                    _ => Err(ExcelError::Value),
                }
            }
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
    /// Binary search assuming ascending order (`2`).
    BinaryAscending,
    /// Binary search assuming descending order (`-2`).
    BinaryDescending,
}

impl SearchMode {
    /// Parses a `CellValue` into a `SearchMode`.
    pub fn parse(val: &CellValue) -> Result<Self, ExcelError> {
        match val {
            CellValue::Empty => Ok(Self::FirstToLast),
            _ => {
                let n = val.coerce_to_number()?;
                if !n.is_finite() {
                    return Err(ExcelError::Value);
                }
                let int_val = n.trunc() as i64;
                match int_val {
                    1 => Ok(Self::FirstToLast),
                    -1 => Ok(Self::LastToFirst),
                    2 => Ok(Self::BinaryAscending),
                    -2 => Ok(Self::BinaryDescending),
                    _ => Err(ExcelError::Value),
                }
            }
        }
    }
}

/// Strict type-aware equality for lookup functions (`XLOOKUP`, `XMATCH`).
///
/// Unlike general formula equality (`CellValue::excel_eq`), lookup equality requires
/// matching variants so that `Empty` does not match `0` or `""`, and numbers never match strings.
#[must_use]
pub fn lookup_eq(a: &CellValue, b: &CellValue) -> bool {
    match (a, b) {
        (CellValue::Number(x), CellValue::Number(y)) => x == y,
        (CellValue::Text(x), CellValue::Text(y)) => {
            crate::value::compare_ascii_case_insensitive(x, y).is_eq()
        }
        (CellValue::Boolean(x), CellValue::Boolean(y)) => x == y,
        (CellValue::Empty, CellValue::Empty) => true,
        _ => false,
    }
}

/// Extracts a 1D slice of `CellValue`s from a `FunctionArg`, enforcing that 2D matrix grids
/// with both `rows > 1` and `cols > 1` are rejected with `ExcelError::Value`, and scalar errors
/// bubble immediately.
pub fn extract_1d_slice<'a>(arg: &'a FunctionArg<'_>) -> Result<&'a [CellValue], ExcelError> {
    match arg {
        FunctionArg::Array {
            rows, cols, values, ..
        } => {
            if *rows > 1 && *cols > 1 {
                return Err(ExcelError::Value);
            }
            Ok(values.as_slice())
        }
        FunctionArg::Scalar(val) => {
            if let CellValue::Error(err) = val {
                return Err(*err);
            }
            Ok(std::slice::from_ref(val))
        }
        _ => Err(ExcelError::Value),
    }
}

/// Searches `slice` for `lookup_val` using the specified `match_mode` and `search_mode`.
///
/// Returns `Ok(Some(index))` with 0-based index if found, `Ok(None)` if not found,
/// or `Err(ExcelError)` on invalid configuration or input errors.
pub fn find_match_index(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
    search_mode: SearchMode,
) -> Result<Option<usize>, ExcelError> {
    // If lookup value is an error, bubble it up immediately
    if let CellValue::Error(err) = lookup_val {
        return Err(*err);
    }

    // Binary search cannot be combined with wildcard match mode
    if match_mode == MatchMode::Wildcard
        && matches!(
            search_mode,
            SearchMode::BinaryAscending | SearchMode::BinaryDescending
        )
    {
        return Err(ExcelError::Value);
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

/// Performs linear forward search (indices `0..slice.len()`).
fn find_linear_forward(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    match match_mode {
        MatchMode::Exact => {
            for (i, cell) in slice.iter().enumerate() {
                if lookup_eq(cell, lookup_val) {
                    return Ok(Some(i));
                }
            }
            Ok(None)
        }
        MatchMode::Wildcard => match lookup_val {
            CellValue::Text(s) => {
                let (tokens, _) = parse_wildcard_tokens(s);
                for (i, cell) in slice.iter().enumerate() {
                    if let CellValue::Text(candidate) = cell
                        && matches_wildcard(&tokens, candidate)
                    {
                        return Ok(Some(i));
                    }
                }
                Ok(None)
            }
            _ => {
                for (i, cell) in slice.iter().enumerate() {
                    if lookup_eq(cell, lookup_val) {
                        return Ok(Some(i));
                    }
                }
                Ok(None)
            }
        },
        MatchMode::ExactOrNextSmaller => {
            let mut exact_match = None;
            let mut best_smaller: Option<(usize, &CellValue)> = None;

            for (i, cell) in slice.iter().enumerate() {
                if lookup_eq(cell, lookup_val) {
                    exact_match = Some(i);
                    break;
                }
                if std::mem::discriminant(cell) == std::mem::discriminant(lookup_val)
                    && cell.excel_cmp(lookup_val).is_lt()
                {
                    match best_smaller {
                        None => best_smaller = Some((i, cell)),
                        Some((_, best_val)) => {
                            if cell.excel_cmp(best_val).is_gt() {
                                best_smaller = Some((i, cell));
                            }
                        }
                    }
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
                if lookup_eq(cell, lookup_val) {
                    exact_match = Some(i);
                    break;
                }
                if std::mem::discriminant(cell) == std::mem::discriminant(lookup_val)
                    && cell.excel_cmp(lookup_val).is_gt()
                {
                    match best_larger {
                        None => best_larger = Some((i, cell)),
                        Some((_, best_val)) => {
                            if cell.excel_cmp(best_val).is_lt() {
                                best_larger = Some((i, cell));
                            }
                        }
                    }
                }
            }

            if exact_match.is_some() {
                return Ok(exact_match);
            }
            Ok(best_larger.map(|(idx, _)| idx))
        }
    }
}

/// Performs linear reverse search (indices `slice.len() - 1` down to `0`).
fn find_linear_reverse(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    match match_mode {
        MatchMode::Exact => {
            for (i, cell) in slice.iter().enumerate().rev() {
                if lookup_eq(cell, lookup_val) {
                    return Ok(Some(i));
                }
            }
            Ok(None)
        }
        MatchMode::Wildcard => match lookup_val {
            CellValue::Text(s) => {
                let (tokens, _) = parse_wildcard_tokens(s);
                for (i, cell) in slice.iter().enumerate().rev() {
                    if let CellValue::Text(candidate) = cell
                        && matches_wildcard(&tokens, candidate)
                    {
                        return Ok(Some(i));
                    }
                }
                Ok(None)
            }
            _ => {
                for (i, cell) in slice.iter().enumerate().rev() {
                    if lookup_eq(cell, lookup_val) {
                        return Ok(Some(i));
                    }
                }
                Ok(None)
            }
        },
        MatchMode::ExactOrNextSmaller => {
            let mut exact_match = None;
            let mut best_smaller: Option<(usize, &CellValue)> = None;

            for (i, cell) in slice.iter().enumerate().rev() {
                if lookup_eq(cell, lookup_val) {
                    exact_match = Some(i);
                    break;
                }
                if std::mem::discriminant(cell) == std::mem::discriminant(lookup_val)
                    && cell.excel_cmp(lookup_val).is_lt()
                {
                    match best_smaller {
                        None => best_smaller = Some((i, cell)),
                        Some((_, best_val)) => {
                            if cell.excel_cmp(best_val).is_gt() {
                                best_smaller = Some((i, cell));
                            }
                        }
                    }
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
                if lookup_eq(cell, lookup_val) {
                    exact_match = Some(i);
                    break;
                }
                if std::mem::discriminant(cell) == std::mem::discriminant(lookup_val)
                    && cell.excel_cmp(lookup_val).is_gt()
                {
                    match best_larger {
                        None => best_larger = Some((i, cell)),
                        Some((_, best_val)) => {
                            if cell.excel_cmp(best_val).is_lt() {
                                best_larger = Some((i, cell));
                            }
                        }
                    }
                }
            }

            if exact_match.is_some() {
                return Ok(exact_match);
            }
            Ok(best_larger.map(|(idx, _)| idx))
        }
    }
}

/// Performs binary search assuming ascending sort order.
fn find_binary_ascending(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    match match_mode {
        MatchMode::Exact => {
            let mut low = 0;
            let mut high = slice.len();
            while low < high {
                let mid = low + (high - low) / 2;
                match slice[mid].excel_cmp(lookup_val) {
                    std::cmp::Ordering::Equal => {
                        if lookup_eq(&slice[mid], lookup_val) {
                            return Ok(Some(mid));
                        }
                        return Ok(None);
                    }
                    std::cmp::Ordering::Less => low = mid + 1,
                    std::cmp::Ordering::Greater => high = mid,
                }
            }
            Ok(None)
        }
        MatchMode::ExactOrNextSmaller => {
            let mut low = 0;
            let mut high = slice.len();
            let mut best = None;
            while low < high {
                let mid = low + (high - low) / 2;
                match slice[mid].excel_cmp(lookup_val) {
                    std::cmp::Ordering::Equal => {
                        if lookup_eq(&slice[mid], lookup_val) {
                            return Ok(Some(mid));
                        }
                        return Ok(None);
                    }
                    std::cmp::Ordering::Less => {
                        if std::mem::discriminant(&slice[mid]) == std::mem::discriminant(lookup_val)
                        {
                            best = Some(mid);
                        }
                        low = mid + 1;
                    }
                    std::cmp::Ordering::Greater => {
                        high = mid;
                    }
                }
            }
            Ok(best)
        }
        MatchMode::ExactOrNextLarger => {
            let mut low = 0;
            let mut high = slice.len();
            let mut best = None;
            while low < high {
                let mid = low + (high - low) / 2;
                match slice[mid].excel_cmp(lookup_val) {
                    std::cmp::Ordering::Equal => {
                        if lookup_eq(&slice[mid], lookup_val) {
                            return Ok(Some(mid));
                        }
                        return Ok(None);
                    }
                    std::cmp::Ordering::Less => {
                        low = mid + 1;
                    }
                    std::cmp::Ordering::Greater => {
                        if std::mem::discriminant(&slice[mid]) == std::mem::discriminant(lookup_val)
                        {
                            best = Some(mid);
                        }
                        high = mid;
                    }
                }
            }
            Ok(best)
        }
        MatchMode::Wildcard => Err(ExcelError::Value),
    }
}

/// Performs binary search assuming descending sort order.
fn find_binary_descending(
    lookup_val: &CellValue,
    slice: &[CellValue],
    match_mode: MatchMode,
) -> Result<Option<usize>, ExcelError> {
    match match_mode {
        MatchMode::Exact => {
            let mut low = 0;
            let mut high = slice.len();
            while low < high {
                let mid = low + (high - low) / 2;
                match slice[mid].excel_cmp(lookup_val) {
                    std::cmp::Ordering::Equal => {
                        if lookup_eq(&slice[mid], lookup_val) {
                            return Ok(Some(mid));
                        }
                        return Ok(None);
                    }
                    std::cmp::Ordering::Greater => low = mid + 1,
                    std::cmp::Ordering::Less => high = mid,
                }
            }
            Ok(None)
        }
        MatchMode::ExactOrNextSmaller => {
            let mut low = 0;
            let mut high = slice.len();
            let mut best = None;
            while low < high {
                let mid = low + (high - low) / 2;
                match slice[mid].excel_cmp(lookup_val) {
                    std::cmp::Ordering::Equal => {
                        if lookup_eq(&slice[mid], lookup_val) {
                            return Ok(Some(mid));
                        }
                        return Ok(None);
                    }
                    std::cmp::Ordering::Greater => {
                        low = mid + 1;
                    }
                    std::cmp::Ordering::Less => {
                        if std::mem::discriminant(&slice[mid]) == std::mem::discriminant(lookup_val)
                        {
                            best = Some(mid);
                        }
                        high = mid;
                    }
                }
            }
            Ok(best)
        }
        MatchMode::ExactOrNextLarger => {
            let mut low = 0;
            let mut high = slice.len();
            let mut best = None;
            while low < high {
                let mid = low + (high - low) / 2;
                match slice[mid].excel_cmp(lookup_val) {
                    std::cmp::Ordering::Equal => {
                        if lookup_eq(&slice[mid], lookup_val) {
                            return Ok(Some(mid));
                        }
                        return Ok(None);
                    }
                    std::cmp::Ordering::Greater => {
                        if std::mem::discriminant(&slice[mid]) == std::mem::discriminant(lookup_val)
                        {
                            best = Some(mid);
                        }
                        low = mid + 1;
                    }
                    std::cmp::Ordering::Less => {
                        high = mid;
                    }
                }
            }
            Ok(best)
        }
        MatchMode::Wildcard => Err(ExcelError::Value),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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

        // 25 next smaller -> 20.0 (index 1)
        let smaller = find_match_index(
            &CellValue::Number(25.0),
            &items,
            MatchMode::ExactOrNextSmaller,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(smaller, Some(1));

        // 25 next larger -> 30.0 (index 2)
        let larger = find_match_index(
            &CellValue::Number(25.0),
            &items,
            MatchMode::ExactOrNextLarger,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(larger, Some(2));

        // 5 next smaller -> None
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
    fn test_wildcard_search() {
        let items = vec![
            CellValue::Text("Apple".into()),
            CellValue::Text("Apricot".into()),
            CellValue::Text("Banana".into()),
        ];
        let res = find_match_index(
            &CellValue::Text("Ap*".into()),
            &items,
            MatchMode::Wildcard,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res, Some(0));

        let rev = find_match_index(
            &CellValue::Text("Ap*".into()),
            &items,
            MatchMode::Wildcard,
            SearchMode::LastToFirst,
        )
        .unwrap();
        assert_eq!(rev, Some(1));
    }

    #[test]
    fn test_binary_search_ascending_and_descending() {
        let asc = vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
            CellValue::Number(40.0),
        ];
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
    fn test_wildcard_binary_rejected() {
        let items = vec![CellValue::Text("A".into())];
        let err = find_match_index(
            &CellValue::Text("A*".into()),
            &items,
            MatchMode::Wildcard,
            SearchMode::BinaryAscending,
        );
        assert_eq!(err, Err(ExcelError::Value));
    }

    #[test]
    fn test_lookup_eq_strict_types() {
        assert!(lookup_eq(&CellValue::Number(0.0), &CellValue::Number(0.0)));
        assert!(lookup_eq(&CellValue::Number(-0.0), &CellValue::Number(0.0)));
        assert!(!lookup_eq(&CellValue::Empty, &CellValue::Number(0.0)));
        assert!(!lookup_eq(&CellValue::Number(0.0), &CellValue::Empty));
        assert!(!lookup_eq(&CellValue::Empty, &CellValue::Text("".into())));
        assert!(!lookup_eq(&CellValue::Text("".into()), &CellValue::Empty));
        assert!(!lookup_eq(
            &CellValue::Number(1.0),
            &CellValue::Text("1".into())
        ));
        assert!(lookup_eq(&CellValue::Empty, &CellValue::Empty));
        assert!(lookup_eq(
            &CellValue::Text("ABC".into()),
            &CellValue::Text("abc".into())
        ));
    }

    #[test]
    fn test_exact_match_empty_and_zero_separation() {
        let items = vec![
            CellValue::Empty,
            CellValue::Number(0.0),
            CellValue::Text("".into()),
        ];
        // Searching for 0.0 matches index 1, not index 0 (Empty)
        let res_zero = find_match_index(
            &CellValue::Number(0.0),
            &items,
            MatchMode::Exact,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res_zero, Some(1));

        // Searching for "" matches index 2, not index 0 (Empty)
        let res_str = find_match_index(
            &CellValue::Text("".into()),
            &items,
            MatchMode::Exact,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res_str, Some(2));
    }

    #[test]
    fn test_wildcard_numeric_and_boolean_fallback() {
        let items = vec![
            CellValue::Number(42.0),
            CellValue::Boolean(true),
            CellValue::Text("42".into()),
        ];
        // Searching for number 42 with wildcard mode matches number at index 0
        let res_num = find_match_index(
            &CellValue::Number(42.0),
            &items,
            MatchMode::Wildcard,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res_num, Some(0));

        // Searching for boolean TRUE with wildcard mode matches boolean at index 1
        let res_bool = find_match_index(
            &CellValue::Boolean(true),
            &items,
            MatchMode::Wildcard,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res_bool, Some(1));
    }

    #[test]
    fn test_approximate_match_skips_heterogeneous_types_and_errors() {
        let items = vec![
            CellValue::Text("Zebra".into()),
            CellValue::Error(ExcelError::Div0),
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Boolean(true),
            CellValue::Number(30.0),
        ];

        // Next smaller than 25.0 should pick 10.0 (index 2), ignoring Text, Error, Empty, Boolean
        let res_smaller = find_match_index(
            &CellValue::Number(25.0),
            &items,
            MatchMode::ExactOrNextSmaller,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res_smaller, Some(2));

        // Next larger than 25.0 should pick 30.0 (index 5), ignoring Div0 and Zebra
        let res_larger = find_match_index(
            &CellValue::Number(25.0),
            &items,
            MatchMode::ExactOrNextLarger,
            SearchMode::FirstToLast,
        )
        .unwrap();
        assert_eq!(res_larger, Some(5));
    }

    #[test]
    fn test_extract_1d_slice_bubbles_scalar_errors() {
        let arg = FunctionArg::Scalar(CellValue::Error(ExcelError::Div0));
        assert_eq!(extract_1d_slice(&arg), Err(ExcelError::Div0));
    }
}
