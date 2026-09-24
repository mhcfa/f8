//! Excel cell and range address formatting with relative/absolute `$` pinning.

use crate::compiler::layout::a1::col_to_excel_col;
use crate::model::id::SheetId;

/// Formats a worksheet identifier into a single-quoted Excel sheet qualifier (e.g. `'Summary'!`).
///
/// Internal single quotes within sheet names are escaped by doubling them (e.g. `'O''Reilly'!`).
#[must_use]
pub fn format_sheet_qualifier(sheet: &SheetId) -> String {
    let escaped = sheet.as_str().replace('\'', "''");
    format!("'{escaped}'!")
}

/// Formats 0-indexed `(col, row)` coordinates into an Excel A1 address with optional `$` pinning.
///
/// # Examples
/// - `(2, 9, false, false)` -> `"C10"`
/// - `(2, 9, true, true)` -> `"$C$10"`
/// - `(2, 13, false, true)` -> `"C$14"`
/// - `(2, 4, true, false)` -> `"$C5"`
#[must_use]
pub fn format_cell_address(col: u32, row: u32, col_pinned: bool, row_pinned: bool) -> String {
    let col_str = col_to_excel_col(col);
    let row_num = row + 1;
    let col_prefix = if col_pinned { "$" } else { "" };
    let row_prefix = if row_pinned { "$" } else { "" };
    format!("{col_prefix}{col_str}{row_prefix}{row_num}")
}

/// Formats a single physical cell coordinate, prepending a sheet qualifier if targeting a different worksheet.
#[must_use]
pub fn format_single_cell(
    sheet: &SheetId,
    col: u32,
    row: u32,
    col_pinned: bool,
    row_pinned: bool,
    origin_sheet: &SheetId,
) -> String {
    let addr = format_cell_address(col, row, col_pinned, row_pinned);
    if sheet == origin_sheet {
        addr
    } else {
        format!("{}{}", format_sheet_qualifier(sheet), addr)
    }
}

/// Formats a rectangular range, prepending a sheet qualifier to the first coordinate if targeting a different worksheet.
#[must_use]
pub fn format_range(
    sheet: &SheetId,
    start: (u32, u32),
    end: (u32, u32),
    col_pinned: bool,
    row_pinned: bool,
    origin_sheet: &SheetId,
) -> String {
    let start_addr = format_cell_address(start.0, start.1, col_pinned, row_pinned);
    let end_addr = format_cell_address(end.0, end.1, col_pinned, row_pinned);
    if sheet == origin_sheet {
        format!("{start_addr}:{end_addr}")
    } else {
        format!("{}{start_addr}:{end_addr}", format_sheet_qualifier(sheet))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cell_address() {
        assert_eq!(format_cell_address(2, 9, false, false), "C10");
        assert_eq!(format_cell_address(2, 9, true, true), "$C$10");
        assert_eq!(format_cell_address(2, 13, false, true), "C$14");
        assert_eq!(format_cell_address(2, 4, true, false), "$C5");
        assert_eq!(format_cell_address(0, 0, false, false), "A1");
        assert_eq!(format_cell_address(26, 99, true, true), "$AA$100");
    }

    #[test]
    fn test_format_single_cell_sheet_qualification() {
        let sheet_a = SheetId::new("Model");
        let sheet_b = SheetId::new("Assumptions");

        assert_eq!(
            format_single_cell(&sheet_a, 2, 9, false, false, &sheet_a),
            "C10"
        );
        assert_eq!(
            format_single_cell(&sheet_b, 1, 3, true, true, &sheet_a),
            "'Assumptions'!$B$4"
        );
    }

    #[test]
    fn test_format_range() {
        let sheet_a = SheetId::new("Model");
        let sheet_b = SheetId::new("Data");

        assert_eq!(
            format_range(&sheet_a, (2, 9), (6, 9), true, true, &sheet_a),
            "$C$10:$G$10"
        );
        assert_eq!(
            format_range(&sheet_a, (2, 13), (2, 17), false, true, &sheet_a),
            "C$14:C$18"
        );
        assert_eq!(
            format_range(&sheet_b, (2, 13), (6, 17), true, true, &sheet_a),
            "'Data'!$C$14:$G$18"
        );
    }

    #[test]
    fn test_sheet_name_quote_escaping() {
        let sheet = SheetId::new("John's Data");
        assert_eq!(format_sheet_qualifier(&sheet), "'John''s Data'!");
    }
}
