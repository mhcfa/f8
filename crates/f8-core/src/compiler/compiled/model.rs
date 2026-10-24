//! Data contracts for compiled model inspection and auditing.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::compiler::ir::id::CellId;
use crate::model::format::{CurrencyCode, FormatPreset, SemanticStyle, Shade};
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
    /// Pre-formatted runtime value string (e.g. `"$115.0"`, `"15.0%"`).
    #[serde(default)]
    pub formatted_value: Option<String>,
    /// Formatting preset if assigned.
    pub format: Option<FormatPreset>,
    /// Display precision override if specified.
    pub decimals: Option<u32>,
    /// Currency code if assigned.
    pub currency: Option<CurrencyCode>,
    /// Visual highlight emphasis flag.
    pub is_highlight: bool,
    /// Semantic background shade if assigned.
    pub shade: Option<Shade>,
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
    /// Direct incoming precedent cell coordinates and evaluated values.
    pub precedents: Vec<CellNeighborInspection>,
    /// Direct outgoing dependent cell coordinates and evaluated values.
    pub dependents: Vec<CellNeighborInspection>,
}

/// Direct neighbor cell inspection (precedent or dependent) within a cell audit card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellNeighborInspection {
    /// Coordinate identifier `(TableId, RowId, ColId)`.
    pub cell_id: CellId,
    /// Canonical Excel sheet address (e.g. `"Financials!C14"`).
    pub sheet_address: String,
    /// Evaluated runtime value if available.
    pub value: Option<CellValue>,
    /// Pre-formatted runtime value string (e.g. `"$115.0"`, `"15.0%"`).
    pub formatted_value: Option<String>,
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
    #[serde(
        serialize_with = "serialize_cells_map",
        deserialize_with = "deserialize_cells_map"
    )]
    pub cells: IndexMap<CellId, TableCellInfo>,
}

fn serialize_cells_map<S>(
    cells: &IndexMap<CellId, TableCellInfo>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(Some(cells.len()))?;
    for (k, v) in cells {
        map.serialize_entry(&k.to_string(), v)?;
    }
    map.end()
}

fn deserialize_cells_map<'de, D>(
    deserializer: D,
) -> Result<IndexMap<CellId, TableCellInfo>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let string_map: IndexMap<String, TableCellInfo> = IndexMap::deserialize(deserializer)?;
    let mut cells = IndexMap::with_capacity(string_map.len());
    for (key, val) in string_map {
        if let Some((table, rest)) = key.split_once("[\"")
            && let Some((row, rest2)) = rest.split_once("\", \"")
            && let Some((col, _)) = rest2.split_once("\"]")
        {
            let cell_id = CellId::new(table, row, col);
            cells.insert(cell_id, val);
            continue;
        }
        return Err(serde::de::Error::custom(format!(
            "Invalid CellId key: {key}"
        )));
    }
    Ok(cells)
}

/// Column inspection metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnInspection {
    /// Column identifier if this is a data column.
    pub id: Option<ColId>,
    /// Horizontal spanning group banner label if declared.
    pub group: Option<String>,
    /// Per-column stacked header labels.
    pub headers: Vec<String>,
    /// `true` if this column is a blank spacing gutter.
    pub is_gutter: bool,
    /// `true` if this column is a descriptor axis (carrying row/item labels).
    pub is_descriptor: bool,
    /// Structural presentation style if this is a data column.
    pub style: Option<SemanticStyle>,
    /// Semantic background shade if this is a data column.
    pub shade: Option<Shade>,
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
    /// Structural presentation style if this is a calculable data row.
    pub style: Option<SemanticStyle>,
    /// Semantic background shade if this is a calculable data row.
    pub shade: Option<Shade>,
}

/// Cell-level audit information within a table inspection view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCellInfo {
    /// Evaluated cell value.
    pub value: CellValue,
    /// Pre-formatted runtime value string according to cell format preset, currency, and decimals.
    pub formatted_value: Option<String>,
    /// `true` if calculated by formula.
    pub is_formula: bool,
    /// High-level formula expression string if applicable.
    pub formula_expr: Option<String>,
    /// Lowered Excel formula string if applicable.
    pub excel_formula: Option<String>,
    /// Physical cell address on sheet (e.g. `"C14"`).
    pub physical_a1: String,
    /// Formatting preset if assigned.
    pub format: Option<FormatPreset>,
    /// Display precision override if specified.
    pub decimals: Option<u32>,
    /// Currency code if assigned.
    pub currency: Option<CurrencyCode>,
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
    /// Pre-formatted runtime value string (e.g. `"$182.4"`, `"15.0%"`).
    #[serde(default)]
    pub formatted_value: Option<String>,
    /// Lowered Excel formula string if applicable.
    pub excel_formula: Option<String>,
    /// `true` if this calculation subtree was already expanded earlier in traversal.
    #[serde(default)]
    pub is_pruned: bool,
    /// Nested child calculation nodes.
    pub children: Vec<DependencyNode>,
}

/// Structural classification of a cell within a physical worksheet layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetCellKind {
    /// Blank or unused cell within the sheet bounding box.
    Empty,
    /// Table title banner cell.
    TableTitle,
    /// Table subtitle banner cell.
    TableSubtitle,
    /// Multi-tier column header cell.
    ColumnHeader,
    /// Structural section header row cell.
    SectionHeader,
    /// Explicit spacing gutter cell.
    Gutter,
    /// Calculable data or literal input cell.
    Data,
}

/// Inspection record for a discrete physical cell on a worksheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetCellInspection {
    /// 0-indexed column index on worksheet.
    pub physical_col: u32,
    /// Canonical Excel cell address (e.g. `"C7"`).
    pub physical_a1: String,
    /// Programmatic table identifier if cell belongs to a table.
    pub table_id: Option<TableId>,
    /// Programmatic row identifier if this is a data cell.
    pub row_id: Option<RowId>,
    /// Programmatic column identifier if this is a data cell.
    pub col_id: Option<ColId>,
    /// Cell structural classification.
    pub kind: SheetCellKind,
    /// `true` if this cell is on a descriptor column axis (carrying row/item labels).
    pub is_descriptor: bool,
    /// Evaluated runtime value.
    pub value: CellValue,
    /// Formatted display string if applicable.
    pub formatted_value: Option<String>,
    /// High-level formula expression string if applicable.
    pub formula_expr: Option<String>,
    /// Lowered concrete Excel formula string if applicable.
    pub excel_formula: Option<String>,
}

/// Physical row record containing all cells across sheet columns within bounding box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetRowInspection {
    /// 0-indexed row index on worksheet.
    pub physical_row: u32,
    /// 1-indexed Excel row number (e.g. 7).
    pub excel_row: u32,
    /// Cells spanning active sheet columns.
    pub cells: Vec<SheetCellInspection>,
}

/// Comprehensive physical grid inspection for an entire worksheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetInspection {
    /// Worksheet programmatic identifier.
    pub sheet_id: SheetId,
    /// Worksheet display name.
    pub title: String,
    /// Ordered list of table identifiers placed on this worksheet.
    pub tables: Vec<TableId>,
    /// Minimum column index in sheet bounding box (0-indexed).
    pub min_col: u32,
    /// Maximum column index in sheet bounding box (0-indexed).
    pub max_col: u32,
    /// Minimum row index in sheet bounding box (0-indexed).
    pub min_row: u32,
    /// Maximum row index in sheet bounding box (0-indexed).
    pub max_row: u32,
    /// Sheet-wide column widths (0-indexed col -> character widths).
    pub col_widths: IndexMap<u32, f64>,
    /// Physical row records spanning active rows.
    pub rows: Vec<SheetRowInspection>,
}
