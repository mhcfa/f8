//! OpenXML workbook assembly and `.xlsx` binary emission engine (Pass 6b).
//!
//! Orchestrates the creation of native `.xlsx` workbooks via `rust_xlsxwriter`,
//! applying sheet settings, layout dimensions, deduplicated format caches,
//! structural headers, hierarchical row outlines, formula caching, and values-only freezing.

use std::path::Path;

use rust_xlsxwriter::{DocProperties, ExcelDateTime, Format, Formula, Workbook, Worksheet};
use serde::{Deserialize, Serialize};

use super::error::EmissionError;
use super::format::{CellFormatKey, FormatCache, parse_hex_color};
use crate::compiler::eval::EvaluatedModel;
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::expand::matches_table_id;
use crate::compiler::ir::id::CellId;
use crate::compiler::layout::{LayoutMap, SectionHeaderRow};
use crate::compiler::lowering::LoweredFormulaMap;
use crate::model::id::{RowId, SheetId, TableId};
use crate::model::manifest::ProjectManifest;
use crate::model::table::{RowGroup, RowGroupChild, TableSpec, ValueLiteral};
use crate::model::theme::Theme;
use crate::value::CellValue;

/// Configuration options for OpenXML workbook emission.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EmissionOptions {
    /// When true, emits evaluated values instead of formulas (frozen audit workbook).
    pub values_only: bool,
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

    /// Compiles a model and writes the `.xlsx` workbook to a destination path on disk.
    ///
    /// # Errors
    /// Returns [`EmissionError`] if any compilation or emission error occurs, or if disk I/O fails.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_to_file<P: AsRef<Path>>(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        grid: &CellGrid,
        layout_map: &LayoutMap,
        lowered_formulas: &LoweredFormulaMap,
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
        options: &EmissionOptions,
        path: P,
    ) -> Result<(), EmissionError> {
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
        workbook.save(path)?;
        Ok(())
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
        let author = options.author.as_deref().unwrap_or("Finx Compiler");
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

            // Apply sheet-wide row heights
            for (&row, &height) in &sheet_layout.row_heights {
                worksheet.set_row_height(row, height)?;
            }

            // 4. Resolve tables belonging to this worksheet in declared order
            for table_path in &sheet_spec.tables {
                let table = tables
                    .iter()
                    .find(|t| matches_table_id(table_path, &t.id))
                    .ok_or_else(|| EmissionError::MissingTable {
                        table: TableId::from(
                            table_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default(),
                        ),
                    })?;

                let placement =
                    layout_map
                        .get_table(&table.id)
                        .ok_or_else(|| EmissionError::MissingTable {
                            table: table.id.clone(),
                        })?;

                // Table Title Banner
                if let Some(title_row) = placement.title_row
                    && !table.title.trim().is_empty()
                {
                    worksheet.write_string_with_format(
                        title_row,
                        placement.start_col as u16,
                        &table.title,
                        &format_cache.title_format(),
                    )?;
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

                // Super-Headers (with single-cell guard against MergeRangeSingleCell)
                for sh in &placement.super_header_rows {
                    if sh.start_col < sh.end_col {
                        worksheet.merge_range(
                            sh.row,
                            sh.start_col as u16,
                            sh.row,
                            sh.end_col as u16,
                            &sh.label,
                            &format_cache.super_header_format(),
                        )?;
                    } else {
                        worksheet.write_string_with_format(
                            sh.row,
                            sh.start_col as u16,
                            &sh.label,
                            &format_cache.super_header_format(),
                        )?;
                    }
                }

                // Column Key Headers
                for (c_idx, col_spec) in table.columns.iter().enumerate() {
                    let col_idx = (placement.start_col + c_idx as u32) as u16;
                    let label = col_spec.label.as_deref().unwrap_or(col_spec.id.as_str());
                    let is_desc = col_spec.role.is_descriptor();
                    let fmt = format_cache.col_header_format(is_desc);
                    worksheet.write_string_with_format(
                        placement.col_header_row,
                        col_idx,
                        label,
                        &fmt,
                    )?;
                }

                // Collapsible Row Group Outlining
                for group in &table.row_groups {
                    apply_row_group_outlines(
                        group,
                        table,
                        layout_map,
                        &sheet_layout.section_header_rows,
                        worksheet,
                    )?;
                }

                // Stream Data Cells
                for row_spec in &table.rows {
                    for col_spec in &table.columns {
                        let cell_id =
                            CellId::new(table.id.clone(), row_spec.id.clone(), col_spec.id.clone());
                        let coord = layout_map.get_cell(&cell_id).ok_or_else(|| {
                            EmissionError::MissingCell {
                                cell: cell_id.clone(),
                            }
                        })?;

                        let node = grid.get(&cell_id);
                        let is_input = node.is_some_and(|n| n.payload.is_literal());
                        let is_descriptor = col_spec.role.is_descriptor();

                        let format_key = CellFormatKey::new(
                            coord.style_role,
                            node.and_then(|n| n.format),
                            node.and_then(|n| n.decimals),
                            node.and_then(|n| n.currency),
                            is_input,
                            is_descriptor,
                        );
                        let cell_format = format_cache.get_cell_format(&format_key)?;

                        if options.values_only {
                            // Values-only freezing: write evaluated result as literal
                            if let Some(val) = evaluated.and_then(|e| e.get_cell(&cell_id)) {
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
                                worksheet.write_blank(coord.row, coord.col as u16, &cell_format)?;
                            }
                        } else {
                            // Live formulas mode
                            if let Some(formula_str) = lowered_formulas.get(&cell_id) {
                                let mut formula = Formula::new(formula_str);
                                if let Some(val) = evaluated.and_then(|e| e.get_cell(&cell_id)) {
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
                                worksheet.write_blank(coord.row, coord.col as u16, &cell_format)?;
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

            // 5. Interleaved Section Headers
            for sec in &sheet_layout.section_header_rows {
                worksheet.write_string_with_format(
                    sec.row,
                    sec.descriptor_col as u16,
                    &sec.label,
                    &format_cache.section_header_format(),
                )?;
            }
        }

        Ok(workbook)
    }
}

/// Recursively applies row group outline levels to the worksheet.
fn apply_row_group_outlines(
    group: &RowGroup,
    table: &TableSpec,
    layout_map: &LayoutMap,
    section_headers: &[SectionHeaderRow],
    worksheet: &mut Worksheet,
) -> Result<(), EmissionError> {
    let (first_row_id, last_row_id) = find_first_and_last_row_ids(group);

    if let (Some(first_id), Some(last_id)) = (first_row_id, last_row_id) {
        let first_col_id =
            table
                .columns
                .first()
                .map(|c| &c.id)
                .ok_or_else(|| EmissionError::MissingTable {
                    table: table.id.clone(),
                })?;

        // 1. Determine start row: section header row if present, else first child row
        let start_row = if let Some(sh) = section_headers
            .iter()
            .find(|sh| sh.table_id == table.id && sh.group_id == group.id.as_str())
        {
            sh.row
        } else {
            let first_cell_id =
                CellId::new(table.id.clone(), first_id.clone(), first_col_id.clone());
            let first_coord =
                layout_map
                    .get_cell(&first_cell_id)
                    .ok_or(EmissionError::MissingCell {
                        cell: first_cell_id,
                    })?;
            first_coord.row
        };

        // 2. Determine end row: physical row of last child data row
        let last_cell_id = CellId::new(table.id.clone(), last_id.clone(), first_col_id.clone());
        let last_coord = layout_map
            .get_cell(&last_cell_id)
            .ok_or(EmissionError::MissingCell { cell: last_cell_id })?;
        let end_row = last_coord.row;

        if start_row <= end_row {
            worksheet.group_rows(start_row, end_row)?;
        }
    }

    // 3. Recurse into nested sub-groups
    for child in &group.children {
        if let RowGroupChild::Group(sub_group) = child {
            apply_row_group_outlines(sub_group, table, layout_map, section_headers, worksheet)?;
        }
    }

    Ok(())
}

/// Finds references to the first and last `RowId`s contained within a hierarchical `RowGroup`.
fn find_first_and_last_row_ids(group: &RowGroup) -> (Option<&RowId>, Option<&RowId>) {
    let mut first = None;
    let mut last = None;
    fn walk<'a>(g: &'a RowGroup, first: &mut Option<&'a RowId>, last: &mut Option<&'a RowId>) {
        for child in &g.children {
            match child {
                RowGroupChild::Row(r) => {
                    if first.is_none() {
                        *first = Some(r);
                    }
                    *last = Some(r);
                }
                RowGroupChild::Group(sub) => {
                    walk(sub, first, last);
                }
            }
        }
    }
    walk(group, &mut first, &mut last);
    (first, last)
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
