//! Pure in-memory whole-workbook GitHub-Flavored Markdown renderer.

use indexmap::{IndexMap, IndexSet};

use crate::compiler::compiled::CompiledModel;
use crate::compiler::compiled::error::InspectionError;
use crate::compiler::compiled::model::{CellError, ModelMarkdownDetail, ModelMarkdownOptions};
use crate::compiler::ir::id::CellId;
use crate::compiler::layout::{
    SheetLayout, TablePlacement, col_to_excel_col, coordinate_to_a1,
    grid_table_descriptor_req_widths,
};
use crate::model::format::SemanticStyle;
use crate::model::id::{SheetId, TableId};
use crate::model::manifest::{FreezePanes, SheetLayoutSpec, SheetSpec};
use crate::model::table::{
    AxisRole, ColumnSpec, DEFAULT_HORIZONTAL_TABLE_GAP, DEFAULT_VERTICAL_TABLE_GAP, DataColumnSpec,
    PlacementSpec, RowSpec, TableSpec,
};
use crate::value::CellValue;

const WIDTH_EPS: f64 = 1e-9;

fn round2(v: f64) -> f64 {
    let r = (v * 100.0).round() / 100.0;
    if r == 0.0 { 0.0 } else { r }
}

fn format_width(v: f64) -> String {
    let rounded = round2(v);
    let formatted = format!("{rounded:.2}");
    if formatted.contains('.') {
        formatted
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        formatted
    }
}

fn format_a1(col: u32, row: u32) -> String {
    coordinate_to_a1(col, row)
        .unwrap_or_else(|_| format!("{}{}", col_to_excel_col(col), row.saturating_add(1)))
}

fn format_row_range(start_row: u32, end_row: u32) -> String {
    let r1 = start_row.saturating_add(1);
    let r2 = end_row.saturating_add(1);
    if start_row == end_row {
        format!("R{r1}")
    } else {
        format!("R{r1}:R{r2}")
    }
}

fn format_col_range(start_col: u32, end_col: u32) -> String {
    let c1 = col_to_excel_col(start_col);
    if start_col == end_col {
        c1
    } else {
        format!("{c1}..{}", col_to_excel_col(end_col))
    }
}

fn format_freeze_panes(fp: FreezePanes) -> String {
    let a1 = format_a1(fp.col, fp.row);
    let split_desc = match (fp.row > 0, fp.col > 0) {
        (true, true) => format!(
            "{}, {}",
            format_row_range(0, fp.row - 1),
            format_col_range(0, fp.col - 1)
        ),
        (true, false) => format_row_range(0, fp.row - 1),
        (false, true) => format_col_range(0, fp.col - 1),
        (false, false) => "no split".to_string(),
    };
    format!(" | Freeze: {a1} ({split_desc})")
}

#[derive(Clone, Copy)]
struct PlacedTable<'a> {
    decl_index: usize,
    table: &'a TableSpec,
    placement: &'a TablePlacement,
}

struct LayoutBand<'a> {
    min_row: u32,
    max_row: u32,
    min_col: u32,
    max_col: u32,
    tables: Vec<PlacedTable<'a>>,
}

struct LayoutLane<'a> {
    min_col: u32,
    max_col: u32,
    min_row: u32,
    max_row: u32,
    tables: Vec<PlacedTable<'a>>,
}

#[derive(Debug, Clone, Copy)]
enum ColRun {
    DescSingle {
        start_col: u32,
        end_col: u32,
    },
    DescSpanned {
        start_col: u32,
        end_col: u32,
        req_w: f64,
    },
    Measure {
        start_col: u32,
        end_col: u32,
    },
    Gutter {
        start_col: u32,
        end_col: u32,
    },
}

fn decompose_bands<'a>(mut placed: Vec<PlacedTable<'a>>) -> Vec<LayoutBand<'a>> {
    placed.sort_by_key(|pt| {
        (
            pt.placement.start_row,
            pt.placement.start_col,
            pt.decl_index,
        )
    });
    let mut bands: Vec<LayoutBand<'a>> = Vec::new();
    for pt in placed {
        let p = pt.placement;
        match bands.last_mut() {
            Some(current) if p.start_row <= current.max_row => {
                current.max_row = current.max_row.max(p.end_row);
                current.min_col = current.min_col.min(p.start_col);
                current.max_col = current.max_col.max(p.end_col);
                current.tables.push(pt);
            }
            Some(_) | None => {
                bands.push(LayoutBand {
                    min_row: p.start_row,
                    max_row: p.end_row,
                    min_col: p.start_col,
                    max_col: p.end_col,
                    tables: vec![pt],
                });
            }
        }
    }
    bands
}

fn decompose_lanes<'a>(mut band_tables: Vec<PlacedTable<'a>>) -> Vec<LayoutLane<'a>> {
    band_tables.sort_by_key(|pt| {
        (
            pt.placement.start_col,
            pt.placement.start_row,
            pt.decl_index,
        )
    });
    let mut lanes: Vec<LayoutLane<'a>> = Vec::new();
    for pt in band_tables {
        let p = pt.placement;
        match lanes.last_mut() {
            Some(current) if p.start_col <= current.max_col => {
                current.max_col = current.max_col.max(p.end_col);
                current.min_row = current.min_row.min(p.start_row);
                current.max_row = current.max_row.max(p.end_row);
                current.tables.push(pt);
            }
            Some(_) | None => {
                lanes.push(LayoutLane {
                    min_col: p.start_col,
                    max_col: p.end_col,
                    min_row: p.start_row,
                    max_row: p.end_row,
                    tables: vec![pt],
                });
            }
        }
    }
    for lane in &mut lanes {
        lane.tables.sort_by_key(|pt| {
            (
                pt.placement.start_row,
                pt.placement.start_col,
                pt.decl_index,
            )
        });
    }
    lanes
}

fn format_placement(placement: &PlacementSpec) -> String {
    match placement {
        PlacementSpec::Stack { gap } => {
            if *gap == DEFAULT_VERTICAL_TABLE_GAP {
                "stack".to_string()
            } else {
                format!("stack (gap:{gap}r)")
            }
        }
        PlacementSpec::Below { target, gap } => {
            if *gap == DEFAULT_VERTICAL_TABLE_GAP {
                format!("below `{target}`")
            } else {
                format!("below `{target}` (gap:{gap}r)")
            }
        }
        PlacementSpec::RightOf { target, gap } => {
            if *gap == DEFAULT_HORIZONTAL_TABLE_GAP {
                format!("right_of `{target}`")
            } else {
                format!("right_of `{target}` (gap:{gap}c)")
            }
        }
    }
}

fn max_subtitle_chars(subtitles: &[String]) -> usize {
    subtitles
        .iter()
        .map(|s| normalize_inline_text(s).chars().count())
        .max()
        .unwrap_or(0)
}

fn format_table_structure(
    table: &TableSpec,
    placement: &TablePlacement,
    table_w: f64,
    char_width_multiplier: f64,
) -> String {
    let mut tokens: Vec<String> = Vec::with_capacity(5);

    if placement.title_row.is_none() {
        tokens.push("untitled".to_string());
    } else if let Some(title_end_col) = placement.title_end_col
        && title_end_col > placement.end_col
    {
        let extra = title_end_col - placement.end_col;
        tokens.push(format!(
            "title->{} (+{extra} empty)",
            col_to_excel_col(title_end_col)
        ));
    }

    if !table.subtitles.is_empty() {
        let count = table.subtitles.len();
        let max_chars = max_subtitle_chars(&table.subtitles);
        let sub_w = round2(max_chars as f64 * char_width_multiplier);
        if sub_w > table_w + WIDTH_EPS {
            tokens.push(format!("sub:{count}({max_chars}ch>w)"));
        } else {
            tokens.push(format!("sub:{count}"));
        }
    }

    let has_group = table.columns.iter().any(|c| c.group().is_some());
    match placement.header_row_range {
        Some((h_start, h_end)) => {
            let range = format_row_range(h_start, h_end);
            if has_group {
                tokens.push(format!("hdr:{range}(grouped)"));
            } else {
                tokens.push(format!("hdr:{range}"));
            }
        }
        None => {
            tokens.push("hdr:none".to_string());
        }
    }

    let (b_start, b_end) = placement.body_row_range;
    tokens.push(format!("body:{}", format_row_range(b_start, b_end)));

    if table.collapsible && placement.end_row > placement.start_row {
        tokens.push("collapsible".to_string());
    }

    tokens.join(" ")
}

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

        let wrap_counts = match options.detail {
            ModelMarkdownDetail::Layout => {
                let mut counts: IndexMap<&TableId, usize> = IndexMap::new();
                for (cell_id, coord) in &self.layout_map.cells {
                    if coord.wrap_text {
                        *counts.entry(&cell_id.table).or_insert(0) += 1;
                    }
                }
                counts
            }
            ModelMarkdownDetail::Values | ModelMarkdownDetail::Index => IndexMap::new(),
        };

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

            match options.detail {
                ModelMarkdownDetail::Values => {
                    self.push_sheet_tables(&sheet_tables, &mut lines, |table_spec, out| {
                        self.push_values_table(table_spec, options.show_ids, out);
                    });
                }
                ModelMarkdownDetail::Index => {
                    self.push_sheet_tables(&sheet_tables, &mut lines, |table_spec, out| {
                        self.push_index_table(table_spec, out);
                    });
                }
                ModelMarkdownDetail::Layout => {
                    self.push_layout_sheet(
                        sheet_spec,
                        &selected_table_ids,
                        &wrap_counts,
                        &mut lines,
                    );
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

    fn push_sheet_tables<F>(
        &self,
        sheet_tables: &[&TableSpec],
        lines: &mut Vec<String>,
        render_table: F,
    ) where
        F: Fn(&TableSpec, &mut Vec<String>),
    {
        if sheet_tables.is_empty() {
            lines.push(String::new());
            lines.push("_(no tables)_".to_string());
            return;
        }

        for table_spec in sheet_tables {
            lines.push(String::new());
            lines.push(render_table_heading(table_spec));
            for subtitle in &table_spec.subtitles {
                let norm_sub = normalize_inline_text(subtitle);
                lines.push(format!("_{}_", norm_sub.trim()));
            }
            render_table(table_spec, lines);
        }
    }

    fn col_width_at(&self, sheet_layout: Option<&SheetLayout>, col: u32) -> f64 {
        sheet_layout
            .and_then(|sl| sl.col_widths.get(&col).copied())
            .unwrap_or(self.theme.metrics.gutter_col_width)
    }

    fn range_width(&self, sheet_layout: Option<&SheetLayout>, start_col: u32, end_col: u32) -> f64 {
        let mut total = 0.0;
        for col in start_col..=end_col {
            total += self.col_width_at(sheet_layout, col);
        }
        round2(total)
    }

    fn format_sheet_mode(
        &self,
        sheet_spec: &SheetSpec,
        sheet_layout: Option<&SheetLayout>,
        placed_tables: &[PlacedTable<'_>],
    ) -> String {
        match sheet_spec.layout {
            SheetLayoutSpec::Auto => "auto".to_string(),
            SheetLayoutSpec::Grid { col_width } => {
                let base_w = round2(col_width.unwrap_or(self.theme.metrics.min_col_width));
                let first_data_col = placed_tables.iter().find_map(|pt| {
                    pt.table
                        .columns
                        .iter()
                        .zip(&pt.placement.col_spans)
                        .find_map(|(col_spec, &(phys_start, _))| match col_spec {
                            ColumnSpec::Data(_) => Some(phys_start),
                            ColumnSpec::Gutter => None,
                        })
                });
                let actual_w = first_data_col
                    .and_then(|c| sheet_layout.and_then(|sl| sl.col_widths.get(&c).copied()))
                    .map_or(base_w, round2);
                if (actual_w - base_w).abs() > WIDTH_EPS {
                    format!(
                        "grid (W={}, base={})",
                        format_width(actual_w),
                        format_width(base_w)
                    )
                } else {
                    format!("grid (W={})", format_width(actual_w))
                }
            }
        }
    }

    fn format_rle_columns(&self, sheet_layout: Option<&SheetLayout>, max_col: u32) -> String {
        let mut runs: Vec<(u32, u32, f64)> = Vec::new();
        for col in 0..=max_col {
            let w = round2(self.col_width_at(sheet_layout, col));
            match runs.last_mut() {
                Some((_, run_end, run_w)) if (w - *run_w).abs() <= WIDTH_EPS => {
                    *run_end = col;
                }
                Some(_) | None => {
                    runs.push((col, col, w));
                }
            }
        }
        runs.into_iter()
            .map(|(start_col, end_col, w)| {
                format!(
                    "{}:{}",
                    format_col_range(start_col, end_col),
                    format_width(w)
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn format_col_runs(
        &self,
        sheet_layout: Option<&SheetLayout>,
        layout: SheetLayoutSpec,
        table: &TableSpec,
        placement: &TablePlacement,
    ) -> String {
        let desc_req_widths = match layout {
            SheetLayoutSpec::Auto => None,
            SheetLayoutSpec::Grid { .. } => {
                let has_spanned_desc = table.columns.iter().zip(&placement.col_spans).any(
                    |(col_spec, &(phys_start, phys_end))| {
                        phys_end > phys_start
                            && matches!(col_spec, ColumnSpec::Data(d) if d.role.is_descriptor())
                    },
                );
                has_spanned_desc.then(|| {
                    grid_table_descriptor_req_widths(
                        table,
                        &self.grid,
                        &self.evaluated,
                        &self.theme,
                    )
                })
            }
        };

        let mut runs: Vec<ColRun> = Vec::new();
        for (c_idx, (col_spec, &(phys_start, phys_end))) in
            table.columns.iter().zip(&placement.col_spans).enumerate()
        {
            match col_spec {
                ColumnSpec::Data(d) => match d.role {
                    AxisRole::Descriptor => {
                        let spanned_req_w = match layout {
                            SheetLayoutSpec::Auto => None,
                            SheetLayoutSpec::Grid { .. } if phys_end > phys_start => Some(
                                desc_req_widths
                                    .as_deref()
                                    .and_then(|widths| widths.get(c_idx).copied())
                                    .unwrap_or(0.0),
                            ),
                            SheetLayoutSpec::Grid { .. } => None,
                        };
                        if let Some(req_w) = spanned_req_w {
                            runs.push(ColRun::DescSpanned {
                                start_col: phys_start,
                                end_col: phys_end,
                                req_w,
                            });
                        } else if let Some(ColRun::DescSingle { end_col, .. }) = runs.last_mut() {
                            *end_col = phys_end;
                        } else {
                            runs.push(ColRun::DescSingle {
                                start_col: phys_start,
                                end_col: phys_end,
                            });
                        }
                    }
                    AxisRole::Measure => {
                        if let Some(ColRun::Measure { end_col, .. }) = runs.last_mut() {
                            *end_col = phys_end;
                        } else {
                            runs.push(ColRun::Measure {
                                start_col: phys_start,
                                end_col: phys_end,
                            });
                        }
                    }
                },
                ColumnSpec::Gutter => {
                    if let Some(ColRun::Gutter { end_col, .. }) = runs.last_mut() {
                        *end_col = phys_end;
                    } else {
                        runs.push(ColRun::Gutter {
                            start_col: phys_start,
                            end_col: phys_end,
                        });
                    }
                }
            }
        }

        let default_gutter = self.theme.metrics.gutter_col_width;
        runs.into_iter()
            .map(|run| match run {
                ColRun::DescSingle { start_col, end_col } => {
                    let col_range = format_col_range(start_col, end_col);
                    let phys_count = end_col.saturating_sub(start_col).saturating_add(1);
                    format!("desc {col_range}({phys_count}c)")
                }
                ColRun::DescSpanned {
                    start_col,
                    end_col,
                    req_w,
                } => {
                    let col_range = format_col_range(start_col, end_col);
                    let phys_count = end_col.saturating_sub(start_col).saturating_add(1);
                    format!(
                        "desc {col_range}({phys_count}c, req:{})",
                        format_width(req_w)
                    )
                }
                ColRun::Measure { start_col, end_col } => {
                    let col_range = format_col_range(start_col, end_col);
                    let phys_count = end_col.saturating_sub(start_col).saturating_add(1);
                    format!("measure {col_range}({phys_count}c)")
                }
                ColRun::Gutter { start_col, end_col } => {
                    let col_range = format_col_range(start_col, end_col);
                    let phys_count = end_col.saturating_sub(start_col).saturating_add(1);
                    let widened = (start_col..=end_col).any(|c| {
                        (self.col_width_at(sheet_layout, c) - default_gutter).abs() > WIDTH_EPS
                    });
                    if widened {
                        let gutter_w =
                            format_width(self.range_width(sheet_layout, start_col, end_col));
                        format!("gutter {col_range}({phys_count}c, w:{gutter_w})")
                    } else {
                        format!("gutter {col_range}({phys_count}c)")
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn format_layout_table_item(
        &self,
        sheet_layout: Option<&SheetLayout>,
        layout: SheetLayoutSpec,
        pt: PlacedTable<'_>,
        selected_table_ids: &IndexSet<&TableId>,
        wrap_counts: &IndexMap<&TableId, usize>,
    ) -> String {
        let table = pt.table;
        let p = pt.placement;
        let context = if selected_table_ids.contains(&table.id) {
            ""
        } else {
            " (context)"
        };
        let start_a1 = format_a1(p.start_col, p.start_row);
        let end_a1 = format_a1(p.end_col, p.end_row);
        let rows = p.end_row.saturating_sub(p.start_row).saturating_add(1);
        let cols = p.end_col.saturating_sub(p.start_col).saturating_add(1);
        let table_w_num = self.range_width(sheet_layout, p.start_col, p.end_col);
        let table_w = format_width(table_w_num);
        let placement_str = format_placement(&table.placement);
        let structure_str = format_table_structure(
            table,
            p,
            table_w_num,
            self.theme.metrics.char_width_multiplier,
        );
        let col_runs = self.format_col_runs(sheet_layout, layout, table, p);

        let mut line = format!(
            "`{}`{context} [{start_a1}:{end_a1}] {rows}r x {cols}c (w:{table_w}) | {placement_str} | {structure_str} | cols: {col_runs}",
            table.id
        );
        if let Some(&wrap_count) = wrap_counts.get(&table.id)
            && wrap_count > 0
        {
            line.push_str(&format!(" | wrap:{wrap_count}"));
        }
        line
    }

    fn push_layout_sheet(
        &self,
        sheet_spec: &SheetSpec,
        selected_table_ids: &IndexSet<&TableId>,
        wrap_counts: &IndexMap<&TableId, usize>,
        lines: &mut Vec<String>,
    ) {
        let placed_tables: Vec<PlacedTable<'_>> = sheet_spec
            .tables
            .iter()
            .enumerate()
            .filter_map(|(decl_index, tid)| {
                let table = self.tables.get(tid)?;
                let placement = self.layout_map.get_table(tid)?;
                Some(PlacedTable {
                    decl_index,
                    table,
                    placement,
                })
            })
            .collect();

        if placed_tables.is_empty() {
            lines.push(String::new());
            lines.push("_(no tables)_".to_string());
            return;
        }

        let sheet_id = SheetId::from(sheet_spec.name.as_str());
        let sheet_layout = self.layout_map.get_sheet(&sheet_id);

        let (sheet_min_col, sheet_max_col, sheet_min_row, sheet_max_row) = {
            let mut min_col = u32::MAX;
            let mut max_col = 0u32;
            let mut min_row = u32::MAX;
            let mut max_row = 0u32;
            for pt in &placed_tables {
                min_col = min_col.min(pt.placement.start_col);
                max_col = max_col.max(pt.placement.end_col);
                min_row = min_row.min(pt.placement.start_row);
                max_row = max_row.max(pt.placement.end_row);
            }
            (min_col, max_col, min_row, max_row)
        };

        let mode = self.format_sheet_mode(sheet_spec, sheet_layout, &placed_tables);
        let min_a1 = format_a1(sheet_min_col, sheet_min_row);
        let max_a1 = format_a1(sheet_max_col, sheet_max_row);
        let total_rows = sheet_max_row
            .saturating_sub(sheet_min_row)
            .saturating_add(1);
        let total_cols = sheet_max_col
            .saturating_sub(sheet_min_col)
            .saturating_add(1);
        let canvas_w = format_width(self.range_width(sheet_layout, sheet_min_col, sheet_max_col));

        let freeze_panes = sheet_layout.map_or(sheet_spec.freeze_panes, |sl| sl.freeze_panes);
        let show_gridlines = sheet_layout.map_or(sheet_spec.show_gridlines, |sl| sl.show_gridlines);
        let tab_color = sheet_layout.map_or(sheet_spec.tab_color.as_deref(), |sl| {
            sl.tab_color.as_deref()
        });

        let mut meta_line = format!(
            "Layout: {mode} | Bounds: {min_a1}:{max_a1} ({total_rows}r x {total_cols}c, w:{canvas_w})"
        );
        if let Some(fp) = freeze_panes {
            meta_line.push_str(&format_freeze_panes(fp));
        }
        if show_gridlines {
            meta_line.push_str(" | Gridlines: on");
        }
        if let Some(hex) = tab_color {
            meta_line.push_str(" | Tab: ");
            meta_line.push_str(hex);
        }
        lines.push(meta_line);

        lines.push(format!(
            "Columns: {}",
            self.format_rle_columns(sheet_layout, sheet_max_col)
        ));
        lines.push(String::new());

        let bands = decompose_bands(placed_tables);
        let mut prev_band_max_row: Option<u32> = None;

        for (band_idx, band) in bands.into_iter().enumerate() {
            let band_num = band_idx + 1;
            let band_row_range = format_row_range(band.min_row, band.max_row);
            let band_rows = band.max_row.saturating_sub(band.min_row).saturating_add(1);
            let band_col_range = format_col_range(band.min_col, band.max_col);
            let band_w = format_width(self.range_width(sheet_layout, band.min_col, band.max_col));

            let mut band_line = format!(
                "- Band {band_num} ({band_row_range}, {band_rows}r x cols {band_col_range}, w:{band_w}"
            );
            if let Some(prev_max) = prev_band_max_row {
                let gap = band.min_row.saturating_sub(prev_max.saturating_add(1));
                band_line.push_str(&format!(", gap:{gap}r"));
            }
            if band.min_col > sheet_min_col {
                let empty_end = band.min_col - 1;
                let n = band.min_col - sheet_min_col;
                let range = format_col_range(sheet_min_col, empty_end);
                let empty_w =
                    format_width(self.range_width(sheet_layout, sheet_min_col, empty_end));
                band_line.push_str(&format!(", left_empty:{range} ({n}c, w:{empty_w})"));
            }
            if band.max_col < sheet_max_col {
                let empty_start = band.max_col + 1;
                let n = sheet_max_col - band.max_col;
                let range = format_col_range(empty_start, sheet_max_col);
                let empty_w =
                    format_width(self.range_width(sheet_layout, empty_start, sheet_max_col));
                band_line.push_str(&format!(", right_empty:{range} ({n}c, w:{empty_w})"));
            }
            band_line.push_str("):");
            lines.push(band_line);

            prev_band_max_row = Some(band.max_row);

            let band_min_row = band.min_row;
            let band_max_row = band.max_row;
            let lanes = decompose_lanes(band.tables);

            if lanes.len() == 1 {
                if let Some(lane) = lanes.into_iter().next() {
                    for pt in lane.tables {
                        lines.push(format!(
                            "  - {}",
                            self.format_layout_table_item(
                                sheet_layout,
                                sheet_spec.layout,
                                pt,
                                selected_table_ids,
                                wrap_counts,
                            )
                        ));
                    }
                }
            } else {
                let mut prev_lane_max_col: Option<u32> = None;
                for lane in lanes {
                    let lane_col_range = format_col_range(lane.min_col, lane.max_col);
                    let lane_cols = lane.max_col.saturating_sub(lane.min_col).saturating_add(1);
                    let lane_w =
                        format_width(self.range_width(sheet_layout, lane.min_col, lane.max_col));
                    let used_row_range = format_row_range(lane.min_row, lane.max_row);
                    let used_rows = lane.max_row.saturating_sub(lane.min_row).saturating_add(1);

                    let mut lane_line =
                        format!("  - Lane {lane_col_range} ({lane_cols}c, w:{lane_w}");
                    if let Some(prev_max_col) = prev_lane_max_col {
                        let gap_cols = lane.min_col.saturating_sub(prev_max_col.saturating_add(1));
                        if gap_cols > 0 {
                            let gap_start = prev_max_col + 1;
                            let gap_end = lane.min_col - 1;
                            let gap_range = format_col_range(gap_start, gap_end);
                            let gap_w =
                                format_width(self.range_width(sheet_layout, gap_start, gap_end));
                            lane_line
                                .push_str(&format!(", gap:{gap_cols}c ({gap_range}, w:{gap_w})"));
                        } else {
                            lane_line.push_str(", gap:0c");
                        }
                    }
                    lane_line.push_str(&format!(", {used_row_range} = {used_rows}r"));
                    if lane.min_row > band_min_row {
                        let n = lane.min_row - band_min_row;
                        let range = format_row_range(band_min_row, lane.min_row - 1);
                        lane_line.push_str(&format!(", empty_above:{range} ({n}r)"));
                    }
                    if lane.max_row < band_max_row {
                        let n = band_max_row - lane.max_row;
                        let range = format_row_range(lane.max_row + 1, band_max_row);
                        lane_line.push_str(&format!(", empty_below:{range} ({n}r)"));
                    }
                    lane_line.push_str("):");
                    lines.push(lane_line);

                    prev_lane_max_col = Some(lane.max_col);

                    for pt in lane.tables {
                        lines.push(format!(
                            "    - {}",
                            self.format_layout_table_item(
                                sheet_layout,
                                sheet_spec.layout,
                                pt,
                                selected_table_ids,
                                wrap_counts,
                            )
                        ));
                    }
                }
            }
        }
    }
}
