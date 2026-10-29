//! OpenXML workbook assembly and `.xlsx` binary emission engine (Pass 6b).
//!
//! Orchestrates the creation of native `.xlsx` workbooks via `rust_xlsxwriter`,
//! applying sheet settings, layout dimensions, deduplicated format caches,
//! structural headers, hierarchical row outlines, formula caching, and values-only freezing.

use rust_xlsxwriter::{DocProperties, ExcelDateTime, Format, Formula, Workbook, Worksheet};
use serde::{Deserialize, Serialize};

use super::error::EmissionError;
use super::format::{CellFormatKey, FormatCache, parse_hex_color};
use crate::compiler::eval::EvaluatedModel;
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::id::CellId;
use crate::compiler::layout::{LayoutMap, StyleRole};
use crate::compiler::lowering::LoweredFormulaMap;
use crate::model::format::SemanticStyle;
use crate::model::id::SheetId;
use crate::model::manifest::ProjectManifest;
use crate::model::table::{ColumnSpec, RowSpec, TableSpec, ValueLiteral};
use crate::model::theme::Theme;
use crate::value::CellValue;

/// Configuration options for OpenXML workbook emission.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EmissionOptions {
    /// When true, emits evaluated values instead of formulas (frozen audit workbook).
    pub values_only: bool,
    /// When true, emits formulas without cached evaluation result tags (<v>).
    pub omit_formula_cache: bool,
    /// Document title metadata in docProps/core.xml.
    pub title: Option<String>,
    /// Document author metadata in docProps/core.xml.
    pub author: Option<String>,
}

impl EmissionOptions {
    /// Creates default emission options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method enabling or disabling values-only freezing mode.
    #[must_use]
    pub const fn with_values_only(mut self, values_only: bool) -> Self {
        self.values_only = values_only;
        self
    }

    /// Builder method enabling or disabling formula result caching.
    #[must_use]
    pub const fn with_omit_formula_cache(mut self, omit_formula_cache: bool) -> Self {
        self.omit_formula_cache = omit_formula_cache;
        self
    }

    /// Builder method setting document title metadata.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Builder method setting document author metadata.
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }
}

/// OpenXML workbook assembly engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkbookEmitter;

impl WorkbookEmitter {
    /// Compiles a model into an in-memory `.xlsx` OpenXML byte buffer.
    ///
    /// # Errors
    /// Returns [`EmissionError`] if any worksheet, table, or cell coordinate is missing
    /// from the spatial layout, or if an OpenXML format or packaging error occurs.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_to_buffer(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        grid: &CellGrid,
        layout_map: &LayoutMap,
        lowered_formulas: &LoweredFormulaMap,
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
        options: &EmissionOptions,
    ) -> Result<Vec<u8>, EmissionError> {
        let mut workbook = Self::assemble_workbook(
            manifest,
            tables,
            grid,
            layout_map,
            lowered_formulas,
            theme,
            evaluated,
            options,
        )?;
        let buffer = workbook.save_to_buffer()?;
        Ok(buffer)
    }

    /// Assembles an OpenXML workbook with worksheets, formatting, outlines, and cells.
    #[allow(clippy::too_many_arguments)]
    fn assemble_workbook(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        grid: &CellGrid,
        layout_map: &LayoutMap,
        lowered_formulas: &LoweredFormulaMap,
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
        options: &EmissionOptions,
    ) -> Result<Workbook, EmissionError> {
        let mut workbook = Workbook::new();

        // 1. Configure deterministic package properties
        let mut properties = DocProperties::new();
        let author = options.author.as_deref().unwrap_or("f8 Compiler");
        properties = properties.set_author(author);
        if let Some(title) = &options.title {
            properties = properties.set_title(title);
        }
        let fixed_datetime = ExcelDateTime::from_ymd(2026, 1, 1)?;
        properties = properties.set_creation_datetime(&fixed_datetime);
        workbook.set_properties(&properties);

        // 2. Initialize format deduplication cache
        let mut format_cache = FormatCache::new(theme)?;

        // 3. Process worksheets in declared order
        for sheet_spec in &manifest.sheets {
            let sheet_id = SheetId::from(sheet_spec.name.as_str());
            let sheet_layout =
                layout_map
                    .get_sheet(&sheet_id)
                    .ok_or_else(|| EmissionError::MissingSheet {
                        sheet: sheet_id.clone(),
                    })?;

            let worksheet = workbook.add_worksheet();
            worksheet.set_name(&sheet_spec.name)?;

            // Apply tab color if configured
            if let Some(color_hex) = &sheet_layout.tab_color {
                let color = parse_hex_color("tab_color", color_hex)?;
                worksheet.set_tab_color(color);
            }

            // Apply gridline visibility
            worksheet.set_screen_gridlines(sheet_layout.show_gridlines);

            // Apply freeze panes if configured
            if let Some(freeze) = sheet_layout.freeze_panes {
                worksheet.set_freeze_panes(freeze.row, freeze.col as u16)?;
            }

            // Apply sheet-wide column widths
            for (&col, &width) in &sheet_layout.col_widths {
                worksheet.set_column_width(col as u16, width)?;
            }

            // Note: Row heights are left automatic so Excel dynamically sizes rows based on
            // font metrics, maintaining natural line height and standard unified row heights.

            // 4. Resolve tables belonging to this worksheet in declared order
            for table_id in &sheet_spec.tables {
                let table = tables.iter().find(|t| t.id == *table_id).ok_or_else(|| {
                    EmissionError::MissingTable {
                        table: table_id.clone(),
                    }
                })?;

                let placement =
                    layout_map
                        .get_table(&table.id)
                        .ok_or_else(|| EmissionError::MissingTable {
                            table: table.id.clone(),
                        })?;

                // Table Title Banner: Fill entire table width from start_col to end_col
                if let Some(title_row) = placement.title_row
                    && !table.title.trim().is_empty()
                {
                    worksheet.write_string_with_format(
                        title_row,
                        placement.start_col as u16,
                        &table.title,
                        &format_cache.title_format(),
                    )?;
                    for col in (placement.start_col + 1)..=placement.end_col {
                        worksheet.write_blank(
                            title_row,
                            col as u16,
                            &format_cache.title_format(),
                        )?;
                    }
                }

                // Table Subtitle Banner
                if let (Some(subtitle_row), Some(subtitle)) =
                    (placement.subtitle_row, &table.subtitle)
                {
                    worksheet.write_string_with_format(
                        subtitle_row,
                        placement.start_col as u16,
                        subtitle,
                        &format_cache.subtitle_format(),
                    )?;
                }

                // Multi-Tier Column Headers
                if let Some((header_start_row, header_end_row)) = placement.header_row_range {
                    let total_tiers = (header_end_row - header_start_row + 1) as usize;
                    for t in 0..total_tiers {
                        let tier_row = header_start_row + t as u32;
                        let is_bottom_tier = t == total_tiers - 1;

                        // Step A (t == 0 Group Banners): CenterAcross contiguous runs with identical group label
                        if t == 0 {
                            let mut col_idx = 0;
                            while let Some(col_spec) = table.columns.get(col_idx) {
                                let group_opt = match col_spec {
                                    ColumnSpec::Data(d) => d.group.as_deref(),
                                    ColumnSpec::Gutter => None,
                                };

                                if let Some(label) = group_opt {
                                    let start_c = col_idx;
                                    let mut end_c = col_idx;
                                    while let Some(next_spec) = table.columns.get(end_c + 1) {
                                        let next_group = match next_spec {
                                            ColumnSpec::Data(d) => d.group.as_deref(),
                                            ColumnSpec::Gutter => None,
                                        };
                                        if next_group == Some(label) {
                                            end_c += 1;
                                        } else {
                                            break;
                                        }
                                    }

                                    let first_phys_col =
                                        (placement.start_col + start_c as u32) as u16;
                                    worksheet.write_string_with_format(
                                        tier_row,
                                        first_phys_col,
                                        label,
                                        &format_cache.super_header_format(),
                                    )?;
                                    for c in (start_c + 1)..=end_c {
                                        let phys_col = (placement.start_col + c as u32) as u16;
                                        worksheet.write_blank(
                                            tier_row,
                                            phys_col,
                                            &format_cache.super_header_format(),
                                        )?;
                                    }

                                    col_idx = end_c + 1;
                                } else {
                                    col_idx += 1;
                                }
                            }
                        }

                        // Step B (Per-column stacked headers across all t in 0..total_tiers)
                        for (c_idx, col_spec) in table.columns.iter().enumerate() {
                            let col_idx = (placement.start_col + c_idx as u32) as u16;
                            match col_spec {
                                ColumnSpec::Gutter => {
                                    // Gutter column: continue header bottom border and fill across gutter
                                    if is_bottom_tier || format_cache.has_col_header_fill() {
                                        let fmt = if is_bottom_tier {
                                            format_cache.col_header_format(false)
                                        } else {
                                            format_cache.col_header_upper_format(false)
                                        };
                                        worksheet.write_blank(tier_row, col_idx, &fmt)?;
                                    }
                                }
                                ColumnSpec::Data(d) => {
                                    if t == 0 && d.group.is_some() {
                                        continue;
                                    }
                                    let k_len = d.headers.len();
                                    let first_hdr_tier = total_tiers.saturating_sub(k_len);
                                    let is_desc = d.role.is_descriptor();
                                    let fmt = if is_bottom_tier {
                                        format_cache.col_header_format(is_desc)
                                    } else {
                                        format_cache.col_header_upper_format(is_desc)
                                    };

                                    if let Some(label) =
                                        t.checked_sub(first_hdr_tier).and_then(|k| d.headers.get(k))
                                    {
                                        worksheet.write_string_with_format(
                                            tier_row, col_idx, label, &fmt,
                                        )?;
                                    } else if is_bottom_tier || format_cache.has_col_header_fill() {
                                        worksheet.write_blank(tier_row, col_idx, &fmt)?;
                                    }
                                }
                            }
                        }
                    }
                }

                // Opt-In Table-Level Collapsing
                if table.collapsible
                    && let Some(placement) = layout_map.get_table(&table.id)
                    && placement.end_row > placement.start_row
                {
                    worksheet.group_symbols_above(true);
                    worksheet.group_rows(placement.start_row + 1, placement.end_row)?;
                }

                // Stream Table Body Rows
                for (r_idx, row_spec) in table.rows.iter().enumerate() {
                    let current_phys_row = placement.body_row_range.0 + r_idx as u32;

                    match row_spec {
                        RowSpec::Gutter => {
                            // Gutter row: row height set in layout, cells remain blank without borders
                        }
                        RowSpec::SectionHeader(sh) => {
                            worksheet.write_string_with_format(
                                current_phys_row,
                                placement.start_col as u16,
                                &sh.title,
                                &format_cache.section_header_format(sh.indent),
                            )?;

                            if let Some(ref note) = sh.note {
                                worksheet.insert_note(
                                    current_phys_row,
                                    placement.start_col as u16,
                                    &rust_xlsxwriter::Note::new(note),
                                )?;
                            }

                            if format_cache.has_section_header_fill() {
                                let band_fmt = format_cache.section_header_format(None);
                                for col in (placement.start_col + 1)..=placement.end_col {
                                    worksheet.write_blank(
                                        current_phys_row,
                                        col as u16,
                                        &band_fmt,
                                    )?;
                                }
                            }
                        }
                        RowSpec::Data(row_data) => {
                            for (c_idx, col_spec) in table.columns.iter().enumerate() {
                                let col_data = match col_spec {
                                    ColumnSpec::Data(d) => d,
                                    ColumnSpec::Gutter => {
                                        // Gutter column: continue row formatting (horizontal borders, shade, highlight)
                                        let role = StyleRole {
                                            horizontal: row_data
                                                .style
                                                .unwrap_or(SemanticStyle::Data),
                                            vertical: SemanticStyle::Data,
                                            is_highlight: row_data.highlight.unwrap_or(false),
                                            shade: row_data.shade.filter(|s| !s.is_none()),
                                        };

                                        if role != StyleRole::data() {
                                            let format_key = CellFormatKey::new(
                                                role, None, None, None, false, false, 0,
                                            );
                                            let cell_format =
                                                format_cache.get_cell_format(&format_key)?;
                                            let phys_col =
                                                (placement.start_col + c_idx as u32) as u16;
                                            worksheet.write_blank(
                                                current_phys_row,
                                                phys_col,
                                                &cell_format,
                                            )?;
                                        }
                                        continue;
                                    }
                                };

                                let cell_id = CellId::new(
                                    table.id.clone(),
                                    row_data.id.clone(),
                                    col_data.id.clone(),
                                );
                                let coord = layout_map.get_cell(&cell_id).ok_or_else(|| {
                                    EmissionError::MissingCell {
                                        cell: cell_id.clone(),
                                    }
                                })?;

                                let node = grid.get(&cell_id);
                                let is_input = node.is_some_and(|n| n.payload.is_literal());
                                let is_descriptor = col_data.role.is_descriptor();

                                let format_key = CellFormatKey::new(
                                    coord.style_role,
                                    node.and_then(|n| n.format),
                                    node.and_then(|n| n.decimals),
                                    node.and_then(|n| n.currency),
                                    is_input,
                                    is_descriptor,
                                    coord.indent,
                                );
                                let cell_format = format_cache.get_cell_format(&format_key)?;

                                if options.values_only {
                                    // Values-only freezing: write evaluated result as literal
                                    if let Some(val) = evaluated.and_then(|e| e.get_cell(&cell_id))
                                    {
                                        emit_cell_value(
                                            worksheet,
                                            coord.row,
                                            coord.col as u16,
                                            val,
                                            &cell_format,
                                        )?;
                                    } else if let Some(node) = node {
                                        match &node.payload {
                                            CellPayload::Literal(lit) => {
                                                emit_value_literal(
                                                    worksheet,
                                                    coord.row,
                                                    coord.col as u16,
                                                    lit,
                                                    &cell_format,
                                                )?;
                                            }
                                            CellPayload::Formula(_) => {
                                                worksheet.write_blank(
                                                    coord.row,
                                                    coord.col as u16,
                                                    &cell_format,
                                                )?;
                                            }
                                        }
                                    } else {
                                        worksheet.write_blank(
                                            coord.row,
                                            coord.col as u16,
                                            &cell_format,
                                        )?;
                                    }
                                } else {
                                    // Live formulas mode
                                    if let Some(formula_str) = lowered_formulas.get(&cell_id) {
                                        let mut formula = Formula::new(formula_str);
                                        if !options.omit_formula_cache
                                            && let Some(val) =
                                                evaluated.and_then(|e| e.get_cell(&cell_id))
                                        {
                                            formula = formula.set_result(val.to_string());
                                        }
                                        worksheet.write_formula_with_format(
                                            coord.row,
                                            coord.col as u16,
                                            formula,
                                            &cell_format,
                                        )?;
                                    } else if let Some(node) = node {
                                        match &node.payload {
                                            CellPayload::Literal(lit) => {
                                                emit_value_literal(
                                                    worksheet,
                                                    coord.row,
                                                    coord.col as u16,
                                                    lit,
                                                    &cell_format,
                                                )?;
                                            }
                                            CellPayload::Formula(_) => {
                                                worksheet.write_blank(
                                                    coord.row,
                                                    coord.col as u16,
                                                    &cell_format,
                                                )?;
                                            }
                                        }
                                    } else {
                                        worksheet.write_blank(
                                            coord.row,
                                            coord.col as u16,
                                            &cell_format,
                                        )?;
                                    }
                                }

                                // Attach audit note if present
                                if let Some(note) = node.and_then(|n| n.note.as_deref()) {
                                    worksheet.insert_note(
                                        coord.row,
                                        coord.col as u16,
                                        &rust_xlsxwriter::Note::new(note),
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(workbook)
    }
}

/// Emits an evaluated `CellValue` with format.
fn emit_cell_value(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: &CellValue,
    format: &Format,
) -> Result<(), EmissionError> {
    match value {
        CellValue::Number(n) => {
            worksheet.write_number_with_format(row, col, *n, format)?;
        }
        CellValue::Text(s) => {
            worksheet.write_string_with_format(row, col, s, format)?;
        }
        CellValue::Boolean(b) => {
            worksheet.write_boolean_with_format(row, col, *b, format)?;
        }
        CellValue::Empty => {
            worksheet.write_blank(row, col, format)?;
        }
        CellValue::Error(e) => {
            worksheet.write_string_with_format(row, col, e.as_str(), format)?;
        }
    }
    Ok(())
}

/// Emits a fallback `ValueLiteral` with format.
fn emit_value_literal(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    lit: &ValueLiteral,
    format: &Format,
) -> Result<(), EmissionError> {
    match lit {
        ValueLiteral::Number(n) => {
            worksheet.write_number_with_format(row, col, *n, format)?;
        }
        ValueLiteral::String(s) => {
            worksheet.write_string_with_format(row, col, s, format)?;
        }
        ValueLiteral::Boolean(b) => {
            worksheet.write_boolean_with_format(row, col, *b, format)?;
        }
        ValueLiteral::Null => {
            worksheet.write_blank(row, col, format)?;
        }
    }
    Ok(())
}
