//! Pass 1–6a unified compilation artifact and in-memory inspection facade.

pub mod error;
pub mod model;

use std::collections::HashSet;

use indexmap::IndexMap;

pub use error::InspectionError;
pub use model::{
    CellInspection, CellNeighborInspection, ColumnInspection, DependencyNode, DependencyTree,
    ModelSummary, RowInspection, SheetCellInspection, SheetCellKind, SheetInspection,
    SheetRowInspection, SheetSummary, TableCellInfo, TableInspection, TableSummary,
};

use crate::compiler::emission::{EmissionOptions, WorkbookEmitter};
use crate::compiler::error::CompilerError;
use crate::compiler::eval::EvaluatedModel;
use crate::compiler::graph::DependencyGraph;
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::compiler::layout::{LayoutMap, coordinate_to_a1};
use crate::compiler::lowering::LoweredFormulaMap;
use crate::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use crate::model::id::{ColId, RowId, SheetId, TableId};
use crate::model::manifest::ProjectManifest;
use crate::model::table::{ColumnSpec, RowSpec, TableSpec};
use crate::model::theme::Theme;
use crate::value::{CellValue, format_cell_value};

/// Unified compiled model consolidating all pass artifacts.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModel {
    /// Declarative project manifest.
    pub manifest: ProjectManifest,
    /// Declarative table specifications indexed by table ID.
    pub tables: IndexMap<TableId, TableSpec>,
    /// Resolved visual and typographic theme.
    pub theme: Theme,
    /// Model symbol table.
    pub symbol_table: SymbolTable,
    /// Expanded intermediate representation cell grid.
    pub grid: CellGrid,
    /// Directed dependency graph.
    pub graph: DependencyGraph,
    /// Topologically evaluated in-memory cell values.
    pub evaluated: EvaluatedModel,
    /// Planned 2D spatial layout and physical sheet coordinates.
    pub layout_map: LayoutMap,
    /// Lowered concrete Excel formula strings.
    pub lowered_map: LoweredFormulaMap,
}

impl CompiledModel {
    /// Directly emits an OpenXML `.xlsx` workbook byte buffer from the compiled model.
    ///
    /// # Errors
    /// Returns [`CompilerError`] if workbook emission fails.
    pub fn emit_workbook(&self, options: &EmissionOptions) -> Result<Vec<u8>, CompilerError> {
        let tables_vec: Vec<TableSpec> = self.tables.values().cloned().collect();
        let buffer = WorkbookEmitter::emit_to_buffer(
            &self.manifest,
            &tables_vec,
            &self.grid,
            &self.layout_map,
            &self.lowered_map,
            &self.theme,
            Some(&self.evaluated),
            options,
        )?;
        Ok(buffer)
    }

    /// Returns a high-level summary of the model, sheets, tables, and cell driver/formula counts.
    #[must_use]
    pub fn summary(&self) -> ModelSummary {
        let mut sheets = Vec::with_capacity(self.manifest.sheets.len());
        let mut total_tables = 0;
        let mut total_cells = 0;
        let mut total_drivers = 0;
        let mut total_formulas = 0;

        for sheet_spec in &self.manifest.sheets {
            let sheet_id = SheetId::from(sheet_spec.name.as_str());
            let mut sheet_tables = Vec::new();
            let mut sheet_cells = 0;

            for table_id in self.symbol_table.tables_on_sheet(&sheet_id) {
                let Some(table_spec) = self.tables.get(table_id) else {
                    continue;
                };
                let mut cell_count = 0;
                let mut driver_count = 0;
                let mut formula_count = 0;

                for col in &table_spec.columns {
                    let Some(col_id) = col.id() else { continue };
                    for row in &table_spec.rows {
                        let Some(row_id) = row.id() else { continue };
                        let cell_id =
                            CellId::new(table_spec.id.clone(), row_id.clone(), col_id.clone());
                        if let Some(cell_node) = self.grid.get(&cell_id) {
                            cell_count += 1;
                            match cell_node.payload {
                                CellPayload::Literal(_) => driver_count += 1,
                                CellPayload::Formula(_) => formula_count += 1,
                            }
                        }
                    }
                }

                sheet_cells += cell_count;
                total_drivers += driver_count;
                total_formulas += formula_count;

                sheet_tables.push(TableSummary {
                    id: table_spec.id.clone(),
                    title: table_spec.title.clone(),
                    row_count: table_spec.rows.len(),
                    col_count: table_spec.columns.len(),
                    cell_count,
                    driver_count,
                    formula_count,
                });
            }

            total_tables += sheet_tables.len();
            total_cells += sheet_cells;

            sheets.push(SheetSummary {
                name: sheet_id,
                tables: sheet_tables,
                total_cells: sheet_cells,
            });
        }

        ModelSummary {
            model_name: self.manifest.model_name.clone(),
            theme_name: self.theme.name.clone(),
            sheets,
            total_sheets: self.manifest.sheets.len(),
            total_tables,
            total_cells,
            total_drivers,
            total_formulas,
        }
    }

    /// Returns the topologically evaluated runtime value for an individual cell.
    ///
    /// # Errors
    /// Returns [`InspectionError::CellNotFound`] if the cell coordinate does not exist.
    pub fn cell_value(
        &self,
        table: &TableId,
        row: &RowId,
        col: &ColId,
    ) -> Result<CellValue, InspectionError> {
        let cell_id = CellId::new(table.clone(), row.clone(), col.clone());
        if !self.grid.contains(&cell_id) {
            return Err(InspectionError::CellNotFound(cell_id));
        }
        Ok(self
            .evaluated
            .get_cell(&cell_id)
            .cloned()
            .unwrap_or(CellValue::Empty))
    }

    /// Returns the pre-formatted display string for an individual cell.
    ///
    /// # Errors
    /// Returns [`InspectionError::CellNotFound`] if the cell coordinate does not exist.
    pub fn cell_formatted(
        &self,
        table: &TableId,
        row: &RowId,
        col: &ColId,
    ) -> Result<String, InspectionError> {
        let cell_id = CellId::new(table.clone(), row.clone(), col.clone());
        let cell_node = self
            .grid
            .get(&cell_id)
            .ok_or_else(|| InspectionError::CellNotFound(cell_id.clone()))?;
        let evaluated_value = self
            .evaluated
            .get_cell(&cell_id)
            .unwrap_or(&CellValue::Empty);
        Ok(self.format_value(
            evaluated_value,
            cell_node.format,
            cell_node.decimals,
            cell_node.currency,
        ))
    }

    /// Inspects an individual cell by table, row, and column identifiers.
    ///
    /// # Errors
    /// Returns [`InspectionError::CellNotFound`] if the cell coordinate does not exist.
    pub fn inspect_cell(
        &self,
        table: &TableId,
        row: &RowId,
        col: &ColId,
    ) -> Result<CellInspection, InspectionError> {
        let cell_id = CellId::new(table.clone(), row.clone(), col.clone());
        let cell_node = self
            .grid
            .get(&cell_id)
            .ok_or_else(|| InspectionError::CellNotFound(cell_id.clone()))?;

        let physical_coord = self
            .layout_map
            .get_cell(&cell_id)
            .ok_or_else(|| InspectionError::CellNotFound(cell_id.clone()))?;

        let physical_address = format!("{}!{}", physical_coord.sheet, physical_coord.a1);
        let evaluated_value = self
            .evaluated
            .get_cell(&cell_id)
            .cloned()
            .unwrap_or(CellValue::Empty);

        let (is_formula, formula_expr) = match &cell_node.payload {
            CellPayload::Formula(expr) => (true, Some(expr.to_string())),
            CellPayload::Literal(_) => (false, None),
        };

        let excel_formula = self.lowered_map.get(&cell_id).map(ToString::to_string);
        let formatted_value = Some(self.format_value(
            &evaluated_value,
            cell_node.format,
            cell_node.decimals,
            cell_node.currency,
        ));
        let precedents = self
            .graph
            .dependencies_of(&cell_id)
            .into_iter()
            .map(|dep| {
                let (sheet_address, value, formatted_value, _) =
                    self.get_node_display_metadata(dep);
                CellNeighborInspection {
                    cell_id: dep.clone(),
                    sheet_address,
                    value,
                    formatted_value,
                }
            })
            .collect();
        let dependents = self
            .graph
            .dependents_of(&cell_id)
            .into_iter()
            .map(|dep| {
                let (sheet_address, value, formatted_value, _) =
                    self.get_node_display_metadata(dep);
                CellNeighborInspection {
                    cell_id: dep.clone(),
                    sheet_address,
                    value,
                    formatted_value,
                }
            })
            .collect();

        Ok(CellInspection {
            coordinate: cell_id,
            sheet: cell_node.sheet.clone(),
            physical_address,
            evaluated_value,
            formatted_value,
            format: cell_node.format,
            decimals: cell_node.decimals,
            currency: cell_node.currency,
            is_highlight: cell_node.is_highlight,
            shade: cell_node.shade,
            indent: cell_node.indent,
            is_formula,
            formula_expr,
            excel_formula,
            note: cell_node.note.clone(),
            precedents,
            dependents,
        })
    }

    /// Inspects an entire 2D table grid with columns, rows, and cell records.
    ///
    /// # Errors
    /// Returns [`InspectionError::TableNotFound`] if the table does not exist in the model.
    pub fn inspect_table(&self, table: &TableId) -> Result<TableInspection, InspectionError> {
        let table_spec = self
            .tables
            .get(table)
            .ok_or_else(|| InspectionError::TableNotFound(table.clone()))?;

        let sheet = self
            .symbol_table
            .get_sheet(table)
            .cloned()
            .ok_or_else(|| InspectionError::TableNotFound(table.clone()))?;

        let columns = table_spec
            .columns
            .iter()
            .map(|col| match col {
                crate::model::table::ColumnSpec::Data(d) => ColumnInspection {
                    id: Some(d.id.clone()),
                    headers: d.headers.clone(),
                    is_gutter: false,
                    is_descriptor: d.role.is_descriptor(),
                    style: Some(d.style.unwrap_or(SemanticStyle::Data)),
                    shade: d.shade.filter(|s| !s.is_none()),
                },
                crate::model::table::ColumnSpec::Gutter => ColumnInspection {
                    id: None,
                    headers: Vec::new(),
                    is_gutter: true,
                    is_descriptor: false,
                    style: None,
                    shade: None,
                },
            })
            .collect();

        let rows = table_spec
            .rows
            .iter()
            .map(|row| match row {
                crate::model::table::RowSpec::Data(d) => {
                    let indent = table_spec
                        .columns
                        .iter()
                        .filter_map(|col| col.id())
                        .filter_map(|col_id| {
                            let cell_id = CellId::new(table.clone(), d.id.clone(), col_id.clone());
                            self.grid.get(&cell_id).map(|cell| cell.indent)
                        })
                        .max()
                        .unwrap_or(0);
                    RowInspection {
                        id: Some(d.id.clone()),
                        section_title: None,
                        is_section_header: false,
                        is_gutter: false,
                        indent,
                        style: Some(d.style.unwrap_or(SemanticStyle::Data)),
                        shade: d.shade.filter(|s| !s.is_none()),
                    }
                }
                crate::model::table::RowSpec::SectionHeader(s) => RowInspection {
                    id: None,
                    section_title: Some(s.title.clone()),
                    is_section_header: true,
                    is_gutter: false,
                    indent: s.indent.unwrap_or(0),
                    style: None,
                    shade: None,
                },
                crate::model::table::RowSpec::Gutter => RowInspection {
                    id: None,
                    section_title: None,
                    is_section_header: false,
                    is_gutter: true,
                    indent: 0,
                    style: None,
                    shade: None,
                },
            })
            .collect();

        let mut cells = IndexMap::new();
        for row in &table_spec.rows {
            let Some(row_id) = row.id() else { continue };
            for col in &table_spec.columns {
                let Some(col_id) = col.id() else { continue };
                let cell_id = CellId::new(table.clone(), row_id.clone(), col_id.clone());
                let cell_node = self
                    .grid
                    .get(&cell_id)
                    .ok_or_else(|| InspectionError::CellNotFound(cell_id.clone()))?;

                let physical_coord = self
                    .layout_map
                    .get_cell(&cell_id)
                    .ok_or_else(|| InspectionError::CellNotFound(cell_id.clone()))?;

                let value = self
                    .evaluated
                    .get_cell(&cell_id)
                    .cloned()
                    .unwrap_or(CellValue::Empty);

                let formatted_value = Some(self.format_value(
                    &value,
                    cell_node.format,
                    cell_node.decimals,
                    cell_node.currency,
                ));

                let (is_formula, formula_expr) = match &cell_node.payload {
                    CellPayload::Formula(expr) => (true, Some(expr.to_string())),
                    CellPayload::Literal(_) => (false, None),
                };

                let excel_formula = self.lowered_map.get(&cell_id).map(ToString::to_string);

                cells.insert(
                    cell_id,
                    TableCellInfo {
                        value,
                        formatted_value,
                        is_formula,
                        formula_expr,
                        excel_formula,
                        physical_a1: physical_coord.a1.clone(),
                        format: cell_node.format,
                        decimals: cell_node.decimals,
                        currency: cell_node.currency,
                    },
                );
            }
        }

        Ok(TableInspection {
            table_id: table.clone(),
            title: table_spec.title.clone(),
            subtitle: table_spec.subtitle.clone(),
            sheet,
            columns,
            rows,
            cells,
        })
    }

    /// Inspects an entire physical worksheet grid compositing all tables placed on it.
    ///
    /// # Errors
    /// Returns [`InspectionError::SheetNotFound`] if the sheet does not exist in the model.
    pub fn inspect_sheet(&self, sheet: &SheetId) -> Result<SheetInspection, InspectionError> {
        let sheet_spec = self
            .manifest
            .sheets
            .iter()
            .find(|s| s.name == sheet.as_str())
            .ok_or_else(|| InspectionError::SheetNotFound(sheet.clone()))?;

        let sheet_layout = self
            .layout_map
            .get_sheet(sheet)
            .ok_or_else(|| InspectionError::SheetNotFound(sheet.clone()))?;

        let mut sheet_tables = Vec::new();
        for table_id in &sheet_spec.tables {
            if let Some(table_spec) = self.tables.get(table_id) {
                sheet_tables.push(table_spec);
            }
        }

        if sheet_tables.is_empty() {
            return Ok(SheetInspection {
                sheet_id: sheet.clone(),
                title: sheet_spec.name.clone(),
                tables: Vec::new(),
                min_col: 0,
                max_col: 0,
                min_row: 0,
                max_row: 0,
                col_widths: sheet_layout.col_widths.clone(),
                rows: Vec::new(),
            });
        }

        let min_col = sheet_tables
            .iter()
            .filter_map(|t| self.layout_map.get_table(&t.id).map(|p| p.start_col))
            .min()
            .unwrap_or(0);
        let max_col = sheet_tables
            .iter()
            .filter_map(|t| self.layout_map.get_table(&t.id).map(|p| p.end_col))
            .max()
            .unwrap_or(0);
        let min_row = sheet_tables
            .iter()
            .filter_map(|t| self.layout_map.get_table(&t.id).map(|p| p.start_row))
            .min()
            .unwrap_or(0);
        let max_row = sheet_tables
            .iter()
            .filter_map(|t| self.layout_map.get_table(&t.id).map(|p| p.end_row))
            .max()
            .unwrap_or(0);

        let mut cells_map: IndexMap<(u32, u32), SheetCellInspection> = IndexMap::new();

        for table in &sheet_tables {
            let Some(placement) = self.layout_map.get_table(&table.id) else {
                continue;
            };

            // 1. Title Banner
            if let Some(title_row) = placement.title_row
                && !table.title.trim().is_empty()
            {
                for col in placement.start_col..=placement.end_col {
                    let physical_a1 = coordinate_to_a1(col, title_row)
                        .unwrap_or_else(|_| format!("R{title_row}C{col}"));
                    if col == placement.start_col {
                        cells_map.insert(
                            (title_row, col),
                            SheetCellInspection {
                                physical_col: col,
                                physical_a1,
                                table_id: Some(table.id.clone()),
                                row_id: None,
                                col_id: None,
                                kind: SheetCellKind::TableTitle,
                                is_descriptor: false,
                                value: CellValue::Text(table.title.clone()),
                                formatted_value: Some(table.title.clone()),
                                formula_expr: None,
                                excel_formula: None,
                            },
                        );
                    } else {
                        cells_map.insert(
                            (title_row, col),
                            SheetCellInspection {
                                physical_col: col,
                                physical_a1,
                                table_id: Some(table.id.clone()),
                                row_id: None,
                                col_id: None,
                                kind: SheetCellKind::TableTitle,
                                is_descriptor: false,
                                value: CellValue::Empty,
                                formatted_value: None,
                                formula_expr: None,
                                excel_formula: None,
                            },
                        );
                    }
                }
            }

            // 2. Subtitle Banner
            if let (Some(subtitle_row), Some(subtitle)) = (placement.subtitle_row, &table.subtitle)
            {
                for col in placement.start_col..=placement.end_col {
                    let physical_a1 = coordinate_to_a1(col, subtitle_row)
                        .unwrap_or_else(|_| format!("R{subtitle_row}C{col}"));
                    if col == placement.start_col {
                        cells_map.insert(
                            (subtitle_row, col),
                            SheetCellInspection {
                                physical_col: col,
                                physical_a1,
                                table_id: Some(table.id.clone()),
                                row_id: None,
                                col_id: None,
                                kind: SheetCellKind::TableSubtitle,
                                is_descriptor: false,
                                value: CellValue::Text(subtitle.clone()),
                                formatted_value: Some(subtitle.clone()),
                                formula_expr: None,
                                excel_formula: None,
                            },
                        );
                    } else {
                        cells_map.insert(
                            (subtitle_row, col),
                            SheetCellInspection {
                                physical_col: col,
                                physical_a1,
                                table_id: Some(table.id.clone()),
                                row_id: None,
                                col_id: None,
                                kind: SheetCellKind::TableSubtitle,
                                is_descriptor: false,
                                value: CellValue::Empty,
                                formatted_value: None,
                                formula_expr: None,
                                excel_formula: None,
                            },
                        );
                    }
                }
            }

            // 3. Title Gutter
            if let Some(gutter_row) = placement.title_gutter_row {
                for col in placement.start_col..=placement.end_col {
                    let physical_a1 = coordinate_to_a1(col, gutter_row)
                        .unwrap_or_else(|_| format!("R{gutter_row}C{col}"));
                    cells_map.insert(
                        (gutter_row, col),
                        SheetCellInspection {
                            physical_col: col,
                            physical_a1,
                            table_id: Some(table.id.clone()),
                            row_id: None,
                            col_id: None,
                            kind: SheetCellKind::Gutter,
                            is_descriptor: false,
                            value: CellValue::Empty,
                            formatted_value: None,
                            formula_expr: None,
                            excel_formula: None,
                        },
                    );
                }
            }

            // 4. Multi-tier Column Headers
            if let Some((header_start, header_end)) = placement.header_row_range {
                let total_tiers = (header_end - header_start + 1) as usize;
                for t in 0..total_tiers {
                    let tier_row = header_start + t as u32;
                    let is_bottom_tier = t == total_tiers - 1;
                    for (c_idx, col_spec) in table.columns.iter().enumerate() {
                        let physical_col = placement.start_col + c_idx as u32;
                        let physical_a1 = coordinate_to_a1(physical_col, tier_row)
                            .unwrap_or_else(|_| format!("R{tier_row}C{physical_col}"));
                        match col_spec {
                            ColumnSpec::Gutter => {
                                cells_map.insert(
                                    (tier_row, physical_col),
                                    SheetCellInspection {
                                        physical_col,
                                        physical_a1,
                                        table_id: Some(table.id.clone()),
                                        row_id: None,
                                        col_id: None,
                                        kind: SheetCellKind::Gutter,
                                        is_descriptor: false,
                                        value: CellValue::Empty,
                                        formatted_value: None,
                                        formula_expr: None,
                                        excel_formula: None,
                                    },
                                );
                            }
                            ColumnSpec::Data(d) => {
                                let k_len = d.headers.len();
                                let mut header_text = if t >= total_tiers - k_len {
                                    let k = t - (total_tiers - k_len);
                                    d.headers.get(k).cloned()
                                } else {
                                    None
                                };
                                if is_bottom_tier && header_text.is_none() {
                                    header_text = Some(d.id.to_string());
                                }
                                cells_map.insert(
                                    (tier_row, physical_col),
                                    SheetCellInspection {
                                        physical_col,
                                        physical_a1,
                                        table_id: Some(table.id.clone()),
                                        row_id: None,
                                        col_id: Some(d.id.clone()),
                                        kind: SheetCellKind::ColumnHeader,
                                        is_descriptor: false,
                                        value: header_text
                                            .as_deref()
                                            .map_or(CellValue::Empty, |s| {
                                                CellValue::Text(s.to_string())
                                            }),
                                        formatted_value: header_text,
                                        formula_expr: None,
                                        excel_formula: None,
                                    },
                                );
                            }
                        }
                    }
                }
            }

            // 5. Body Rows
            let mut current_row = placement.body_row_range.0;
            for row_spec in &table.rows {
                match row_spec {
                    RowSpec::Gutter => {
                        for (c_idx, _) in table.columns.iter().enumerate() {
                            let physical_col = placement.start_col + c_idx as u32;
                            let physical_a1 = coordinate_to_a1(physical_col, current_row)
                                .unwrap_or_else(|_| format!("R{current_row}C{physical_col}"));
                            cells_map.insert(
                                (current_row, physical_col),
                                SheetCellInspection {
                                    physical_col,
                                    physical_a1,
                                    table_id: Some(table.id.clone()),
                                    row_id: None,
                                    col_id: None,
                                    kind: SheetCellKind::Gutter,
                                    is_descriptor: false,
                                    value: CellValue::Empty,
                                    formatted_value: None,
                                    formula_expr: None,
                                    excel_formula: None,
                                },
                            );
                        }
                        current_row += 1;
                    }
                    RowSpec::SectionHeader(s) => {
                        for (c_idx, _) in table.columns.iter().enumerate() {
                            let physical_col = placement.start_col + c_idx as u32;
                            let physical_a1 = coordinate_to_a1(physical_col, current_row)
                                .unwrap_or_else(|_| format!("R{current_row}C{physical_col}"));
                            if c_idx == 0 {
                                cells_map.insert(
                                    (current_row, physical_col),
                                    SheetCellInspection {
                                        physical_col,
                                        physical_a1,
                                        table_id: Some(table.id.clone()),
                                        row_id: None,
                                        col_id: None,
                                        kind: SheetCellKind::SectionHeader,
                                        is_descriptor: false,
                                        value: CellValue::Text(s.title.clone()),
                                        formatted_value: Some(s.title.clone()),
                                        formula_expr: None,
                                        excel_formula: None,
                                    },
                                );
                            } else {
                                cells_map.insert(
                                    (current_row, physical_col),
                                    SheetCellInspection {
                                        physical_col,
                                        physical_a1,
                                        table_id: Some(table.id.clone()),
                                        row_id: None,
                                        col_id: None,
                                        kind: SheetCellKind::SectionHeader,
                                        is_descriptor: false,
                                        value: CellValue::Empty,
                                        formatted_value: None,
                                        formula_expr: None,
                                        excel_formula: None,
                                    },
                                );
                            }
                        }
                        current_row += 1;
                    }
                    RowSpec::Data(data_row) => {
                        for (c_idx, col_spec) in table.columns.iter().enumerate() {
                            let physical_col = placement.start_col + c_idx as u32;
                            let physical_a1 = coordinate_to_a1(physical_col, current_row)
                                .unwrap_or_else(|_| format!("R{current_row}C{physical_col}"));
                            match col_spec {
                                ColumnSpec::Gutter => {
                                    cells_map.insert(
                                        (current_row, physical_col),
                                        SheetCellInspection {
                                            physical_col,
                                            physical_a1,
                                            table_id: Some(table.id.clone()),
                                            row_id: Some(data_row.id.clone()),
                                            col_id: None,
                                            kind: SheetCellKind::Gutter,
                                            is_descriptor: false,
                                            value: CellValue::Empty,
                                            formatted_value: None,
                                            formula_expr: None,
                                            excel_formula: None,
                                        },
                                    );
                                }
                                ColumnSpec::Data(data_col) => {
                                    let cell_id = CellId::new(
                                        table.id.clone(),
                                        data_row.id.clone(),
                                        data_col.id.clone(),
                                    );
                                    let cell_node = self.grid.get(&cell_id);
                                    let value = self
                                        .evaluated
                                        .get_cell(&cell_id)
                                        .cloned()
                                        .unwrap_or(CellValue::Empty);
                                    let (format, decimals, currency) = cell_node
                                        .map_or((None, None, None), |n| {
                                            (n.format, n.decimals, n.currency)
                                        });
                                    let formatted_value =
                                        Some(self.format_value(&value, format, decimals, currency));
                                    let (formula_expr, excel_formula) = match cell_node
                                        .map(|n| &n.payload)
                                    {
                                        Some(CellPayload::Formula(expr)) => (
                                            Some(expr.to_string()),
                                            self.lowered_map.get(&cell_id).map(ToString::to_string),
                                        ),
                                        _ => (None, None),
                                    };
                                    cells_map.insert(
                                        (current_row, physical_col),
                                        SheetCellInspection {
                                            physical_col,
                                            physical_a1,
                                            table_id: Some(table.id.clone()),
                                            row_id: Some(data_row.id.clone()),
                                            col_id: Some(data_col.id.clone()),
                                            kind: SheetCellKind::Data,
                                            is_descriptor: data_col.role.is_descriptor(),
                                            value,
                                            formatted_value,
                                            formula_expr,
                                            excel_formula,
                                        },
                                    );
                                }
                            }
                        }
                        current_row += 1;
                    }
                }
            }
        }

        let mut rows = Vec::with_capacity((max_row - min_row + 1) as usize);
        for r in min_row..=max_row {
            let mut row_cells = Vec::with_capacity((max_col - min_col + 1) as usize);
            for c in min_col..=max_col {
                if let Some(cell) = cells_map.swap_remove(&(r, c)) {
                    row_cells.push(cell);
                } else {
                    let physical_a1 =
                        coordinate_to_a1(c, r).unwrap_or_else(|_| format!("R{r}C{c}"));
                    row_cells.push(SheetCellInspection {
                        physical_col: c,
                        physical_a1,
                        table_id: None,
                        row_id: None,
                        col_id: None,
                        kind: SheetCellKind::Empty,
                        is_descriptor: false,
                        value: CellValue::Empty,
                        formatted_value: None,
                        formula_expr: None,
                        excel_formula: None,
                    });
                }
            }
            rows.push(SheetRowInspection {
                physical_row: r,
                excel_row: r + 1,
                cells: row_cells,
            });
        }

        let tables = sheet_tables.iter().map(|t| t.id.clone()).collect();
        Ok(SheetInspection {
            sheet_id: sheet.clone(),
            title: sheet_spec.name.clone(),
            tables,
            min_col,
            max_col,
            min_row,
            max_row,
            col_widths: sheet_layout.col_widths.clone(),
            rows,
        })
    }

    /// Queries upstream calculation precedents for a cell, returning a direct or recursive calculation tree.
    ///
    /// # Errors
    /// Returns [`InspectionError::CellNotFound`] if the cell does not exist in the model grid.
    pub fn precedents_of(
        &self,
        cell: &CellId,
        recursive: bool,
    ) -> Result<DependencyTree, InspectionError> {
        if !self.grid.contains(cell) {
            return Err(InspectionError::CellNotFound(cell.clone()));
        }

        let direct = self.graph.dependencies_of(cell);
        let direct_count = direct.len();

        if !recursive {
            let nodes = direct
                .into_iter()
                .map(|dep| {
                    let (sheet_address, value, formatted_value, excel_formula) =
                        self.get_node_display_metadata(dep);
                    DependencyNode {
                        cell_id: dep.clone(),
                        sheet_address,
                        value,
                        formatted_value,
                        excel_formula,
                        is_pruned: false,
                        children: Vec::new(),
                    }
                })
                .collect();

            return Ok(DependencyTree {
                root: cell.clone(),
                direct_count,
                transitive_count: direct_count,
                nodes,
            });
        }

        let mut all_visited = HashSet::new();
        let mut visited_ancestors = HashSet::new();
        let mut expanded = HashSet::new();
        visited_ancestors.insert(cell.clone());

        let mut nodes = Vec::with_capacity(direct_count);
        for dep in direct {
            nodes.push(self.build_precedent_tree_node(
                dep,
                &mut visited_ancestors,
                &mut expanded,
                &mut all_visited,
            ));
        }

        let transitive_count = all_visited.len();

        Ok(DependencyTree {
            root: cell.clone(),
            direct_count,
            transitive_count,
            nodes,
        })
    }

    /// Queries downstream blast-radius dependents for a cell, returning a direct or recursive impact tree.
    ///
    /// # Errors
    /// Returns [`InspectionError::CellNotFound`] if the cell does not exist in the model grid.
    pub fn dependents_of(
        &self,
        cell: &CellId,
        recursive: bool,
    ) -> Result<DependencyTree, InspectionError> {
        if !self.grid.contains(cell) {
            return Err(InspectionError::CellNotFound(cell.clone()));
        }

        let direct = self.graph.dependents_of(cell);
        let direct_count = direct.len();

        if !recursive {
            let nodes = direct
                .into_iter()
                .map(|dep| {
                    let (sheet_address, value, formatted_value, excel_formula) =
                        self.get_node_display_metadata(dep);
                    DependencyNode {
                        cell_id: dep.clone(),
                        sheet_address,
                        value,
                        formatted_value,
                        excel_formula,
                        is_pruned: false,
                        children: Vec::new(),
                    }
                })
                .collect();

            return Ok(DependencyTree {
                root: cell.clone(),
                direct_count,
                transitive_count: direct_count,
                nodes,
            });
        }

        let mut all_visited = HashSet::new();
        let mut visited_ancestors = HashSet::new();
        let mut expanded = HashSet::new();
        visited_ancestors.insert(cell.clone());

        let mut nodes = Vec::with_capacity(direct_count);
        for dep in direct {
            nodes.push(self.build_dependent_tree_node(
                dep,
                &mut visited_ancestors,
                &mut expanded,
                &mut all_visited,
            ));
        }

        let transitive_count = all_visited.len();

        Ok(DependencyTree {
            root: cell.clone(),
            direct_count,
            transitive_count,
            nodes,
        })
    }

    fn build_precedent_tree_node(
        &self,
        cell_id: &CellId,
        visited_ancestors: &mut HashSet<CellId>,
        expanded: &mut HashSet<CellId>,
        all_visited: &mut HashSet<CellId>,
    ) -> DependencyNode {
        all_visited.insert(cell_id.clone());
        let (sheet_address, value, formatted_value, excel_formula) =
            self.get_node_display_metadata(cell_id);

        let deps = self.graph.dependencies_of(cell_id);
        if deps.is_empty() {
            return DependencyNode {
                cell_id: cell_id.clone(),
                sheet_address,
                value,
                formatted_value,
                excel_formula,
                is_pruned: false,
                children: Vec::new(),
            };
        }

        if visited_ancestors.contains(cell_id) || expanded.contains(cell_id) {
            return DependencyNode {
                cell_id: cell_id.clone(),
                sheet_address,
                value,
                formatted_value,
                excel_formula,
                is_pruned: true,
                children: Vec::new(),
            };
        }

        visited_ancestors.insert(cell_id.clone());
        expanded.insert(cell_id.clone());

        let mut children = Vec::with_capacity(deps.len());
        for dep in deps {
            children.push(self.build_precedent_tree_node(
                dep,
                visited_ancestors,
                expanded,
                all_visited,
            ));
        }
        visited_ancestors.remove(cell_id);

        DependencyNode {
            cell_id: cell_id.clone(),
            sheet_address,
            value,
            formatted_value,
            excel_formula,
            is_pruned: false,
            children,
        }
    }

    fn build_dependent_tree_node(
        &self,
        cell_id: &CellId,
        visited_ancestors: &mut HashSet<CellId>,
        expanded: &mut HashSet<CellId>,
        all_visited: &mut HashSet<CellId>,
    ) -> DependencyNode {
        all_visited.insert(cell_id.clone());
        let (sheet_address, value, formatted_value, excel_formula) =
            self.get_node_display_metadata(cell_id);

        let deps = self.graph.dependents_of(cell_id);
        if deps.is_empty() {
            return DependencyNode {
                cell_id: cell_id.clone(),
                sheet_address,
                value,
                formatted_value,
                excel_formula,
                is_pruned: false,
                children: Vec::new(),
            };
        }

        if visited_ancestors.contains(cell_id) || expanded.contains(cell_id) {
            return DependencyNode {
                cell_id: cell_id.clone(),
                sheet_address,
                value,
                formatted_value,
                excel_formula,
                is_pruned: true,
                children: Vec::new(),
            };
        }

        visited_ancestors.insert(cell_id.clone());
        expanded.insert(cell_id.clone());

        let mut children = Vec::with_capacity(deps.len());
        for dep in deps {
            children.push(self.build_dependent_tree_node(
                dep,
                visited_ancestors,
                expanded,
                all_visited,
            ));
        }
        visited_ancestors.remove(cell_id);

        DependencyNode {
            cell_id: cell_id.clone(),
            sheet_address,
            value,
            formatted_value,
            excel_formula,
            is_pruned: false,
            children,
        }
    }

    fn format_value(
        &self,
        value: &CellValue,
        format: Option<FormatPreset>,
        decimals: Option<u32>,
        currency: Option<CurrencyCode>,
    ) -> String {
        let effective_decimals =
            decimals.or_else(|| format.and_then(|f| self.theme.number_formats.default_decimals(f)));
        format_cell_value(value, format, effective_decimals, currency)
    }

    fn get_node_display_metadata(
        &self,
        cell_id: &CellId,
    ) -> (String, Option<CellValue>, Option<String>, Option<String>) {
        let sheet_address = self
            .layout_map
            .get_cell(cell_id)
            .map(|c| format!("{}!{}", c.sheet, c.a1))
            .unwrap_or_default();
        let value = self.evaluated.get_cell(cell_id).cloned();
        let formatted_value = value.as_ref().map(|v| {
            let (format, decimals, currency) = self
                .grid
                .get(cell_id)
                .map(|cell| (cell.format, cell.decimals, cell.currency))
                .unwrap_or((None, None, None));
            self.format_value(v, format, decimals, currency)
        });
        let excel_formula = self.lowered_map.get(cell_id).map(ToString::to_string);
        (sheet_address, value, formatted_value, excel_formula)
    }

    /// Resolves coordinate arguments from either 3 positional tokens or 1 canonical IR string into a [`CellId`].
    ///
    /// # Errors
    /// Returns [`InspectionError::InvalidCoordinate`] if the arguments do not form a valid coordinate.
    pub fn parse_coordinate(
        &self,
        target: &str,
        row: Option<&str>,
        col: Option<&str>,
    ) -> Result<CellId, InspectionError> {
        let (table_id, row_id, col_id) = parse_coordinate_tokens(target, row, col)?;
        Ok(CellId::new(table_id, row_id, col_id))
    }
}

/// Helper to clean quotes and whitespace from an identifier.
fn clean_id(s: &str) -> &str {
    let trimmed = s.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|t| t.strip_suffix('\''))
        })
        .unwrap_or(trimmed)
        .trim()
}

/// Parses coordinate arguments from either 3 positional tokens or 1 canonical IR string.
///
/// Supported formats:
/// - 3 tokens: `("IncomeStatement", Some("cogs"), Some("y2024e"))`
/// - Canonical IR string: `("IncomeStatement[\"cogs\", \"y2024e\"]", None, None)`
/// - Canonical IR string (unquoted IDs): `("IncomeStatement[cogs, y2024e]", None, None)`
pub fn parse_coordinate_tokens(
    target: &str,
    row: Option<&str>,
    col: Option<&str>,
) -> Result<(TableId, RowId, ColId), InspectionError> {
    let input_repr = match (row, col) {
        (Some(r), Some(c)) => format!("{target} {r} {c}"),
        (Some(r), None) => format!("{target} {r}"),
        (None, Some(c)) => format!("{target} {c}"),
        (None, None) => target.to_string(),
    };

    match (row, col) {
        (Some(r), Some(c)) => {
            let target_trimmed = target.trim();
            let row_trimmed = r.trim();
            let col_trimmed = c.trim();

            if target_trimmed.contains('[')
                || target_trimmed.is_empty()
                || row_trimmed.is_empty()
                || col_trimmed.is_empty()
            {
                return Err(InspectionError::InvalidCoordinate(input_repr));
            }

            let table_id = TableId::from(clean_id(target_trimmed));
            let row_id = RowId::from(clean_id(row_trimmed));
            let col_id = ColId::from(clean_id(col_trimmed));
            Ok((table_id, row_id, col_id))
        }
        (None, None) => {
            let trimmed = target.trim();
            let Some((table_part, rest)) = trimmed.split_once('[') else {
                return Err(InspectionError::InvalidCoordinate(input_repr));
            };

            let table_name = clean_id(table_part);
            if table_name.is_empty() || table_part.contains('[') || table_part.contains(']') {
                return Err(InspectionError::InvalidCoordinate(input_repr));
            }

            let rest_trimmed = rest.trim();
            let Some(inner) = rest_trimmed.strip_suffix(']') else {
                return Err(InspectionError::InvalidCoordinate(input_repr));
            };

            if inner.contains('[') || inner.contains(']') {
                return Err(InspectionError::InvalidCoordinate(input_repr));
            }

            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() != 2 {
                return Err(InspectionError::InvalidCoordinate(input_repr));
            }

            let row_name = clean_id(parts[0]);
            let col_name = clean_id(parts[1]);

            if row_name.is_empty() || col_name.is_empty() {
                return Err(InspectionError::InvalidCoordinate(input_repr));
            }

            Ok((
                TableId::from(table_name),
                RowId::from(row_name),
                ColId::from(col_name),
            ))
        }
        _ => Err(InspectionError::InvalidCoordinate(input_repr)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_coordinate_tokens_three_args() {
        let (table, row, col) =
            parse_coordinate_tokens("IncomeStatement", Some("cogs"), Some("y2024e"))
                .expect("valid 3-arg coordinate");
        assert_eq!(table.as_str(), "IncomeStatement");
        assert_eq!(row.as_str(), "cogs");
        assert_eq!(col.as_str(), "y2024e");
    }

    #[test]
    fn test_parse_coordinate_tokens_canonical_ir() {
        let (table, row, col) =
            parse_coordinate_tokens("IncomeStatement[\"cogs\", \"y2024e\"]", None, None)
                .expect("valid canonical IR coordinate");
        assert_eq!(table.as_str(), "IncomeStatement");
        assert_eq!(row.as_str(), "cogs");
        assert_eq!(col.as_str(), "y2024e");
    }

    #[test]
    fn test_parse_coordinate_tokens_unquoted_ir() {
        let (table, row, col) =
            parse_coordinate_tokens("IncomeStatement[cogs, y2024e]", None, None)
                .expect("valid unquoted IR coordinate");
        assert_eq!(table.as_str(), "IncomeStatement");
        assert_eq!(row.as_str(), "cogs");
        assert_eq!(col.as_str(), "y2024e");
    }

    #[test]
    fn test_parse_coordinate_tokens_single_quoted_ir() {
        let (table, row, col) =
            parse_coordinate_tokens("IncomeStatement['cogs', 'y2024e']", None, None)
                .expect("valid single-quoted IR coordinate");
        assert_eq!(table.as_str(), "IncomeStatement");
        assert_eq!(row.as_str(), "cogs");
        assert_eq!(col.as_str(), "y2024e");
    }

    #[test]
    fn test_parse_coordinate_tokens_invalid_partial_args() {
        let err = parse_coordinate_tokens("IncomeStatement", Some("cogs"), None).unwrap_err();
        assert!(
            matches!(err, InspectionError::InvalidCoordinate(ref repr) if repr == "IncomeStatement cogs")
        );

        let err2 = parse_coordinate_tokens("IncomeStatement", None, None).unwrap_err();
        assert!(
            matches!(err2, InspectionError::InvalidCoordinate(ref repr) if repr == "IncomeStatement")
        );
    }

    #[test]
    fn test_parse_coordinate_tokens_invalid_ir_syntax() {
        let err = parse_coordinate_tokens("IncomeStatement[cogs]", None, None).unwrap_err();
        assert!(matches!(err, InspectionError::InvalidCoordinate(_)));

        let err2 = parse_coordinate_tokens("IncomeStatement[cogs, y2024e, extra]", None, None)
            .unwrap_err();
        assert!(matches!(err2, InspectionError::InvalidCoordinate(_)));
    }
}
