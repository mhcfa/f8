//! Pass 5: Spatial layout planning and 2D grid coordinate assignment.
//!
//! Arranges tables, gutters, headers, and freeze panes into `LayoutMap`.

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
use crate::compiler::ir::id::CellId;
use crate::model::format::{CurrencyCode, FormatPreset, SemanticStyle, Shade};
use crate::model::id::{SheetId, TableId};
use crate::model::manifest::{FreezePanes, ProjectManifest};
use crate::model::table::{ColumnSpec, RowSpec, TableSpec, ValueLiteral};
use crate::model::theme::Theme;
use crate::value::{CellValue, format_accounting_number};

/// Semantic styling role assigned to a physical coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StyleRole {
    /// Horizontal summary borders (top/bottom).
    pub horizontal: SemanticStyle,
    /// Vertical summary borders (left/right).
    pub vertical: SemanticStyle,
    /// Visual highlight emphasis.
    pub is_highlight: bool,
    /// Semantic background shade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade: Option<Shade>,
}

impl StyleRole {
    /// Creates a default data styling role with no borders, no highlight, and no shade.
    #[must_use]
    pub const fn data() -> Self {
        Self {
            horizontal: SemanticStyle::Data,
            vertical: SemanticStyle::Data,
            is_highlight: false,
            shade: None,
        }
    }

    /// Builder method attaching a semantic background shade.
    #[must_use]
    pub const fn with_shade(mut self, shade: Option<Shade>) -> Self {
        self.shade = shade;
        self
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
    /// Descriptor text indentation level.
    pub indent: u8,
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
    /// Inclusive physical row range of header tiers, or None if the table has zero headers.
    pub header_row_range: Option<(u32, u32)>,
    /// Inclusive physical row range of table body rows (data, section headers, gutters): `(start_row, end_row)`.
    pub body_row_range: (u32, u32),
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
                .any(|sheet| sheet.tables.contains(&table.id));
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
            for table_id in &sheet_spec.tables {
                let matched = tables.iter().find(|t| t.id == *table_id).ok_or_else(|| {
                    LayoutError::UnknownTable {
                        table: table_id.clone(),
                        sheet: sheet_id.clone(),
                    }
                })?;
                sheet_tables.push(matched);
            }

            let mut row_heights: IndexMap<u32, f64> = IndexMap::new();
            let mut col_widths: IndexMap<u32, f64> = IndexMap::new();

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

                // Multi-tier Column Header Rows
                let max_header_levels = table
                    .columns
                    .iter()
                    .filter_map(|col| match col {
                        ColumnSpec::Data(d) => Some(d.header_depth()),
                        ColumnSpec::Gutter => None,
                    })
                    .max()
                    .unwrap_or(0);

                let header_row_range = if max_header_levels > 0 {
                    let header_start = current_row;
                    for _ in 0..max_header_levels {
                        if current_row >= MAX_EXCEL_ROWS {
                            return Err(LayoutError::GridLimitExceeded {
                                dimension: "rows",
                                max: MAX_EXCEL_ROWS,
                                actual: current_row,
                            });
                        }
                        row_heights.insert(current_row, theme.metrics.header_row_height);
                        current_row += 1;
                    }
                    let header_end = current_row - 1;
                    Some((header_start, header_end))
                } else {
                    None
                };

                let body_start_row = current_row;

                for row_spec in &table.rows {
                    if current_row >= MAX_EXCEL_ROWS {
                        return Err(LayoutError::GridLimitExceeded {
                            dimension: "rows",
                            max: MAX_EXCEL_ROWS,
                            actual: current_row,
                        });
                    }

                    match row_spec {
                        RowSpec::Gutter => {
                            row_heights.insert(current_row, theme.metrics.gutter_row_height);
                            current_row += 1;
                        }
                        RowSpec::SectionHeader(_) => {
                            row_heights.insert(current_row, theme.metrics.section_row_height);
                            current_row += 1;
                        }
                        RowSpec::Data(data_row) => {
                            row_heights.insert(current_row, theme.metrics.data_row_height);

                            for (c_idx, col_spec) in table.columns.iter().enumerate() {
                                let physical_col = start_col + (c_idx as u32);
                                let ColumnSpec::Data(data_col) = col_spec else {
                                    continue;
                                };

                                let cell_id = CellId::new(
                                    table.id.clone(),
                                    data_row.id.clone(),
                                    data_col.id.clone(),
                                );
                                let a1 = coordinate_to_a1(physical_col, current_row)?;

                                let (style_role, indent) = if let Some(node) = grid.get(&cell_id) {
                                    (
                                        StyleRole {
                                            horizontal: node.horizontal,
                                            vertical: node.vertical,
                                            is_highlight: node.is_highlight,
                                            shade: node.shade,
                                        },
                                        node.indent,
                                    )
                                } else {
                                    let shade = data_row
                                        .shade
                                        .filter(|s| !s.is_none())
                                        .or_else(|| data_col.shade.filter(|s| !s.is_none()));
                                    (
                                        StyleRole {
                                            horizontal: data_row
                                                .style
                                                .unwrap_or(SemanticStyle::Data),
                                            vertical: data_col.style.unwrap_or(SemanticStyle::Data),
                                            is_highlight: data_row.highlight.unwrap_or(false)
                                                || data_col.highlight.unwrap_or(false),
                                            shade,
                                        },
                                        0,
                                    )
                                };

                                layout_cells.insert(
                                    cell_id,
                                    PhysicalCoordinate {
                                        sheet: sheet_id.clone(),
                                        col: physical_col,
                                        row: current_row,
                                        a1,
                                        style_role,
                                        indent,
                                    },
                                );
                            }

                            current_row += 1;
                        }
                    }
                }

                let body_end_row = current_row.saturating_sub(1);
                let table_end_row = body_end_row;

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
                        header_row_range,
                        body_row_range: (body_start_row, body_end_row),
                    },
                );
            }

            // 3. Sheet-wide column width arbitration across stacked tables
            ColumnArbitrationContext {
                tables: &sheet_tables,
                grid,
                layout_cells: &layout_cells,
                theme,
                evaluated,
            }
            .arbitrate(start_col, &mut col_widths);

            layout_sheets.insert(
                sheet_id.clone(),
                SheetLayout {
                    sheet: sheet_id,
                    col_widths,
                    row_heights,
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

/// Context and read-only inputs for arbitrating sheet-wide column widths across stacked tables.
struct ColumnArbitrationContext<'a> {
    tables: &'a [&'a TableSpec],
    grid: &'a CellGrid,
    layout_cells: &'a IndexMap<CellId, PhysicalCoordinate>,
    theme: &'a Theme,
    evaluated: Option<&'a EvaluatedModel>,
}

impl ColumnArbitrationContext<'_> {
    /// Arbitrates sheet-wide column widths across all stacked tables sharing worksheet columns.
    fn arbitrate(&self, start_col: u32, col_widths: &mut IndexMap<u32, f64>) {
        let max_col_count = self
            .tables
            .iter()
            .map(|t| t.columns.len() as u32)
            .max()
            .unwrap_or(0);

        for col_offset in 0..max_col_count {
            let physical_col = start_col + col_offset;

            // Check if every table occupying this column is a Gutter column
            let all_gutter = self
                .tables
                .iter()
                .all(|t| match t.columns.get(col_offset as usize) {
                    Some(ColumnSpec::Gutter) | None => true,
                    Some(ColumnSpec::Data(_)) => false,
                });

            if all_gutter {
                col_widths.insert(physical_col, self.theme.metrics.gutter_col_width);
                continue;
            }

            let mut max_chars_across_tables: usize = 0;

            for table in self.tables {
                let Some(col_spec) = table.columns.get(col_offset as usize) else {
                    continue;
                };
                let ColumnSpec::Data(data_col) = col_spec else {
                    continue;
                };

                let mut table_max_chars: usize = 0;

                // 1. Column header text length (max across all stacked per-column header lines; excludes spanning group)
                let header_len = data_col.max_header_chars();
                table_max_chars = table_max_chars.max(header_len);

                // 2. Row cells text length
                for row in &table.rows {
                    let RowSpec::Data(data_row) = row else {
                        continue;
                    };
                    let cell_id =
                        CellId::new(table.id.clone(), data_row.id.clone(), data_col.id.clone());
                    let cell_node = self.grid.get(&cell_id);
                    let format = cell_node.and_then(|n| n.format);
                    let decimals = cell_node.and_then(|n| n.decimals).or_else(|| {
                        format.and_then(|f| self.theme.number_formats.default_decimals(f))
                    });
                    let currency = cell_node.and_then(|n| n.currency);

                    let mut char_len =
                        match (self.evaluated.and_then(|e| e.get_cell(&cell_id)), cell_node) {
                            (Some(val), _) => {
                                cell_value_string_length(val, format, decimals, currency)
                            }
                            (None, Some(node)) => match &node.payload {
                                CellPayload::Literal(lit) => {
                                    literal_value_length(lit, format, decimals, currency)
                                }
                                CellPayload::Formula(_) => 0,
                            },
                            (None, None) => 0,
                        };

                    if let Some(coord) = self.layout_cells.get(&cell_id)
                        && coord.indent > 0
                    {
                        char_len += (coord.indent as usize) * 2;
                    }

                    table_max_chars = table_max_chars.max(char_len);
                }

                max_chars_across_tables = max_chars_across_tables.max(table_max_chars);
            }

            // Apply theme font multiplier and padding, enforcing min_col_width
            let computed_width = (max_chars_across_tables as f64
                * self.theme.metrics.char_width_multiplier
                + self.theme.metrics.col_padding)
                .max(self.theme.metrics.min_col_width);

            let final_width = (computed_width * 100.0).round() / 100.0;
            let existing = col_widths.get(&physical_col).copied().unwrap_or(0.0);
            col_widths.insert(physical_col, existing.max(final_width));
        }
    }
}

fn literal_value_length(
    lit: &ValueLiteral,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
) -> usize {
    match lit {
        ValueLiteral::String(s) => s.chars().count(),
        ValueLiteral::Number(n) => formatted_number_estimate(*n, format, decimals, currency),
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
fn cell_value_string_length(
    val: &CellValue,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
) -> usize {
    match val {
        CellValue::Text(s) => s.chars().count(),
        CellValue::Number(n) => formatted_number_estimate(*n, format, decimals, currency),
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

/// Provides an institutional width estimate for formatted numbers using accounting typography and trailing alignment space.
fn formatted_number_estimate(
    n: f64,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
) -> usize {
    if !n.is_finite() {
        return 4; // NaN, Inf
    }
    let formatted = format_accounting_number(n, format, decimals, currency);
    let trailing_pad = usize::from(
        !formatted.ends_with(')')
            && matches!(
                format,
                Some(FormatPreset::Currency | FormatPreset::Number | FormatPreset::Percentage)
            ),
    );
    formatted.chars().count() + trailing_pad
}
