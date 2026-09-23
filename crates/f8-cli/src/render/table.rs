use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use f8_core::compiler::compiled::TableInspection;
use f8_core::compiler::ir::id::CellId;
use f8_core::model::format::{CurrencyCode, FormatPreset};
use f8_core::value::CellValue;

/// View mode for table cell content rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// Evaluated numbers with accounting typography.
    #[default]
    Values,
    /// Live formula expressions for calculations, literals for inputs.
    Formulas,
    /// Dual stacked view showing evaluated value and formula.
    Both,
}

/// Output format for 2D table grid rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TableFormat {
    /// Aligned Unicode terminal box-drawing table.
    #[default]
    Table,
    /// GitHub Flavored Markdown table.
    Markdown,
    /// Machine-readable JSON representation.
    Json,
}

/// Formats a 2D table inspection matrix according to the requested view mode and format.
#[must_use]
pub fn render_table(table: &TableInspection, view: ViewMode, format: TableFormat) -> String {
    match format {
        TableFormat::Json => match serde_json::to_string_pretty(table) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"{e}\"}}"),
        },
        TableFormat::Markdown => render_markdown_table(table, view),
        TableFormat::Table => render_unicode_table(table, view),
    }
}

/// Formats a cell value according to accounting rules and format presets.
#[must_use]
pub fn format_cell_value(
    value: &CellValue,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
) -> String {
    match value {
        CellValue::Number(n) => format_accounting_number(*n, format, decimals, currency),
        CellValue::Text(s) => s.clone(),
        CellValue::Boolean(b) => {
            if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        CellValue::Empty => String::new(),
        CellValue::Error(e) => format!("#{e}"),
    }
}

/// Formats an accounting number with parentheses for negative values, commas, and currency/percent symbols.
#[must_use]
pub fn format_accounting_number(
    val: f64,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
) -> String {
    let is_neg = val < 0.0 && val.abs() > 1e-9;
    let abs_val = val.abs();

    let dec = match format {
        Some(FormatPreset::Percentage) => decimals.unwrap_or(1) as usize,
        Some(FormatPreset::Multiple) => decimals.unwrap_or(1) as usize,
        Some(FormatPreset::BasisPoints) => decimals.unwrap_or(0) as usize,
        Some(FormatPreset::Currency) => decimals.unwrap_or(1) as usize,
        Some(FormatPreset::Number) => decimals.unwrap_or(0) as usize,
        _ => decimals
            .map(|d| d as usize)
            .unwrap_or(if abs_val.fract().abs() > 1e-6 { 1 } else { 0 }),
    };

    let (scaled_val, suffix) = match format {
        Some(FormatPreset::Percentage) => (abs_val * 100.0, "%"),
        Some(FormatPreset::Multiple) => (abs_val, "x"),
        Some(FormatPreset::BasisPoints) => (abs_val * 10000.0, " bps"),
        _ => (abs_val, ""),
    };

    let formatted_num = format_with_commas(scaled_val, dec);

    match format {
        Some(FormatPreset::Currency) => {
            let sym = currency.unwrap_or_default().symbol();
            if is_neg {
                format!("({sym} {formatted_num})")
            } else {
                format!("{sym} {formatted_num}")
            }
        }
        Some(FormatPreset::Percentage)
        | Some(FormatPreset::Multiple)
        | Some(FormatPreset::BasisPoints) => {
            if is_neg {
                format!("({formatted_num}{suffix})")
            } else {
                format!("{formatted_num}{suffix}")
            }
        }
        _ => {
            if is_neg {
                format!("({formatted_num})")
            } else {
                formatted_num
            }
        }
    }
}

fn format_with_commas(val: f64, decimals: usize) -> String {
    let raw = format!("{val:.decimals$}");
    let parts: Vec<&str> = raw.split('.').collect();
    let int_part = parts[0];
    let frac_part = parts.get(1);

    let mut result = String::new();
    let chars: Vec<char> = int_part.chars().collect();
    let len = chars.len();
    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(ch);
    }
    if let Some(frac) = frac_part {
        result.push('.');
        result.push_str(frac);
    }
    result
}

fn str_width(s: &str) -> usize {
    s.chars().count()
}

struct RenderColumn {
    id: Option<String>,
    header: String,
    is_descriptor: bool,
}

fn extract_render_columns(table: &TableInspection) -> Vec<RenderColumn> {
    let mut cols = Vec::new();
    for (i, col) in table.columns.iter().enumerate() {
        if col.is_gutter {
            continue;
        }
        let header = col
            .headers
            .last()
            .cloned()
            .or_else(|| col.id.as_ref().map(ToString::to_string))
            .unwrap_or_default();
        let is_descriptor = i == 0
            || col
                .headers
                .first()
                .is_some_and(|h| h == "Line Item" || h == "Parameter")
            || col.id.as_ref().is_some_and(|id| id.as_str() == "line_item");
        cols.push(RenderColumn {
            id: col.id.as_ref().map(ToString::to_string),
            header,
            is_descriptor,
        });
    }
    cols
}

struct RenderCell {
    primary: String,
    secondary: Option<String>,
}

fn get_cell_text(
    table: &TableInspection,
    row_id: &str,
    col: &RenderColumn,
    view: ViewMode,
) -> RenderCell {
    let Some(col_id) = &col.id else {
        return RenderCell {
            primary: String::new(),
            secondary: None,
        };
    };

    let cell_id = CellId::new(table.table_id.clone(), row_id, col_id.as_str());

    let Some(cell) = table.cells.get(&cell_id) else {
        return RenderCell {
            primary: String::new(),
            secondary: None,
        };
    };

    let val_str = format_cell_value(&cell.value, cell.format, cell.decimals, cell.currency);
    let val_with_indent = if col.is_descriptor {
        let row_indent = table
            .rows
            .iter()
            .find(|r| r.id.as_ref().is_some_and(|id| id.as_str() == row_id))
            .map_or(0, |r| r.indent);
        format!("{}{}", "  ".repeat(row_indent as usize), val_str)
    } else {
        val_str
    };

    match view {
        ViewMode::Values => RenderCell {
            primary: val_with_indent,
            secondary: None,
        },
        ViewMode::Formulas => {
            let formula_text = if cell.is_formula {
                cell.formula_expr.clone().unwrap_or_default()
            } else {
                val_with_indent
            };
            RenderCell {
                primary: formula_text,
                secondary: None,
            }
        }
        ViewMode::Both => {
            let formula_text = if cell.is_formula {
                Some(format!(
                    "={}",
                    cell.formula_expr.as_deref().unwrap_or_default()
                ))
            } else {
                None
            };
            RenderCell {
                primary: val_with_indent,
                secondary: formula_text,
            }
        }
    }
}

fn render_unicode_table(table: &TableInspection, view: ViewMode) -> String {
    let render_cols = extract_render_columns(table);
    if render_cols.is_empty() {
        return String::new();
    }

    let mut col_widths: Vec<usize> = render_cols
        .iter()
        .map(|c| str_width(&c.header).max(4))
        .collect();

    for row in &table.rows {
        if row.is_gutter || row.is_section_header {
            continue;
        }
        let Some(row_id) = &row.id else { continue };
        for (col_idx, col) in render_cols.iter().enumerate() {
            let cell = get_cell_text(table, row_id.as_str(), col, view);
            let w1 = str_width(&cell.primary);
            let w2 = cell.secondary.as_ref().map_or(0, |s| str_width(s));
            let max_w = w1.max(w2);
            if max_w > col_widths[col_idx] {
                col_widths[col_idx] = max_w;
            }
        }
    }

    let total_inner_width: usize =
        col_widths.iter().map(|w| w + 2).sum::<usize>() + (render_cols.len().saturating_sub(1));

    let mut out = String::new();

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
    for (i, col) in render_cols.iter().enumerate() {
        let w = col_widths[i];
        if col.is_descriptor {
            out.push_str(&format!(" {:<w$} │", col.header));
        } else {
            out.push_str(&format!(" {:>w$} │", col.header));
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
    for row in &table.rows {
        if row.is_gutter {
            continue;
        }
        if row.is_section_header {
            let title = row.section_title.as_deref().unwrap_or_default();
            let title_width = total_inner_width.saturating_sub(2);
            out.push_str(&format!("│ {:<title_width$} │\n", title));
            out.push('├');
            for (i, &w) in col_widths.iter().enumerate() {
                if i > 0 {
                    out.push('┼');
                }
                out.push_str(&"─".repeat(w + 2));
            }
            out.push_str("┤\n");
            continue;
        }

        let Some(row_id) = &row.id else { continue };

        // Line 1: Primary cell text
        out.push('│');
        for (i, col) in render_cols.iter().enumerate() {
            let cell = get_cell_text(table, row_id.as_str(), col, view);
            let w = col_widths[i];
            if col.is_descriptor {
                out.push_str(&format!(" {:<w$} │", cell.primary));
            } else {
                out.push_str(&format!(" {:>w$} │", cell.primary));
            }
        }
        out.push('\n');

        // Line 2 (only for ViewMode::Both if at least one cell has secondary content)
        if view == ViewMode::Both {
            let has_any_secondary = render_cols.iter().any(|col| {
                get_cell_text(table, row_id.as_str(), col, view)
                    .secondary
                    .is_some()
            });

            if has_any_secondary {
                out.push('│');
                for (i, col) in render_cols.iter().enumerate() {
                    let cell = get_cell_text(table, row_id.as_str(), col, view);
                    let w = col_widths[i];
                    let sec = cell.secondary.as_deref().unwrap_or("");
                    if col.is_descriptor {
                        out.push_str(&format!(" {:<w$} │", sec));
                    } else {
                        out.push_str(&format!(" {:>w$} │", sec));
                    }
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

fn render_markdown_table(table: &TableInspection, view: ViewMode) -> String {
    let render_cols = extract_render_columns(table);
    if render_cols.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    // Header row
    out.push('|');
    for col in &render_cols {
        out.push_str(&format!(" {} |", col.header));
    }
    out.push('\n');

    // Alignment row
    out.push('|');
    for col in &render_cols {
        if col.is_descriptor {
            out.push_str(" :--- |");
        } else {
            out.push_str(" ---: |");
        }
    }
    out.push('\n');

    // Rows
    for row in &table.rows {
        if row.is_gutter {
            continue;
        }
        if row.is_section_header {
            let title = row.section_title.as_deref().unwrap_or_default();
            out.push_str(&format!("| **{title}** |"));
            for _ in 1..render_cols.len() {
                out.push_str(" |");
            }
            out.push('\n');
            continue;
        }

        let Some(row_id) = &row.id else { continue };

        out.push('|');
        for col in &render_cols {
            let cell = get_cell_text(table, row_id.as_str(), col, view);
            let text = match (&cell.secondary, view) {
                (Some(sec), ViewMode::Both) => format!("{}<br>`{}`", cell.primary, sec),
                _ => cell.primary,
            };
            out.push_str(&format!(" {text} |"));
        }
        out.push('\n');
    }

    out
}
