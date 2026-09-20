//! Pass 5: Spatial layout planning and 2D grid coordinate assignment.
//!
//! Arranges tables, gutters, headers, and freeze panes into `LayoutMap`
//! as documented in `docs/compiler/spatial-layout.md`.

pub mod a1;
pub mod error;

pub use a1::{
    MAX_EXCEL_COLUMNS, MAX_EXCEL_ROWS, MAX_OUTLINE_LEVEL, col_to_excel_col, coordinate_to_a1,
};
pub use error::LayoutError;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::compiler::eval::EvaluatedModel;
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::expand::matches_table_id;
use crate::compiler::ir::id::CellId;
use crate::model::format::{FormatPreset, SemanticStyle};
use crate::model::id::{RowId, SheetId, TableId};
use crate::model::manifest::{FreezePanes, ProjectManifest};
use crate::model::table::{RowGroup, RowGroupChild, TableSpec, ValueLiteral};
use crate::model::theme::Theme;
use crate::value::CellValue;

/// Semantic styling role assigned to a physical coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleRole {
    /// Horizontal summary borders (top/bottom).
    pub horizontal: SemanticStyle,
    /// Vertical summary borders (left/right).
    pub vertical: SemanticStyle,
    /// Visual highlight emphasis.
    pub is_highlight: bool,
}

impl StyleRole {
    /// Creates a default data styling role with no borders and no highlight.
    #[must_use]
    pub const fn data() -> Self {
        Self {
            horizontal: SemanticStyle::Data,
            vertical: SemanticStyle::Data,
            is_highlight: false,
        }
    }
}

/// Physical 2D location and presentation styling role for a cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalCoordinate {
    /// Worksheet containing this cell.
    pub sheet: SheetId,
    /// 0-indexed column index on worksheet.
    pub col: u32,
    /// 0-indexed row index on worksheet.
    pub row: u32,
    /// Canonical 1-indexed Excel A1 address string (e.g. `"B2"`, `"AA10"`).
    pub a1: String,
    /// Semantic presentation style role.
    pub style_role: StyleRole,
}

/// Placement and span of a super-header group tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperHeaderPlacement {
    /// 0-indexed physical row index.
    pub row: u32,
    /// Identifier of the column group.
    pub group_id: String,
    /// Display label for the super-header.
    pub label: String,
    /// 0-indexed physical starting column index (inclusive).
    pub start_col: u32,
    /// 0-indexed physical ending column index (inclusive).
    pub end_col: u32,
}

/// Visual section header row inserted before a hierarchical row group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionHeaderRow {
    /// 0-indexed physical row index.
    pub row: u32,
    /// Parent table identifier.
    pub table_id: TableId,
    /// Row group identifier.
    pub group_id: String,
    /// Display label for the section header.
    pub label: String,
    /// Excel outline nesting level (1..=7).
    pub group_level: u8,
    /// 0-indexed physical column index displaying the descriptor text.
    pub descriptor_col: u32,
}

/// Physical bounding box and structural row offsets for a table on a worksheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TablePlacement {
    /// Worksheet containing this table.
    pub sheet: SheetId,
    /// 0-indexed physical starting column (inclusive).
    pub start_col: u32,
    /// 0-indexed physical starting row (inclusive, top of title or first header).
    pub start_row: u32,
    /// 0-indexed physical ending column (inclusive).
    pub end_col: u32,
    /// 0-indexed physical ending row (inclusive, bottom of last data row).
    pub end_row: u32,
    /// Physical row index of the table title banner if present.
    pub title_row: Option<u32>,
    /// Physical row index of the table subtitle banner if present.
    pub subtitle_row: Option<u32>,
    /// Physical row index of the blank buffer gutter below title/subtitle if present.
    pub title_gutter_row: Option<u32>,
    /// Placements of super-header tiers.
    pub super_header_rows: Vec<SuperHeaderPlacement>,
    /// Physical row index of the column key header row.
    pub col_header_row: u32,
    /// Physical row index range `(start_row, end_row)` of data calculation rows (inclusive).
    pub data_row_range: (u32, u32),
}

/// Sheet-level physical layout metrics, widths, heights, and visual rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetLayout {
    /// Worksheet identifier.
    pub sheet: SheetId,
    /// Sheet-wide column widths (0-indexed col -> character widths).
    pub col_widths: IndexMap<u32, f64>,
    /// Sheet-wide row heights (0-indexed row -> points).
    pub row_heights: IndexMap<u32, f64>,
    /// Interleaved visual section header rows.
    pub section_header_rows: Vec<SectionHeaderRow>,
    /// Frozen panes coordinate if configured.
    pub freeze_panes: Option<FreezePanes>,
    /// Gridline display visibility flag.
    pub show_gridlines: bool,
    /// Tab accent color hex string if present.
    pub tab_color: Option<String>,
}

/// Complete deterministic spatial layout contract produced by Pass 5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutMap {
    /// Physical location and style role for every discrete `CellId`.
    pub cells: IndexMap<CellId, PhysicalCoordinate>,
    /// Bounding boxes and structural row offsets for every table.
    pub tables: IndexMap<TableId, TablePlacement>,
    /// Sheet-level layout metrics, widths, heights, and visual rows.
    pub sheets: IndexMap<SheetId, SheetLayout>,
}

impl LayoutMap {
    /// Retrieves physical coordinates for a cell identifier if present.
    #[must_use]
    pub fn get_cell(&self, id: &CellId) -> Option<&PhysicalCoordinate> {
        self.cells.get(id)
    }

    /// Retrieves table bounding box placement for a table identifier if present.
    #[must_use]
    pub fn get_table(&self, id: &TableId) -> Option<&TablePlacement> {
        self.tables.get(id)
    }

    /// Retrieves sheet layout configuration for a sheet identifier if present.
    #[must_use]
    pub fn get_sheet(&self, id: &SheetId) -> Option<&SheetLayout> {
        self.sheets.get(id)
    }
}

/// Spatial layout planner orchestrating Pass 5 grid coordinate assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LayoutPlanner;

impl LayoutPlanner {
    /// Plans 2D physical coordinates, bounding boxes, row heights, and column widths
    /// for all tables and sheets declared in the manifest.
    ///
    /// # Errors
    /// Returns [`LayoutError`] if unassigned tables exist, referenced tables cannot be resolved,
    /// outline hierarchy exceeds depth 7, or coordinates exceed Excel grid limits.
    pub fn plan(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        grid: &CellGrid,
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
    ) -> Result<LayoutMap, LayoutError> {
        // 1. Verify every table belongs to at least one sheet
        for table in tables {
            let assigned = manifest
                .sheets
                .iter()
                .any(|sheet| sheet.tables.iter().any(|p| matches_table_id(p, &table.id)));
            if !assigned {
                return Err(LayoutError::UnassignedTable {
                    table: table.id.clone(),
                });
            }
        }

        let mut layout_cells: IndexMap<CellId, PhysicalCoordinate> = IndexMap::new();
        let mut layout_tables: IndexMap<TableId, TablePlacement> = IndexMap::new();
        let mut layout_sheets: IndexMap<SheetId, SheetLayout> = IndexMap::new();

        // 2. Process each worksheet independently
        for sheet_spec in &manifest.sheets {
            let sheet_id = SheetId::from(sheet_spec.name.as_str());

            // Resolve tables belonging to this sheet in declared order
            let mut sheet_tables = Vec::new();
            for table_path in &sheet_spec.tables {
                let matched = tables
                    .iter()
                    .find(|t| matches_table_id(table_path, &t.id))
                    .ok_or_else(|| LayoutError::UnknownTable {
                        table: TableId::from(
                            table_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default(),
                        ),
                        sheet: sheet_id.clone(),
                    })?;
                sheet_tables.push(matched);
            }

            let mut row_heights: IndexMap<u32, f64> = IndexMap::new();
            let mut col_widths: IndexMap<u32, f64> = IndexMap::new();
            let mut section_header_rows: Vec<SectionHeaderRow> = Vec::new();

            // Default margins: Row 0 is top gutter, Column 0 is left gutter
            row_heights.insert(0, theme.metrics.gutter_row_height);
            col_widths.insert(0, theme.metrics.gutter_col_width);

            let mut current_row: u32 = 1;
            let start_col: u32 = 1; // Primary table starts at Column B

            for (table_idx, table) in sheet_tables.iter().enumerate() {
                // Inter-table gutters: separate stacked tables with 2 blank rows
                if table_idx > 0 {
                    row_heights.insert(current_row, theme.metrics.gutter_row_height);
                    current_row += 1;
                    row_heights.insert(current_row, theme.metrics.gutter_row_height);
                    current_row += 1;
                }

                let table_start_row = current_row;
                let num_cols = table.columns.len() as u32;
                if num_cols == 0 {
                    continue;
                }
                let end_col = start_col + num_cols - 1;

                if end_col >= MAX_EXCEL_COLUMNS {
                    return Err(LayoutError::GridLimitExceeded {
                        dimension: "columns",
                        max: MAX_EXCEL_COLUMNS,
                        actual: end_col,
                    });
                }

                // Table Title Row
                let title_row = if !table.title.trim().is_empty() {
                    let r = current_row;
                    row_heights.insert(r, theme.metrics.title_row_height);
                    current_row += 1;
                    Some(r)
                } else {
                    None
                };

                // Table Subtitle Row
                let subtitle_row = if table.subtitle.is_some() {
                    let r = current_row;
                    row_heights.insert(r, theme.metrics.data_row_height);
                    current_row += 1;
                    Some(r)
                } else {
                    None
                };

                // Title Buffer Gutter
                let title_gutter_row = if title_row.is_some() || subtitle_row.is_some() {
                    let r = current_row;
                    row_heights.insert(r, theme.metrics.title_gutter_row_height);
                    current_row += 1;
                    Some(r)
                } else {
                    None
                };

                // Super-Header Rows
                let mut super_header_rows = Vec::new();
                if !table.column_groups.is_empty() {
                    let super_row = current_row;
                    row_heights.insert(super_row, theme.metrics.header_row_height);
                    current_row += 1;

                    for cg in &table.column_groups {
                        let (Some(first_col_id), Some(last_col_id)) =
                            (cg.columns.first(), cg.columns.last())
                        else {
                            continue;
                        };

                        let first_idx = table
                            .columns
                            .iter()
                            .position(|c| &c.id == first_col_id)
                            .unwrap_or(0) as u32;
                        let last_idx = table
                            .columns
                            .iter()
                            .position(|c| &c.id == last_col_id)
                            .unwrap_or(0) as u32;

                        super_header_rows.push(SuperHeaderPlacement {
                            row: super_row,
                            group_id: cg.id.to_string(),
                            label: cg.label.clone(),
                            start_col: start_col + first_idx,
                            end_col: start_col + last_idx,
                        });
                    }
                }

                // Column Key Header Row
                let col_header_row = current_row;
                row_heights.insert(col_header_row, theme.metrics.header_row_height);
                current_row += 1;

                // Build row group outline hierarchy and first-descendant lookup
                let group_info = analyze_row_groups(&table.row_groups, &table.id)?;

                let data_start_row = current_row;

                for row_spec in &table.rows {
                    // Check if one or more section headers must be interleaved before this row
                    if let Some(groups_starting_here) = group_info.starts_at.get(&row_spec.id) {
                        for (group_id, group_label, depth) in groups_starting_here {
                            let sec_row = current_row;
                            row_heights.insert(sec_row, theme.metrics.section_row_height);
                            section_header_rows.push(SectionHeaderRow {
                                row: sec_row,
                                table_id: table.id.clone(),
                                group_id: group_id.clone(),
                                label: group_label.clone(),
                                group_level: *depth as u8,
                                descriptor_col: start_col,
                            });
                            current_row += 1;
                        }
                    }

                    // Place the data calculation row
                    let physical_row = current_row;
                    if physical_row >= MAX_EXCEL_ROWS {
                        return Err(LayoutError::GridLimitExceeded {
                            dimension: "rows",
                            max: MAX_EXCEL_ROWS,
                            actual: physical_row,
                        });
                    }

                    row_heights.insert(physical_row, theme.metrics.data_row_height);

                    for (c_idx, col_spec) in table.columns.iter().enumerate() {
                        let physical_col = start_col + (c_idx as u32);
                        let cell_id =
                            CellId::new(table.id.clone(), row_spec.id.clone(), col_spec.id.clone());
                        let a1 = coordinate_to_a1(physical_col, physical_row)?;

                        let style_role = if let Some(node) = grid.get(&cell_id) {
                            StyleRole {
                                horizontal: node.horizontal,
                                vertical: node.vertical,
                                is_highlight: node.is_highlight,
                            }
                        } else {
                            StyleRole {
                                horizontal: row_spec.style.unwrap_or(SemanticStyle::Data),
                                vertical: col_spec.style.unwrap_or(SemanticStyle::Data),
                                is_highlight: row_spec.highlight.unwrap_or(false)
                                    || col_spec.highlight.unwrap_or(false),
                            }
                        };

                        layout_cells.insert(
                            cell_id,
                            PhysicalCoordinate {
                                sheet: sheet_id.clone(),
                                col: physical_col,
                                row: physical_row,
                                a1,
                                style_role,
                            },
                        );
                    }

                    current_row += 1;
                }

                let data_end_row = current_row.saturating_sub(1);
                let table_end_row = data_end_row;

                layout_tables.insert(
                    table.id.clone(),
                    TablePlacement {
                        sheet: sheet_id.clone(),
                        start_col,
                        start_row: table_start_row,
                        end_col,
                        end_row: table_end_row,
                        title_row,
                        subtitle_row,
                        title_gutter_row,
                        super_header_rows,
                        col_header_row,
                        data_row_range: (data_start_row, data_end_row),
                    },
                );
            }

            // 3. Sheet-wide column width arbitration across stacked tables
            arbitrate_sheet_column_widths(
                &sheet_tables,
                grid,
                start_col,
                theme,
                evaluated,
                &section_header_rows,
                &mut col_widths,
            );

            layout_sheets.insert(
                sheet_id.clone(),
                SheetLayout {
                    sheet: sheet_id,
                    col_widths,
                    row_heights,
                    section_header_rows,
                    freeze_panes: sheet_spec.freeze_panes,
                    show_gridlines: sheet_spec.show_gridlines,
                    tab_color: sheet_spec.tab_color.clone(),
                },
            );
        }

        Ok(LayoutMap {
            cells: layout_cells,
            tables: layout_tables,
            sheets: layout_sheets,
        })
    }
}

/// Analysis metadata for row groups.
struct RowGroupAnalysis {
    /// Maps each RowId that starts one or more groups to `Vec<(group_id, label, depth)>` in ascending depth order.
    starts_at: IndexMap<RowId, Vec<(String, String, usize)>>,
}

/// Recursively analyzes hierarchical row groups, checking depth limits and determining section header insertion points.
fn analyze_row_groups(
    groups: &[RowGroup],
    table_id: &TableId,
) -> Result<RowGroupAnalysis, LayoutError> {
    let mut starts_at: IndexMap<RowId, Vec<(String, String, usize)>> = IndexMap::new();

    for group in groups {
        analyze_group_recursive(group, 1, table_id, &mut starts_at)?;
    }

    for list in starts_at.values_mut() {
        list.sort_by_key(|(_, _, depth)| *depth);
    }

    Ok(RowGroupAnalysis { starts_at })
}

fn analyze_group_recursive(
    group: &RowGroup,
    depth: usize,
    table_id: &TableId,
    starts_at: &mut IndexMap<RowId, Vec<(String, String, usize)>>,
) -> Result<RowId, LayoutError> {
    if depth > MAX_OUTLINE_LEVEL {
        return Err(LayoutError::OutlineDepthExceeded {
            table: table_id.clone(),
            group: group.id.to_string(),
            depth,
        });
    }

    let mut first_row: Option<RowId> = None;

    for child in &group.children {
        match child {
            RowGroupChild::Row(r_id) => {
                if first_row.is_none() {
                    first_row = Some(r_id.clone());
                }
            }
            RowGroupChild::Group(sub_group) => {
                let sub_first = analyze_group_recursive(sub_group, depth + 1, table_id, starts_at)?;
                if first_row.is_none() {
                    first_row = Some(sub_first);
                }
            }
        }
    }

    if let Some(first) = first_row {
        starts_at.entry(first.clone()).or_default().push((
            group.id.to_string(),
            group.label.clone(),
            depth,
        ));
        Ok(first)
    } else {
        // Empty group, shouldn't occur after validation
        Ok(RowId::from(""))
    }
}

/// Arbitrates sheet-wide column widths across all stacked tables sharing worksheet columns.
fn arbitrate_sheet_column_widths(
    tables: &[&TableSpec],
    grid: &CellGrid,
    start_col: u32,
    theme: &Theme,
    evaluated: Option<&EvaluatedModel>,
    section_headers: &[SectionHeaderRow],
    col_widths: &mut IndexMap<u32, f64>,
) {
    // Find maximum column index used across all tables on this sheet
    let max_col_count = tables
        .iter()
        .map(|t| t.columns.len() as u32)
        .max()
        .unwrap_or(0);

    for col_offset in 0..max_col_count {
        let physical_col = start_col + col_offset;
        let mut max_chars_across_tables: usize = 0;

        for table in tables {
            if (col_offset as usize) >= table.columns.len() {
                continue;
            }

            let col_spec = &table.columns[col_offset as usize];
            let mut table_max_chars: usize = 0;

            // 1. Column header text length
            let header_len = col_spec
                .label
                .as_deref()
                .unwrap_or(col_spec.id.as_str())
                .chars()
                .count();
            table_max_chars = table_max_chars.max(header_len);

            // 2. Section headers in descriptor column (col_offset == 0)
            if col_offset == 0 {
                for sh in section_headers {
                    if sh.table_id == table.id {
                        let sec_len = sh.label.chars().count() + ((sh.group_level as usize) * 2);
                        table_max_chars = table_max_chars.max(sec_len);
                    }
                }
            }

            // 3. Row cells text length
            for row in &table.rows {
                let cell_id = CellId::new(table.id.clone(), row.id.clone(), col_spec.id.clone());
                let cell_node = grid.get(&cell_id);
                let format = cell_node.and_then(|n| n.format);

                let char_len = match (evaluated.and_then(|e| e.get_cell(&cell_id)), cell_node) {
                    (Some(val), _) => cell_value_string_length(val, format),
                    (None, Some(node)) => match &node.payload {
                        CellPayload::Literal(lit) => literal_value_length(lit, format),
                        CellPayload::Formula(_) => 0,
                    },
                    (None, None) => 0,
                };

                table_max_chars = table_max_chars.max(char_len);
            }

            max_chars_across_tables = max_chars_across_tables.max(table_max_chars);
        }

        // Apply theme font multiplier and padding, enforcing min_col_width
        let computed_width = (max_chars_across_tables as f64 * theme.metrics.char_width_multiplier
            + theme.metrics.col_padding)
            .max(theme.metrics.min_col_width);

        let final_width = (computed_width * 100.0).round() / 100.0;
        let existing = col_widths.get(&physical_col).copied().unwrap_or(0.0);
        col_widths.insert(physical_col, existing.max(final_width));
    }
}

fn literal_value_length(lit: &ValueLiteral, format: Option<FormatPreset>) -> usize {
    match lit {
        ValueLiteral::String(s) => s.chars().count(),
        ValueLiteral::Number(n) => formatted_number_estimate(*n, format),
        ValueLiteral::Boolean(b) => {
            if *b {
                4
            } else {
                5
            }
        }
        ValueLiteral::Null => 0,
    }
}

/// Computes the approximate character length of an evaluated `CellValue`.
fn cell_value_string_length(val: &CellValue, format: Option<FormatPreset>) -> usize {
    match val {
        CellValue::Text(s) => s.chars().count(),
        CellValue::Number(n) => formatted_number_estimate(*n, format),
        CellValue::Boolean(b) => {
            if *b {
                4
            } else {
                5
            }
        }
        CellValue::Error(e) => e.to_string().chars().count(),
        CellValue::Empty => 0,
    }
}

/// Provides an institutional width estimate for formatted numbers including commas and symbols.
fn formatted_number_estimate(n: f64, format: Option<FormatPreset>) -> usize {
    if !n.is_finite() {
        return 4; // NaN, Inf
    }

    let is_negative = n < 0.0;
    let abs_n = n.abs();

    // Integer digits count
    let int_digits = if abs_n < 1.0 {
        1
    } else {
        (abs_n.log10().floor() as usize) + 1
    };

    // Commas count (1 comma per 3 digits over 3)
    let commas = if int_digits > 3 {
        (int_digits - 1) / 3
    } else {
        0
    };

    let mut base_len = int_digits + commas;

    match format {
        Some(FormatPreset::Currency) => {
            base_len += 2; // "$ " prefix
            base_len += 3; // ".00"
            if is_negative {
                base_len += 2; // "()"
            }
        }
        Some(FormatPreset::Percentage) => {
            base_len += 3; // ".0%"
            if is_negative {
                base_len += 1; // "-"
            }
        }
        Some(FormatPreset::Multiple) => {
            base_len += 3; // ".0x"
            if is_negative {
                base_len += 1; // "-"
            }
        }
        Some(FormatPreset::BasisPoints) => {
            base_len += 4; // " bps"
            if is_negative {
                base_len += 1; // "-"
            }
        }
        _ => {
            base_len += 3; // ".00" default decimals
            if is_negative {
                base_len += 2; // "()" or "-"
            }
        }
    }

    base_len
}
