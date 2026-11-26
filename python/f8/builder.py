"""Fluent authoring DSL and in-memory specification synthesis."""

from __future__ import annotations

import json
import math
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal, cast

from f8._core import CompiledModel
from f8._core import compile as core_compile
from f8.types import (
    AxisRole,
    BandSpecDict,
    CellValueLiteral,
    ColumnSpecDict,
    CurrencyCode,
    DataColumnSpecDict,
    DataRowSpecDict,
    DetailedCellDict,
    FormatPreset,
    GridSheetLayoutSpecDict,
    GutterColumnSpecDict,
    GutterRowSpecDict,
    HeaderFooterDict,
    LaneSpecDict,
    PageOrientation,
    PaperSize,
    PrintFitMode,
    ProjectManifestDict,
    RowSpecDict,
    SectionHeaderRowSpecDict,
    SemanticStyle,
    Shade,
    SheetLayoutMode,
    SheetPrintDict,
    SheetPrintOverrideDict,
    SheetSpecDict,
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
type AxisSelector = str | Iterable[str] | slice


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
        subtitle: str | Sequence[str] | None = None,
        currency: CurrencyCode | None = None,
        collapsible: bool = False,
        sheet: Sheet | None = None,
    ) -> None:
        self.id = id
        self.title = title
        if subtitle is None:
            self.subtitles: list[str] = []
        elif isinstance(subtitle, str):
            self.subtitles = [subtitle]
        else:
            self.subtitles = list(subtitle)
        self.currency = currency
        self.collapsible = collapsible
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

    def _resolve_row_selector(self, selector: AxisSelector) -> list[str]:
        if isinstance(selector, str):
            if selector not in self._data_rows:
                raise KeyError(f"Row '{selector}' does not exist in table '{self.id}'")
            return [selector]

        if isinstance(selector, slice):
            if selector.step is not None:
                raise ValueError("Row slice selector cannot specify a step")
            if not isinstance(selector.start, str) or not isinstance(selector.stop, str):
                raise ValueError(
                    "Row slice selector requires non-None string 'start' and 'stop' bounds"
                )
            if selector.start not in self._data_rows:
                raise KeyError(f"Row '{selector.start}' does not exist in table '{self.id}'")
            if selector.stop not in self._data_rows:
                raise KeyError(f"Row '{selector.stop}' does not exist in table '{self.id}'")
            data_row_ids = [r.id for r in self._rows if isinstance(r, _DataRow)]
            start_idx = data_row_ids.index(selector.start)
            stop_idx = data_row_ids.index(selector.stop)
            if start_idx > stop_idx:
                msg = (
                    f"Inverted row slice '{selector.start}'..'{selector.stop}' in table '{self.id}'"
                )
                raise ValueError(msg)
            return data_row_ids[start_idx : stop_idx + 1]

        row_ids = list(selector)
        if not row_ids:
            raise ValueError("Row selector iterable cannot be empty")
        for row_id in row_ids:
            if row_id not in self._data_rows:
                raise KeyError(f"Row '{row_id}' does not exist in table '{self.id}'")
        return row_ids

    def _resolve_col_selector(self, selector: AxisSelector) -> list[str]:
        if isinstance(selector, str):
            if selector not in self._col_ids:
                raise KeyError(f"Column '{selector}' does not exist in table '{self.id}'")
            return [selector]

        if isinstance(selector, slice):
            if selector.step is not None:
                raise ValueError("Column slice selector cannot specify a step")
            if not isinstance(selector.start, str) or not isinstance(selector.stop, str):
                raise ValueError(
                    "Column slice selector requires non-None string 'start' and 'stop' bounds"
                )
            if selector.start not in self._col_ids:
                raise KeyError(f"Column '{selector.start}' does not exist in table '{self.id}'")
            if selector.stop not in self._col_ids:
                raise KeyError(f"Column '{selector.stop}' does not exist in table '{self.id}'")
            data_col_ids = [c.id for c in self._columns if isinstance(c, _DataColumn)]
            start_idx = data_col_ids.index(selector.start)
            stop_idx = data_col_ids.index(selector.stop)
            if start_idx > stop_idx:
                msg = (
                    f"Inverted column slice '{selector.start}'..'{selector.stop}' "
                    f"in table '{self.id}'"
                )
                raise ValueError(msg)
            return data_col_ids[start_idx : stop_idx + 1]

        col_ids = list(selector)
        if not col_ids:
            raise ValueError("Column selector iterable cannot be empty")
        for col_id in col_ids:
            if col_id not in self._col_ids:
                raise KeyError(f"Column '{col_id}' does not exist in table '{self.id}'")
        return col_ids

    def _resolve_cell_override(
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
    ) -> Cell:
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

        return Cell(
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
        if col not in self._col_ids:
            raise KeyError(f"Column '{col}' does not exist in table '{self.id}'")

        self._data_rows[row].cells[col] = self._resolve_cell_override(
            row=row,
            col=col,
            cell=cell,
            value=value,
            expr=expr,
            style=style,
            highlight=highlight,
            shade=shade,
            format=format,
            decimals=decimals,
            currency=currency,
            indent=indent,
            note=note,
        )
        return self

    def set_cells(
        self,
        row: AxisSelector,
        cols: AxisSelector,
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
        """Sets or merges 0D cell overrides across resolved `(row, cols)` coordinates."""
        row_ids = self._resolve_row_selector(row)
        col_ids = self._resolve_col_selector(cols)

        updates: list[tuple[str, str, Cell]] = []
        for r_id in row_ids:
            for c_id in col_ids:
                resolved = self._resolve_cell_override(
                    row=r_id,
                    col=c_id,
                    cell=cell,
                    value=value,
                    expr=expr,
                    style=style,
                    highlight=highlight,
                    shade=shade,
                    format=format,
                    decimals=decimals,
                    currency=currency,
                    indent=indent,
                    note=note,
                )
                updates.append((r_id, c_id, resolved))

        for r_id, c_id, resolved_cell in updates:
            self._data_rows[r_id].cells[c_id] = resolved_cell

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
        if self.subtitles:
            table_spec["subtitles"] = list(self.subtitles)
        if self.currency is not None:
            table_spec["currency"] = _normalize_currency(self.currency)
        return table_spec


type TableRef = str | Table
type LaneItem = Lane | TableRef
type BandItem = Band | TableRef


def _validate_non_negative_int(value: object, field_name: str) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int):
        msg = f"'{field_name}' must be a non-negative int, got {type(value).__name__}"
        raise TypeError(msg)
    if value < 0:
        msg = f"'{field_name}' must be non-negative, got {value}"
        raise ValueError(msg)
    return value


def _as_object_tuple(
    value: object,
    *,
    forbidden: tuple[type, ...],
    label: str,
) -> tuple[object, ...]:
    if isinstance(value, forbidden) or not isinstance(value, Sequence):
        msg = f"{label}, got {type(value).__name__}"
        raise TypeError(msg)
    return tuple(cast(Sequence[object], value))


@dataclass(frozen=True, init=False)
class Lane:
    """Vertical stack of tables sharing one column lane within a band."""

    tables: tuple[str, ...]
    table_gap: int | None = None
    _table_refs: tuple[TableRef, ...] = field(init=False, repr=False, compare=False, hash=False)

    def __init__(
        self,
        tables: Sequence[TableRef],
        *,
        table_gap: int | None = None,
    ) -> None:
        raw_items = _as_object_tuple(
            tables,
            forbidden=(str, bytes, Table),
            label="Lane 'tables' must be a sequence of Table or str",
        )
        if not raw_items:
            raise ValueError("Lane 'tables' cannot be empty")
        normalized_ids: list[str] = []
        raw_refs: list[TableRef] = []
        for item in raw_items:
            if isinstance(item, Table):
                normalized_ids.append(item.id)
                raw_refs.append(item)
            elif isinstance(item, str):
                normalized_ids.append(item)
                raw_refs.append(item)
            else:
                msg = f"Lane table item must be a Table or str, got {type(item).__name__}"
                raise TypeError(msg)
        validated_gap = _validate_non_negative_int(table_gap, "table_gap")
        if len(normalized_ids) == 1 and validated_gap is not None:
            msg = f"Lane with a single table cannot specify 'table_gap' ({validated_gap})"
            raise ValueError(msg)
        object.__setattr__(self, "tables", tuple(normalized_ids))
        object.__setattr__(self, "table_gap", validated_gap)
        object.__setattr__(self, "_table_refs", tuple(raw_refs))

    def __f8_table_refs__(self) -> tuple[TableRef, ...]:
        """Returns the original `Table` or `str` references passed at construction time."""
        return self._table_refs


_ALLOWED_PRINT_KEYS = frozenset(
    {"paper_size", "orientation", "fit", "center_horizontally", "header", "footer"}
)
_ALLOWED_HEADER_FOOTER_KEYS = frozenset({"left", "center", "right"})

type _PrintScope = Literal["model", "sheet"]


def _validate_optional_str(value: object, field_name: str) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        msg = f"'{field_name}' must be a str or None, got {type(value).__name__}"
        raise TypeError(msg)
    return value


def _validate_optional_paper_size(value: object) -> PaperSize | None:
    if value is None:
        return None
    if not isinstance(value, str):
        msg = f"'paper_size' must be a str or None, got {type(value).__name__}"
        raise TypeError(msg)
    if value == "a4":
        return "a4"
    if value == "letter":
        return "letter"
    msg = f"Invalid paper_size {value!r}; expected 'a4' or 'letter'"
    raise ValueError(msg)


def _validate_optional_orientation(value: object) -> PageOrientation | None:
    if value is None:
        return None
    if not isinstance(value, str):
        msg = f"'orientation' must be a str or None, got {type(value).__name__}"
        raise TypeError(msg)
    if value == "landscape":
        return "landscape"
    if value == "portrait":
        return "portrait"
    msg = f"Invalid orientation {value!r}; expected 'landscape' or 'portrait'"
    raise ValueError(msg)


def _validate_optional_fit(value: object) -> PrintFitMode | None:
    if value is None:
        return None
    if not isinstance(value, str):
        msg = f"'fit' must be a str or None, got {type(value).__name__}"
        raise TypeError(msg)
    if value == "fit_width":
        return "fit_width"
    if value == "fit_page":
        return "fit_page"
    msg = f"Invalid fit {value!r}; expected 'fit_width' or 'fit_page'"
    raise ValueError(msg)


def _validate_optional_bool(value: object, field_name: str) -> bool | None:
    if value is None:
        return None
    if not isinstance(value, bool):
        msg = f"'{field_name}' must be a bool or None, got {type(value).__name__}"
        raise TypeError(msg)
    return value


def _validate_bool(value: object, field_name: str) -> bool:
    if not isinstance(value, bool):
        msg = f"'{field_name}' must be a bool, got {type(value).__name__}"
        raise TypeError(msg)
    return value


def _header_footer_to_dict(hf: HeaderFooter) -> HeaderFooterDict:
    out: HeaderFooterDict = {}
    if hf.left is not None:
        out["left"] = hf.left
    if hf.center is not None:
        out["center"] = hf.center
    if hf.right is not None:
        out["right"] = hf.right
    return out


def _coerce_header_footer(
    value: object,
    section: str,
    *,
    scope: _PrintScope,
) -> HeaderFooter | Literal["none"] | None:
    if value is None:
        return None
    if value == "none":
        if scope == "sheet":
            return "none"
        msg = f"Model-level print '{section}' cannot be 'none'"
        raise ValueError(msg)
    if isinstance(value, HeaderFooter):
        return value
    if isinstance(value, Mapping):
        mapping = cast(Mapping[object, object], value)
        unknown = sorted(str(k) for k in mapping if k not in _ALLOWED_HEADER_FOOTER_KEYS)
        if unknown:
            msg = f"Unknown print {section} key(s): {', '.join(unknown)}"
            raise ValueError(msg)
        return HeaderFooter(
            left=_validate_optional_str(mapping.get("left"), f"print {section} 'left'"),
            center=_validate_optional_str(mapping.get("center"), f"print {section} 'center'"),
            right=_validate_optional_str(mapping.get("right"), f"print {section} 'right'"),
        )
    if isinstance(value, str):
        expected = (
            "HeaderFooter, mapping, or 'none'" if scope == "sheet" else "HeaderFooter or mapping"
        )
        msg = f"Invalid print {section} value {value!r}; expected {expected}"
        raise ValueError(msg)
    msg = f"Print {section} must be a HeaderFooter or mapping, got {type(value).__name__}"
    raise TypeError(msg)


@dataclass(frozen=True)
class HeaderFooter:
    """Structured 3-zone worksheet print header or footer."""

    left: str | None = None
    center: str | None = None
    right: str | None = None

    def __post_init__(self) -> None:
        _validate_optional_str(self.left, "HeaderFooter.left")
        _validate_optional_str(self.center, "HeaderFooter.center")
        _validate_optional_str(self.right, "HeaderFooter.right")
        if self.left is None and self.center is None and self.right is None:
            raise ValueError(
                "HeaderFooter must specify at least one of 'left', 'center', or 'right'"
            )


@dataclass(frozen=True, init=False)
class PrintSetup:
    """Worksheet print settings (also used for per-sheet cascade overrides)."""

    paper_size: PaperSize | None = None
    orientation: PageOrientation | None = None
    fit: PrintFitMode | None = None
    center_horizontally: bool | None = None
    header: HeaderFooter | Literal["none"] | None = None
    footer: HeaderFooter | Literal["none"] | None = None

    def __init__(
        self,
        paper_size: PaperSize | None = None,
        orientation: PageOrientation | None = None,
        fit: PrintFitMode | None = None,
        center_horizontally: bool | None = None,
        header: HeaderFooter | HeaderFooterDict | Literal["none"] | None = None,
        footer: HeaderFooter | HeaderFooterDict | Literal["none"] | None = None,
    ) -> None:
        object.__setattr__(self, "paper_size", _validate_optional_paper_size(paper_size))
        object.__setattr__(self, "orientation", _validate_optional_orientation(orientation))
        object.__setattr__(self, "fit", _validate_optional_fit(fit))
        object.__setattr__(
            self,
            "center_horizontally",
            _validate_optional_bool(center_horizontally, "center_horizontally"),
        )
        object.__setattr__(self, "header", _coerce_header_footer(header, "header", scope="sheet"))
        object.__setattr__(self, "footer", _coerce_header_footer(footer, "footer", scope="sheet"))


def _clone_header_footer_dict(hf: HeaderFooterDict) -> HeaderFooterDict:
    cloned: HeaderFooterDict = {}
    if "left" in hf:
        cloned["left"] = hf["left"]
    if "center" in hf:
        cloned["center"] = hf["center"]
    if "right" in hf:
        cloned["right"] = hf["right"]
    return cloned


def _clone_print_dict(print_spec: SheetPrintDict | None) -> SheetPrintDict | None:
    if print_spec is None:
        return None
    cloned: SheetPrintDict = {"paper_size": print_spec["paper_size"]}
    if "orientation" in print_spec:
        cloned["orientation"] = print_spec["orientation"]
    if "fit" in print_spec:
        cloned["fit"] = print_spec["fit"]
    if "center_horizontally" in print_spec:
        cloned["center_horizontally"] = print_spec["center_horizontally"]
    if "header" in print_spec:
        cloned["header"] = _clone_header_footer_dict(print_spec["header"])
    if "footer" in print_spec:
        cloned["footer"] = _clone_header_footer_dict(print_spec["footer"])
    return cloned


def _parse_print_setup(value: object, *, scope: _PrintScope) -> PrintSetup:
    if isinstance(value, PrintSetup):
        _coerce_header_footer(value.header, "header", scope=scope)
        _coerce_header_footer(value.footer, "footer", scope=scope)
        return value
    if isinstance(value, Mapping):
        mapping = cast(Mapping[object, object], value)
        unknown = sorted(str(k) for k in mapping if k not in _ALLOWED_PRINT_KEYS)
        if unknown:
            msg = f"Unknown print configuration key(s): {', '.join(unknown)}"
            raise ValueError(msg)
        return PrintSetup(
            paper_size=cast(PaperSize | None, mapping.get("paper_size")),
            orientation=cast(PageOrientation | None, mapping.get("orientation")),
            fit=cast(PrintFitMode | None, mapping.get("fit")),
            center_horizontally=cast(bool | None, mapping.get("center_horizontally")),
            header=_coerce_header_footer(mapping.get("header"), "header", scope=scope),
            footer=_coerce_header_footer(mapping.get("footer"), "footer", scope=scope),
        )
    expected = "PrintSetup, mapping, or 'none'" if scope == "sheet" else "PrintSetup or mapping"
    if isinstance(value, str):
        msg = f"Invalid print configuration {value!r}; expected {expected}"
        raise ValueError(msg)
    msg = f"Print configuration must be a {expected}, got {type(value).__name__}"
    raise TypeError(msg)


def _resolve_header_footer_section(
    base_section: HeaderFooterDict | None,
    override: HeaderFooter | Literal["none"] | None,
) -> HeaderFooterDict | None:
    if override == "none":
        return None
    if override is not None:
        return _header_footer_to_dict(override)
    return _clone_header_footer_dict(base_section) if base_section is not None else None


def _resolve_print(
    base: SheetPrintDict | None,
    override: object,
    *,
    scope: _PrintScope,
) -> SheetPrintDict | None:
    if override is None:
        return _clone_print_dict(base)
    if scope == "sheet" and override == "none":
        return None
    parsed = _parse_print_setup(override, scope=scope)

    paper_size = (
        parsed.paper_size
        if parsed.paper_size is not None
        else (base["paper_size"] if base is not None else None)
    )
    if paper_size is None:
        label = "Model" if scope == "model" else "Sheet"
        raise ValueError(f"{label} print configuration requires 'paper_size'")

    resolved: SheetPrintDict = {"paper_size": paper_size}

    orientation = (
        parsed.orientation
        if parsed.orientation is not None
        else (base.get("orientation") if base is not None else None)
    )
    if orientation is not None:
        resolved["orientation"] = orientation

    fit = parsed.fit if parsed.fit is not None else (base.get("fit") if base is not None else None)
    if fit is not None:
        resolved["fit"] = fit

    center = (
        parsed.center_horizontally
        if parsed.center_horizontally is not None
        else (base.get("center_horizontally") if base is not None else None)
    )
    if center is not None:
        resolved["center_horizontally"] = center

    header = _resolve_header_footer_section(
        base.get("header") if base is not None else None, parsed.header
    )
    if header is not None:
        resolved["header"] = header

    footer = _resolve_header_footer_section(
        base.get("footer") if base is not None else None, parsed.footer
    )
    if footer is not None:
        resolved["footer"] = footer

    return resolved


@dataclass(frozen=True, init=False)
class Band:
    """Horizontal band of side-by-side column lanes on a worksheet."""

    lanes: tuple[Lane, ...]
    gap: int | None = None
    lane_gap: int | None = None
    page_break_before: bool = False

    def __init__(
        self,
        lanes: Sequence[LaneItem],
        *,
        gap: int | None = None,
        lane_gap: int | None = None,
        page_break_before: bool = False,
    ) -> None:
        raw_items = _as_object_tuple(
            lanes,
            forbidden=(str, bytes, Table, Lane),
            label="Band 'lanes' must be a sequence of Lane, Table, or str",
        )
        if not raw_items:
            raise ValueError("Band 'lanes' cannot be empty")
        validated_lanes: list[Lane] = []
        for item in raw_items:
            if isinstance(item, Lane):
                validated_lanes.append(item)
            elif isinstance(item, (Table, str)):
                validated_lanes.append(Lane([item]))
            else:
                msg = f"Band lane item must be a Lane, Table, or str, got {type(item).__name__}"
                raise TypeError(msg)
        lane_tuple = tuple(validated_lanes)
        validated_gap = _validate_non_negative_int(gap, "gap")
        validated_lane_gap = _validate_non_negative_int(lane_gap, "lane_gap")
        if len(lane_tuple) == 1 and validated_lane_gap is not None:
            msg = f"Band with a single lane cannot specify 'lane_gap' ({validated_lane_gap})"
            raise ValueError(msg)
        validated_page_break = _validate_bool(page_break_before, "page_break_before")
        object.__setattr__(self, "lanes", lane_tuple)
        object.__setattr__(self, "gap", validated_gap)
        object.__setattr__(self, "lane_gap", validated_lane_gap)
        object.__setattr__(self, "page_break_before", validated_page_break)


def _validate_sheet_layout(
    layout: object,
    col_width: object,
) -> tuple[SheetLayoutMode, float | None]:
    if layout == "auto":
        validated_layout: SheetLayoutMode = "auto"
    elif layout == "grid":
        validated_layout = "grid"
    else:
        msg = f"Invalid sheet layout '{layout}'. Expected 'auto' or 'grid'."
        raise ValueError(msg)

    if col_width is None:
        return validated_layout, None

    if isinstance(col_width, bool) or not isinstance(col_width, (int, float)):
        msg = f"col_width must be a finite number, got {type(col_width).__name__}"
        raise TypeError(msg)
    if validated_layout == "auto":
        msg = "col_width can only be specified when layout='grid'"
        raise ValueError(msg)
    if (
        (isinstance(col_width, float) and not math.isfinite(col_width))
        or col_width < 1.0
        or col_width > 255.0
    ):
        msg = f"col_width must be a finite number in [1.0, 255.0], got {col_width!r}"
        raise ValueError(msg)
    return validated_layout, float(col_width)


class Sheet:
    """Fluent container for worksheets containing financial tables."""

    def __init__(
        self,
        name: str,
        tab_color: str | None = None,
        show_gridlines: bool = False,
        freeze_panes: tuple[int, int] | None = None,
        model: Model | None = None,
        *,
        layout: SheetLayoutMode = "auto",
        col_width: float | None = None,
        print: PrintSetup | SheetPrintOverrideDict | Literal["none"] | None = None,
    ) -> None:
        if freeze_panes is not None:
            r, c = freeze_panes
            if r < 0 or c < 0:
                msg = (
                    "freeze_panes must be a tuple of two non-negative integers (row, col), "
                    f"got {freeze_panes!r}"
                )
                raise ValueError(msg)

        validated_layout, validated_col_width = _validate_sheet_layout(layout, col_width)
        base_print = model.print if model is not None else None
        resolved_print = _resolve_print(base_print, print, scope="sheet")

        self.name = name
        self.tab_color = tab_color
        self.show_gridlines = show_gridlines
        self.freeze_panes = freeze_panes
        self.layout: SheetLayoutMode = validated_layout
        self.col_width: float | None = validated_col_width
        self.print: SheetPrintDict | None = resolved_print
        self._model = model
        self._tables: list[Table] = []
        self._table_ids: set[str] = set()
        self._tables_by_id: dict[str, Table] = {}
        self._bands: list[Band] | None = None

    def add_table(
        self,
        id: str,
        title: str,
        subtitle: str | Sequence[str] | None = None,
        currency: CurrencyCode | None = None,
        collapsible: bool = False,
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
        )
        self._tables.append(table)
        self._table_ids.add(id)
        self._tables_by_id[id] = table
        return table

    def _resolve_table_ref(self, ref: TableRef) -> str:
        if isinstance(ref, Table):
            if self._tables_by_id.get(ref.id) is not ref:
                msg = f"Table '{ref.id}' does not belong to sheet '{self.name}'"
                raise ValueError(msg)
            return ref.id
        if ref not in self._tables_by_id:
            msg = f"Table '{ref}' does not belong to sheet '{self.name}'"
            raise ValueError(msg)
        return ref

    def set_bands(self, bands: Sequence[BandItem]) -> Sheet:
        """Sets the explicit 2D band/lane layout for all tables currently on this worksheet."""
        raw_items = _as_object_tuple(
            bands,
            forbidden=(str, bytes, Table, Band),
            label="Sheet.set_bands expects a sequence of Band, Table, or str items",
        )

        normalized_bands: list[Band] = []
        seen_ids: set[str] = set()

        for band_idx, item in enumerate(raw_items):
            if isinstance(item, Band):
                if band_idx == 0 and item.gap is not None:
                    msg = (
                        f"Sheet '{self.name}': band 0 cannot specify 'gap' ({item.gap}) "
                        "because no preceding band exists"
                    )
                    raise ValueError(msg)
                if band_idx == 0 and item.page_break_before:
                    msg = (
                        f"Sheet '{self.name}': band 0 cannot specify 'page_break_before=True' "
                        "because no preceding band exists"
                    )
                    raise ValueError(msg)
                norm_lanes: list[Lane] = []
                for lane in item.lanes:
                    lane_refs = lane.__f8_table_refs__()
                    lane_ids: list[str] = []
                    for ref in lane_refs:
                        tid = self._resolve_table_ref(ref)
                        if tid in seen_ids:
                            msg = f"Duplicate table '{tid}' in bands for sheet '{self.name}'"
                            raise ValueError(msg)
                        seen_ids.add(tid)
                        lane_ids.append(tid)
                    norm_lanes.append(Lane(lane_ids, table_gap=lane.table_gap))
                normalized_bands.append(
                    Band(
                        norm_lanes,
                        gap=item.gap,
                        lane_gap=item.lane_gap,
                        page_break_before=item.page_break_before,
                    )
                )
            elif isinstance(item, (Table, str)):
                tid = self._resolve_table_ref(item)
                if tid in seen_ids:
                    msg = f"Duplicate table '{tid}' in bands for sheet '{self.name}'"
                    raise ValueError(msg)
                seen_ids.add(tid)
                normalized_bands.append(Band([Lane([tid])]))
            else:
                msg = f"Band item must be a Band, Table, or str, got {type(item).__name__}"
                raise TypeError(msg)

        missing = [t.id for t in self._tables if t.id not in seen_ids]
        if missing:
            msg = f"Sheet '{self.name}': set_bands is missing table(s): {', '.join(missing)}"
            raise ValueError(msg)

        self._bands = normalized_bands
        return self

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

    @property
    def bands(self) -> list[Band]:
        """Normalized list of bands defining this worksheet's 2D layout."""
        if self._bands is not None:
            return list(self._bands)
        return [Band([Lane([table.id])]) for table in self._tables]

    def __f8_spec__(self) -> SheetSpecDict:
        """Serializes the worksheet into a canonical `SheetSpecDict` payload."""
        if self._bands is not None:
            band_table_ids = {
                tid for band in self._bands for lane in band.lanes for tid in lane.tables
            }
            missing = [t.id for t in self._tables if t.id not in band_table_ids]
            if missing:
                msg = (
                    f"Sheet '{self.name}': table(s) added after set_bands are missing from "
                    f"bands: {', '.join(missing)}"
                )
                raise ValueError(msg)
            resolved_bands = self._bands
        else:
            resolved_bands = [Band([Lane([table.id])]) for table in self._tables]

        bands_spec: list[BandSpecDict] = []
        for band in resolved_bands:
            lanes_spec: list[LaneSpecDict] = []
            for lane in band.lanes:
                lane_dict: LaneSpecDict = {"tables": list(lane.tables)}
                if lane.table_gap is not None:
                    lane_dict["table_gap"] = lane.table_gap
                lanes_spec.append(lane_dict)
            band_dict: BandSpecDict = {"lanes": lanes_spec}
            if band.gap is not None:
                band_dict["gap"] = band.gap
            if band.lane_gap is not None:
                band_dict["lane_gap"] = band.lane_gap
            if band.page_break_before:
                band_dict["page_break_before"] = True
            bands_spec.append(band_dict)

        spec: SheetSpecDict = {
            "name": self.name,
            "show_gridlines": self.show_gridlines,
            "bands": bands_spec,
        }
        if self.tab_color is not None:
            spec["tab_color"] = self.tab_color
        if self.freeze_panes is not None:
            spec["freeze_panes"] = {
                "row": self.freeze_panes[0],
                "col": self.freeze_panes[1],
            }
        if self.layout == "grid":
            layout_spec: GridSheetLayoutSpecDict = {"kind": "grid"}
            if self.col_width is not None:
                layout_spec["col_width"] = self.col_width
            spec["layout"] = layout_spec
        if self.print is not None:
            cloned_print = _clone_print_dict(self.print)
            if cloned_print is not None:
                spec["print"] = cloned_print
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

    def __init__(
        self,
        name: str = "Model",
        theme: ThemeInput = "corporate_navy",
        *,
        print: PrintSetup | SheetPrintDict | None = None,
    ) -> None:
        self.name = name
        self.theme: Mapping[str, object] | str = _resolve_theme_input(theme)
        self.print: SheetPrintDict | None = _resolve_print(None, print, scope="model")
        self._sheets: list[Sheet] = []
        self._sheet_names: set[str] = set()

    def add_sheet(
        self,
        name: str,
        tab_color: str | None = None,
        show_gridlines: bool = False,
        freeze_panes: tuple[int, int] | None = None,
        *,
        layout: SheetLayoutMode = "auto",
        col_width: float | None = None,
        print: PrintSetup | SheetPrintOverrideDict | Literal["none"] | None = None,
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
            layout=layout,
            col_width=col_width,
            print=print,
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


__all__ = [
    "AxisSelector",
    "Band",
    "BandItem",
    "Cell",
    "CellInputValue",
    "HeaderFooter",
    "Lane",
    "LaneItem",
    "Model",
    "PrintSetup",
    "Sheet",
    "Table",
    "TableRef",
]
