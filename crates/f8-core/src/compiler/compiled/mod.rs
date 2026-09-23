//! Pass 1–6a unified compilation artifact and in-memory inspection facade.

pub mod error;
pub mod model;

use std::collections::HashSet;

use indexmap::IndexMap;

pub use error::InspectionError;
pub use model::{
    CellInspection, ColumnInspection, DependencyNode, DependencyTree, ModelSummary, RowInspection,
    SheetSummary, TableCellInfo, TableInspection, TableSummary,
};

use crate::compiler::emission::{EmissionOptions, WorkbookEmitter};
use crate::compiler::error::CompilerError;
use crate::compiler::eval::EvaluatedModel;
use crate::compiler::graph::DependencyGraph;
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::compiler::layout::LayoutMap;
use crate::compiler::lowering::LoweredFormulaMap;
use crate::model::id::{ColId, RowId, SheetId, TableId};
use crate::model::manifest::ProjectManifest;
use crate::model::table::TableSpec;
use crate::model::theme::Theme;
use crate::value::CellValue;

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
        let precedents = self
            .graph
            .dependencies_of(&cell_id)
            .into_iter()
            .cloned()
            .collect();
        let dependents = self
            .graph
            .dependents_of(&cell_id)
            .into_iter()
            .cloned()
            .collect();

        Ok(CellInspection {
            coordinate: cell_id,
            sheet: cell_node.sheet.clone(),
            physical_address,
            evaluated_value,
            format: cell_node.format,
            decimals: cell_node.decimals,
            currency: cell_node.currency,
            is_highlight: cell_node.is_highlight,
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
                },
                crate::model::table::ColumnSpec::Gutter => ColumnInspection {
                    id: None,
                    headers: Vec::new(),
                    is_gutter: true,
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
                    }
                }
                crate::model::table::RowSpec::SectionHeader(s) => RowInspection {
                    id: None,
                    section_title: Some(s.title.clone()),
                    is_section_header: true,
                    is_gutter: false,
                    indent: s.indent.unwrap_or(0),
                },
                crate::model::table::RowSpec::Gutter => RowInspection {
                    id: None,
                    section_title: None,
                    is_section_header: false,
                    is_gutter: true,
                    indent: 0,
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

                let (is_formula, formula_expr) = match &cell_node.payload {
                    CellPayload::Formula(expr) => (true, Some(expr.to_string())),
                    CellPayload::Literal(_) => (false, None),
                };

                let excel_formula = self.lowered_map.get(&cell_id).map(ToString::to_string);

                cells.insert(
                    cell_id,
                    TableCellInfo {
                        value,
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
                    let (sheet_address, value, excel_formula) = self.get_node_display_metadata(dep);
                    DependencyNode {
                        cell_id: dep.clone(),
                        sheet_address,
                        value,
                        excel_formula,
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
        visited_ancestors.insert(cell.clone());

        let mut nodes = Vec::with_capacity(direct_count);
        for dep in direct {
            nodes.push(self.build_precedent_tree_node(
                dep,
                &mut visited_ancestors,
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
                    let (sheet_address, value, excel_formula) = self.get_node_display_metadata(dep);
                    DependencyNode {
                        cell_id: dep.clone(),
                        sheet_address,
                        value,
                        excel_formula,
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
        visited_ancestors.insert(cell.clone());

        let mut nodes = Vec::with_capacity(direct_count);
        for dep in direct {
            nodes.push(self.build_dependent_tree_node(
                dep,
                &mut visited_ancestors,
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
        all_visited: &mut HashSet<CellId>,
    ) -> DependencyNode {
        all_visited.insert(cell_id.clone());
        let (sheet_address, value, excel_formula) = self.get_node_display_metadata(cell_id);

        let mut children = Vec::new();
        if !visited_ancestors.contains(cell_id) {
            visited_ancestors.insert(cell_id.clone());
            let deps = self.graph.dependencies_of(cell_id);
            for dep in deps {
                children.push(self.build_precedent_tree_node(dep, visited_ancestors, all_visited));
            }
            visited_ancestors.remove(cell_id);
        }

        DependencyNode {
            cell_id: cell_id.clone(),
            sheet_address,
            value,
            excel_formula,
            children,
        }
    }

    fn build_dependent_tree_node(
        &self,
        cell_id: &CellId,
        visited_ancestors: &mut HashSet<CellId>,
        all_visited: &mut HashSet<CellId>,
    ) -> DependencyNode {
        all_visited.insert(cell_id.clone());
        let (sheet_address, value, excel_formula) = self.get_node_display_metadata(cell_id);

        let mut children = Vec::new();
        if !visited_ancestors.contains(cell_id) {
            visited_ancestors.insert(cell_id.clone());
            let deps = self.graph.dependents_of(cell_id);
            for dep in deps {
                children.push(self.build_dependent_tree_node(dep, visited_ancestors, all_visited));
            }
            visited_ancestors.remove(cell_id);
        }

        DependencyNode {
            cell_id: cell_id.clone(),
            sheet_address,
            value,
            excel_formula,
            children,
        }
    }

    fn get_node_display_metadata(
        &self,
        cell_id: &CellId,
    ) -> (String, Option<CellValue>, Option<String>) {
        let sheet_address = self
            .layout_map
            .get_cell(cell_id)
            .map(|c| format!("{}!{}", c.sheet, c.a1))
            .unwrap_or_default();
        let value = self.evaluated.get_cell(cell_id).cloned();
        let excel_formula = self.lowered_map.get(cell_id).map(ToString::to_string);
        (sheet_address, value, excel_formula)
    }
}
