"""Fluent authoring DSL and in-memory specification synthesis."""

from __future__ import annotations

import json
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from f8._core import CompiledModel
from f8._core import compile as core_compile
from f8.types import (
    AxisRole,
    BelowPlacementSpecDict,
    CellValueLiteral,
    ColumnSpecDict,
    CurrencyCode,
    DataColumnSpecDict,
    DataRowSpecDict,
    DetailedCellDict,
    FormatPreset,
    GutterColumnSpecDict,
    GutterRowSpecDict,
    ProjectManifestDict,
    RightOfPlacementSpecDict,
    RowSpecDict,
    SectionHeaderRowSpecDict,
    SemanticStyle,
    Shade,
    SheetSpecDict,
    StackPlacementSpecDict,
    TableSpecDict,
    ThemeInput,
)


class _Unset:
    """Sentinel type representing an unprovided value argument."""


_UNSET = _Unset()


@dataclass(frozen=True)
class Cell:
    """Detailed 0D cell override specification."""

    value: CellValueLiteral | _Unset = _UNSET
    expr: str | None = None
    style: SemanticStyle | None = None
    highlight: bool | None = None
    shade: Shade | None = None
    format: FormatPreset | None = None
    decimals: int | None = None
    currency: CurrencyCode | None = None
    indent: int | None = None
    note: str | None = None

    def __post_init__(self) -> None:
        has_value = self.value is not _UNSET
        has_expr = self.expr is not None
        if has_value and has_expr:
            raise ValueError("Cell cannot specify both 'value' and 'expr'")
        if not has_value and not has_expr:
            raise ValueError("Cell must specify either 'value' or 'expr'")


type CellInputValue = CellValueLiteral | Cell | DetailedCellDict


def _normalize_currency(currency: str) -> CurrencyCode:
    """Normalizes an authoring currency code to canonical lowercase ISO representation."""
    return cast(CurrencyCode, currency.lower())


def _serialize_cell_input(
    cell: CellInputValue,
) -> CellValueLiteral | DetailedCellDict:
    if isinstance(cell, Cell):
        has_meta = (
            cell.style is not None
            or cell.highlight is not None
            or cell.shade is not None
            or cell.format is not None
            or cell.decimals is not None
            or cell.currency is not None
            or cell.indent is not None
            or cell.note is not None
        )
        if cell.expr is not None or has_meta:
            d: DetailedCellDict = {}
            if not isinstance(cell.value, _Unset):
                d["value"] = cell.value
            if cell.expr is not None:
                d["expr"] = cell.expr
            if cell.style is not None:
                d["style"] = cell.style
            if cell.highlight is not None:
                d["highlight"] = cell.highlight
            if cell.shade is not None:
                d["shade"] = cell.shade
            if cell.format is not None:
                d["format"] = cell.format
            if cell.decimals is not None:
                d["decimals"] = cell.decimals
            if cell.currency is not None:
                d["currency"] = _normalize_currency(cell.currency)
            if cell.indent is not None:
                d["indent"] = cell.indent
            if cell.note is not None:
                d["note"] = cell.note
            return d
        if not isinstance(cell.value, _Unset):
            return cell.value
        return None

    if isinstance(cell, dict):
        curr = cell.get("currency")
        if isinstance(curr, str):
            norm_dict: DetailedCellDict = {
                **cell,
                "currency": _normalize_currency(curr),
            }
            return norm_dict
        return cell

    return cell


@dataclass
class _DataColumn:
    id: str
    role: AxisRole
    group: str | None
    headers: list[str]
    style: SemanticStyle | None
    highlight: bool | None
    shade: Shade | None
    format: FormatPreset | None
    decimals: int | None
    currency: CurrencyCode | None
    expr: str | None


class _GutterColumn:
    pass


type _ColumnItem = _DataColumn | _GutterColumn


@dataclass
class _DataRow:
    id: str
    cells: dict[str, CellInputValue]
    expr: str | None
    style: SemanticStyle | None
    highlight: bool | None
    shade: Shade | None
    format: FormatPreset | None
    decimals: int | None
    currency: CurrencyCode | None


@dataclass
class _SectionHeaderRow:
    title: str
    indent: int | None
    note: str | None


class _GutterRow:
    pass


type _RowItem = _DataRow | _SectionHeaderRow | _GutterRow


class Table:
    """Fluent builder for tabular financial statements."""

    def __init__(
        self,
        id: str,
        title: str,
        subtitle: str | None = None,
        currency: CurrencyCode | None = None,
        collapsible: bool = False,
        sheet: Sheet | None = None,
        *,
        right_of: str | Table | None = None,
        below: str | Table | None = None,
        gap: int | None = None,
    ) -> None:
        if right_of is not None and below is not None:
            raise ValueError("Cannot specify both 'right_of' and 'below'")
        if gap is not None and gap < 0:
            raise ValueError(f"Table 'gap' must be non-negative, got {gap}")

        self.id = id
        self.title = title
        self.subtitle = subtitle
        self.currency = currency
        self.collapsible = collapsible
        self.right_of: str | None = right_of.id if isinstance(right_of, Table) else right_of
        self.below: str | None = below.id if isinstance(below, Table) else below
        self.gap: int | None = gap
        self._sheet = sheet
        self._columns: list[_ColumnItem] = []
        self._col_ids: set[str] = set()
        self._rows: list[_RowItem] = []
        self._row_ids: set[str] = set()
        self._data_rows: dict[str, _DataRow] = {}

    def add_column(
        self,
        id: str,
        role: AxisRole = "measure",
        header: str | None = None,
        headers: list[str] | None = None,
        group: str | None = None,
        style: SemanticStyle | None = None,
        highlight: bool | None = None,
        shade: Shade | None = None,
        format: FormatPreset | None = None,
        decimals: int | None = None,
        currency: CurrencyCode | None = None,
        expr: str | None = None,
    ) -> Table:
        """Appends a data or descriptor column specification to the table."""
        if header is not None and headers is not None:
            raise ValueError("Cannot specify both 'header' and 'headers'")
        if group is not None and not group.strip():
            raise ValueError("Column 'group' cannot be empty")
        if id in self._col_ids:
            raise ValueError(f"Column '{id}' already exists in table '{self.id}'")

        if header is not None:
            norm_headers = [header]
        elif headers is not None:
            norm_headers = list(headers)
        else:
            norm_headers = []

        col = _DataColumn(
            id=id,
            role=role,
            group=group,
            headers=norm_headers,
            style=style,
            highlight=highlight,
            shade=shade,
            format=format,
            decimals=decimals,
            currency=currency,
            expr=expr,
        )
        self._columns.append(col)
        self._col_ids.add(id)
        return self

    def add_column_gutter(self) -> Table:
        """Appends an empty visual spacing gutter column to the table."""
        self._columns.append(_GutterColumn())
        return self

    def add_row(
        self,
        id: str,
        cells: Mapping[str, CellInputValue] | None = None,
        expr: str | None = None,
        style: SemanticStyle | None = None,
        highlight: bool | None = None,
        shade: Shade | None = None,
        format: FormatPreset | None = None,
        decimals: int | None = None,
        currency: CurrencyCode | None = None,
    ) -> Table:
        """Appends a data row with optional cell values and 1D vector expression."""
        if id in self._row_ids:
            raise ValueError(f"Row '{id}' already exists in table '{self.id}'")

        stored_cells = dict(cells) if cells is not None else {}
        row = _DataRow(
            id=id,
            cells=stored_cells,
            expr=expr,
            style=style,
            highlight=highlight,
            shade=shade,
            format=format,
            decimals=decimals,
            currency=currency,
        )
        self._rows.append(row)
        self._row_ids.add(id)
        self._data_rows[id] = row
        return self

    def add_row_gutter(self) -> Table:
        """Appends an empty visual spacing gutter row to the table."""
        self._rows.append(_GutterRow())
        return self

    def add_section_header(
        self,
        title: str,
        indent: int | None = None,
        note: str | None = None,
    ) -> Table:
        """Appends a non-calculable section header divider row to the table."""
        self._rows.append(_SectionHeaderRow(title=title, indent=indent, note=note))
        return self

    def set_cell(
        self,
        row: str,
        col: str,
        cell: CellInputValue | _Unset = _UNSET,
        value: CellValueLiteral | _Unset = _UNSET,
        expr: str | None = None,
        style: SemanticStyle | None = None,
        highlight: bool | None = None,
        shade: Shade | None = None,
        format: FormatPreset | None = None,
        decimals: int | None = None,
        currency: CurrencyCode | None = None,
        indent: int | None = None,
        note: str | None = None,
    ) -> Table:
        """Sets or merges a 0D cell override at `(row, col)`."""
        if row not in self._data_rows:
            raise KeyError(f"Row '{row}' does not exist in table '{self.id}'")

        target_row = self._data_rows[row]
        has_existing = col in target_row.cells

        existing_val: CellValueLiteral | _Unset = _UNSET
        existing_expr: str | None = None
        existing_style: SemanticStyle | None = None
        existing_highlight: bool | None = None
        existing_shade: Shade | None = None
        existing_format: FormatPreset | None = None
        existing_decimals: int | None = None
        existing_currency: CurrencyCode | None = None
        existing_indent: int | None = None
        existing_note: str | None = None

        if has_existing:
            existing_cell = target_row.cells[col]
            if isinstance(existing_cell, Cell):
                existing_val = existing_cell.value
                existing_expr = existing_cell.expr
                existing_style = existing_cell.style
                existing_highlight = existing_cell.highlight
                existing_shade = existing_cell.shade
                existing_format = existing_cell.format
                existing_decimals = existing_cell.decimals
                existing_currency = existing_cell.currency
                existing_indent = existing_cell.indent
                existing_note = existing_cell.note
            elif isinstance(existing_cell, dict):
                existing_val = existing_cell.get("value", _UNSET)
                existing_expr = existing_cell.get("expr", None)
                existing_style = existing_cell.get("style", None)
                existing_highlight = existing_cell.get("highlight", None)
                existing_shade = existing_cell.get("shade", None)
                existing_format = existing_cell.get("format", None)
                existing_decimals = existing_cell.get("decimals", None)
                existing_currency = existing_cell.get("currency", None)
                existing_indent = existing_cell.get("indent", None)
                existing_note = existing_cell.get("note", None)
            else:
                existing_val = existing_cell

        base_val: CellValueLiteral | _Unset = _UNSET
        base_expr: str | None = None
        base_style: SemanticStyle | None = None
        base_highlight: bool | None = None
        base_shade: Shade | None = None
        base_format: FormatPreset | None = None
        base_decimals: int | None = None
        base_currency: CurrencyCode | None = None
        base_indent: int | None = None
        base_note: str | None = None

        if isinstance(cell, Cell):
            base_val = cell.value
            base_expr = cell.expr
            base_style = cell.style
            base_highlight = cell.highlight
            base_shade = cell.shade
            base_format = cell.format
            base_decimals = cell.decimals
            base_currency = cell.currency
            base_indent = cell.indent
            base_note = cell.note
        elif isinstance(cell, dict):
            base_val = cell.get("value", _UNSET)
            base_expr = cell.get("expr", None)
            base_style = cell.get("style", None)
            base_highlight = cell.get("highlight", None)
            base_shade = cell.get("shade", None)
            base_format = cell.get("format", None)
            base_decimals = cell.get("decimals", None)
            base_currency = cell.get("currency", None)
            base_indent = cell.get("indent", None)
            base_note = cell.get("note", None)
        elif cell is not _UNSET:
            base_val = cell

        kw_val_specified = value is not _UNSET
        kw_expr_specified = expr is not None

        input_val = value if kw_val_specified else base_val
        input_expr = expr if kw_expr_specified else base_expr

        has_input_payload = (input_val is not _UNSET) or (input_expr is not None)

        if has_input_payload:
            if input_val is not _UNSET and input_expr is not None:
                msg = (
                    f"Cell override for row '{row}', col '{col}' "
                    "cannot specify both 'value' and 'expr'"
                )
                raise ValueError(msg)
            final_val = input_val
            final_expr = input_expr
        elif has_existing:
            final_val = existing_val
            final_expr = existing_expr
        else:
            msg = (
                f"Cell override for row '{row}', col '{col}' must specify either 'value' or 'expr'"
            )
            raise ValueError(msg)

        final_style = (
            style
            if style is not None
            else (base_style if base_style is not None else existing_style)
        )
        final_highlight = (
            highlight
            if highlight is not None
            else (base_highlight if base_highlight is not None else existing_highlight)
        )
        final_shade = (
            shade
            if shade is not None
            else (base_shade if base_shade is not None else existing_shade)
        )
        final_format = (
            format
            if format is not None
            else (base_format if base_format is not None else existing_format)
        )
        final_decimals = (
            decimals
            if decimals is not None
            else (base_decimals if base_decimals is not None else existing_decimals)
        )
        final_currency = (
            currency
            if currency is not None
            else (base_currency if base_currency is not None else existing_currency)
        )
        final_indent = (
            indent
            if indent is not None
            else (base_indent if base_indent is not None else existing_indent)
        )
        final_note = (
            note if note is not None else (base_note if base_note is not None else existing_note)
        )

        target_row.cells[col] = Cell(
            value=final_val,
            expr=final_expr,
            style=final_style,
            highlight=final_highlight,
            shade=final_shade,
            format=final_format,
            decimals=final_decimals,
            currency=final_currency,
            indent=final_indent,
            note=final_note,
        )
        return self

    def __f8_spec__(self) -> TableSpecDict:
        """Serializes the table into a canonical `TableSpecDict` payload."""
        cols_spec: list[ColumnSpecDict] = []
        for col in self._columns:
            if isinstance(col, _GutterColumn):
                gutter_col: GutterColumnSpecDict = {"kind": "gutter"}
                cols_spec.append(gutter_col)
                continue

            data_col: DataColumnSpecDict = {
                "kind": "data",
                "id": col.id,
                "role": col.role,
                "headers": list(col.headers),
            }
            if col.group is not None:
                data_col["group"] = col.group
            if col.style is not None:
                data_col["style"] = col.style
            if col.highlight is not None:
                data_col["highlight"] = col.highlight
            if col.shade is not None:
                data_col["shade"] = col.shade
            if col.format is not None:
                data_col["format"] = col.format
            if col.decimals is not None:
                data_col["decimals"] = col.decimals
            if col.currency is not None:
                data_col["currency"] = _normalize_currency(col.currency)
            if col.expr is not None:
                data_col["expr"] = col.expr
            cols_spec.append(data_col)

        rows_spec: list[RowSpecDict] = []
        for row in self._rows:
            if isinstance(row, _GutterRow):
                gutter_row: GutterRowSpecDict = {"kind": "gutter"}
                rows_spec.append(gutter_row)
            elif isinstance(row, _SectionHeaderRow):
                sh_row: SectionHeaderRowSpecDict = {
                    "kind": "section_header",
                    "title": row.title,
                }
                if row.indent is not None:
                    sh_row["indent"] = row.indent
                if row.note is not None:
                    sh_row["note"] = row.note
                rows_spec.append(sh_row)
            else:
                data_row: DataRowSpecDict = {
                    "kind": "data",
                    "id": row.id,
                    "cells": {
                        col_id: _serialize_cell_input(cell_in)
                        for col_id, cell_in in row.cells.items()
                    },
                }
                if row.style is not None:
                    data_row["style"] = row.style
                if row.highlight is not None:
                    data_row["highlight"] = row.highlight
                if row.shade is not None:
                    data_row["shade"] = row.shade
                if row.format is not None:
                    data_row["format"] = row.format
                if row.decimals is not None:
                    data_row["decimals"] = row.decimals
                if row.currency is not None:
                    data_row["currency"] = _normalize_currency(row.currency)
                if row.expr is not None:
                    data_row["expr"] = row.expr
                rows_spec.append(data_row)

        table_spec: TableSpecDict = {
            "id": self.id,
            "title": self.title,
            "collapsible": self.collapsible,
            "columns": cols_spec,
            "rows": rows_spec,
        }
        if self.subtitle is not None:
            table_spec["subtitle"] = self.subtitle
        if self.currency is not None:
            table_spec["currency"] = _normalize_currency(self.currency)
        if self.right_of is not None:
            right_of_spec: RightOfPlacementSpecDict = {
                "kind": "right_of",
                "target": self.right_of,
            }
            if self.gap is not None:
                right_of_spec["gap"] = self.gap
            table_spec["placement"] = right_of_spec
        elif self.below is not None:
            below_spec: BelowPlacementSpecDict = {
                "kind": "below",
                "target": self.below,
            }
            if self.gap is not None:
                below_spec["gap"] = self.gap
            table_spec["placement"] = below_spec
        elif self.gap is not None:
            stack_spec: StackPlacementSpecDict = {
                "kind": "stack",
                "gap": self.gap,
            }
            table_spec["placement"] = stack_spec
        return table_spec


class Sheet:
    """Fluent container for worksheets containing financial tables."""

    def __init__(
        self,
        name: str,
        tab_color: str | None = None,
        show_gridlines: bool = True,
        freeze_panes: tuple[int, int] | None = None,
        model: Model | None = None,
    ) -> None:
        if freeze_panes is not None:
            r, c = freeze_panes
            if r < 0 or c < 0:
                msg = (
                    "freeze_panes must be a tuple of two non-negative integers (row, col), "
                    f"got {freeze_panes!r}"
                )
                raise ValueError(msg)

        self.name = name
        self.tab_color = tab_color
        self.show_gridlines = show_gridlines
        self.freeze_panes = freeze_panes
        self._model = model
        self._tables: list[Table] = []
        self._table_ids: set[str] = set()

    def add_table(
        self,
        id: str,
        title: str,
        subtitle: str | None = None,
        currency: CurrencyCode | None = None,
        collapsible: bool = False,
        *,
        right_of: str | Table | None = None,
        below: str | Table | None = None,
        gap: int | None = None,
    ) -> Table:
        """Creates and appends a new `Table` to this worksheet."""
        if self._model is not None:
            for s in self._model.sheets:
                for t in s.tables:
                    if t.id == id:
                        msg = f"Table '{id}' already exists in model '{self._model.name}'"
                        raise ValueError(msg)
        elif id in self._table_ids:
            msg = f"Table '{id}' already exists in sheet '{self.name}'"
            raise ValueError(msg)

        table = Table(
            id=id,
            title=title,
            subtitle=subtitle,
            currency=currency,
            collapsible=collapsible,
            sheet=self,
            right_of=right_of,
            below=below,
            gap=gap,
        )
        self._tables.append(table)
        self._table_ids.add(id)
        return table

    def get_table(self, id: str) -> Table:
        """Looks up a `Table` on this worksheet by its `id`."""
        for table in self._tables:
            if table.id == id:
                return table
        raise KeyError(f"Table '{id}' not found in sheet '{self.name}'")

    @property
    def tables(self) -> list[Table]:
        """Ordered list of tables belonging to this worksheet."""
        return list(self._tables)

    def __f8_spec__(self) -> SheetSpecDict:
        """Serializes the worksheet into a canonical `SheetSpecDict` payload."""
        spec: SheetSpecDict = {
            "name": self.name,
            "show_gridlines": self.show_gridlines,
            "tables": [table.id for table in self._tables],
        }
        if self.tab_color is not None:
            spec["tab_color"] = self.tab_color
        if self.freeze_panes is not None:
            spec["freeze_panes"] = {
                "row": self.freeze_panes[0],
                "col": self.freeze_panes[1],
            }
        return spec


def _resolve_theme_input(theme: ThemeInput) -> Mapping[str, object] | str:
    """Resolves a theme input into a built-in preset name or inline theme dictionary."""
    if isinstance(theme, Path):
        parsed: object = json.loads(theme.read_text(encoding="utf-8"))
        if not isinstance(parsed, dict):
            msg = f"Theme JSON file '{theme}' must contain a JSON object at root"
            raise ValueError(msg)
        return cast(dict[str, object], parsed)
    return theme


class Model:
    """Fluent root container for financial models and specification synthesis."""

    def __init__(self, name: str = "Model", theme: ThemeInput = "corporate_navy") -> None:
        self.name = name
        self.theme: Mapping[str, object] | str = _resolve_theme_input(theme)
        self._sheets: list[Sheet] = []
        self._sheet_names: set[str] = set()

    def add_sheet(
        self,
        name: str,
        tab_color: str | None = None,
        show_gridlines: bool = True,
        freeze_panes: tuple[int, int] | None = None,
    ) -> Sheet:
        """Creates and appends a new `Sheet` to the model."""
        if name in self._sheet_names:
            raise ValueError(f"Sheet '{name}' already exists in model '{self.name}'")

        sheet = Sheet(
            name=name,
            tab_color=tab_color,
            show_gridlines=show_gridlines,
            freeze_panes=freeze_panes,
            model=self,
        )
        self._sheets.append(sheet)
        self._sheet_names.add(name)
        return sheet

    def get_sheet(self, name: str) -> Sheet:
        """Looks up a `Sheet` in the model by `name`."""
        for sheet in self._sheets:
            if sheet.name == name:
                return sheet
        raise KeyError(f"Sheet '{name}' not found in model '{self.name}'")

    def get_table(self, id: str) -> Table:
        """Looks up a `Table` across all worksheets in the model by `id`."""
        for sheet in self._sheets:
            for table in sheet.tables:
                if table.id == id:
                    return table
        raise KeyError(f"Table '{id}' not found in model '{self.name}'")

    @property
    def sheets(self) -> list[Sheet]:
        """Ordered list of worksheets in the model."""
        return list(self._sheets)

    @property
    def tables(self) -> list[Table]:
        """Ordered list of all tables across all worksheets in the model."""
        return [table for sheet in self._sheets for table in sheet.tables]

    def __f8_spec__(self) -> tuple[ProjectManifestDict, list[TableSpecDict]]:
        """Serializes the model into `(ProjectManifestDict, list[TableSpecDict])`."""
        manifest: ProjectManifestDict = {
            "model_name": self.name,
            "theme": self.theme,
            "sheets": [sheet.__f8_spec__() for sheet in self._sheets],
        }
        tables: list[TableSpecDict] = [table.__f8_spec__() for table in self.tables]
        return manifest, tables

    def compile(self) -> CompiledModel:
        """Compiles, verifies, and evaluates the model into a `CompiledModel`."""
        manifest, tables = self.__f8_spec__()
        return core_compile(manifest, tables)


__all__ = ["Cell", "CellInputValue", "Model", "Sheet", "Table"]
