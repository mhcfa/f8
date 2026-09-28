"""Type definitions and TypedDict data contracts for the f8 Python SDK."""

from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Literal, TypedDict

if TYPE_CHECKING:
    from f8.builder import CellInputValue as CellInputValue

type CellValueDict = dict[str, object] | str

type SheetCellKind = Literal[
    "empty",
    "table_title",
    "table_subtitle",
    "column_header",
    "section_header",
    "gutter",
    "data",
]


class CellCoordinate(TypedDict):
    """Discrete cell coordinate identifier."""

    table: str
    row: str
    col: str


class TableSummary(TypedDict):
    """Table-level dimension and cell classification summary."""

    id: str
    title: str
    row_count: int
    col_count: int
    cell_count: int
    driver_count: int
    formula_count: int


class SheetSummary(TypedDict):
    """Worksheet-level summary of contained tables and cell counts."""

    name: str
    tables: list[TableSummary]
    total_cells: int


class ModelSummary(TypedDict):
    """High-level compiled model statistics and structure."""

    model_name: str
    theme_name: str
    sheets: list[SheetSummary]
    total_sheets: int
    total_tables: int
    total_cells: int
    total_drivers: int
    total_formulas: int


class CellNeighborInspection(TypedDict):
    """Immediate precedent or dependent cell neighbor inspection."""

    cell_id: CellCoordinate
    sheet_address: str
    value: CellValueDict | None
    formatted_value: str | None


class CellInspection(TypedDict):
    """Detailed inspection record for an individual cell."""

    coordinate: CellCoordinate
    sheet: str
    physical_address: str
    evaluated_value: CellValueDict
    formatted_value: str | None
    format: str | None
    decimals: int | None
    currency: str | None
    is_highlight: bool
    indent: int
    is_formula: bool
    formula_expr: str | None
    excel_formula: str | None
    note: str | None
    precedents: list[CellNeighborInspection]
    dependents: list[CellNeighborInspection]


class DependencyNode(TypedDict):
    """Node in a hierarchical precedent or dependent calculation tree."""

    cell_id: CellCoordinate
    sheet_address: str
    value: CellValueDict | None
    formatted_value: str | None
    excel_formula: str | None
    is_pruned: bool
    children: list[DependencyNode]


class DependencyTree(TypedDict):
    """Hierarchical dependency or blast-radius calculation tree."""

    root: CellCoordinate
    direct_count: int
    transitive_count: int
    nodes: list[DependencyNode]


class ColumnInspection(TypedDict):
    """Column inspection metadata within a table view."""

    id: str | None
    headers: list[str]
    is_gutter: bool
    is_descriptor: bool


class RowInspection(TypedDict):
    """Row inspection metadata within a table view."""

    id: str | None
    section_title: str | None
    is_section_header: bool
    is_gutter: bool
    indent: int


class TableCellInfo(TypedDict):
    """Cell audit metadata within a 2D table inspection view."""

    value: CellValueDict
    is_formula: bool
    formula_expr: str | None
    excel_formula: str | None
    physical_a1: str
    format: str | None
    decimals: int | None
    currency: str | None


class TableInspection(TypedDict):
    """Comprehensive 2D tabular inspection for an entire table."""

    table_id: str
    title: str
    subtitle: str | None
    sheet: str
    columns: list[ColumnInspection]
    rows: list[RowInspection]
    cells: dict[str, TableCellInfo]


class SheetCellInspection(TypedDict):
    """Inspection record for a discrete physical cell on a worksheet."""

    physical_col: int
    physical_a1: str
    table_id: str | None
    row_id: str | None
    col_id: str | None
    kind: SheetCellKind
    is_descriptor: bool
    value: CellValueDict
    formatted_value: str | None
    formula_expr: str | None
    excel_formula: str | None


class SheetRowInspection(TypedDict):
    """Physical row record spanning all cells in a sheet bounding box."""

    physical_row: int
    excel_row: int
    cells: list[SheetCellInspection]


class SheetInspection(TypedDict):
    """Comprehensive physical grid inspection for an entire worksheet."""

    sheet_id: str
    title: str
    tables: list[str]
    min_col: int
    max_col: int
    min_row: int
    max_row: int
    col_widths: dict[int, float]
    rows: list[SheetRowInspection]


class EmissionOptions(TypedDict, total=False):
    """Options controlling OpenXML workbook emission."""

    values_only: bool
    omit_formula_cache: bool
    title: str
    author: str


type AxisRole = Literal["measure", "descriptor"]

type FormatPreset = Literal[
    "currency",
    "percentage",
    "number",
    "multiple",
    "basis_points",
    "date",
    "text",
    "integer",
]

type SemanticStyle = Literal["data", "subtotal", "total"]

type CurrencyCode = Literal[
    "USD",
    "EUR",
    "GBP",
    "JPY",
    "CHF",
    "CAD",
    "AUD",
    "CNY",
    "HKD",
    "SGD",
    "SEK",
    "NOK",
    "NZD",
    "INR",
    "BRL",
    "usd",
    "eur",
    "gbp",
    "jpy",
    "chf",
    "cad",
    "aud",
    "cny",
    "hkd",
    "sgd",
    "sek",
    "nok",
    "nzd",
    "inr",
    "brl",
]

type CellValueLiteral = float | int | str | bool | None


class DetailedCellDict(TypedDict, total=False):
    """Detailed 0D cell override specification dictionary."""

    value: CellValueLiteral
    expr: str
    style: SemanticStyle
    highlight: bool
    format: FormatPreset
    decimals: int
    currency: CurrencyCode
    indent: int
    note: str


class FreezePanesDict(TypedDict):
    """0-indexed split coordinate for worksheet freeze panes."""

    row: int
    col: int


class SheetSpecDict(TypedDict, total=False):
    """Worksheet manifest specification dictionary."""

    name: str
    tab_color: str
    freeze_panes: FreezePanesDict
    show_gridlines: bool
    tables: list[str]


class ProjectManifestDict(TypedDict):
    """Complete project manifest specification dictionary."""

    model_name: str
    theme: str
    sheets: list[SheetSpecDict]


class DataColumnSpecDict(TypedDict, total=False):
    """Specification dictionary for a tabular data column."""

    kind: Literal["data"]
    id: str
    role: AxisRole
    headers: list[str]
    style: SemanticStyle
    highlight: bool
    format: FormatPreset
    decimals: int
    currency: CurrencyCode
    expr: str


class GutterColumnSpecDict(TypedDict):
    """Specification dictionary for an empty spacing column gutter."""

    kind: Literal["gutter"]


type ColumnSpecDict = DataColumnSpecDict | GutterColumnSpecDict


class DataRowSpecDict(TypedDict, total=False):
    """Specification dictionary for a tabular data row."""

    kind: Literal["data"]
    id: str
    style: SemanticStyle
    highlight: bool
    format: FormatPreset
    decimals: int
    currency: CurrencyCode
    expr: str
    cells: dict[str, CellValueLiteral | DetailedCellDict]


class SectionHeaderRowSpecDict(TypedDict, total=False):
    """Specification dictionary for a table section header row."""

    kind: Literal["section_header"]
    title: str
    indent: int
    note: str


class GutterRowSpecDict(TypedDict):
    """Specification dictionary for an empty spacing row gutter."""

    kind: Literal["gutter"]


type RowSpecDict = DataRowSpecDict | SectionHeaderRowSpecDict | GutterRowSpecDict


class TableSpecDict(TypedDict, total=False):
    """Specification dictionary for a complete financial table."""

    id: str
    title: str
    subtitle: str
    currency: CurrencyCode
    collapsible: bool
    columns: list[ColumnSpecDict]
    rows: list[RowSpecDict]


type ManifestInput = Mapping[str, object]
type TableInput = Mapping[str, object]
type ThemeInput = Mapping[str, object] | str


def __getattr__(name: str) -> object:
    if name == "CellInputValue":
        from f8.builder import CellInputValue

        return CellInputValue
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
