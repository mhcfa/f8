use f8_core::compiler::col_to_excel_col;
use f8_core::compiler::compiled::SheetInspection;
use f8_core::value::CellValue;

use crate::render::table::{TableFormat, ViewMode};

/// Formats a physical 2D sheet inspection matrix according to requested view mode and format.
#[must_use]
pub fn render_sheet(sheet: &SheetInspection, view: ViewMode, format: TableFormat) -> String {
    match format {
        TableFormat::Json => match serde_json::to_string_pretty(sheet) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        },
        TableFormat::Markdown => render_markdown_sheet(sheet, view),
        TableFormat::Table => render_unicode_sheet(sheet, view),
    }
}

fn str_width(s: &str) -> usize {
    s.chars().count()
}

struct RenderSheetCell {
    primary: String,
    secondary: Option<String>,
    is_numeric: bool,
}

fn format_sheet_cell(
    cell: &f8_core::compiler::compiled::SheetCellInspection,
    view: ViewMode,
) -> RenderSheetCell {
    use f8_core::compiler::compiled::SheetCellKind;

    match cell.kind {
        SheetCellKind::Empty | SheetCellKind::Gutter => RenderSheetCell {
            primary: String::new(),
            secondary: None,
            is_numeric: false,
        },
        SheetCellKind::TableTitle => {
            if !cell.value.is_empty() {
                let table_tag = cell
                    .table_id
                    .as_ref()
                    .map_or(String::new(), |t| format!(" [{t}]"));
                let title = cell.formatted_value.as_deref().unwrap_or_default();
                RenderSheetCell {
                    primary: format!("{title}{table_tag}"),
                    secondary: None,
                    is_numeric: false,
                }
            } else {
                RenderSheetCell {
                    primary: String::new(),
                    secondary: None,
                    is_numeric: false,
                }
            }
        }
        SheetCellKind::TableSubtitle => {
            if !cell.value.is_empty() {
                RenderSheetCell {
                    primary: cell.formatted_value.clone().unwrap_or_default(),
                    secondary: None,
                    is_numeric: false,
                }
            } else {
                RenderSheetCell {
                    primary: String::new(),
                    secondary: None,
                    is_numeric: false,
                }
            }
        }
        SheetCellKind::SectionHeader => {
            if !cell.value.is_empty() {
                RenderSheetCell {
                    primary: format!(
                        "**{}**",
                        cell.formatted_value.as_deref().unwrap_or_default()
                    ),
                    secondary: None,
                    is_numeric: false,
                }
            } else {
                RenderSheetCell {
                    primary: String::new(),
                    secondary: None,
                    is_numeric: false,
                }
            }
        }
        SheetCellKind::ColumnHeader => {
            if !cell.value.is_empty() {
                let text = cell.formatted_value.as_deref().unwrap_or_default();
                let col_tag = cell.col_id.as_ref().map_or(String::new(), |c| {
                    if text.eq_ignore_ascii_case(c.as_str()) {
                        String::new()
                    } else {
                        format!(" [{c}]")
                    }
                });
                RenderSheetCell {
                    primary: format!("{text}{col_tag}"),
                    secondary: None,
                    is_numeric: false,
                }
            } else {
                RenderSheetCell {
                    primary: String::new(),
                    secondary: None,
                    is_numeric: false,
                }
            }
        }
        SheetCellKind::Data => {
            if cell.is_descriptor {
                let label = cell.formatted_value.as_deref().unwrap_or_default();
                let row_tag = cell.row_id.as_ref().map_or(String::new(), |r| {
                    if label.eq_ignore_ascii_case(r.as_str()) {
                        String::new()
                    } else {
                        format!(" [{r}]")
                    }
                });
                RenderSheetCell {
                    primary: format!("{label}{row_tag}"),
                    secondary: None,
                    is_numeric: false,
                }
            } else {
                let is_numeric = matches!(cell.value, CellValue::Number(_));
                match view {
                    ViewMode::Values => RenderSheetCell {
                        primary: cell
                            .formatted_value
                            .clone()
                            .unwrap_or_else(|| cell.value.to_string()),
                        secondary: None,
                        is_numeric,
                    },
                    ViewMode::Formulas => {
                        let text = cell.excel_formula.clone().unwrap_or_else(|| {
                            cell.formatted_value
                                .clone()
                                .unwrap_or_else(|| cell.value.to_string())
                        });
                        RenderSheetCell {
                            primary: text,
                            secondary: None,
                            is_numeric: false,
                        }
                    }
                    ViewMode::Expr => {
                        let text = cell.formula_expr.clone().unwrap_or_else(|| {
                            cell.formatted_value
                                .clone()
                                .unwrap_or_else(|| cell.value.to_string())
                        });
                        RenderSheetCell {
                            primary: text,
                            secondary: None,
                            is_numeric: false,
                        }
                    }
                    ViewMode::Addresses => RenderSheetCell {
                        primary: cell.physical_a1.clone(),
                        secondary: None,
                        is_numeric: false,
                    },
                    ViewMode::Both => RenderSheetCell {
                        primary: cell
                            .formatted_value
                            .clone()
                            .unwrap_or_else(|| cell.value.to_string()),
                        secondary: cell.excel_formula.clone(),
                        is_numeric,
                    },
                }
            }
        }
    }
}

fn render_markdown_sheet(sheet: &SheetInspection, view: ViewMode) -> String {
    if sheet.rows.is_empty() {
        return format!("### Sheet: {} (Empty)\n", sheet.title);
    }

    let min_a1 = format!("{}{}", col_to_excel_col(sheet.min_col), sheet.min_row + 1);
    let max_a1 = format!("{}{}", col_to_excel_col(sheet.max_col), sheet.max_row + 1);

    let mut out = String::new();
    out.push_str(&format!(
        "### Sheet: {} (Range: {}:{})\n",
        sheet.title, min_a1, max_a1
    ));
    if !sheet.tables.is_empty() {
        let tables_str: Vec<_> = sheet.tables.iter().map(|t| t.as_str()).collect();
        out.push_str(&format!("Tables: {}\n\n", tables_str.join(", ")));
    } else {
        out.push('\n');
    }

    // Header row: Row | Col B | Col C | ...
    out.push_str("| Row |");
    for c in sheet.min_col..=sheet.max_col {
        out.push_str(&format!(" Col {} |", col_to_excel_col(c)));
    }
    out.push('\n');

    // Alignment row
    out.push_str("| :--- |");
    for _ in sheet.min_col..=sheet.max_col {
        out.push_str(" :--- |");
    }
    out.push('\n');

    // Data rows
    for row in &sheet.rows {
        out.push_str(&format!("| {} |", row.excel_row));
        for cell in &row.cells {
            let rendered = format_sheet_cell(cell, view);
            let text = match (&rendered.secondary, view) {
                (Some(sec), ViewMode::Both) => format!("{}<br>`{}`", rendered.primary, sec),
                _ => rendered.primary,
            };
            out.push_str(&format!(" {text} |"));
        }
        out.push('\n');
    }

    out
}

fn render_unicode_sheet(sheet: &SheetInspection, view: ViewMode) -> String {
    if sheet.rows.is_empty() {
        return format!("Sheet: {} (Empty)\n", sheet.title);
    }

    let min_a1 = format!("{}{}", col_to_excel_col(sheet.min_col), sheet.min_row + 1);
    let max_a1 = format!("{}{}", col_to_excel_col(sheet.max_col), sheet.max_row + 1);

    let num_cols = (sheet.max_col - sheet.min_col + 1) as usize;
    // column 0 is "Row", columns 1..=num_cols are sheet columns
    let mut col_widths = vec![4; num_cols + 1]; // "Row" is 3 chars, default min 4

    // Header labels
    let mut col_headers = Vec::with_capacity(num_cols + 1);
    col_headers.push("Row".to_string());
    for (i, c) in (sheet.min_col..=sheet.max_col).enumerate() {
        let label = format!("Col {}", col_to_excel_col(c));
        col_widths[i + 1] = str_width(&label).max(4);
        col_headers.push(label);
    }

    // Inspect cells to update widths
    for row in &sheet.rows {
        let row_num_str = row.excel_row.to_string();
        col_widths[0] = col_widths[0].max(str_width(&row_num_str));

        for (i, cell) in row.cells.iter().enumerate() {
            let rendered = format_sheet_cell(cell, view);
            let w1 = str_width(&rendered.primary);
            let w2 = rendered.secondary.as_ref().map_or(0, |s| str_width(s));
            let max_w = w1.max(w2);
            if max_w > col_widths[i + 1] {
                col_widths[i + 1] = max_w;
            }
        }
    }

    let mut out = String::new();
    out.push_str(&format!(
        "Sheet: {} (Range: {}:{})\n",
        sheet.title, min_a1, max_a1
    ));
    if !sheet.tables.is_empty() {
        let tables_str: Vec<_> = sheet.tables.iter().map(|t| t.as_str()).collect();
        out.push_str(&format!("Tables: {}\n", tables_str.join(", ")));
    }

    // Top border
    out.push('┌');
    for (i, &w) in col_widths.iter().enumerate() {
        if i > 0 {
            out.push('┬');
        }
        out.push_str(&"─".repeat(w + 2));
    }
    out.push_str("┐\n");

    // Header row
    out.push('│');
    for (i, header) in col_headers.iter().enumerate() {
        let w = col_widths[i];
        if i == 0 {
            out.push_str(&format!(" {:>w$} │", header));
        } else {
            out.push_str(&format!(" {:<w$} │", header));
        }
    }
    out.push('\n');

    // Header separator
    out.push('├');
    for (i, &w) in col_widths.iter().enumerate() {
        if i > 0 {
            out.push('┼');
        }
        out.push_str(&"─".repeat(w + 2));
    }
    out.push_str("┤\n");

    // Data rows
    for row in &sheet.rows {
        // Line 1: primary content
        out.push('│');
        let row_num_str = row.excel_row.to_string();
        let w0 = col_widths[0];
        out.push_str(&format!(" {:>w0$} │", row_num_str));

        for (i, cell) in row.cells.iter().enumerate() {
            let rendered = format_sheet_cell(cell, view);
            let w = col_widths[i + 1];
            if rendered.is_numeric {
                out.push_str(&format!(" {:>w$} │", rendered.primary));
            } else {
                out.push_str(&format!(" {:<w$} │", rendered.primary));
            }
        }
        out.push('\n');

        // Line 2: secondary content (ViewMode::Both)
        if view == ViewMode::Both {
            let has_secondary = row
                .cells
                .iter()
                .any(|c| format_sheet_cell(c, view).secondary.is_some());

            if has_secondary {
                out.push('│');
                out.push_str(&format!(" {:>w0$} │", ""));

                for (i, cell) in row.cells.iter().enumerate() {
                    let rendered = format_sheet_cell(cell, view);
                    let w = col_widths[i + 1];
                    let sec = rendered.secondary.as_deref().unwrap_or("");
                    out.push_str(&format!(" {:<w$} │", sec));
                }
                out.push('\n');
            }
        }
    }

    // Bottom border
    out.push('└');
    for (i, &w) in col_widths.iter().enumerate() {
        if i > 0 {
            out.push('┴');
        }
        out.push_str(&"─".repeat(w + 2));
    }
    out.push_str("┘\n");

    out
}
