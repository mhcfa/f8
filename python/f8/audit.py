"""Table auditing, coordinate inspection, and tabular data export protocols."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Literal, cast, overload

from f8.types import (
    CellScalarValue,
    ColumnInspection,
    RowInspection,
    SplitDict,
    TableCellInfo,
    TableInspection,
    TableOrient,
    TableViewMode,
)

_EXCEL_ERROR_MAP: dict[str, str] = {
    "Div0": "#DIV/0!",
    "Value": "#VALUE!",
    "Ref": "#REF!",
    "Num": "#NUM!",
    "Na": "#N/A",
    "Name": "#NAME?",
}


def unbox_cell_value(val: object) -> CellScalarValue:
    """Unboxes a Serde-tagged or raw cell value into an idiomatic Python scalar."""
    if val is None:
        return None
    if isinstance(val, str):
        if val.lower() == "empty":
            return None
        return val
    if isinstance(val, Mapping):
        mapping = cast(Mapping[str, object], val)
        if "Number" in mapping:
            num: object = mapping["Number"]
            if isinstance(num, (int, float)):
                return float(num)
            return None
        if "Text" in mapping:
            text: object = mapping["Text"]
            return str(text) if text is not None else ""
        if "Boolean" in mapping:
            b: object = mapping["Boolean"]
            return bool(b)
        if "Empty" in mapping:
            return None
        if "Error" in mapping:
            err: object = mapping["Error"]
            if isinstance(err, str):
                return _EXCEL_ERROR_MAP.get(err, f"#{err}!")
            return "#ERROR!"
        return None
    if isinstance(val, bool):
        return val
    if isinstance(val, (int, float)):
        return float(val)
    return str(val)


def format_with_unit(formatted: str, unit: str = "") -> str:
    """Applies a unit suffix to a pre-formatted accounting string, splicing inside closing ')'."""
    if not formatted or not unit:
        return formatted
    if formatted.endswith(")"):
        return f"{formatted[:-1]}{unit})"
    return f"{formatted}{unit}"


class MarkdownTable(str):
    """GitHub-Flavored Markdown table string with IPython and Quarto display integration."""

    def _repr_markdown_(self) -> str:
        """Returns the raw GFM string for IPython/Jupyter/Quarto rich display."""
        return str(self)


class _RenderColumn:
    """Internal helper representing a column prepared for 2D visual rendering."""

    def __init__(self, col_id: str | None, header: str, is_descriptor: bool) -> None:
        self.id = col_id
        self.header = header
        self.is_descriptor = is_descriptor


class TableAuditor:
    """High-level inspection, cell lookup, and zero-dependency tabular export wrapper."""

    def __init__(self, raw: TableInspection) -> None:
        self._raw = raw
        self._id: str = raw["table_id"]
        self._title: str = raw["title"]
        self._subtitle: str | None = raw.get("subtitle")
        self._sheet: str = raw["sheet"]

        self._columns: list[str] = [
            cid
            for c in raw.get("columns", [])
            if not c.get("is_gutter", False) and (cid := c.get("id")) is not None
        ]
        self._measure_columns: list[str] = [
            cid
            for c in raw.get("columns", [])
            if not c.get("is_gutter", False)
            and not c.get("is_descriptor", False)
            and (cid := c.get("id")) is not None
        ]
        self._rows: list[str] = [
            rid
            for r in raw.get("rows", [])
            if not r.get("is_gutter", False)
            and not r.get("is_section_header", False)
            and (rid := r.get("id")) is not None
        ]
        self._row_set: set[str] = set(self._rows)

        self._cell_map: dict[tuple[str, str], TableCellInfo] = {}
        prefix = f'{self._id}["'
        suffix = '"]'
        for k, cell_info in raw.get("cells", {}).items():
            if k.startswith(prefix) and k.endswith(suffix):
                inner = k[len(prefix) : -len(suffix)]
                parts = inner.split('", "', 1)
                if len(parts) == 2:
                    self._cell_map[(parts[0], parts[1])] = cell_info

    @property
    def id(self) -> str:
        """Unique table identifier."""
        return self._id

    @property
    def title(self) -> str:
        """Table display title."""
        return self._title

    @property
    def subtitle(self) -> str | None:
        """Optional table subtitle or units label."""
        return self._subtitle

    @property
    def sheet(self) -> str:
        """Parent worksheet identifier."""
        return self._sheet

    @property
    def columns(self) -> list[str]:
        """Ordered list of non-gutter column IDs."""
        return list(self._columns)

    @property
    def measure_columns(self) -> list[str]:
        """Ordered list of non-gutter, non-descriptor measure column IDs."""
        return list(self._measure_columns)

    @property
    def rows(self) -> list[str]:
        """Ordered list of non-gutter, non-section data row IDs."""
        return list(self._rows)

    @property
    def shape(self) -> tuple[int, int]:
        """Dimensions of the calculable data matrix as (row_count, col_count)."""
        return (len(self._rows), len(self._columns))

    def __len__(self) -> int:
        """Number of calculable data rows in the table."""
        return len(self._rows)

    def cell(self, row: str, col: str) -> TableCellInfo:
        """Looks up the raw TableCellInfo metadata for a given coordinate.

        Raises:
            KeyError: If coordinate does not exist in the table.
        """
        coord = (row, col)
        if coord in self._cell_map:
            return self._cell_map[coord]
        canonical_key = f'{self._id}["{row}", "{col}"]'
        if canonical_key in self._raw.get("cells", {}):
            info = self._raw["cells"][canonical_key]
            self._cell_map[coord] = info
            return info
        raise KeyError(f"Coordinate ('{row}', '{col}') not found in table '{self._id}'")

    def value(self, row: str, col: str) -> CellScalarValue:
        """Returns the unboxed scalar value for the coordinate."""
        return unbox_cell_value(self.cell(row, col).get("value"))

    def num(self, row: str, col: str) -> float:
        """Returns the evaluated numeric cell value as a float.

        Raises:
            KeyError: If coordinate does not exist in the table.
            TypeError: If the cell value is not a number (e.g. text, boolean, None, or error).
        """
        val = self.value(row, col)
        if isinstance(val, bool) or not isinstance(val, (int, float)):
            raise TypeError(
                f"Cell ('{row}', '{col}') in table '{self._id}' did not evaluate to a float "
                f"(got {val!r})"
            )
        return float(val)

    def formatted(self, row: str, col: str, *, unit: str = "") -> str:
        """Returns the pre-formatted display string with an optional unit suffix."""
        cell_info = self.cell(row, col)
        val_str = cell_info.get("formatted_value")
        if val_str is None:
            raw_val = unbox_cell_value(cell_info.get("value"))
            val_str = str(raw_val) if raw_val is not None else ""
        return format_with_unit(val_str, unit)

    def row_values(
        self,
        row: str,
        cols: Sequence[str] | None = None,
    ) -> list[CellScalarValue]:
        """Returns 1D list of unboxed scalar values across measure columns (or specified cols)."""
        if row not in self._row_set:
            raise KeyError(f"Row '{row}' not found in table '{self._id}'")
        target_cols = self._measure_columns if cols is None else cols
        return [self.value(row, col) for col in target_cols]

    def row_nums(
        self,
        row: str,
        cols: Sequence[str] | None = None,
    ) -> list[float]:
        """Returns 1D list of numeric float values across measure columns (or specified cols)."""
        if row not in self._row_set:
            raise KeyError(f"Row '{row}' not found in table '{self._id}'")
        target_cols = self._measure_columns if cols is None else cols
        return [self.num(row, col) for col in target_cols]

    def formula(self, row: str, col: str) -> str | None:
        """Returns the lowered Excel formula string, or None if driver/input."""
        return self.cell(row, col).get("excel_formula")

    def expr(self, row: str, col: str) -> str | None:
        """Returns the high-level f8 expression string, or None if driver/input."""
        return self.cell(row, col).get("formula_expr")

    def address(self, row: str, col: str) -> str:
        """Returns the physical Excel sheet address (e.g. 'C18')."""
        return self.cell(row, col).get("physical_a1", "")

    def __getitem__(self, key: tuple[str, str]) -> CellScalarValue:
        """Coordinate indexing via table[row, col] returning unboxed scalar value."""
        key_obj: object = key
        if type(key_obj) is not tuple or len(key) != 2:
            raise TypeError(
                "TableAuditor coordinate indexing requires a (row, col) tuple, "
                "e.g. table['revenue', '2024E']"
            )
        row_obj: object = key[0]
        col_obj: object = key[1]
        if type(row_obj) is not str or type(col_obj) is not str:
            raise TypeError(
                "TableAuditor coordinate indexing requires a (row, col) tuple, "
                "e.g. table['revenue', '2024E']"
            )
        return self.value(key[0], key[1])

    @overload
    def to_dict(
        self, orient: Literal["records"] = "records"
    ) -> list[dict[str, CellScalarValue]]: ...
    @overload
    def to_dict(self, orient: Literal["dict"]) -> dict[str, list[CellScalarValue]]: ...
    @overload
    def to_dict(self, orient: Literal["index"]) -> dict[str, dict[str, CellScalarValue]]: ...
    @overload
    def to_dict(self, orient: Literal["split"]) -> SplitDict: ...
    def to_dict(
        self,
        orient: TableOrient = "records",
    ) -> (
        list[dict[str, CellScalarValue]]
        | dict[str, list[CellScalarValue]]
        | dict[str, dict[str, CellScalarValue]]
        | SplitDict
    ):
        """Exports evaluated tabular data to standard dictionary layouts.

        Guarantees uniform rectangular output across all orientations, mapping empty
        cells to None.
        """
        if orient == "records":
            records: list[dict[str, CellScalarValue]] = []
            for r in self._rows:
                row_dict: dict[str, CellScalarValue] = {}
                for c in self._columns:
                    cell_info = self._cell_map.get((r, c))
                    row_dict[c] = (
                        unbox_cell_value(cell_info.get("value")) if cell_info is not None else None
                    )
                records.append(row_dict)
            return records

        if orient == "dict":
            col_dict: dict[str, list[CellScalarValue]] = {c: [] for c in self._columns}
            for r in self._rows:
                for c in self._columns:
                    cell_info = self._cell_map.get((r, c))
                    val = (
                        unbox_cell_value(cell_info.get("value")) if cell_info is not None else None
                    )
                    col_dict[c].append(val)
            return col_dict

        if orient == "index":
            index_dict: dict[str, dict[str, CellScalarValue]] = {}
            for r in self._rows:
                row_dict: dict[str, CellScalarValue] = {}
                for c in self._columns:
                    cell_info = self._cell_map.get((r, c))
                    row_dict[c] = (
                        unbox_cell_value(cell_info.get("value")) if cell_info is not None else None
                    )
                index_dict[r] = row_dict
            return index_dict

        if orient == "split":
            data: list[list[CellScalarValue]] = []
            for r in self._rows:
                row_data: list[CellScalarValue] = []
                for c in self._columns:
                    cell_info = self._cell_map.get((r, c))
                    row_data.append(
                        unbox_cell_value(cell_info.get("value")) if cell_info is not None else None
                    )
                data.append(row_data)
            return SplitDict(
                columns=list(self._columns),
                index=list(self._rows),
                data=data,
            )

        raise ValueError(
            f"Invalid orient '{orient}'. Expected one of 'records', 'dict', 'index', 'split'."
        )

    def to_markdown(
        self,
        view: TableViewMode = "values",
        *,
        rows: Sequence[str] | None = None,
        cols: Sequence[str] | None = None,
        include_section_headers: bool | None = None,
        show_ids: bool = False,
    ) -> MarkdownTable:
        """Renders an aligned GitHub-Flavored Markdown table representation.

        Args:
            view: One of 'values' (pre-formatted accounting typography), 'formulas'
                (lowered Excel formulas), 'expr' (f8 DSL expressions), or 'addresses'
                (physical sheet A1 coordinates).
            rows: Optional subset of row IDs to include. When specified, section headers
                are omitted by default unless include_section_headers=True.
            cols: Optional subset of column IDs to include. Descriptor columns are
                automatically retained.
            include_section_headers: Whether to render section header divider rows.
                Defaults to True when rows is None, and False when rows is specified.
            show_ids: When True, appends [col_id] and [row_id] suffixes when display labels
                differ from coordinate IDs. Defaults to False.
        """
        valid_views = ("values", "formulas", "expr", "addresses")
        if view not in valid_views:
            raise ValueError(
                f"Invalid view mode '{view}'. "
                "Expected one of 'values', 'formulas', 'expr', 'addresses'."
            )

        non_gutter_cols: list[ColumnInspection] = [
            c for c in self._raw.get("columns", []) if not c.get("is_gutter", False)
        ]
        if not non_gutter_cols:
            return MarkdownTable("")

        col_spec_by_id: dict[str, ColumnInspection] = {
            cid: c for c in non_gutter_cols if (cid := c.get("id")) is not None
        }

        if cols is None:
            selected_cols = non_gutter_cols
        else:
            for cid in cols:
                if cid not in col_spec_by_id:
                    raise KeyError(f"Column '{cid}' not found in table '{self._id}'")
            requested_col_set = set(cols)
            selected_cols = [
                c
                for c in non_gutter_cols
                if c.get("is_descriptor", False) and c.get("id") not in requested_col_set
            ] + [col_spec_by_id[cid] for cid in cols]

        if not selected_cols:
            return MarkdownTable("")

        raw_rows: list[RowInspection] = [
            r for r in self._raw.get("rows", []) if not r.get("is_gutter", False)
        ]
        row_spec_by_id: dict[str, RowInspection] = {
            rid: r
            for r in raw_rows
            if not r.get("is_section_header", False) and (rid := r.get("id")) is not None
        }

        if rows is None:
            show_sections = True if include_section_headers is None else include_section_headers
            selected_rows = [
                r for r in raw_rows if show_sections or not r.get("is_section_header", False)
            ]
        else:
            for rid in rows:
                if rid not in row_spec_by_id:
                    raise KeyError(f"Row '{rid}' not found in table '{self._id}'")
            show_sections = False if include_section_headers is None else include_section_headers
            if not show_sections:
                selected_rows = [row_spec_by_id[rid] for rid in rows]
            else:
                requested_row_set = set(rows)
                selected_rows = [
                    r
                    for r in raw_rows
                    if r.get("is_section_header", False) or r.get("id") in requested_row_set
                ]

        render_cols: list[_RenderColumn] = []
        for c in selected_cols:
            col_id = c.get("id")
            is_descriptor = c.get("is_descriptor", False)
            stacked_header = " ".join(h for h in c.get("headers", []) if h)
            group = c.get("group")
            if group and stacked_header:
                display_header = f"{group}: {stacked_header}"
            elif group:
                display_header = group
            elif stacked_header:
                display_header = stacked_header
            else:
                display_header = col_id or ""

            if show_ids and col_id and display_header.lower() != col_id.lower():
                header_label = f"{display_header} [{col_id}]"
            else:
                header_label = display_header

            render_cols.append(_RenderColumn(col_id, header_label, is_descriptor))

        row_indent_map: dict[str, int] = {
            rid: r.get("indent", 0) for rid, r in row_spec_by_id.items()
        }
        row_summary_map: dict[str, bool] = {
            rid: r.get("style") in ("subtotal", "total") for rid, r in row_spec_by_id.items()
        }

        def get_cell_text(row_id: str, col: _RenderColumn) -> str:
            if col.id is None:
                return ""
            coord = (row_id, col.id)
            cell_info = self._cell_map.get(coord)
            is_summary = row_summary_map.get(row_id, False)

            val_str = (cell_info.get("formatted_value") or "") if cell_info is not None else ""
            if not val_str and cell_info is not None:
                raw_val = unbox_cell_value(cell_info.get("value"))
                val_str = str(raw_val) if raw_val is not None else ""

            if col.is_descriptor:
                indent = row_indent_map.get(row_id, 0)
                if not val_str:
                    label = row_id
                elif show_ids and val_str.lower() != row_id.lower():
                    label = f"{val_str} [{row_id}]"
                else:
                    label = val_str
                if is_summary and label:
                    return f"{'  ' * indent}**{label}**"
                return f"{'  ' * indent}{label}"

            if view == "values":
                raw_text = val_str if val_str else "—"
            elif view == "formulas":
                if cell_info is not None and cell_info.get("is_formula"):
                    raw_text = cell_info.get("excel_formula") or val_str
                else:
                    raw_text = val_str
            elif view == "expr":
                if cell_info is not None and cell_info.get("is_formula"):
                    raw_text = cell_info.get("formula_expr") or ""
                else:
                    raw_text = val_str
            else:
                raw_text = cell_info.get("physical_a1", "") if cell_info is not None else ""

            if is_summary and raw_text:
                return f"**{raw_text}**"
            return raw_text

        col_widths: list[int] = [max(len(col.header), 4) for col in render_cols]

        for r in selected_rows:
            if r.get("is_section_header"):
                continue
            row_id = r.get("id")
            if not row_id:
                continue
            for col_idx, col in enumerate(render_cols):
                text = get_cell_text(row_id, col)
                if len(text) > col_widths[col_idx]:
                    col_widths[col_idx] = len(text)

        out_lines: list[str] = []

        # Single flattened GFM header row
        header_cells: list[str] = []
        for col_idx, col in enumerate(render_cols):
            h = col.header
            w = col_widths[col_idx]
            header_cells.append(f" {h:<{w}} " if col.is_descriptor else f" {h:>{w}} ")
        out_lines.append(f"|{'|'.join(header_cells)}|")

        # Alignment row
        align_cells: list[str] = []
        for col_idx, col in enumerate(render_cols):
            w = col_widths[col_idx]
            align_cells.append(
                f" :{'-' * (w - 1)} " if col.is_descriptor else f" {'-' * (w - 1)}: "
            )
        out_lines.append(f"|{'|'.join(align_cells)}|")

        # Data & section header rows
        for r in selected_rows:
            if r.get("is_section_header"):
                title = r.get("section_title") or ""
                empty_cols = " |" * (len(render_cols) - 1)
                out_lines.append(f"| **{title}** |{empty_cols}")
                continue
            row_id = r.get("id")
            if not row_id:
                continue
            row_cells: list[str] = []
            for col_idx, col in enumerate(render_cols):
                text = get_cell_text(row_id, col)
                w = col_widths[col_idx]
                row_cells.append(f" {text:<{w}} " if col.is_descriptor else f" {text:>{w}} ")
            out_lines.append(f"|{'|'.join(row_cells)}|")

        return MarkdownTable("\n".join(out_lines) + "\n")

    def _repr_markdown_(self) -> str:
        """IPython / Quarto rich Markdown representation hook."""
        return str(self.to_markdown())
