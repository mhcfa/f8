//! Excel A1 coordinate conversion and boundary validation.

use super::error::LayoutError;

/// Maximum column count supported by OpenXML / Excel (columns 0..=16,383, i.e. A..XFD).
pub const MAX_EXCEL_COLUMNS: u32 = 16_384;

/// Maximum row count supported by OpenXML / Excel (rows 0..=1,048,575, i.e. 1..1,048,576).
pub const MAX_EXCEL_ROWS: u32 = 1_048_576;

/// Maximum outline grouping level supported by native Excel (levels 1..=7).
pub const MAX_OUTLINE_LEVEL: usize = 7;

/// Converts a 0-indexed column integer into its canonical Excel letter representation.
///
/// # Examples
/// - `0` -> `"A"`
/// - `25` -> `"Z"`
/// - `26` -> `"AA"`
/// - `701` -> `"ZZ"`
/// - `702` -> `"AAA"`
/// - `16383` -> `"XFD"`
#[must_use]
pub fn col_to_excel_col(mut col: u32) -> String {
    let mut s = String::new();
    loop {
        let rem = (col % 26) as u8;
        s.push((b'A' + rem) as char);
        if col < 26 {
            break;
        }
        col = (col / 26) - 1;
    }
    s.chars().rev().collect()
}

/// Formats 0-indexed `(col, row)` coordinates into a standard Excel A1 string.
///
/// # Errors
/// Returns [`LayoutError::GridLimitExceeded`] if `col >= 16,384` or `row >= 1,048,576`.
pub fn coordinate_to_a1(col: u32, row: u32) -> Result<String, LayoutError> {
    if col >= MAX_EXCEL_COLUMNS {
        return Err(LayoutError::GridLimitExceeded {
            dimension: "columns",
            max: MAX_EXCEL_COLUMNS,
            actual: col,
        });
    }
    if row >= MAX_EXCEL_ROWS {
        return Err(LayoutError::GridLimitExceeded {
            dimension: "rows",
            max: MAX_EXCEL_ROWS,
            actual: row,
        });
    }

    Ok(format!("{}{}", col_to_excel_col(col), row + 1))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_col_to_excel_col() {
        assert_eq!(col_to_excel_col(0), "A");
        assert_eq!(col_to_excel_col(1), "B");
        assert_eq!(col_to_excel_col(25), "Z");
        assert_eq!(col_to_excel_col(26), "AA");
        assert_eq!(col_to_excel_col(27), "AB");
        assert_eq!(col_to_excel_col(51), "AZ");
        assert_eq!(col_to_excel_col(52), "BA");
        assert_eq!(col_to_excel_col(701), "ZZ");
        assert_eq!(col_to_excel_col(702), "AAA");
        assert_eq!(col_to_excel_col(16_383), "XFD");
    }

    #[test]
    fn test_coordinate_to_a1() {
        assert_eq!(coordinate_to_a1(0, 0).unwrap(), "A1");
        assert_eq!(coordinate_to_a1(1, 1).unwrap(), "B2");
        assert_eq!(coordinate_to_a1(2, 4).unwrap(), "C5");
        assert_eq!(coordinate_to_a1(25, 9).unwrap(), "Z10");
        assert_eq!(coordinate_to_a1(26, 99).unwrap(), "AA100");
        assert_eq!(coordinate_to_a1(16_383, 1_048_575).unwrap(), "XFD1048576");
    }

    #[test]
    fn test_coordinate_bounds_rejection() {
        assert!(matches!(
            coordinate_to_a1(16_384, 0),
            Err(LayoutError::GridLimitExceeded {
                dimension: "columns",
                max: 16_384,
                actual: 16_384,
            })
        ));

        assert!(matches!(
            coordinate_to_a1(0, 1_048_576),
            Err(LayoutError::GridLimitExceeded {
                dimension: "rows",
                max: 1_048_576,
                actual: 1_048_576,
            })
        ));
    }
}
