//! Data contracts for compiled model inspection and auditing.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::compiler::ir::id::CellId;
use crate::model::format::{CurrencyCode, FormatPreset};
use crate::model::id::{ColId, RowId, SheetId, TableId};
use crate::value::CellValue;

/// High-level model summary and hierarchical statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSummary {
    /// Model name declared in project manifest.
    pub model_name: String,
    /// Active theme identifier.
    pub theme_name: String,
    /// Ordered list of worksheet summaries.
    pub sheets: Vec<SheetSummary>,
    /// Total number of sheets.
    pub total_sheets: usize,
    /// Total number of tables across all sheets.
    pub total_tables: usize,
    /// Total number of cells across all tables.
    pub total_cells: usize,
    /// Total number of driver/literal input cells.
    pub total_drivers: usize,
    /// Total number of formula calculation cells.
    pub total_formulas: usize,
}

/// Sheet-level summary of contained tables and cell counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetSummary {
    /// Worksheet identifier.
    pub name: SheetId,
    /// Ordered list of tables placed on this worksheet.
    pub tables: Vec<TableSummary>,
    /// Total number of cells on this worksheet.
    pub total_cells: usize,
}

/// Table-level summary of dimensions and cell classifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSummary {
    /// Table programmatic identifier.
    pub id: TableId,
    /// Human-readable display title.
    pub title: String,
    /// Declared row count.
    pub row_count: usize,
    /// Declared column count.
    pub col_count: usize,
    /// Total cell count in table ($R \times C$).
    pub cell_count: usize,
    /// Number of literal input driver cells.
    pub driver_count: usize,
    /// Number of formula calculation cells.
    pub formula_count: usize,
}

/// Comprehensive inspection record for an individual cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellInspection {
    /// Coordinate identifier `(TableId, RowId, ColId)`.
    pub coordinate: CellId,
    /// Parent worksheet identifier.
    pub sheet: SheetId,
    /// Canonical Excel sheet address (e.g. `"Financials!C18"`).
    pub physical_address: String,
    /// Topologically evaluated runtime value.
    pub evaluated_value: CellValue,
    /// Formatting preset if assigned.
    pub format: Option<FormatPreset>,
    /// Display precision override if specified.
    pub decimals: Option<u32>,
    /// Currency code if assigned.
    pub currency: Option<CurrencyCode>,
    /// Visual highlight emphasis flag.
    pub is_highlight: bool,
    /// Descriptor text indentation level.
    pub indent: u8,
    /// `true` if this cell is calculated by formula, `false` if constant literal.
    pub is_formula: bool,
    /// Source DSL formula expression string if this is a formula cell.
    pub formula_expr: Option<String>,
    /// Lowered concrete Excel formula string (prefixed with `=`) if applicable.
    pub excel_formula: Option<String>,
    /// Audit note if attached.
    pub note: Option<String>,
    /// Direct incoming precedent cell coordinates.
    pub precedents: Vec<CellId>,
    /// Direct outgoing dependent cell coordinates.
    pub dependents: Vec<CellId>,
}

/// 2D tabular inspection record for an entire table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableInspection {
    /// Table programmatic identifier.
    pub table_id: TableId,
    /// Table display title.
    pub title: String,
    /// Table display subtitle if defined.
    pub subtitle: Option<String>,
    /// Worksheet containing this table.
    pub sheet: SheetId,
    /// Declared columns with header label tiers.
    pub columns: Vec<ColumnInspection>,
    /// Declared rows with line item descriptors and indentations.
    pub rows: Vec<RowInspection>,
    /// 2D cell records indexed by coordinate.
    pub cells: IndexMap<CellId, TableCellInfo>,
}

/// Column inspection metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnInspection {
    /// Column identifier if this is a data column.
    pub id: Option<ColId>,
    /// Multi-tier header labels.
    pub headers: Vec<String>,
    /// `true` if this column is a blank spacing gutter.
    pub is_gutter: bool,
}

/// Row inspection metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowInspection {
    /// Row identifier if this is a calculable data row.
    pub id: Option<RowId>,
    /// Title banner if this is a section header row.
    pub section_title: Option<String>,
    /// `true` if this row is a visual section header.
    pub is_section_header: bool,
    /// `true` if this row is a blank spacing gutter.
    pub is_gutter: bool,
    /// Typography indentation level.
    pub indent: u8,
}

/// Cell-level audit information within a table inspection view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCellInfo {
    /// Evaluated cell value.
    pub value: CellValue,
    /// `true` if calculated by formula.
    pub is_formula: bool,
    /// High-level formula expression string if applicable.
    pub formula_expr: Option<String>,
    /// Lowered Excel formula string if applicable.
    pub excel_formula: Option<String>,
    /// Physical cell address on sheet (e.g. `"C14"`).
    pub physical_a1: String,
}

/// Hierarchical dependency or blast-radius calculation tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyTree {
    /// Target root cell for which the tree was queried.
    pub root: CellId,
    /// Number of immediate direct child nodes.
    pub direct_count: usize,
    /// Total count of unique transitively reached cell nodes.
    pub transitive_count: usize,
    /// Hierarchical tree nodes representing incoming or outgoing calculation paths.
    pub nodes: Vec<DependencyNode>,
}

/// A node within a hierarchical dependency or blast-radius tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyNode {
    /// Coordinate identifier of this cell.
    pub cell_id: CellId,
    /// Canonical Excel sheet address (e.g. `"Financials!C14"`).
    pub sheet_address: String,
    /// Evaluated runtime value.
    pub value: Option<CellValue>,
    /// Lowered Excel formula string if applicable.
    pub excel_formula: Option<String>,
    /// Nested child calculation nodes.
    pub children: Vec<DependencyNode>,
}
