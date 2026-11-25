//! Pass 5: Spatial layout planning and 2D grid coordinate assignment.
//!
//! Arranges tables, gutters, headers, and freeze panes into `LayoutMap`.

pub mod a1;
pub mod error;

pub use a1::{MAX_EXCEL_COLUMNS, MAX_EXCEL_ROWS, col_to_excel_col, coordinate_to_a1};
pub use error::LayoutError;

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

use crate::compiler::eval::EvaluatedModel;
use crate::compiler::ir::cell::{CellGrid, CellNode, CellPayload};
use crate::compiler::ir::id::CellId;
use crate::model::format::{CurrencyCode, FormatPreset, SemanticStyle, Shade};
use crate::model::id::{SheetId, TableId};
use crate::model::manifest::{
    DEFAULT_HORIZONTAL_TABLE_GAP, DEFAULT_VERTICAL_TABLE_GAP, FreezePanes, MAX_GRID_COL_WIDTH,
    MIN_GRID_COL_WIDTH, ProjectManifest, SheetLayoutSpec, SheetSpec,
};
use crate::model::table::{
    AxisRole, ColumnSpec, DataColumnSpec, DataRowSpec, RowSpec, TableSpec, ValueLiteral,
};
use crate::model::theme::{MetricsConfig, Theme};
use crate::value::{CellValue, format_accounting_number};

const GRID_WIDTH_EPS: f64 = 1e-9;

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn grid_descriptor_header_chars(header: &str) -> usize {
    if header.is_empty() {
        0
    } else {
        header.chars().count() + 1
    }
}

fn grid_descriptor_text_chars(text: &str, indent: u8) -> usize {
    if text.is_empty() {
        0
    } else {
        text.chars().count() + 1 + usize::from(indent) * 2
    }
}

fn grid_descriptor_req_width_from_chars(max_desc_chars: usize, metrics: &MetricsConfig) -> f64 {
    if max_desc_chars == 0 {
        0.0
    } else {
        round2(max_desc_chars as f64 * metrics.char_width_multiplier + metrics.col_padding)
    }
}

/// Computes Pass 5's required visual width (`req_w`) for each column of a table on a `grid` sheet
/// (1:1 with `table.columns`; non-descriptor columns and empty descriptors yield `0.0`).
pub(crate) fn grid_table_descriptor_req_widths(
    table: &TableSpec,
    grid: &CellGrid,
    evaluated: &EvaluatedModel,
    theme: &Theme,
) -> Vec<f64> {
    let mut col_desc_chars = vec![0usize; table.columns.len()];

    for (c_idx, col_spec) in table.columns.iter().enumerate() {
        let ColumnSpec::Data(data_col) = col_spec else {
            continue;
        };
        if !data_col.role.is_descriptor() {
            continue;
        }
        for h in &data_col.headers {
            if let Some(slot) = col_desc_chars.get_mut(c_idx) {
                *slot = (*slot).max(grid_descriptor_header_chars(h));
            }
        }
    }

    for row_spec in &table.rows {
        let RowSpec::Data(data_row) = row_spec else {
            continue;
        };
        for (c_idx, col_spec) in table.columns.iter().enumerate() {
            let ColumnSpec::Data(data_col) = col_spec else {
                continue;
            };
            if !data_col.role.is_descriptor() {
                continue;
            }
            let cell_id = CellId::new(table.id.clone(), data_row.id.clone(), data_col.id.clone());
            if let Some(val) = evaluated.get_cell(&cell_id) {
                match val {
                    CellValue::Text(s) => {
                        let indent = grid.get(&cell_id).map_or(0, |n| n.indent);
                        if let Some(slot) = col_desc_chars.get_mut(c_idx) {
                            *slot = (*slot).max(grid_descriptor_text_chars(s, indent));
                        }
                    }
                    CellValue::Number(_)
                    | CellValue::Boolean(_)
                    | CellValue::Error(_)
                    | CellValue::Empty => {}
                }
            }
        }
    }

    col_desc_chars
        .into_iter()
        .map(|chars| grid_descriptor_req_width_from_chars(chars, &theme.metrics))
        .collect()
}

fn contains_line_break(s: &str) -> bool {
    s.contains('\n') || s.contains('\r')
}

#[derive(Debug, Clone, Copy)]
struct CellFormatMeta {
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
}

fn resolve_cell_format_meta(cell_node: Option<&CellNode>, theme: &Theme) -> CellFormatMeta {
    let format = cell_node.and_then(|n| n.format);
    let decimals = cell_node
        .and_then(|n| n.decimals)
        .or_else(|| format.and_then(|f| theme.number_formats.default_decimals(f)));
    let currency = cell_node.and_then(|n| n.currency);
    CellFormatMeta {
        format,
        decimals,
        currency,
    }
}

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
    /// Whether this cell requires multi-line text wrapping.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrap_text: bool,
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
    /// 0-indexed physical ending column of the table title banner (inclusive):
    /// `sheet_max_end_col` for the last lane of any band, `lane_max_end_col` for non-last lanes,
    /// and `None` when `title_row` is `None`.
    pub title_end_col: Option<u32>,
    /// Inclusive physical row range of table subtitle banners, or None if the table has zero subtitles.
    pub subtitle_row_range: Option<(u32, u32)>,
    /// Physical row index of the blank buffer gutter below title/subtitle if present.
    pub title_gutter_row: Option<u32>,
    /// Inclusive physical row range of header tiers, or None if the table has zero headers.
    pub header_row_range: Option<(u32, u32)>,
    /// Inclusive physical row range of table body rows (data, section headers, gutters): `(start_row, end_row)`.
    pub body_row_range: (u32, u32),
    /// Inclusive physical column range `(phys_start, phys_end)` for each column in `table.columns` (1:1).
    pub col_spans: Vec<(u32, u32)>,
}

/// Sheet-level physical layout metrics and column widths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetLayout {
    /// Worksheet identifier.
    pub sheet: SheetId,
    /// Sheet-wide column widths (0-indexed col -> character widths).
    pub col_widths: IndexMap<u32, f64>,
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
    /// Sheet-level layout metrics and column widths.
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
    /// Plans 2D physical coordinates, bounding boxes, and column widths
    /// for all tables and sheets declared in the manifest.
    ///
    /// # Errors
    /// Returns [`LayoutError`] if unassigned tables exist, referenced tables cannot be resolved,
    /// uniform grid constraints are violated, or coordinates exceed Excel grid limits.
    pub fn plan(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        grid: &CellGrid,
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
    ) -> Result<LayoutMap, LayoutError> {
        // 1. Verify every table belongs to at least one sheet
        let assigned_tables: IndexSet<&TableId> = manifest
            .sheets
            .iter()
            .flat_map(SheetSpec::table_ids)
            .collect();
        for table in tables {
            if !assigned_tables.contains(&table.id) {
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

            // Resolve bands, lanes, and tables belonging to this sheet in canonical order,
            // defensively ignoring empty bands/lanes or zero-column tables if called directly.
            let mut resolved_bands: Vec<ResolvedBand<'_>> = Vec::new();
            let mut placed_tables: Vec<&TableSpec> = Vec::new();

            for band_spec in &sheet_spec.bands {
                let mut resolved_lanes: Vec<ResolvedLane<'_>> = Vec::new();
                for lane_spec in &band_spec.lanes {
                    let mut lane_tables: Vec<&TableSpec> = Vec::new();
                    for table_id in &lane_spec.tables {
                        let matched =
                            tables.iter().find(|t| t.id == *table_id).ok_or_else(|| {
                                LayoutError::UnknownTable {
                                    table: table_id.clone(),
                                    sheet: sheet_id.clone(),
                                }
                            })?;
                        if !matched.columns.is_empty() {
                            lane_tables.push(matched);
                            placed_tables.push(matched);
                        }
                    }
                    if !lane_tables.is_empty() {
                        resolved_lanes.push(ResolvedLane {
                            table_gap: lane_spec.table_gap.unwrap_or(DEFAULT_VERTICAL_TABLE_GAP),
                            tables: lane_tables,
                        });
                    }
                }
                if !resolved_lanes.is_empty() {
                    resolved_bands.push(ResolvedBand {
                        gap: band_spec.gap.unwrap_or(DEFAULT_VERTICAL_TABLE_GAP),
                        lane_gap: band_spec.lane_gap.unwrap_or(DEFAULT_HORIZONTAL_TABLE_GAP),
                        lanes: resolved_lanes,
                    });
                }
            }

            let mut col_widths: IndexMap<u32, f64> = IndexMap::new();

            // Default margin: Column 0 is left gutter
            col_widths.insert(0, theme.metrics.gutter_col_width);

            let mut sheet_placements: IndexMap<TableId, TablePlacement> = IndexMap::new();
            let mut ctx = SheetPlanContext {
                sheet_id: &sheet_id,
                resolved_bands: &resolved_bands,
                sheet_tables: &placed_tables,
                grid,
                theme,
                evaluated,
                sheet_placements: &mut sheet_placements,
                layout_cells: &mut layout_cells,
                col_widths: &mut col_widths,
            };

            match sheet_spec.layout {
                SheetLayoutSpec::Auto => ctx.plan_auto()?,
                SheetLayoutSpec::Grid { col_width } => ctx.plan_grid(col_width)?,
            }

            layout_tables.extend(sheet_placements);

            layout_sheets.insert(
                sheet_id.clone(),
                SheetLayout {
                    sheet: sheet_id,
                    col_widths,
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

/// Vertical structural row offsets computed for a placed table.
#[derive(Debug, Clone, Copy)]
struct VerticalRows {
    start_row: u32,
    end_row: u32,
    title_row: Option<u32>,
    subtitle_row_range: Option<(u32, u32)>,
    title_gutter_row: Option<u32>,
    header_row_range: Option<(u32, u32)>,
    body_row_range: (u32, u32),
}

fn plan_table_vertical_rows(
    table: &TableSpec,
    table_start_row: u32,
) -> Result<VerticalRows, LayoutError> {
    let mut current_row = table_start_row;

    // Table Title Row
    let title_row = if !table.title.trim().is_empty() {
        let r = current_row;
        if r >= MAX_EXCEL_ROWS {
            return Err(LayoutError::GridLimitExceeded {
                dimension: "rows",
                max: MAX_EXCEL_ROWS,
                actual: r,
            });
        }
        current_row += 1;
        Some(r)
    } else {
        None
    };

    // Table Subtitle Rows
    let subtitle_row_range = if !table.subtitles.is_empty() {
        let subtitle_start = current_row;
        for _ in &table.subtitles {
            if current_row >= MAX_EXCEL_ROWS {
                return Err(LayoutError::GridLimitExceeded {
                    dimension: "rows",
                    max: MAX_EXCEL_ROWS,
                    actual: current_row,
                });
            }
            current_row += 1;
        }
        let subtitle_end = current_row - 1;
        Some((subtitle_start, subtitle_end))
    } else {
        None
    };

    // Title Buffer Gutter
    let title_gutter_row = if title_row.is_some() || subtitle_row_range.is_some() {
        let r = current_row;
        if r >= MAX_EXCEL_ROWS {
            return Err(LayoutError::GridLimitExceeded {
                dimension: "rows",
                max: MAX_EXCEL_ROWS,
                actual: r,
            });
        }
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
            current_row += 1;
        }
        let header_end = current_row - 1;
        Some((header_start, header_end))
    } else {
        None
    };

    let body_start_row = current_row;

    for _ in &table.rows {
        if current_row >= MAX_EXCEL_ROWS {
            return Err(LayoutError::GridLimitExceeded {
                dimension: "rows",
                max: MAX_EXCEL_ROWS,
                actual: current_row,
            });
        }
        current_row += 1;
    }

    let body_end_row = current_row.saturating_sub(1);

    Ok(VerticalRows {
        start_row: table_start_row,
        end_row: body_end_row,
        title_row,
        subtitle_row_range,
        title_gutter_row,
        header_row_range,
        body_row_range: (body_start_row, body_end_row),
    })
}

fn resolve_cell_style_and_indent(
    cell_id: &CellId,
    data_row: &DataRowSpec,
    data_col: &DataColumnSpec,
    grid: &CellGrid,
) -> (StyleRole, u8) {
    if let Some(node) = grid.get(cell_id) {
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
                horizontal: data_row.style.unwrap_or(SemanticStyle::Data),
                vertical: data_col.style.unwrap_or(SemanticStyle::Data),
                is_highlight: data_row.highlight.unwrap_or(false)
                    || data_col.highlight.unwrap_or(false),
                shade,
            },
            0,
        )
    }
}

fn populate_table_layout_cells(
    sheet_id: &SheetId,
    table: &TableSpec,
    body_start_row: u32,
    col_spans: &[(u32, u32)],
    grid: &CellGrid,
    layout_cells: &mut IndexMap<CellId, PhysicalCoordinate>,
) -> Result<(), LayoutError> {
    let mut current_row = body_start_row;
    for row_spec in &table.rows {
        match row_spec {
            RowSpec::Gutter | RowSpec::SectionHeader(_) => {
                current_row += 1;
            }
            RowSpec::Data(data_row) => {
                for (col_spec, &(physical_col, _)) in table.columns.iter().zip(col_spans) {
                    let ColumnSpec::Data(data_col) = col_spec else {
                        continue;
                    };

                    let cell_id =
                        CellId::new(table.id.clone(), data_row.id.clone(), data_col.id.clone());
                    let a1 = coordinate_to_a1(physical_col, current_row)?;
                    let (style_role, indent) =
                        resolve_cell_style_and_indent(&cell_id, data_row, data_col, grid);

                    layout_cells.insert(
                        cell_id,
                        PhysicalCoordinate {
                            sheet: sheet_id.clone(),
                            col: physical_col,
                            row: current_row,
                            a1,
                            style_role,
                            indent,
                            wrap_text: false,
                        },
                    );
                }
                current_row += 1;
            }
        }
    }
    Ok(())
}

struct ResolvedLane<'a> {
    table_gap: u32,
    tables: Vec<&'a TableSpec>,
}

struct ResolvedBand<'a> {
    gap: u32,
    lane_gap: u32,
    lanes: Vec<ResolvedLane<'a>>,
}

/// Per-worksheet planning context shared across `auto` and `grid` layout strategies.
struct SheetPlanContext<'a> {
    sheet_id: &'a SheetId,
    resolved_bands: &'a [ResolvedBand<'a>],
    sheet_tables: &'a [&'a TableSpec],
    grid: &'a CellGrid,
    theme: &'a Theme,
    evaluated: Option<&'a EvaluatedModel>,
    sheet_placements: &'a mut IndexMap<TableId, TablePlacement>,
    layout_cells: &'a mut IndexMap<CellId, PhysicalCoordinate>,
    col_widths: &'a mut IndexMap<u32, f64>,
}

impl<'a> SheetPlanContext<'a> {
    fn plan_auto(&mut self) -> Result<(), LayoutError> {
        let mut prev_band_max_end_row: Option<u32> = None;
        let mut sheet_max_end_col: u32 = 0;

        for band in self.resolved_bands {
            let band_start_row = match prev_band_max_end_row {
                Some(max_end_row) => {
                    let start_row = max_end_row.saturating_add(1).saturating_add(band.gap);
                    if start_row >= MAX_EXCEL_ROWS {
                        return Err(LayoutError::GridLimitExceeded {
                            dimension: "rows",
                            max: MAX_EXCEL_ROWS,
                            actual: start_row,
                        });
                    }
                    start_row
                }
                None => 1,
            };

            let mut band_max_end_row: Option<u32> = None;
            let mut prev_lane_max_end_col: Option<u32> = None;

            for lane in &band.lanes {
                let lane_start_col = match prev_lane_max_end_col {
                    Some(max_end_col) => {
                        let start_col = max_end_col.saturating_add(1).saturating_add(band.lane_gap);
                        if start_col >= MAX_EXCEL_COLUMNS {
                            return Err(LayoutError::GridLimitExceeded {
                                dimension: "columns",
                                max: MAX_EXCEL_COLUMNS,
                                actual: start_col,
                            });
                        }
                        start_col
                    }
                    None => 1,
                };

                let mut lane_max_end_col: Option<u32> = None;
                let mut prev_table_end_row: Option<u32> = None;

                for &table in &lane.tables {
                    let num_cols = table.columns.len() as u32;
                    let table_start_row = match prev_table_end_row {
                        Some(prev_end_row) => {
                            let start_row = prev_end_row
                                .saturating_add(1)
                                .saturating_add(lane.table_gap);
                            if start_row >= MAX_EXCEL_ROWS {
                                return Err(LayoutError::GridLimitExceeded {
                                    dimension: "rows",
                                    max: MAX_EXCEL_ROWS,
                                    actual: start_row,
                                });
                            }
                            start_row
                        }
                        None => band_start_row,
                    };

                    let end_col = lane_start_col.saturating_add(num_cols - 1);
                    if end_col >= MAX_EXCEL_COLUMNS {
                        return Err(LayoutError::GridLimitExceeded {
                            dimension: "columns",
                            max: MAX_EXCEL_COLUMNS,
                            actual: end_col,
                        });
                    }

                    let col_spans: Vec<(u32, u32)> = (0..num_cols)
                        .map(|i| {
                            let c = lane_start_col + i;
                            (c, c)
                        })
                        .collect();

                    let vertical = plan_table_vertical_rows(table, table_start_row)?;

                    populate_table_layout_cells(
                        self.sheet_id,
                        table,
                        vertical.body_row_range.0,
                        &col_spans,
                        self.grid,
                        self.layout_cells,
                    )?;

                    let placement = TablePlacement {
                        sheet: self.sheet_id.clone(),
                        start_col: lane_start_col,
                        start_row: vertical.start_row,
                        end_col,
                        end_row: vertical.end_row,
                        title_row: vertical.title_row,
                        title_end_col: None,
                        subtitle_row_range: vertical.subtitle_row_range,
                        title_gutter_row: vertical.title_gutter_row,
                        header_row_range: vertical.header_row_range,
                        body_row_range: vertical.body_row_range,
                        col_spans,
                    };

                    prev_table_end_row = Some(vertical.end_row);
                    lane_max_end_col = Some(lane_max_end_col.map_or(end_col, |m| m.max(end_col)));
                    band_max_end_row = Some(
                        band_max_end_row.map_or(vertical.end_row, |m| m.max(vertical.end_row)),
                    );
                    sheet_max_end_col = sheet_max_end_col.max(end_col);
                    self.sheet_placements.insert(table.id.clone(), placement);
                }

                if let Some(lane_max) = lane_max_end_col {
                    prev_lane_max_end_col = Some(lane_max);
                }
            }

            if let Some(band_max) = band_max_end_row {
                prev_band_max_end_row = Some(band_max);
            }
        }

        // Resolve aligned title banner right bounds per band/lane
        resolve_title_end_cols(self.resolved_bands, self.sheet_placements);

        // Sheet-wide column width arbitration across 2D placed tables
        ColumnArbitrationContext {
            tables: self.sheet_tables,
            placements: self.sheet_placements,
            grid: self.grid,
            layout_cells: self.layout_cells,
            theme: self.theme,
            evaluated: self.evaluated,
        }
        .arbitrate(sheet_max_end_col, self.col_widths);

        Ok(())
    }

    fn plan_grid(&mut self, col_width: Option<f64>) -> Result<(), LayoutError> {
        if self.resolved_bands.is_empty() {
            return Ok(());
        }

        let base_w = round2(col_width.unwrap_or(self.theme.metrics.min_col_width));
        if !base_w.is_finite() || !(MIN_GRID_COL_WIDTH..=MAX_GRID_COL_WIDTH).contains(&base_w) {
            return Err(LayoutError::GridColWidthOutOfBounds {
                sheet: self.sheet_id.clone(),
                width: base_w,
                min: MIN_GRID_COL_WIDTH,
                max: MAX_GRID_COL_WIDTH,
            });
        }

        // 1. Validate headers/cells, compute uniform width W, and record descriptor text demand
        let (uniform_w, desc_max_chars_per_table) = self.scan_grid_demand(base_w)?;

        // 2. Compute raw column spans against finalized sheet-wide uniform width W
        let raw_spans_per_table = self.compute_grid_raw_spans(uniform_w, &desc_max_chars_per_table);

        // 3. Band/lane vertical layout and lane-index assignment
        let (mut plans, max_lane_idx) = self.plan_grid_vertical_and_lanes(raw_spans_per_table)?;

        // 4. Lane-ordered horizontal placement & (lane_idx, start_col) descriptor prefix alignment
        align_grid_lanes(&mut plans, max_lane_idx)?;

        // 5. PhysicalCoordinate emission (non-overlap guaranteed structurally by band/lane layout)
        let mut sheet_max_end_col: u32 = 0;
        for (table_id, plan) in plans {
            populate_table_layout_cells(
                self.sheet_id,
                plan.table,
                plan.vertical.body_row_range.0,
                &plan.col_spans,
                self.grid,
                self.layout_cells,
            )?;

            sheet_max_end_col = sheet_max_end_col.max(plan.end_col);
            self.sheet_placements.insert(
                table_id,
                TablePlacement {
                    sheet: self.sheet_id.clone(),
                    start_col: plan.start_col,
                    start_row: plan.vertical.start_row,
                    end_col: plan.end_col,
                    end_row: plan.vertical.end_row,
                    title_row: plan.vertical.title_row,
                    title_end_col: None,
                    subtitle_row_range: plan.vertical.subtitle_row_range,
                    title_gutter_row: plan.vertical.title_gutter_row,
                    header_row_range: plan.vertical.header_row_range,
                    body_row_range: plan.vertical.body_row_range,
                    col_spans: plan.col_spans,
                },
            );
        }

        // 6. Resolve aligned title banner right bounds per band/lane
        resolve_title_end_cols(self.resolved_bands, self.sheet_placements);

        // 7. Assign uniform width W to every column covered by any data span, gutter width otherwise
        self.assign_grid_col_widths(sheet_max_end_col, uniform_w);

        Ok(())
    }

    fn scan_grid_demand(&self, base_w: f64) -> Result<(f64, Vec<Vec<usize>>), LayoutError> {
        let mut uniform_w = base_w;
        let mut desc_max_chars_per_table: Vec<Vec<usize>> =
            Vec::with_capacity(self.sheet_tables.len());

        for table in self.sheet_tables {
            let mut col_desc_chars = vec![0usize; table.columns.len()];

            if !table.columns.is_empty() {
                self.scan_grid_table_headers(table, base_w, &mut col_desc_chars)?;
                self.scan_grid_table_cells(table, &mut uniform_w, &mut col_desc_chars)?;
            }

            desc_max_chars_per_table.push(col_desc_chars);
        }

        let uniform_w = round2(uniform_w);
        if !uniform_w.is_finite() || uniform_w > MAX_GRID_COL_WIDTH {
            return Err(LayoutError::GridColWidthOutOfBounds {
                sheet: self.sheet_id.clone(),
                width: uniform_w,
                min: MIN_GRID_COL_WIDTH,
                max: MAX_GRID_COL_WIDTH,
            });
        }

        Ok((uniform_w, desc_max_chars_per_table))
    }

    fn scan_grid_table_headers(
        &self,
        table: &TableSpec,
        base_w: f64,
        col_desc_chars: &mut [usize],
    ) -> Result<(), LayoutError> {
        let mult = self.theme.metrics.char_width_multiplier;
        let col_padding = self.theme.metrics.col_padding;

        for (c_idx, col_spec) in table.columns.iter().enumerate() {
            let ColumnSpec::Data(data_col) = col_spec else {
                continue;
            };

            if let Some(group) = &data_col.group
                && contains_line_break(group)
            {
                return Err(LayoutError::GridMultilineHeader {
                    sheet: self.sheet_id.clone(),
                    table: table.id.clone(),
                    col: data_col.id.clone(),
                });
            }

            for h in &data_col.headers {
                if contains_line_break(h) {
                    return Err(LayoutError::GridMultilineHeader {
                        sheet: self.sheet_id.clone(),
                        table: table.id.clone(),
                        col: data_col.id.clone(),
                    });
                }
            }

            match data_col.role {
                AxisRole::Measure => {
                    for h in &data_col.headers {
                        if h.is_empty() {
                            continue;
                        }
                        let required_width =
                            round2((h.chars().count() + 1) as f64 * mult + col_padding);
                        if required_width > base_w + GRID_WIDTH_EPS {
                            return Err(LayoutError::GridMeasureHeaderTooWide {
                                sheet: self.sheet_id.clone(),
                                table: table.id.clone(),
                                col: data_col.id.clone(),
                                header: h.clone(),
                                required_width,
                                grid_col_width: base_w,
                            });
                        }
                    }
                }
                AxisRole::Descriptor => {
                    for h in &data_col.headers {
                        if let Some(slot) = col_desc_chars.get_mut(c_idx) {
                            *slot = (*slot).max(grid_descriptor_header_chars(h));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn scan_grid_table_cells(
        &self,
        table: &TableSpec,
        uniform_w: &mut f64,
        col_desc_chars: &mut [usize],
    ) -> Result<(), LayoutError> {
        let mult = self.theme.metrics.char_width_multiplier;
        let col_padding = self.theme.metrics.col_padding;

        for row_spec in &table.rows {
            let RowSpec::Data(data_row) = row_spec else {
                continue;
            };
            for (c_idx, col_spec) in table.columns.iter().enumerate() {
                let ColumnSpec::Data(data_col) = col_spec else {
                    continue;
                };

                let cell_id =
                    CellId::new(table.id.clone(), data_row.id.clone(), data_col.id.clone());
                let cell_node = self.grid.get(&cell_id);
                let fmt = resolve_cell_format_meta(cell_node, self.theme);
                let indent = cell_node.map_or(0, |n| n.indent);

                let source =
                    self.classify_grid_cell(&cell_id, cell_node, data_col.role, fmt, indent);

                match source {
                    GridCellSource::Text(s) => {
                        if contains_line_break(s) {
                            return Err(LayoutError::GridMultilineCell {
                                sheet: self.sheet_id.clone(),
                                table: table.id.clone(),
                                row: data_row.id.clone(),
                                col: data_col.id.clone(),
                            });
                        }
                        if !s.is_empty() {
                            match data_col.role {
                                AxisRole::Measure => {
                                    let (char_len, _) = measure_text_segments(
                                        s,
                                        data_col.role,
                                        fmt.format,
                                        indent,
                                        &self.theme.metrics,
                                    );
                                    if char_len > 0 {
                                        *uniform_w = (*uniform_w)
                                            .max(round2(char_len as f64 * mult + col_padding));
                                    }
                                }
                                AxisRole::Descriptor => {
                                    if let Some(slot) = col_desc_chars.get_mut(c_idx) {
                                        *slot = (*slot).max(grid_descriptor_text_chars(s, indent));
                                    }
                                }
                            }
                        }
                    }
                    GridCellSource::NonTextChars(char_len) => {
                        if char_len > 0 {
                            *uniform_w =
                                (*uniform_w).max(round2(char_len as f64 * mult + col_padding));
                        }
                    }
                    GridCellSource::Empty => {}
                }
            }
        }

        Ok(())
    }

    fn compute_grid_raw_spans(
        &self,
        uniform_w: f64,
        desc_max_chars_per_table: &[Vec<usize>],
    ) -> Vec<Vec<u32>> {
        self.sheet_tables
            .iter()
            .zip(desc_max_chars_per_table)
            .map(|(table, col_desc_chars)| {
                table
                    .columns
                    .iter()
                    .zip(col_desc_chars)
                    .map(|(col_spec, &max_desc_chars)| match col_spec {
                        ColumnSpec::Gutter => 1,
                        ColumnSpec::Data(d) => match d.role {
                            AxisRole::Measure => 1,
                            AxisRole::Descriptor => {
                                let req_w = grid_descriptor_req_width_from_chars(
                                    max_desc_chars,
                                    &self.theme.metrics,
                                );
                                if req_w <= uniform_w + GRID_WIDTH_EPS {
                                    1
                                } else {
                                    ((req_w / uniform_w) - GRID_WIDTH_EPS)
                                        .ceil()
                                        .max(1.0)
                                        .min(u32::MAX as f64)
                                        as u32
                                }
                            }
                        },
                    })
                    .collect()
            })
            .collect()
    }

    fn plan_grid_vertical_and_lanes(
        &self,
        raw_spans_per_table: Vec<Vec<u32>>,
    ) -> Result<(IndexMap<TableId, GridTablePlan<'a>>, usize), LayoutError> {
        let mut plans: IndexMap<TableId, GridTablePlan<'a>> = IndexMap::new();
        let mut prev_band_max_end_row: Option<u32> = None;
        let mut max_lane_idx: usize = 0;
        let mut raw_spans_iter = raw_spans_per_table.into_iter();

        for (band_idx, band) in self.resolved_bands.iter().enumerate() {
            let band_start_row = match prev_band_max_end_row {
                Some(max_end_row) => {
                    let start_row = max_end_row.saturating_add(1).saturating_add(band.gap);
                    if start_row >= MAX_EXCEL_ROWS {
                        return Err(LayoutError::GridLimitExceeded {
                            dimension: "rows",
                            max: MAX_EXCEL_ROWS,
                            actual: start_row,
                        });
                    }
                    start_row
                }
                None => 1,
            };

            let mut band_max_end_row: Option<u32> = None;

            for (lane_idx, lane) in band.lanes.iter().enumerate() {
                max_lane_idx = max_lane_idx.max(lane_idx);
                let mut prev_table_end_row: Option<u32> = None;

                for &table in &lane.tables {
                    let raw_spans = raw_spans_iter.next().unwrap_or_default();
                    let table_start_row = match prev_table_end_row {
                        Some(prev_end_row) => {
                            let start_row = prev_end_row
                                .saturating_add(1)
                                .saturating_add(lane.table_gap);
                            if start_row >= MAX_EXCEL_ROWS {
                                return Err(LayoutError::GridLimitExceeded {
                                    dimension: "rows",
                                    max: MAX_EXCEL_ROWS,
                                    actual: start_row,
                                });
                            }
                            start_row
                        }
                        None => band_start_row,
                    };

                    let vertical = plan_table_vertical_rows(table, table_start_row)?;
                    prev_table_end_row = Some(vertical.end_row);
                    band_max_end_row = Some(
                        band_max_end_row.map_or(vertical.end_row, |m| m.max(vertical.end_row)),
                    );
                    let leading_prefix = leading_descriptor_prefix(&table.columns, &raw_spans);

                    plans.insert(
                        table.id.clone(),
                        GridTablePlan {
                            table,
                            band_idx,
                            lane_idx,
                            band_lane_gap: band.lane_gap,
                            leading_prefix,
                            raw_spans,
                            vertical,
                            start_col: 1,
                            end_col: 1,
                            col_spans: Vec::new(),
                        },
                    );
                }
            }

            if let Some(band_max) = band_max_end_row {
                prev_band_max_end_row = Some(band_max);
            }
        }

        Ok((plans, max_lane_idx))
    }

    fn assign_grid_col_widths(&mut self, sheet_max_end_col: u32, uniform_w: f64) {
        let mut covered_by_data = vec![false; (sheet_max_end_col as usize).saturating_add(1)];
        for table in self.sheet_tables {
            let Some(placement) = self.sheet_placements.get(&table.id) else {
                continue;
            };
            for (col_spec, &(phys_start, phys_end)) in
                table.columns.iter().zip(&placement.col_spans)
            {
                if matches!(col_spec, ColumnSpec::Data(_)) {
                    for phys_col in phys_start..=phys_end {
                        if let Some(flag) = covered_by_data.get_mut(phys_col as usize) {
                            *flag = true;
                        }
                    }
                }
            }
        }

        for physical_col in 1..=sheet_max_end_col {
            let width = if covered_by_data
                .get(physical_col as usize)
                .copied()
                .unwrap_or(false)
            {
                uniform_w
            } else {
                self.theme.metrics.gutter_col_width
            };
            self.col_widths.insert(physical_col, width);
        }
    }
    fn classify_grid_cell(
        &self,
        cell_id: &CellId,
        cell_node: Option<&'a CellNode>,
        role: AxisRole,
        fmt: CellFormatMeta,
        indent: u8,
    ) -> GridCellSource<'a> {
        let evaluated_val = self.evaluated.and_then(|e| e.get_cell(cell_id));
        match (evaluated_val, cell_node) {
            (Some(val), _) => match val {
                CellValue::Text(s) => GridCellSource::Text(s.as_str()),
                CellValue::Number(_) | CellValue::Boolean(_) | CellValue::Error(_) => {
                    let (char_len, _) = measure_cell_value(
                        val,
                        role,
                        fmt.format,
                        fmt.decimals,
                        fmt.currency,
                        indent,
                        &self.theme.metrics,
                    );
                    GridCellSource::NonTextChars(char_len)
                }
                CellValue::Empty => GridCellSource::Empty,
            },
            (None, Some(node)) => match &node.payload {
                CellPayload::Literal(lit) => match lit {
                    ValueLiteral::String(s) => GridCellSource::Text(s.as_str()),
                    ValueLiteral::Number(_) | ValueLiteral::Boolean(_) => {
                        let (char_len, _) = measure_literal_value(
                            lit,
                            role,
                            fmt.format,
                            fmt.decimals,
                            fmt.currency,
                            indent,
                            &self.theme.metrics,
                        );
                        GridCellSource::NonTextChars(char_len)
                    }
                    ValueLiteral::Null => GridCellSource::Empty,
                },
                CellPayload::Formula(_) => GridCellSource::Empty,
            },
            (None, None) => GridCellSource::Empty,
        }
    }
}

/// Normalized cell payload category for uniform-grid measurement across evaluated and literal paths.
enum GridCellSource<'a> {
    Text(&'a str),
    NonTextChars(usize),
    Empty,
}

/// Intermediate per-table planning state for uniform grid worksheets.
struct GridTablePlan<'a> {
    table: &'a TableSpec,
    band_idx: usize,
    lane_idx: usize,
    band_lane_gap: u32,
    leading_prefix: Option<(usize, u32)>,
    raw_spans: Vec<u32>,
    vertical: VerticalRows,
    start_col: u32,
    end_col: u32,
    col_spans: Vec<(u32, u32)>,
}

fn align_grid_lanes(
    plans: &mut IndexMap<TableId, GridTablePlan<'_>>,
    max_lane_idx: usize,
) -> Result<(), LayoutError> {
    let mut prev_lane_max_end_col_by_band: IndexMap<usize, u32> = IndexMap::new();

    for lane_idx in 0..=max_lane_idx {
        let lane_indices: Vec<usize> = plans
            .values()
            .enumerate()
            .filter_map(|(idx, p)| (p.lane_idx == lane_idx).then_some(idx))
            .collect();

        // 4a. Resolve start_col for every table in this lane
        for &idx in &lane_indices {
            let Some((_, plan)) = plans.get_index(idx) else {
                continue;
            };
            let start_col = if lane_idx == 0 {
                1
            } else {
                let Some(&prev_max_end_col) = prev_lane_max_end_col_by_band.get(&plan.band_idx)
                else {
                    continue;
                };
                let col = prev_max_end_col
                    .saturating_add(1)
                    .saturating_add(plan.band_lane_gap);
                if col >= MAX_EXCEL_COLUMNS {
                    return Err(LayoutError::GridLimitExceeded {
                        dimension: "columns",
                        max: MAX_EXCEL_COLUMNS,
                        actual: col,
                    });
                }
                col
            };
            if let Some((_, plan)) = plans.get_index_mut(idx) {
                plan.start_col = start_col;
            }
        }

        // 4b. Compute max leading prefix span per (lane_idx, start_col) group
        let mut group_max_prefix: IndexMap<u32, u32> = IndexMap::new();
        for &idx in &lane_indices {
            if let Some((_, plan)) = plans.get_index(idx)
                && let Some((_, prefix_span)) = plan.leading_prefix
            {
                let entry = group_max_prefix.entry(plan.start_col).or_insert(0);
                *entry = (*entry).max(prefix_span);
            }
        }

        // 4c. Extend last leading descriptor to group max prefix and build physical col_spans
        let mut cur_lane_max_end_col_by_band: IndexMap<usize, u32> = IndexMap::new();
        for &idx in &lane_indices {
            let Some((_, plan)) = plans.get_index_mut(idx) else {
                continue;
            };
            let mut final_spans = std::mem::take(&mut plan.raw_spans);
            if let Some((last_desc_idx, prefix_span)) = plan.leading_prefix
                && let Some(&max_prefix) = group_max_prefix.get(&plan.start_col)
                && let Some(slot) = final_spans.get_mut(last_desc_idx)
            {
                let extra = max_prefix.saturating_sub(prefix_span);
                *slot = slot.saturating_add(extra);
            }

            let mut col_spans = Vec::with_capacity(final_spans.len());
            let mut cur_col = plan.start_col;
            for &span in &final_spans {
                let phys_start = cur_col;
                let phys_end = phys_start.saturating_add(span.saturating_sub(1));
                if phys_end >= MAX_EXCEL_COLUMNS {
                    return Err(LayoutError::GridLimitExceeded {
                        dimension: "columns",
                        max: MAX_EXCEL_COLUMNS,
                        actual: phys_end,
                    });
                }
                col_spans.push((phys_start, phys_end));
                cur_col = phys_end.saturating_add(1);
            }

            plan.end_col = col_spans.last().map_or(plan.start_col, |&(_, end)| end);
            plan.col_spans = col_spans;

            let band_entry = cur_lane_max_end_col_by_band
                .entry(plan.band_idx)
                .or_insert(0);
            *band_entry = (*band_entry).max(plan.end_col);
        }

        prev_lane_max_end_col_by_band = cur_lane_max_end_col_by_band;
    }

    Ok(())
}

/// Returns `(last_leading_descriptor_col_idx, prefix_span)` when a table has at least one
/// `Measure` column and at least one `Descriptor` column before its first `Measure` column.
///
/// `prefix_span` sums `raw_spans[0..=last_leading_desc_idx]`, aligning the leading descriptor
/// block across tables in the same lane while excluding trailing `Gutter` columns placed after
/// the last leading descriptor (e.g., period spacers skipping earlier periods on stacked tables).
fn leading_descriptor_prefix(columns: &[ColumnSpec], raw_spans: &[u32]) -> Option<(usize, u32)> {
    let first_measure_idx = columns
        .iter()
        .position(|c| matches!(c, ColumnSpec::Data(d) if d.role.is_measure()))?;
    let leading_cols = columns.get(..first_measure_idx)?;
    let last_leading_desc_idx = leading_cols
        .iter()
        .rposition(|c| matches!(c, ColumnSpec::Data(d) if d.role.is_descriptor()))?;
    let prefix_span = raw_spans
        .get(..=last_leading_desc_idx)?
        .iter()
        .copied()
        .fold(0u32, u32::saturating_add);
    Some((last_leading_desc_idx, prefix_span))
}

/// Computes aligned title banner right bounds (`title_end_col`) for all tables on a worksheet.
///
/// Titled tables in the last lane of any band (including single-lane bands) extend `title_end_col`
/// to `sheet_max_end_col`; titled tables in any non-last lane extend to that lane's
/// `lane_max_end_col`. Untitled tables always resolve `title_end_col = None`.
fn resolve_title_end_cols(
    resolved_bands: &[ResolvedBand<'_>],
    sheet_placements: &mut IndexMap<TableId, TablePlacement>,
) {
    let Some(sheet_max_end_col) = sheet_placements.values().map(|p| p.end_col).max() else {
        return;
    };

    for band in resolved_bands {
        let Some(last_lane_idx) = band.lanes.len().checked_sub(1) else {
            continue;
        };

        for (lane_idx, lane) in band.lanes.iter().enumerate() {
            let target_end_col = if lane_idx == last_lane_idx {
                sheet_max_end_col
            } else {
                let Some(lane_max_end_col) = lane
                    .tables
                    .iter()
                    .filter_map(|t| sheet_placements.get(&t.id).map(|p| p.end_col))
                    .max()
                else {
                    continue;
                };
                lane_max_end_col
            };

            for table in &lane.tables {
                if let Some(placement) = sheet_placements.get_mut(&table.id) {
                    placement.title_end_col = placement.title_row.map(|_| target_end_col);
                }
            }
        }
    }
}

/// Context and inputs for arbitrating sheet-wide column widths and cell text wrapping across placed tables.
struct ColumnArbitrationContext<'a> {
    tables: &'a [&'a TableSpec],
    placements: &'a IndexMap<TableId, TablePlacement>,
    grid: &'a CellGrid,
    layout_cells: &'a mut IndexMap<CellId, PhysicalCoordinate>,
    theme: &'a Theme,
    evaluated: Option<&'a EvaluatedModel>,
}

impl ColumnArbitrationContext<'_> {
    /// Arbitrates sheet-wide column widths and cell text wrapping across all tables sharing worksheet columns.
    fn arbitrate(&mut self, sheet_max_end_col: u32, col_widths: &mut IndexMap<u32, f64>) {
        for physical_col in 1..=sheet_max_end_col {
            let has_data_col = self.tables.iter().any(|t| {
                let Some(placement) = self.placements.get(&t.id) else {
                    return false;
                };
                if physical_col < placement.start_col || physical_col > placement.end_col {
                    return false;
                }
                let local_col_idx = (physical_col - placement.start_col) as usize;
                matches!(t.columns.get(local_col_idx), Some(ColumnSpec::Data(_)))
            });

            if !has_data_col {
                col_widths.insert(physical_col, self.theme.metrics.gutter_col_width);
                continue;
            }

            let mut max_chars_across_tables: usize = 0;

            for table in self.tables {
                let Some(placement) = self.placements.get(&table.id) else {
                    continue;
                };
                if physical_col < placement.start_col || physical_col > placement.end_col {
                    continue;
                }
                let local_col_idx = (physical_col - placement.start_col) as usize;
                let Some(ColumnSpec::Data(data_col)) = table.columns.get(local_col_idx) else {
                    continue;
                };

                let mut table_max_chars: usize = 0;

                // 1. Column header text length (max across all stacked per-column header lines; excludes spanning group)
                let header_len = data_col.max_header_chars();
                let header_trailing_pad = match data_col.role {
                    AxisRole::Measure if header_len > 0 => 1,
                    AxisRole::Measure | AxisRole::Descriptor => 0,
                };
                table_max_chars = table_max_chars.max(header_len + header_trailing_pad);

                // 2. Row cells text length and multi-line wrapping
                for row in &table.rows {
                    let RowSpec::Data(data_row) = row else {
                        continue;
                    };
                    let cell_id =
                        CellId::new(table.id.clone(), data_row.id.clone(), data_col.id.clone());
                    let cell_node = self.grid.get(&cell_id);
                    let fmt = resolve_cell_format_meta(cell_node, self.theme);
                    let indent = self.layout_cells.get(&cell_id).map_or(0, |c| c.indent);

                    let (char_len, lines) =
                        match (self.evaluated.and_then(|e| e.get_cell(&cell_id)), cell_node) {
                            (Some(val), _) => measure_cell_value(
                                val,
                                data_col.role,
                                fmt.format,
                                fmt.decimals,
                                fmt.currency,
                                indent,
                                &self.theme.metrics,
                            ),
                            (None, Some(node)) => match &node.payload {
                                CellPayload::Literal(lit) => measure_literal_value(
                                    lit,
                                    data_col.role,
                                    fmt.format,
                                    fmt.decimals,
                                    fmt.currency,
                                    indent,
                                    &self.theme.metrics,
                                ),
                                CellPayload::Formula(_) => (usize::from(indent) * 2, 1),
                            },
                            (None, None) => (usize::from(indent) * 2, 1),
                        };

                    if lines > 1
                        && let Some(coord) = self.layout_cells.get_mut(&cell_id)
                    {
                        coord.wrap_text = true;
                    }

                    table_max_chars = table_max_chars.max(char_len);
                }

                max_chars_across_tables = max_chars_across_tables.max(table_max_chars);
            }

            // Apply theme font multiplier and padding, clamping to [min_col_width, max_col_width]
            let computed_width = (max_chars_across_tables as f64
                * self.theme.metrics.char_width_multiplier
                + self.theme.metrics.col_padding)
                .clamp(
                    self.theme.metrics.min_col_width,
                    self.theme.metrics.max_col_width,
                );

            let final_width = round2(computed_width);
            let existing = col_widths.get(&physical_col).copied().unwrap_or(0.0);
            col_widths.insert(physical_col, existing.max(final_width));
        }
    }
}

fn measure_text_segments(
    s: &str,
    role: AxisRole,
    format: Option<FormatPreset>,
    indent: u8,
    metrics: &MetricsConfig,
) -> (usize, usize) {
    let trailing_pad = usize::from(match format {
        Some(
            FormatPreset::Text
            | FormatPreset::Currency
            | FormatPreset::Number
            | FormatPreset::Percentage
            | FormatPreset::Multiple
            | FormatPreset::BasisPoints
            | FormatPreset::Integer,
        ) => true,
        Some(FormatPreset::Date) => false,
        None => match role {
            AxisRole::Measure => true,
            AxisRole::Descriptor => false,
        },
    });
    let indent_chars = usize::from(indent) * 2;
    let mut max_chars = 0;
    let mut total_lines = 0;

    for raw_segment in s.split('\n') {
        let segment = raw_segment.strip_suffix('\r').unwrap_or(raw_segment);
        let seg_chars = if segment.is_empty() {
            indent_chars
        } else {
            segment.chars().count() + trailing_pad + indent_chars
        };
        max_chars = max_chars.max(seg_chars);
        let seg_lines = if seg_chars == 0 {
            1
        } else {
            let seg_width =
                (seg_chars as f64) * metrics.char_width_multiplier + metrics.col_padding;
            ((seg_width / metrics.max_col_width).ceil() as usize).max(1)
        };
        total_lines += seg_lines;
    }

    (max_chars, total_lines)
}

fn measure_literal_value(
    lit: &ValueLiteral,
    role: AxisRole,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
    indent: u8,
    metrics: &MetricsConfig,
) -> (usize, usize) {
    let indent_chars = usize::from(indent) * 2;
    match lit {
        ValueLiteral::String(s) => measure_text_segments(s, role, format, indent, metrics),
        ValueLiteral::Number(n) => (
            formatted_number_estimate(*n, role, format, decimals, currency) + indent_chars,
            1,
        ),
        ValueLiteral::Boolean(b) => ((if *b { 4 } else { 5 }) + indent_chars, 1),
        ValueLiteral::Null => (indent_chars, 1),
    }
}

/// Computes the horizontal character demand and wrapped line count of an evaluated `CellValue`.
fn measure_cell_value(
    val: &CellValue,
    role: AxisRole,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
    indent: u8,
    metrics: &MetricsConfig,
) -> (usize, usize) {
    let indent_chars = usize::from(indent) * 2;
    match val {
        CellValue::Text(s) => measure_text_segments(s, role, format, indent, metrics),
        CellValue::Number(n) => (
            formatted_number_estimate(*n, role, format, decimals, currency) + indent_chars,
            1,
        ),
        CellValue::Boolean(b) => ((if *b { 4 } else { 5 }) + indent_chars, 1),
        CellValue::Error(e) => (e.to_string().chars().count() + indent_chars, 1),
        CellValue::Empty => (indent_chars, 1),
    }
}

/// Provides an institutional width estimate for formatted numbers using accounting typography and trailing alignment space.
fn formatted_number_estimate(
    n: f64,
    role: AxisRole,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
) -> usize {
    if !n.is_finite() {
        return 4; // NaN, Inf
    }
    let formatted = format_accounting_number(n, format, decimals, currency);
    let has_trailing_space = match format {
        Some(
            FormatPreset::Currency
            | FormatPreset::Number
            | FormatPreset::Percentage
            | FormatPreset::Multiple
            | FormatPreset::BasisPoints
            | FormatPreset::Integer
            | FormatPreset::Date,
        ) => true,
        Some(FormatPreset::Text) => false,
        None => match role {
            AxisRole::Measure => true,
            AxisRole::Descriptor => false,
        },
    };
    let trailing_pad = usize::from(!formatted.ends_with(')') && has_trailing_space);
    formatted.chars().count() + trailing_pad
}
