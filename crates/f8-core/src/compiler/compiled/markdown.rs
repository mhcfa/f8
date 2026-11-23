//! Pure in-memory whole-workbook GitHub-Flavored Markdown renderer.

use indexmap::IndexSet;

use crate::compiler::compiled::CompiledModel;
use crate::compiler::compiled::error::InspectionError;
use crate::compiler::compiled::model::{CellError, ModelMarkdownDetail, ModelMarkdownOptions};
use crate::compiler::ir::id::CellId;
use crate::model::format::SemanticStyle;
use crate::model::id::TableId;
use crate::model::manifest::SheetSpec;
use crate::model::table::{AxisRole, ColumnSpec, DataColumnSpec, RowSpec, TableSpec};
use crate::value::CellValue;

fn normalize_inline_text(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

fn escape_markdown_cell(text: &str) -> String {
    normalize_inline_text(text).replace('|', r"\|")
}

fn format_compact_gfm_row(cells: &[String]) -> String {
    let mut line = String::from("|");
    for cell in cells {
        if cell.is_empty() {
            line.push_str(" |");
        } else {
            line.push(' ');
            line.push_str(cell);
            line.push_str(" |");
        }
    }
    line
}

fn flattened_column_header(col: &DataColumnSpec) -> String {
    let stacked = col
        .headers
        .iter()
        .filter(|h| !h.is_empty())
        .map(|h| normalize_inline_text(h))
        .collect::<Vec<_>>()
        .join(" ");
    let group = col.group.as_deref().map(normalize_inline_text);
    match (
        group.as_deref().filter(|g| !g.is_empty()),
        !stacked.is_empty(),
    ) {
        (Some(g), true) => format!("{g}: {stacked}"),
        (Some(g), false) => g.to_string(),
        (None, true) => stacked,
        (None, false) => normalize_inline_text(col.id.as_str()),
    }
}

const fn is_bold_row_style(style: Option<SemanticStyle>) -> bool {
    match style {
        Some(s) => s.uses_bold_font(),
        None => false,
    }
}

const fn is_summary_style(style: Option<SemanticStyle>) -> bool {
    match style {
        Some(SemanticStyle::Subtotal | SemanticStyle::Total) => true,
        Some(SemanticStyle::Data | SemanticStyle::Bold) | None => false,
    }
}

const fn semantic_style_tag(style: Option<SemanticStyle>) -> Option<&'static str> {
    match style {
        Some(SemanticStyle::Subtotal) => Some("subtotal"),
        Some(SemanticStyle::Total) => Some("total"),
        Some(SemanticStyle::Data | SemanticStyle::Bold) | None => None,
    }
}

fn render_error_line(err: &CellError) -> String {
    let coord = normalize_inline_text(&err.coordinate.to_string());
    let addr = normalize_inline_text(&err.physical_address);
    let code = err.error.as_str();
    match &err.excel_formula {
        Some(formula) => {
            let norm_formula = normalize_inline_text(formula);
            format!("- `{coord}` (`{addr}`): `{code}` (`{norm_formula}`)")
        }
        None => format!("- `{coord}` (`{addr}`): `{code}`"),
    }
}

fn render_table_heading(table: &TableSpec) -> String {
    let norm_title = normalize_inline_text(&table.title);
    let trimmed_title = norm_title.trim();
    let table_id = table.id.as_str();
    if !trimmed_title.is_empty() && trimmed_title.to_lowercase() != table_id.to_lowercase() {
        format!("### `{table_id}` — {trimmed_title}")
    } else {
        format!("### `{table_id}`")
    }
}

enum RenderCol<'a> {
    SyntheticRow,
    Data {
        spec: &'a DataColumnSpec,
        is_first_descriptor: bool,
    },
}

impl CompiledModel {
    /// Renders a compact, deterministic GitHub-Flavored Markdown summary of the compiled workbook.
    ///
    /// # Errors
    /// Returns [`InspectionError::SheetNotFound`] if any sheet in `options.sheets` does not exist,
    /// or [`InspectionError::TableNotFound`] if any table in `options.tables` does not exist.
    pub fn to_markdown(&self, options: &ModelMarkdownOptions) -> Result<String, InspectionError> {
        let mut sheet_filter: IndexSet<&str> = IndexSet::new();
        if let Some(req_sheets) = &options.sheets {
            let known_sheets: IndexSet<&str> = self
                .manifest
                .sheets
                .iter()
                .map(|s| s.name.as_str())
                .collect();
            for sheet in req_sheets {
                if !known_sheets.contains(sheet.as_str()) {
                    return Err(InspectionError::SheetNotFound(sheet.clone()));
                }
                sheet_filter.insert(sheet.as_str());
            }
        }

        let mut table_filter: IndexSet<&TableId> = IndexSet::new();
        if let Some(req_tables) = &options.tables {
            for table in req_tables {
                if !self.tables.contains_key(table) {
                    return Err(InspectionError::TableNotFound(table.clone()));
                }
                table_filter.insert(table);
            }
        }

        let unfiltered = options.sheets.is_none() && options.tables.is_none();
        let mut selected_sheets: Vec<(&SheetSpec, Vec<&TableSpec>)> = Vec::new();
        let mut selected_table_ids: IndexSet<&TableId> = IndexSet::new();

        for sheet_spec in &self.manifest.sheets {
            let include_entire_sheet =
                unfiltered || sheet_filter.contains(sheet_spec.name.as_str());
            let selected_tables: Vec<&TableSpec> = sheet_spec
                .tables
                .iter()
                .filter(|tid| include_entire_sheet || table_filter.contains(*tid))
                .filter_map(|tid| self.tables.get(tid))
                .collect();

            if include_entire_sheet || !selected_tables.is_empty() {
                for table_spec in &selected_tables {
                    selected_table_ids.insert(&table_spec.id);
                }
                selected_sheets.push((sheet_spec, selected_tables));
            }
        }

        let summary = self.summary();
        let all_errors = self.errors();
        let total_errors = all_errors.len();
        let scoped_errors: Vec<CellError> = all_errors
            .into_iter()
            .filter(|err| selected_table_ids.contains(&err.coordinate.table))
            .collect();

        let mut lines: Vec<String> = Vec::new();

        // Line 1: Model title
        let normalized_name = normalize_inline_text(&self.manifest.model_name);
        let trimmed_name = normalized_name.trim();
        let heading_name = if trimmed_name.is_empty() {
            "Model"
        } else {
            trimmed_name
        };
        lines.push(format!("# {heading_name}"));

        // Line 2: Full-model summary counts
        lines.push(format!(
            "Sheets: {} | Tables: {} | Cells: {} ({} drivers, {} formulas) | Errors: {}",
            summary.total_sheets,
            summary.total_tables,
            summary.total_cells,
            summary.total_drivers,
            summary.total_formulas,
            total_errors
        ));

        // Optional scoped runtime errors section
        if !scoped_errors.is_empty() {
            lines.push(String::new());
            lines.push("## Runtime Errors".to_string());
            lines.push(String::new());
            for err in &scoped_errors {
                lines.push(render_error_line(err));
            }
        }

        // Selected sheets and tables
        for (sheet_spec, sheet_tables) in selected_sheets {
            lines.push(String::new());
            lines.push(format!(
                "## Sheet: {}",
                normalize_inline_text(&sheet_spec.name)
            ));

            if sheet_tables.is_empty() {
                lines.push(String::new());
                lines.push("_(no tables)_".to_string());
                continue;
            }

            for table_spec in sheet_tables {
                lines.push(String::new());
                lines.push(render_table_heading(table_spec));
                for subtitle in &table_spec.subtitles {
                    let norm_sub = normalize_inline_text(subtitle);
                    lines.push(format!("_{}_", norm_sub.trim()));
                }

                match options.detail {
                    ModelMarkdownDetail::Values => {
                        self.push_values_table(table_spec, options.show_ids, &mut lines);
                    }
                    ModelMarkdownDetail::Index => {
                        self.push_index_table(table_spec, &mut lines);
                    }
                }
            }
        }

        let mut output = lines.join("\n");
        output.push('\n');
        Ok(output)
    }

    fn formatted_cell_text(&self, cell_id: &CellId) -> String {
        let value = self
            .evaluated
            .get_cell(cell_id)
            .unwrap_or(&CellValue::Empty);
        match self.grid.get(cell_id) {
            Some(node) => self.format_value(value, node.format, node.decimals, node.currency),
            None => value.to_string(),
        }
    }

    fn push_values_table(&self, table: &TableSpec, show_ids: bool, lines: &mut Vec<String>) {
        let data_cols: Vec<&DataColumnSpec> = table
            .columns
            .iter()
            .filter_map(|col| match col {
                ColumnSpec::Data(d) => Some(d),
                ColumnSpec::Gutter => None,
            })
            .collect();

        if data_cols.is_empty() {
            return;
        }

        let has_descriptor = data_cols.iter().any(|col| match col.role {
            AxisRole::Descriptor => true,
            AxisRole::Measure => false,
        });

        let mut render_cols: Vec<RenderCol<'_>> = Vec::with_capacity(data_cols.len() + 1);
        if !has_descriptor && show_ids {
            render_cols.push(RenderCol::SyntheticRow);
        }

        let mut seen_descriptor = false;
        for spec in data_cols {
            let is_first_descriptor = match spec.role {
                AxisRole::Descriptor => {
                    let first = !seen_descriptor;
                    seen_descriptor = true;
                    first
                }
                AxisRole::Measure => false,
            };
            render_cols.push(RenderCol::Data {
                spec,
                is_first_descriptor,
            });
        }

        lines.push(String::new());

        // Compact GFM header row
        let header_cells: Vec<String> = render_cols
            .iter()
            .map(|col| match col {
                RenderCol::SyntheticRow => "Row".to_string(),
                RenderCol::Data { spec, .. } => {
                    let display_header = flattened_column_header(spec);
                    let norm_id = normalize_inline_text(spec.id.as_str());
                    let label =
                        if show_ids && display_header.to_lowercase() != norm_id.to_lowercase() {
                            format!("{display_header} [{norm_id}]")
                        } else {
                            display_header
                        };
                    escape_markdown_cell(&label)
                }
            })
            .collect();
        lines.push(format_compact_gfm_row(&header_cells));

        // Compact GFM alignment row
        let align_cells: Vec<String> = render_cols
            .iter()
            .map(|col| match col {
                RenderCol::SyntheticRow => ":---".to_string(),
                RenderCol::Data { spec, .. } => match spec.role {
                    AxisRole::Descriptor => ":---".to_string(),
                    AxisRole::Measure => "---:".to_string(),
                },
            })
            .collect();
        lines.push(format_compact_gfm_row(&align_cells));

        // Data & section header rows
        for row in &table.rows {
            match row {
                RowSpec::Gutter => continue,
                RowSpec::SectionHeader(section) => {
                    let title = escape_markdown_cell(&section.title);
                    let mut cells = Vec::with_capacity(render_cols.len());
                    cells.push(format!("**{title}**"));
                    cells.resize(render_cols.len(), String::new());
                    lines.push(format_compact_gfm_row(&cells));
                }
                RowSpec::Data(row_spec) => {
                    let is_bold_row = is_bold_row_style(row_spec.style);
                    let indent = self.row_max_indent(table, &row_spec.id);
                    let indent_pad = "  ".repeat(usize::from(indent));
                    let norm_row_id = normalize_inline_text(row_spec.id.as_str());

                    let row_cells: Vec<String> = render_cols
                        .iter()
                        .map(|col| match col {
                            RenderCol::SyntheticRow => {
                                let escaped = escape_markdown_cell(&norm_row_id);
                                if is_bold_row && !escaped.is_empty() {
                                    format!("**{escaped}**")
                                } else {
                                    escaped
                                }
                            }
                            RenderCol::Data {
                                spec,
                                is_first_descriptor,
                            } => {
                                let cell_id = CellId::new(
                                    table.id.clone(),
                                    row_spec.id.clone(),
                                    spec.id.clone(),
                                );
                                let val_str = self.formatted_cell_text(&cell_id);

                                match spec.role {
                                    AxisRole::Descriptor => {
                                        let norm_val = normalize_inline_text(&val_str);
                                        let trimmed_val = norm_val.trim();
                                        let label = if *is_first_descriptor {
                                            if trimmed_val.is_empty() {
                                                norm_row_id.clone()
                                            } else if show_ids
                                                && trimmed_val.to_lowercase()
                                                    != norm_row_id.to_lowercase()
                                            {
                                                format!("{trimmed_val} [{norm_row_id}]")
                                            } else {
                                                trimmed_val.to_string()
                                            }
                                        } else {
                                            trimmed_val.to_string()
                                        };
                                        let escaped = escape_markdown_cell(&label);
                                        if escaped.is_empty() {
                                            String::new()
                                        } else if is_bold_row {
                                            format!("{indent_pad}**{escaped}**")
                                        } else {
                                            format!("{indent_pad}{escaped}")
                                        }
                                    }
                                    AxisRole::Measure => {
                                        let raw_text = if val_str.is_empty() {
                                            "—"
                                        } else {
                                            val_str.as_str()
                                        };
                                        let escaped = escape_markdown_cell(raw_text);
                                        if is_bold_row && !escaped.is_empty() {
                                            format!("**{escaped}**")
                                        } else {
                                            escaped
                                        }
                                    }
                                }
                            }
                        })
                        .collect();

                    lines.push(format_compact_gfm_row(&row_cells));
                }
            }
        }
    }

    fn push_index_table(&self, table: &TableSpec, lines: &mut Vec<String>) {
        let data_cols: Vec<&DataColumnSpec> = table
            .columns
            .iter()
            .filter_map(|col| match col {
                ColumnSpec::Data(d) => Some(d),
                ColumnSpec::Gutter => None,
            })
            .collect();

        let descriptor_cols: Vec<&DataColumnSpec> = data_cols
            .iter()
            .copied()
            .filter(|col| match col.role {
                AxisRole::Descriptor => true,
                AxisRole::Measure => false,
            })
            .collect();

        lines.push(String::new());

        let col_entries: Vec<String> = data_cols
            .iter()
            .map(|col| {
                let norm_id = normalize_inline_text(col.id.as_str());
                let header = flattened_column_header(col);
                let distinct_header = (!header.is_empty()
                    && header.to_lowercase() != norm_id.to_lowercase())
                .then_some(header);
                let is_descriptor = match col.role {
                    AxisRole::Descriptor => true,
                    AxisRole::Measure => false,
                };
                match (distinct_header, is_descriptor) {
                    (Some(h), true) => format!("`{norm_id}` ({h}, descriptor)"),
                    (Some(h), false) => format!("`{norm_id}` ({h})"),
                    (None, true) => format!("`{norm_id}` (descriptor)"),
                    (None, false) => format!("`{norm_id}`"),
                }
            })
            .collect();
        if col_entries.is_empty() {
            lines.push("Columns:".to_string());
        } else {
            lines.push(format!("Columns: {}", col_entries.join(", ")));
        }

        lines.push("Rows:".to_string());
        let mut in_section = false;
        for row in &table.rows {
            match row {
                RowSpec::Gutter => continue,
                RowSpec::SectionHeader(section) => {
                    in_section = true;
                    let title = normalize_inline_text(&section.title);
                    lines.push(format!("- **{}**", title.trim()));
                }
                RowSpec::Data(row_spec) => {
                    let prefix = if in_section { "  - " } else { "- " };
                    let norm_row_id = normalize_inline_text(row_spec.id.as_str());
                    let is_summary = is_summary_style(row_spec.style);
                    let id_token = if is_summary {
                        format!("**`{norm_row_id}`**")
                    } else {
                        format!("`{norm_row_id}`")
                    };

                    let desc_labels: Vec<String> = descriptor_cols
                        .iter()
                        .filter_map(|col| {
                            let cell_id =
                                CellId::new(table.id.clone(), row_spec.id.clone(), col.id.clone());
                            let val = normalize_inline_text(&self.formatted_cell_text(&cell_id));
                            let trimmed = val.trim();
                            (!trimmed.is_empty()).then(|| trimmed.to_string())
                        })
                        .collect();
                    let joined_desc = desc_labels.join(" | ");
                    let desc_suffix = if !joined_desc.is_empty()
                        && joined_desc.to_lowercase() != norm_row_id.to_lowercase()
                    {
                        format!(": {joined_desc}")
                    } else {
                        String::new()
                    };
                    let style_suffix = match semantic_style_tag(row_spec.style) {
                        Some(tag) => format!(" ({tag})"),
                        None => String::new(),
                    };
                    lines.push(format!("{prefix}{id_token}{desc_suffix}{style_suffix}"));
                }
            }
        }
    }
}
