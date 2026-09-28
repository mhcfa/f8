"""Table auditing, coordinate inspection, and tabular data export protocols."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Literal, cast, overload

from f8.types import (
    CellScalarValue,
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


class _RenderColumn:
    """Internal helper representing a column prepared for 2D visual rendering."""

    def __init__(self, col_id: str | None, headers: list[str], is_descriptor: bool) -> None:
        self.id = col_id
        self.headers = headers
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
        self._rows: list[str] = [
            rid
            for r in raw.get("rows", [])
            if not r.get("is_gutter", False)
            and not r.get("is_section_header", False)
            and (rid := r.get("id")) is not None
        ]

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

        Guarantees uniform rectangular output across all orientations, mapping unassigned
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
    ) -> str:
        """Renders an aligned GitHub-Flavored Markdown table representation.

        Args:
            view: One of 'values' (pre-formatted accounting typography), 'formulas'
                (lowered Excel formulas), 'expr' (f8 DSL expressions), or 'addresses'
                (physical sheet A1 coordinates).
        """
        valid_views = ("values", "formulas", "expr", "addresses")
        if view not in valid_views:
            raise ValueError(
                f"Invalid view mode '{view}'. "
                "Expected one of 'values', 'formulas', 'expr', 'addresses'."
            )

        non_gutter_cols = [c for c in self._raw.get("columns", []) if not c.get("is_gutter", False)]
        if not non_gutter_cols:
            return ""

        max_tiers = max((len(c.get("headers", [])) for c in non_gutter_cols), default=0)
        max_tiers = max(max_tiers, 1)

        render_cols: list[_RenderColumn] = []
        for c in non_gutter_cols:
            col_id = c.get("id")
            is_descriptor = c.get("is_descriptor", False)
            raw_headers = c.get("headers", [])
            k = len(raw_headers)
            bottom_raw = raw_headers[-1] if k > 0 else (col_id or "")

            if col_id:
                if not bottom_raw:
                    bottom_header = col_id
                elif bottom_raw.lower() == col_id.lower():
                    bottom_header = bottom_raw
                else:
                    bottom_header = f"{bottom_raw} [{col_id}]"
            else:
                bottom_header = bottom_raw

            pad = max_tiers - max(k, 1)
            headers: list[str] = []
            for tier_idx in range(max_tiers):
                if tier_idx < pad:
                    headers.append("")
                elif tier_idx == max_tiers - 1:
                    headers.append(bottom_header)
                else:
                    headers.append(raw_headers[tier_idx - pad])

            render_cols.append(_RenderColumn(col_id, headers, is_descriptor))

        row_indent_map = {
            r["id"]: r.get("indent", 0) for r in self._raw.get("rows", []) if r.get("id")
        }

        def get_cell_text(row_id: str, col: _RenderColumn) -> str:
            if col.id is None:
                return ""
            coord = (row_id, col.id)
            cell_info = self._cell_map.get(coord)
            if cell_info is None:
                return ""

            val_str = cell_info.get("formatted_value") or ""
            if not val_str:
                raw_val = unbox_cell_value(cell_info.get("value"))
                val_str = str(raw_val) if raw_val is not None else ""

            if col.is_descriptor:
                indent = row_indent_map.get(row_id, 0)
                if not val_str:
                    label = row_id
                elif val_str.lower() == row_id.lower():
                    label = val_str
                else:
                    label = f"{val_str} [{row_id}]"
                return f"{'  ' * indent}{label}"

            if view == "values":
                return val_str
            if view == "formulas":
                if cell_info.get("is_formula"):
                    return cell_info.get("excel_formula") or val_str
                return val_str
            if view == "expr":
                if cell_info.get("is_formula"):
                    return cell_info.get("formula_expr") or ""
                return val_str
            if view == "addresses":
                return cell_info.get("physical_a1", "")
            return val_str

        col_widths: list[int] = []
        for col in render_cols:
            max_h_len = max((len(h) for h in col.headers), default=0)
            col_widths.append(max(max_h_len, 4))

        for r in self._raw.get("rows", []):
            if r.get("is_gutter") or r.get("is_section_header"):
                continue
            row_id = r.get("id")
            if not row_id:
                continue
            for col_idx, col in enumerate(render_cols):
                text = get_cell_text(row_id, col)
                if len(text) > col_widths[col_idx]:
                    col_widths[col_idx] = len(text)

        out_lines: list[str] = []

        # Stacked header tiers
        for tier in range(max_tiers):
            cells: list[str] = []
            for col_idx, col in enumerate(render_cols):
                h = col.headers[tier]
                w = col_widths[col_idx]
                cells.append(f" {h:<{w}} " if col.is_descriptor else f" {h:>{w}} ")
            out_lines.append(f"|{'|'.join(cells)}|")

        # Alignment row
        align_cells: list[str] = []
        for col_idx, col in enumerate(render_cols):
            w = col_widths[col_idx]
            align_cells.append(
                f" :{'-' * (w - 1)} " if col.is_descriptor else f" {'-' * (w - 1)}: "
            )
        out_lines.append(f"|{'|'.join(align_cells)}|")

        # Data & section header rows
        for r in self._raw.get("rows", []):
            if r.get("is_gutter"):
                continue
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

        return "\n".join(out_lines) + "\n"
