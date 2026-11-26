"""Tests for the pure Python modeling DSL and builder classes."""

from __future__ import annotations

import io
import json
import re
import zipfile
from pathlib import Path
from typing import cast

import f8
import pytest
from f8 import (
    AxisSelector,
    Band,
    BandItem,
    Cell,
    CellInputValue,
    CompiledModel,
    HeaderFooter,
    HeaderFooterDict,
    Lane,
    LaneItem,
    Model,
    PageOrientation,
    PaperSize,
    PrintFitMode,
    PrintSetup,
    Shade,
    Sheet,
    SheetLayoutMode,
    SheetPrintDict,
    SheetPrintOverrideDict,
    SpecificationError,
    Table,
    TableRef,
)


def test_scenario_1_end_to_end_model_construction_and_compilation() -> None:
    model = Model(name="ThreeStatement", theme="corporate_navy")
    sheet = model.add_sheet("Income Statement", show_gridlines=True)
    table = sheet.add_table(
        "is_table",
        title="Income Statement",
        subtitle="Audited Historicals & Projections",
        currency="USD",
    )
    table.add_column("line_item", role="descriptor", header="Line Item")
    table.add_column("2023A", role="measure", header="2023A", format="currency", decimals=0)
    table.add_column("2024E", role="measure", header="2024E", format="currency", decimals=0)

    table.add_section_header("Revenue & Gross Profit", note="Audited")
    table.add_row(
        "rev",
        cells={"line_item": "Total Revenue", "2023A": 1000.0},
        expr='is_table["rev", col.prev] * 1.10',
        format="currency",
    )
    table.add_row(
        "cogs",
        cells={"line_item": Cell("Cost of Goods Sold", indent=1), "2023A": 400.0},
        expr='is_table["cogs", col.prev] * 1.05',
        format="currency",
    )
    table.add_row(
        "gp",
        cells={"line_item": Cell("Gross Profit", style="subtotal")},
        expr='is_table["rev", @col] - is_table["cogs", @col]',
        style="subtotal",
        format="currency",
    )

    compiled = model.compile()
    assert isinstance(compiled, CompiledModel)
    summary = compiled.summary()
    assert summary["model_name"] == "ThreeStatement"
    assert summary["total_tables"] == 1
    assert summary["total_cells"] == 9

    # Verify evaluated values
    rev_2024 = compiled.inspect_cell("is_table", "rev", "2024E")
    assert rev_2024["evaluated_value"] == {"Number": 1100.0}
    cogs_2024 = compiled.inspect_cell("is_table", "cogs", "2024E")
    assert cogs_2024["evaluated_value"] == {"Number": 420.0}
    gp_2023 = compiled.inspect_cell("is_table", "gp", "2023A")
    assert gp_2023["evaluated_value"] == {"Number": 600.0}
    gp_2024 = compiled.inspect_cell("is_table", "gp", "2024E")
    assert gp_2024["evaluated_value"] == {"Number": 680.0}


def test_scenario_2_save_via_model_compiled(tmp_path: Path) -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Table 1")
    table.add_column("c1", header="Col 1")
    table.add_row("r1", cells={"c1": 100.0})

    compiled = model.compile()
    target = tmp_path / "from_model" / "out.xlsx"
    compiled.save(target)
    assert target.exists()
    assert target.read_bytes().startswith(b"PK\x03\x04")


def test_scenario_3_explicit_0d_cell_indentation_via_cell_and_dict() -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Table 1")
    table.add_column("line_item", role="descriptor", header="Line Item")
    table.add_column("v", role="measure", header="Value")

    table.add_row(
        "r1",
        cells={
            "line_item": Cell("Cost of Goods Sold", indent=1),
            "v": 100.0,
        },
    )
    table.add_row(
        "r2",
        cells={
            "line_item": {"value": "Operating Expenses", "indent": 2},
            "v": 50.0,
        },
    )

    _manifest, tables = model.__f8_spec__()
    assert tables[0] == {
        "id": "T1",
        "title": "Table 1",
        "collapsible": False,
        "columns": [
            {
                "kind": "data",
                "id": "line_item",
                "role": "descriptor",
                "headers": ["Line Item"],
            },
            {
                "kind": "data",
                "id": "v",
                "role": "measure",
                "headers": ["Value"],
            },
        ],
        "rows": [
            {
                "kind": "data",
                "id": "r1",
                "cells": {
                    "line_item": {"value": "Cost of Goods Sold", "indent": 1},
                    "v": 100.0,
                },
            },
            {
                "kind": "data",
                "id": "r2",
                "cells": {
                    "line_item": {"value": "Operating Expenses", "indent": 2},
                    "v": 50.0,
                },
            },
        ],
    }

    compiled = model.compile()
    assert compiled.summary()["total_cells"] == 4


def test_scenario_4_symmetrical_column_and_row_spacing_gutters() -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Table 1")

    table.add_column("c1", header="Col 1")
    table.add_column_gutter()
    table.add_column("c2", header="Col 2")

    table.add_row("r1", cells={"c1": 10.0, "c2": 20.0})
    table.add_row_gutter()
    table.add_row("r2", cells={"c1": 30.0, "c2": 40.0})

    _manifest, tables = model.__f8_spec__()
    assert tables[0] == {
        "id": "T1",
        "title": "Table 1",
        "collapsible": False,
        "columns": [
            {"kind": "data", "id": "c1", "role": "measure", "headers": ["Col 1"]},
            {"kind": "gutter"},
            {"kind": "data", "id": "c2", "role": "measure", "headers": ["Col 2"]},
        ],
        "rows": [
            {"kind": "data", "id": "r1", "cells": {"c1": 10.0, "c2": 20.0}},
            {"kind": "gutter"},
            {"kind": "data", "id": "r2", "cells": {"c1": 30.0, "c2": 40.0}},
        ],
    }


def test_scenario_5_point_cell_override_via_set_cell() -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Table 1")
    table.add_column("c1", header="Col 1")
    table.add_column("c2", header="Col 2")
    table.add_column("c3", header="Col 3")

    table.add_row("rev", cells={"c1": 100.0}, expr="PREV * 1.05")
    table.add_row("existing", cells={"c1": Cell(50.0, format="currency")})

    # Point override on row with vector formula
    table.set_cell(row="rev", col="c1", value=150.0, note="Audited", indent=0)
    # Updating an existing cell preserving previous value and format
    table.set_cell(row="existing", col="c1", note="Updated note")
    # Passing pre-constructed Cell
    table.set_cell(row="rev", col="c2", cell=Cell(expr="PREV * 1.10"))
    # Positional None value override
    table.set_cell("rev", "c3", None)

    _manifest, tables = model.__f8_spec__()
    assert tables[0] == {
        "id": "T1",
        "title": "Table 1",
        "collapsible": False,
        "columns": [
            {"kind": "data", "id": "c1", "role": "measure", "headers": ["Col 1"]},
            {"kind": "data", "id": "c2", "role": "measure", "headers": ["Col 2"]},
            {"kind": "data", "id": "c3", "role": "measure", "headers": ["Col 3"]},
        ],
        "rows": [
            {
                "kind": "data",
                "id": "rev",
                "expr": "PREV * 1.05",
                "cells": {
                    "c1": {"value": 150.0, "indent": 0, "note": "Audited"},
                    "c2": {"expr": "PREV * 1.10"},
                    "c3": None,
                },
            },
            {
                "kind": "data",
                "id": "existing",
                "cells": {
                    "c1": {"value": 50.0, "format": "currency", "note": "Updated note"},
                },
            },
        ],
    }

    # Non-existent row or column raises KeyError
    with pytest.raises(KeyError, match="Row 'missing' does not exist"):
        table.set_cell(row="missing", col="c1", value=10.0)
    with pytest.raises(KeyError, match="Column 'missing' does not exist"):
        table.set_cell(row="rev", col="missing", value=10.0)

    # Creating a new cell without value or expr raises ValueError
    with pytest.raises(ValueError, match="either 'value' or 'expr'"):
        table.set_cell(row="existing", col="c2", note="Invalid")


def test_scenario_6_header_specification_validation() -> None:
    table = Table("T1", title="Table 1")
    with pytest.raises(ValueError, match="both 'header' and 'headers'"):
        table.add_column("rev", header="Revenue", headers=["Financials", "Revenue"])

    for invalid_group in ("", "   "):
        with pytest.raises(ValueError, match="Column 'group' cannot be empty"):
            table.add_column("rev", group=invalid_group, header="Revenue")

    table.add_column("mrq", group="Historical", headers=["MRQ", "2012-03-31"])
    table.add_column("standalone", headers=["Standalone", "at Close", "2012-12-31"])
    spec = table.__f8_spec__()
    assert spec.get("columns") == [
        {
            "kind": "data",
            "id": "mrq",
            "role": "measure",
            "group": "Historical",
            "headers": ["MRQ", "2012-03-31"],
        },
        {
            "kind": "data",
            "id": "standalone",
            "role": "measure",
            "headers": ["Standalone", "at Close", "2012-12-31"],
        },
    ]


def test_scenario_7_duplicate_identifier_validation() -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1")
    with pytest.raises(ValueError, match="Sheet 'Sheet1' already exists"):
        model.add_sheet("Sheet1")

    table = sheet.add_table("T1", title="Table 1")
    with pytest.raises(ValueError, match="Table 'T1' already exists"):
        sheet.add_table("T1", title="Duplicate Table")

    sheet2 = model.add_sheet("Sheet2")
    with pytest.raises(ValueError, match="Table 'T1' already exists in model"):
        sheet2.add_table("T1", title="Cross-sheet duplicate")

    table.add_column("c1", header="Col 1")
    with pytest.raises(ValueError, match="Column 'c1' already exists"):
        table.add_column("c1", header="Col 1 duplicate")

    table.add_row("r1", cells={"c1": 1.0})
    with pytest.raises(ValueError, match="Row 'r1' already exists"):
        table.add_row("r1", cells={"c1": 2.0})


def test_scenario_8_global_and_sheet_scoped_lookups() -> None:
    model = Model()
    s1 = model.add_sheet("Financials")
    s2 = model.add_sheet("Operations")
    t1 = s1.add_table("is_table", title="Income Statement")
    t2 = s2.add_table("headcount", title="Headcount")

    assert model.get_sheet("Financials") is s1
    assert model.get_sheet("Operations") is s2
    assert model.get_table("is_table") is t1
    assert model.get_table("headcount") is t2
    assert s1.get_table("is_table") is t1
    assert s2.get_table("headcount") is t2

    with pytest.raises(KeyError, match="Sheet 'unknown' not found"):
        model.get_sheet("unknown")

    with pytest.raises(KeyError, match="Table 'unknown' not found"):
        model.get_table("unknown")

    with pytest.raises(KeyError, match="Table 'headcount' not found in sheet 'Financials'"):
        s1.get_table("headcount")


def test_scenario_9_cell_payload_validation_and_sentinel_nulls() -> None:
    # Value cell
    c1 = Cell(100.0)
    assert c1.value == 100.0
    assert c1.expr is None

    # Formula cell
    c2 = Cell(expr="PREV * 1.05")
    assert c2.expr == "PREV * 1.05"

    # Explicit null value
    c3 = Cell(value=None)
    assert c3.value is None

    # Cell with presentation metadata
    c4 = Cell(
        42.0,
        style="total",
        highlight=True,
        format="currency",
        decimals=2,
        currency="USD",
        indent=1,
        note="Source: audit",
    )
    assert c4.value == 42.0
    assert c4.style == "total"
    assert c4.highlight is True
    assert c4.format == "currency"
    assert c4.decimals == 2
    assert c4.currency == "USD"
    assert c4.indent == 1
    assert c4.note == "Source: audit"

    # Both value and expr raises ValueError
    with pytest.raises(ValueError, match="both"):
        Cell(100.0, expr="A + B")

    # Neither value nor expr raises ValueError
    with pytest.raises(ValueError, match="either"):
        Cell()


def test_scenario_10_worksheet_freeze_panes_split_coordinates() -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1", freeze_panes=(4, 2))
    table = sheet.add_table("T1", title="Table 1")
    table.add_column("c1", header="Col 1")
    table.add_row("r1", cells={"c1": 10.0})

    manifest, _ = model.__f8_spec__()
    assert manifest == {
        "model_name": "Model",
        "theme": "corporate_navy",
        "sheets": [
            {
                "name": "Sheet1",
                "show_gridlines": False,
                "freeze_panes": {"row": 4, "col": 2},
                "bands": [{"lanes": [{"tables": ["T1"]}]}],
            }
        ],
    }

    # Invalid freeze_panes via model
    with pytest.raises(ValueError, match="freeze_panes"):
        model.add_sheet("InvalidSheet", freeze_panes=(-1, 2))

    # Invalid freeze_panes via direct Sheet construction
    with pytest.raises(ValueError, match="freeze_panes"):
        Sheet("DirectInvalid", freeze_panes=(2, -1))


def test_currency_normalization() -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Table 1", currency="USD")
    table.add_column("c1", header="Col 1", currency="EUR")
    table.add_column("c2", header="Col 2")
    table.add_row(
        "r1",
        cells={
            "c1": Cell(100.0, currency="GBP"),
            "c2": {"value": 50.0, "currency": "CAD"},
        },
        currency="JPY",
    )

    _manifest, tables = model.__f8_spec__()
    assert tables[0] == {
        "id": "T1",
        "title": "Table 1",
        "currency": "usd",
        "collapsible": False,
        "columns": [
            {
                "kind": "data",
                "id": "c1",
                "role": "measure",
                "headers": ["Col 1"],
                "currency": "eur",
            },
            {
                "kind": "data",
                "id": "c2",
                "role": "measure",
                "headers": ["Col 2"],
            },
        ],
        "rows": [
            {
                "kind": "data",
                "id": "r1",
                "currency": "jpy",
                "cells": {
                    "c1": {"value": 100.0, "currency": "gbp"},
                    "c2": {"value": 50.0, "currency": "cad"},
                },
            }
        ],
    }


def test_cell_input_value_runtime_evaluation() -> None:
    from f8.builder import CellInputValue as BuilderCellInputValue
    from f8.types import CellInputValue as TypesCellInputValue

    assert CellInputValue is BuilderCellInputValue
    assert TypesCellInputValue is BuilderCellInputValue
    assert BuilderCellInputValue.__value__ is not None


def test_custom_theme_dict_on_model_init() -> None:
    custom_theme: dict[str, object] = {
        "name": "firm_custom",
        "typography": {
            "font_family": "Calibri",
            "title_size": 16.0,
            "subtitle_size": 10.0,
            "super_header_size": 11.0,
            "col_header_size": 11.0,
            "section_header_size": 11.0,
            "data_size": 11.0,
        },
        "metrics": {
            "gutter_col_width": 4.0,
            "char_width_multiplier": 1.2,
            "col_padding": 2.5,
            "min_col_width": 14.0,
        },
        "palette": {
            "table_title_fill": "#2B4C7E",
            "table_title_text": "#FFFFFF",
            "table_subtitle_text": "#555555",
            "super_header_fill": "#3B5C8E",
            "super_header_text": "#FFFFFF",
            "col_header_fill": "#EBF1F5",
            "col_header_text": "#2B4C7E",
            "section_header_fill": "#F5F8FA",
            "section_header_text": "#2B4C7E",
            "data_fill": "#FFFFFF",
            "data_text": "#222222",
            "subtotal_fill": "#F5F8FA",
            "subtotal_text": "#2B4C7E",
            "total_fill": "#EBF1F5",
            "total_text": "#2B4C7E",
            "input_text": "#002060",
            "formula_text": "#000000",
            "shade_muted_fill": "#F8FAFC",
            "shade_alt_fill": "#F1F5F9",
            "highlight_fill": "#FEF3C7",
            "highlight_text": "#92400E",
            "gridline_color": "#E5E7EB",
            "border_color": "#D1D5DB",
            "total_border_color": "#2B4C7E",
        },
        "borders": {
            "table_title_border_bottom": "none",
            "super_header_border_bottom": "thin",
            "header_border_bottom": "thin",
            "descriptor_border_right": "none",
            "subtotal_border_top": "thin",
            "total_border_top": "thin",
            "total_border_bottom": "double",
            "col_subtotal_border_left": "thin",
            "col_total_border_left": "thin",
            "col_total_border_right": "double",
        },
        "number_formats": {
            "currency": '{sym}#,##0{dec}_);({sym}#,##0{dec});"-"_)',
            "number": '#,##0{dec}_);(#,##0{dec});"-"_)',
            "percentage": "0{dec}%",
            "multiple": '0{dec}"x"',
            "basis_points": '#,##0{dec}" bps"',
            "date": "yyyy-mm-dd",
            "text": "@",
            "defaults": {
                "currency_decimals": 2,
                "number_decimals": 2,
                "percentage_decimals": 2,
                "multiple_decimals": 2,
                "basis_points_decimals": 0,
            },
        },
    }

    model = Model(name="CustomThemeModel", theme=custom_theme)
    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Theme Table")
    table.add_column("c1", header="C1", format="currency")
    table.add_row("r1", cells={"c1": 100.0})

    manifest, _ = model.__f8_spec__()
    assert manifest["theme"] == custom_theme

    compiled = model.compile()
    assert compiled.summary()["theme_name"] == "firm_custom"
    assert compiled.formatted("T1", "r1", "c1") == "$100.00"


def test_custom_theme_path_on_model_init(tmp_path: Path) -> None:
    workspace_theme: dict[str, object] = {
        "name": "corporate_navy",
        "typography": {
            "font_family": "Aptos",
            "title_size": 10.0,
            "subtitle_size": 9.0,
            "super_header_size": 10.0,
            "col_header_size": 10.0,
            "section_header_size": 10.0,
            "data_size": 10.0,
            "section_header_underline": True,
        },
        "metrics": {
            "gutter_col_width": 3.0,
            "char_width_multiplier": 1.0,
            "col_padding": 1.0,
            "min_col_width": 16.0,
        },
        "palette": {
            "table_title_fill": "#177",
            "table_title_text": "#FFFFFF",
            "table_subtitle_text": "#45556C",
            "super_header_fill": "#E2E8F0",
            "super_header_text": "#000000",
            "col_header_fill": "none",
            "col_header_text": "#000000",
            "section_header_fill": "none",
            "section_header_text": "#000000",
            "data_fill": "none",
            "data_text": "#000000",
            "subtotal_fill": "none",
            "subtotal_text": "#000000",
            "total_fill": "none",
            "total_text": "#000000",
            "input_text": "#0000FF",
            "formula_text": "#000000",
            "shade_muted_fill": "#F8FAFC",
            "shade_alt_fill": "#F1F5F9",
            "highlight_fill": "#FEF3C7",
            "highlight_text": "#000000",
            "gridline_color": "#E2E8F0",
            "border_color": "#000000",
            "total_border_color": "#1B365D",
        },
        "borders": {
            "table_title_border_bottom": "none",
            "super_header_border_bottom": "none",
            "header_border_bottom": "thin",
            "descriptor_border_right": "none",
            "subtotal_border_top": "thin",
            "total_border_top": "thin",
            "total_border_bottom": "double",
            "col_subtotal_border_left": "thin",
            "col_total_border_left": "thin",
            "col_total_border_right": "double",
        },
        "number_formats": {
            "currency": '{sym}#,##0{dec}_);({sym}#,##0{dec});"-"_)',
            "number": '#,##0{dec}_);(#,##0{dec});"-"_)',
            "percentage": '0{dec}%_);(0{dec}%);"-"_)',
            "multiple": '0{dec}"x"',
            "basis_points": '#,##0{dec}" bps"',
            "date": "yyyy-mm-dd",
            "text": "@",
            "integer": '0;(-0);"-"_)',
            "defaults": {
                "currency_decimals": 2,
                "number_decimals": 0,
                "percentage_decimals": 1,
                "multiple_decimals": 1,
                "basis_points_decimals": 0,
            },
        },
    }
    theme_path = tmp_path / "theme.json"
    theme_path.write_text(json.dumps(workspace_theme), encoding="utf-8")

    model = Model(name="WorkspaceThemeModel", theme=theme_path)
    assert isinstance(model.theme, dict)
    assert model.theme == workspace_theme

    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Theme Table")
    table.add_column("c1", header="C1", format="currency")
    table.add_row("r1", cells={"c1": 100.0})

    manifest, _ = model.__f8_spec__()
    assert manifest["theme"] == workspace_theme

    compiled = model.compile()
    assert compiled.summary()["theme_name"] == "corporate_navy"
    assert compiled.formatted("T1", "r1", "c1") == "$100.00"
    assert compiled.inspect_sheet("Sheet1")["col_widths"][1] == 16.0

    invalid_theme_path = tmp_path / "invalid_theme.json"
    invalid_theme_path.write_text("[1, 2, 3]", encoding="utf-8")
    with pytest.raises(ValueError, match="must contain a JSON object at root"):
        Model(name="InvalidTheme", theme=invalid_theme_path)


def test_scenario_11_semantic_background_shade_dsl_and_conflict() -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Shaded Table")
    table.add_column("label", role="descriptor", header="Item")
    table.add_column("2023a", header="2023A", shade="muted")
    table.add_column("stub", header="Stub", shade="alt")
    table.add_column("2024e", header="2024E")

    table.add_row(
        "rev",
        cells={"label": "Revenue", "2023a": 100.0, "stub": 25.0, "2024e": 120.0},
    )
    table.add_row(
        "kpi",
        shade="alt",
        cells={
            "label": "KPI Band",
            "2023a": Cell(10.0, shade="none"),
            "stub": {"value": 12.0, "shade": "muted"},
            "2024e": 15.0,
        },
    )
    table.set_cell("rev", "2024e", shade="muted")

    _manifest, tables = model.__f8_spec__()
    assert tables[0] == {
        "id": "T1",
        "title": "Shaded Table",
        "collapsible": False,
        "columns": [
            {
                "kind": "data",
                "id": "label",
                "role": "descriptor",
                "headers": ["Item"],
            },
            {
                "kind": "data",
                "id": "2023a",
                "role": "measure",
                "headers": ["2023A"],
                "shade": "muted",
            },
            {
                "kind": "data",
                "id": "stub",
                "role": "measure",
                "headers": ["Stub"],
                "shade": "alt",
            },
            {
                "kind": "data",
                "id": "2024e",
                "role": "measure",
                "headers": ["2024E"],
            },
        ],
        "rows": [
            {
                "kind": "data",
                "id": "rev",
                "cells": {
                    "label": "Revenue",
                    "2023a": 100.0,
                    "stub": 25.0,
                    "2024e": {"value": 120.0, "shade": "muted"},
                },
            },
            {
                "kind": "data",
                "id": "kpi",
                "shade": "alt",
                "cells": {
                    "label": "KPI Band",
                    "2023a": {"value": 10.0, "shade": "none"},
                    "stub": {"value": 12.0, "shade": "muted"},
                    "2024e": 15.0,
                },
            },
        ],
    }

    compiled = model.compile()
    table_insp = compiled.inspect_table("T1")
    assert table_insp["columns"][0]["shade"] is None
    assert table_insp["columns"][1]["shade"] == "muted"
    assert table_insp["columns"][2]["shade"] == "alt"
    assert table_insp["rows"][0]["shade"] is None
    assert table_insp["rows"][1]["shade"] == "alt"

    assert compiled.inspect_cell("T1", "rev", "2023a")["shade"] == "muted"
    assert compiled.inspect_cell("T1", "rev", "stub")["shade"] == "alt"
    assert compiled.inspect_cell("T1", "rev", "2024e")["shade"] == "muted"
    assert compiled.inspect_cell("T1", "kpi", "label")["shade"] == "alt"
    assert compiled.inspect_cell("T1", "kpi", "2023a")["shade"] is None
    assert compiled.inspect_cell("T1", "kpi", "stub")["shade"] == "muted"
    assert compiled.inspect_cell("T1", "kpi", "2024e")["shade"] == "alt"

    # Symmetrical vector shade conflict raises SpecificationError
    conflict_model = Model()
    cs = conflict_model.add_sheet("S")
    ct = cs.add_table("Conflict", title="Conflict")
    shade_val: Shade = "alt"
    ct.add_column("c1", header="C1", shade=shade_val)
    ct.add_row("r1", shade="muted", cells={"c1": 1.0})
    with pytest.raises(SpecificationError, match="shade conflict") as exc_info:
        conflict_model.compile()
    assert exc_info.value.table == "Conflict"

    # Rejection of removed "accent" shade in column, row, or cell
    bad_model = Model()
    bs = bad_model.add_sheet("BadSheet")
    bt = bs.add_table("BadTable", title="Bad")
    bt.add_column("c1", header="C1", shade=cast(Shade, "accent"))
    bt.add_row("r1", cells={"c1": 1.0})
    with pytest.raises(SpecificationError, match="accent"):
        bad_model.compile()


def test_scenario_12_band_lane_dsl_validation_and_layout_scenarios() -> None:
    # Public exports
    for export_name in ("Band", "BandItem", "Lane", "LaneItem", "TableRef"):
        assert export_name in f8.__all__
    assert BandItem is not None
    assert LaneItem is not None
    assert TableRef is not None

    def make_card(sheet: Sheet, tid: str, title: str, cols: int, rows: int) -> Table:
        t = sheet.add_table(tid, title=title)
        for c in range(1, cols + 1):
            t.add_column(f"c{c}", header=f"C{c}")
        for r in range(1, rows + 1):
            cells: dict[str, CellInputValue] = {
                f"c{c}": float(r * 10 + c) for c in range(1, cols + 1)
            }
            t.add_row(f"r{r}", cells=cells)
        return t

    # Scenario 1: Default vertical stacking without set_bands
    m_default = Model(name="DefaultStack")
    s_default = m_default.add_sheet("Main")
    make_card(s_default, "T1", "First", 2, 2)
    make_card(s_default, "T2", "Second", 2, 1)
    assert s_default.bands == [Band([Lane(["T1"])]), Band([Lane(["T2"])])]
    m_def_spec, t_def_spec = m_default.__f8_spec__()
    assert m_def_spec["sheets"][0].get("bands") == [
        {"lanes": [{"tables": ["T1"]}]},
        {"lanes": [{"tables": ["T2"]}]},
    ]
    assert "placement" not in t_def_spec[0]
    c_default = m_default.compile()
    assert c_default.table("T1").address("r1", "c1") == "B5"
    assert c_default.table("T2").address("r1", "c1") == "B12"

    # Scenario 2: Wider lower-left card in multi-lane band (auto mode)
    m_wide = Model(name="WiderLowerLeft")
    s_wide = m_wide.add_sheet("Dashboard")
    tl = make_card(s_wide, "TL", "Top Left", 3, 2)
    bl = make_card(s_wide, "BL", "Bottom Left", 5, 3)
    tr = make_card(s_wide, "TR", "Top Right", 2, 1)
    br = make_card(s_wide, "BR", "Bottom Right", 2, 4)
    bot = make_card(s_wide, "Bottom", "Band 2", 4, 1)
    assert s_wide.set_bands([Band([Lane([tl, bl]), Lane([tr, br])]), bot]) is s_wide
    c_wide = m_wide.compile()
    # TL occupies B..D, BL occupies B..F, Lane 1 (TR, BR) starts at H (col 7)
    assert c_wide.table("TL").address("r1", "c1") == "B5"
    assert c_wide.table("BL").address("r1", "c5") == "F12"
    assert c_wide.table("TR").address("r1", "c1") == "H5"
    assert c_wide.table("BR").address("r1", "c1") == "H11"
    # BL and BR both end at Excel row 14 -> Bottom starts at row 17 (body r1 at 20)
    assert c_wide.table("Bottom").address("r1", "c1") == "B20"

    # Scenario 3: Custom container gaps (including 0), single-lane multi-table bands, TableRef
    m_gaps = Model(name="CustomGaps")
    s_gaps = m_gaps.add_sheet("Gaps")
    g_a = make_card(s_gaps, "A", "A", 2, 1)
    g_b = make_card(s_gaps, "B", "B", 1, 1)
    g_c = make_card(s_gaps, "C", "C", 1, 1)
    g_d = make_card(s_gaps, "D", "D", 1, 1)
    g_e = make_card(s_gaps, "E", "E", 1, 1)
    g_f = make_card(s_gaps, "F", "F", 1, 1)
    g_g = make_card(s_gaps, "G", "G", 2, 1)
    s_gaps.set_bands(
        [
            Band([Lane([g_a, g_b], table_gap=1), Lane([g_c, g_d], table_gap=4)], lane_gap=0),
            Band([Lane([g_e, g_f], table_gap=1)], gap=0),
            "G",
        ]
    )
    assert g_g.id == "G"
    m_gaps_spec, _ = m_gaps.__f8_spec__()
    assert m_gaps_spec["sheets"][0].get("bands") == [
        {
            "lane_gap": 0,
            "lanes": [
                {"table_gap": 1, "tables": ["A", "B"]},
                {"table_gap": 4, "tables": ["C", "D"]},
            ],
        },
        {"gap": 0, "lanes": [{"table_gap": 1, "tables": ["E", "F"]}]},
        {"lanes": [{"tables": ["G"]}]},
    ]
    c_gaps = m_gaps.compile()
    assert c_gaps.table("C").address("r1", "c1") == "D5"
    assert c_gaps.table("B").address("r1", "c1") == "B10"
    assert c_gaps.table("D").address("r1", "c1") == "D13"
    assert c_gaps.table("E").address("r1", "c1") == "B17"
    assert c_gaps.table("F").address("r1", "c1") == "B22"
    assert c_gaps.table("G").address("r1", "c1") == "B28"

    # Scenario 4: layout="grid" descriptor-span growth and per-lane descriptor alignment
    m_grid = Model(name="GridScenario4")
    s_grid = m_grid.add_sheet("Summary", layout="grid", col_width=12.0)
    gtl = s_grid.add_table("TL", "Top Left")
    gtl.add_column("item", role="descriptor", header="Item")
    gtl.add_column("v1", header="V1")
    gtl.add_row("r1", cells={"item": "Short", "v1": 10.0})

    gbl = s_grid.add_table("BL", "Bottom Left")
    gbl.add_column("item", role="descriptor", header="Item")
    gbl.add_column("v1", header="V1")
    gbl.add_column("v2", header="V2")
    gbl.add_row("r1", cells={"item": "Consolidated Adjusted EBITDA ($)", "v1": 100.0, "v2": 120.0})

    gtr = s_grid.add_table("TR", "Top Right")
    gtr.add_column("k", role="descriptor", header="Key")
    gtr.add_column("v", header="Val")
    gtr.add_row("r1", cells={"k": "IRR", "v": 0.25})

    gbr = s_grid.add_table("BR", "Bottom Right")
    gbr.add_column("k", role="descriptor", header="Key")
    gbr.add_column("v", header="Val")
    gbr.add_row("r1", cells={"k": "Sponsor Gross Multiple", "v": 2.8})

    gtime = s_grid.add_table("Timeline", "Timeline")
    gtime.add_column("item", role="descriptor", header="Item")
    gtime.add_column("y1", header="Y1")
    gtime.add_row("r1", cells={"item": "Revenue", "y1": 50.0})

    s_grid.set_bands([Band([Lane([gtl, gbl]), Lane([gtr, gbr])], lane_gap=2), gtime])
    c_grid = m_grid.compile()
    # Lane 0 aligns descriptor span to 3 (B..D), so v1/y1 land at E
    assert c_grid.table("TL").address("r1", "v1") == "E5"
    assert c_grid.table("BL").address("r1", "v1") == "E11"
    assert c_grid.table("Timeline").address("r1", "y1") == "E17"
    # Lane 1 starts at col 8 (I = 5 + 1 + 2), aligns descriptor to 2 (I..J), v lands at K
    assert c_grid.table("TR").address("r1", "k") == "I5"
    assert c_grid.table("TR").address("r1", "v") == "K5"
    assert c_grid.table("BR").address("r1", "k") == "I11"
    assert c_grid.table("BR").address("r1", "v") == "K11"

    # Scenario 5: Band/Lane reordering decoupled from declaration order
    m_order = Model(name="ReorderModel")
    s_order = m_order.add_sheet("Dash")
    ot1 = s_order.add_table("T1", "T1")
    ot2 = s_order.add_table("T2", "T2")
    ot3 = s_order.add_table("T3", "T3")
    for ot in (ot1, ot2, ot3):
        ot.add_column("c1", header="C1")
        ot.add_row("r1", expr="1 / 0")
    s_order.set_bands([Band([Lane([ot3, ot1]), Lane([ot2])])])
    assert [t.id for t in s_order.tables] == ["T1", "T2", "T3"]
    c_order = m_order.compile()
    assert [t["id"] for t in c_order.summary()["sheets"][0]["tables"]] == ["T3", "T1", "T2"]
    assert [e["coordinate"]["table"] for e in c_order.errors()] == ["T3", "T1", "T2"]
    assert c_order.inspect_sheet("Dash")["tables"] == ["T3", "T1", "T2"]
    assert c_order.table("T3").address("r1", "c1") == "B5"
    assert c_order.table("T1").address("r1", "c1") == "B11"
    assert c_order.table("T2").address("r1", "c1") == "D5"
    md_order = c_order.to_markdown("layout")
    assert md_order.index("`T3`") < md_order.index("`T1`") < md_order.index("`T2`")

    # Scenario 6: Band/lane-aware title banner alignment
    m_banner = Model(name="BannerScenario6")
    s_b1 = m_banner.add_sheet("Dash")
    btl = make_card(s_b1, "TL", "Top Left", 3, 4)
    bbl = make_card(s_b1, "BL", "Bottom Left", 4, 7)
    btr = make_card(s_b1, "TR", "Top Right", 3, 3)
    bwb = make_card(s_b1, "WideBottom", "Wide Bottom", 10, 2)
    s_b1.set_bands([Band([Lane([btl, bbl]), Lane([btr])]), bwb])
    c_banner = m_banner.compile()
    insp_b1 = c_banner.inspect_sheet("Dash")
    row2 = next(r for r in insp_b1["rows"] if r["excel_row"] == 2)
    assert [(c["physical_a1"], c["kind"], c["table_id"]) for c in row2["cells"]] == [
        ("B2", "table_title", "TL"),
        ("C2", "table_title", "TL"),
        ("D2", "table_title", "TL"),
        ("E2", "table_title", "TL"),
        ("F2", "empty", None),
        ("G2", "table_title", "TR"),
        ("H2", "table_title", "TR"),
        ("I2", "table_title", "TR"),
        ("J2", "table_title", "TR"),
        ("K2", "table_title", "TR"),
    ]
    row11 = next(r for r in insp_b1["rows"] if r["excel_row"] == 11)
    assert [
        (c["physical_a1"], c["kind"], c["table_id"])
        for c in row11["cells"]
        if c["kind"] == "table_title"
    ] == [
        ("B11", "table_title", "BL"),
        ("C11", "table_title", "BL"),
        ("D11", "table_title", "BL"),
        ("E11", "table_title", "BL"),
    ]

    # Scenario 8: Outer SDK strict validation, empty sheets, idempotency, and sheet.bands parity
    s_val = Sheet("Val")
    vt1 = s_val.add_table("T1", "T1")
    vt2 = s_val.add_table("T2", "T2")

    # Removed placement kwargs on Sheet.add_table and Table raise TypeError
    from collections.abc import Callable

    add_table_dyn: Callable[..., object] = cast(Callable[..., object], s_val.add_table)
    table_cls_dyn: Callable[..., object] = cast(Callable[..., object], Table)
    bad_right_of: dict[str, object] = {"right_of": "T1"}
    bad_below: dict[str, object] = {"below": "T1"}
    bad_gap: dict[str, object] = {"gap": 2}
    with pytest.raises(TypeError):
        add_table_dyn("X", "X", **bad_right_of)
    with pytest.raises(TypeError):
        table_cls_dyn("X", "X", **bad_below)
    with pytest.raises(TypeError):
        table_cls_dyn("X", "X", **bad_gap)

    # Lane validation
    with pytest.raises(ValueError, match="cannot be empty"):
        Lane([])
    with pytest.raises(TypeError, match="sequence of Table or str"):
        Lane("T1")
    with pytest.raises(TypeError, match="sequence of Table or str"):
        Lane(cast(list[TableRef], vt1))
    with pytest.raises(ValueError, match="single table cannot specify 'table_gap'"):
        Lane([vt1], table_gap=1)
    with pytest.raises(ValueError, match="single table cannot specify 'table_gap'"):
        Lane([vt1], table_gap=0)
    with pytest.raises(ValueError, match="non-negative"):
        Lane([vt1, vt2], table_gap=-1)
    with pytest.raises(TypeError, match="non-negative int"):
        Lane([vt1, vt2], table_gap=True)
    with pytest.raises(TypeError, match="non-negative int"):
        Lane([vt1, vt2], table_gap=cast(int, 1.5))

    # Band validation
    with pytest.raises(ValueError, match="cannot be empty"):
        Band([])
    with pytest.raises(TypeError, match="must be a Lane, Table, or str"):
        Band(cast(list[LaneItem], [[vt1, vt2]]))
    with pytest.raises(TypeError, match="must be a Lane, Table, or str"):
        Band(cast(list[LaneItem], [123]))
    with pytest.raises(TypeError, match="sequence of Lane, Table, or str"):
        Band(cast(list[LaneItem], Lane([vt1])))
    with pytest.raises(TypeError, match="sequence of Lane, Table, or str"):
        Band(cast(list[LaneItem], vt1))
    with pytest.raises(TypeError, match="sequence of Lane, Table, or str"):
        Band("T1")
    with pytest.raises(ValueError, match="single lane cannot specify 'lane_gap'"):
        Band([Lane([vt1])], lane_gap=1)
    with pytest.raises(ValueError, match="single lane cannot specify 'lane_gap'"):
        Band([vt1], lane_gap=1)
    with pytest.raises(ValueError, match="single lane cannot specify 'lane_gap'"):
        Band([Lane([vt1])], lane_gap=0)
    with pytest.raises(ValueError, match="single lane cannot specify 'lane_gap'"):
        Band([vt1], lane_gap=0)
    with pytest.raises(ValueError, match="non-negative"):
        Band([Lane([vt1]), Lane([vt2])], lane_gap=-1)
    with pytest.raises(ValueError, match="non-negative"):
        Band([Lane([vt1])], gap=-1)
    with pytest.raises(TypeError, match="non-negative int"):
        Band([Lane([vt1]), Lane([vt2])], lane_gap=False)
    with pytest.raises(TypeError, match="non-negative int"):
        Band([Lane([vt1])], gap=True)

    # Sheet.set_bands validation
    with pytest.raises(TypeError, match="sequence of Band"):
        s_val.set_bands(cast(list[BandItem], vt1))
    with pytest.raises(TypeError, match="sequence of Band"):
        s_val.set_bands(cast(list[BandItem], "T1"))
    with pytest.raises(TypeError, match="sequence of Band"):
        s_val.set_bands(cast(list[BandItem], Band([Lane([vt1])])))
    with pytest.raises(ValueError, match="band 0 cannot specify 'gap'"):
        s_val.set_bands([Band([Lane([vt1])], gap=0), vt2])
    with pytest.raises(ValueError, match="band 0 cannot specify 'gap'"):
        s_val.set_bands([Band([vt1], gap=0), vt2])
    with pytest.raises(ValueError, match="band 0 cannot specify 'gap'"):
        s_val.set_bands([Band([Lane([vt1])], gap=2), vt2])
    with pytest.raises(ValueError, match="missing table"):
        s_val.set_bands([vt1])
    with pytest.raises(ValueError, match="missing table"):
        s_val.set_bands([])
    with pytest.raises(ValueError, match="Duplicate table"):
        s_val.set_bands([Band([Lane([vt1, vt1]), Lane([vt2])])])
    with pytest.raises(ValueError, match="Duplicate table"):
        s_val.set_bands([Band([vt1, vt1])])
    with pytest.raises(ValueError, match="Duplicate table"):
        s_val.set_bands([vt1, vt2, vt1])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_val.set_bands([vt1, vt2, "Ghost"])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_val.set_bands([Band([vt1, "Ghost"])])

    # Foreign-sheet Table object (both distinct ID and colliding ID on standalone Sheet)
    foreign_sheet = Sheet("Other")
    foreign_diff = foreign_sheet.add_table("Foreign", "Foreign")
    foreign_same_id = foreign_sheet.add_table("T1", "Foreign T1")
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_val.set_bands([vt1, foreign_diff])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_val.set_bands([foreign_same_id, vt2])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_val.set_bands([Band([Lane([foreign_same_id])]), vt2])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_val.set_bands([Band([foreign_same_id, vt2])])

    # Concurrent value-equal Lanes across standalone sheets preserve per-instance Table ownership
    s_own_a = Sheet("OwnA")
    s_own_b = Sheet("OwnB")
    t_own_a = s_own_a.add_table("T", "T on A")
    t_own_b = s_own_b.add_table("T", "T on B")
    lane_a = Lane([t_own_a])
    lane_b = Lane([t_own_b])
    assert lane_a == lane_b
    assert hash(lane_a) == hash(lane_b)
    assert repr(lane_a) == repr(Lane(["T"]))
    s_own_a.set_bands([Band([lane_a])])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_own_a.set_bands([Band([lane_b])])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_own_b.set_bands([Band([lane_a])])
    reused_band = Band([Lane([t_own_a])])
    s_own_a.set_bands([reused_band])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_own_b.set_bands([reused_band])
    reused_shorthand_band = Band([t_own_a])
    assert reused_shorthand_band == reused_band
    assert hash(reused_shorthand_band) == hash(reused_band)
    assert repr(reused_shorthand_band) == repr(Band([Lane(["T"])]))
    s_own_a.set_bands([reused_shorthand_band])
    with pytest.raises(ValueError, match="does not belong to sheet"):
        s_own_b.set_bands([reused_shorthand_band])
    assert s_own_a.bands == [Band([Lane(["T"])])]
    s_own_b.set_bands(s_own_a.bands)
    assert s_own_b.bands == [Band([Lane(["T"])])]

    # Normalized sheet.bands parity (Table vs str vs band/lane shorthand vs omitted) and idempotency
    assert Lane([vt1]) == Lane(["T1"])
    assert Band([vt1, "T2"]) == Band([Lane(["T1"]), Lane(["T2"])])
    assert s_val.bands == [Band([Lane(["T1"])]), Band([Lane(["T2"])])]
    s_val.set_bands([vt1, vt2])
    assert s_val.bands == [Band([Lane(["T1"])]), Band([Lane(["T2"])])]
    s_val.set_bands([Band([Lane(["T1"]), Lane(["T2"])])])
    assert s_val.bands == [Band([Lane(["T1"]), Lane(["T2"])])]
    s_val.set_bands([Band([vt1, "T2"])])
    assert s_val.bands == [Band([Lane(["T1"]), Lane(["T2"])])]
    # Idempotent replacement
    s_val.set_bands([Band([Lane([vt2, vt1])])])
    assert s_val.bands == [Band([Lane(["T2", "T1"])])]

    # Bare TableRef lane shorthand end-to-end compilation (pair, mixed lane, custom gap)
    m_short = Model("LaneShorthand")
    s_short = m_short.add_sheet("Sensitivities")
    s_l1 = make_card(s_short, "L1", "Left 1", 2, 1)
    s_l2 = make_card(s_short, "L2", "Left 2", 2, 1)
    s_r1 = make_card(s_short, "R1", "Right 1", 2, 1)
    s_p1 = make_card(s_short, "P1", "Pair Left", 2, 1)
    make_card(s_short, "P2", "Pair Right", 2, 1)
    s_foot = make_card(s_short, "Foot", "Footer", 3, 1)
    s_short.set_bands(
        [
            Band([Lane([s_l1, s_l2]), s_r1], lane_gap=1),
            Band([s_p1, "P2"], gap=1, lane_gap=2),
            Band([s_foot], gap=0),
        ]
    )
    assert s_short.bands == [
        Band([Lane(["L1", "L2"]), Lane(["R1"])], lane_gap=1),
        Band([Lane(["P1"]), Lane(["P2"])], gap=1, lane_gap=2),
        Band([Lane(["Foot"])], gap=0),
    ]
    c_short = m_short.compile()
    assert c_short.table("L1").address("r1", "c1") == "B5"
    assert c_short.table("L2").address("r1", "c1") == "B11"
    assert c_short.table("R1").address("r1", "c1") == "E5"
    assert c_short.table("P1").address("r1", "c1") == "B16"
    assert c_short.table("P2").address("r1", "c1") == "F16"
    assert c_short.table("Foot").address("r1", "c1") == "B20"

    # Adding a table after set_bands invalidates partition on __f8_spec__ / compile
    m_late = Model("LateTable")
    s_late = m_late.add_sheet("S1")
    lt1 = make_card(s_late, "T1", "T1", 1, 1)
    s_late.set_bands([lt1])
    make_card(s_late, "T2", "T2", 1, 1)
    with pytest.raises(ValueError, match="missing from bands.*T2"):
        s_late.__f8_spec__()
    with pytest.raises(ValueError, match="missing from bands.*T2"):
        m_late.compile()

    # Empty sheet set_bands([]) succeeds
    s_empty = Sheet("Empty")
    assert s_empty.bands == []
    assert s_empty.set_bands([]) is s_empty
    assert s_empty.bands == []
    assert s_empty.__f8_spec__().get("bands") == []


def test_axis_selector_runtime_evaluation() -> None:
    from f8.builder import AxisSelector as BuilderAxisSelector
    from f8.types import AxisSelector as TypesAxisSelector

    assert AxisSelector is BuilderAxisSelector
    assert TypesAxisSelector is BuilderAxisSelector
    assert BuilderAxisSelector.__value__ is not None


def test_set_cells_scenario_1_multi_regime_timeline_row_authoring() -> None:
    model = Model()
    sheet = model.add_sheet("Schedule")
    table = sheet.add_table("T", title="Multi-Regime Schedule")
    table.add_column("line_item", role="descriptor", header="Line Item")
    table.add_column("standalone", role="measure", header="Standalone")
    table.add_column("adj", role="measure", header="Adj.")
    table.add_column("pf", role="measure", header="PF")
    table.add_column_gutter()
    table.add_column("q1", role="measure", header="Q1")
    table.add_column_gutter()
    table.add_column("q2", role="measure", header="Q2")
    table.add_column("fy1", role="measure", header="FY1")
    table.add_column("fy2", role="measure", header="FY2")

    table.add_row(
        "amort",
        cells={"line_item": "Amortization"},
        expr='T["amort", col.prev] * 2',
    )
    table.set_cells("amort", ["standalone", "adj", "pf"], value=None)
    table.set_cells("amort", slice("q1", "q2"), expr="10.0")

    compiled = model.compile()
    assert compiled.value("T", "amort", "standalone") is None
    assert compiled.value("T", "amort", "adj") is None
    assert compiled.value("T", "amort", "pf") is None
    assert compiled.inspect_cell("T", "amort", "standalone")["evaluated_value"] == "Empty"
    assert compiled.inspect_cell("T", "amort", "adj")["evaluated_value"] == "Empty"
    assert compiled.inspect_cell("T", "amort", "pf")["evaluated_value"] == "Empty"
    assert compiled.num("T", "amort", "q1") == pytest.approx(10.0)
    assert compiled.num("T", "amort", "q2") == pytest.approx(10.0)
    assert compiled.num("T", "amort", "fy1") == pytest.approx(20.0)
    assert compiled.num("T", "amort", "fy2") == pytest.approx(40.0)


def test_set_cells_scenario_2_2d_rectangular_block_and_metadata_patching() -> None:
    model = Model()
    sheet = model.add_sheet("Matrix")
    table = sheet.add_table("T1", title="Block Matrix")
    table.add_column("c1", header="C1")
    table.add_column_gutter()
    table.add_column("c2", header="C2")
    table.add_column("c3", header="C3")

    table.add_row("r1", cells={"c3": 1.0})
    table.add_section_header("Section Divider")
    table.add_row_gutter()
    table.add_row("r2", cells={"c3": 2.0})
    table.add_row("r3", cells={"c1": 3.0, "c2": 3.0, "c3": 3.0})

    table.set_cells(slice("r1", "r2"), slice("c1", "c2"), value=5.0)
    table.set_cells(["r1", "r2"], "c2", shade="alt", note="patched")

    _manifest, tables = model.__f8_spec__()
    rows_spec = [r for r in tables[0].get("rows", []) if r.get("kind") == "data"]
    assert rows_spec[0] == {
        "kind": "data",
        "id": "r1",
        "cells": {
            "c1": 5.0,
            "c2": {"value": 5.0, "shade": "alt", "note": "patched"},
            "c3": 1.0,
        },
    }
    assert rows_spec[1] == {
        "kind": "data",
        "id": "r2",
        "cells": {
            "c1": 5.0,
            "c2": {"value": 5.0, "shade": "alt", "note": "patched"},
            "c3": 2.0,
        },
    }
    assert rows_spec[2] == {
        "kind": "data",
        "id": "r3",
        "cells": {"c1": 3.0, "c2": 3.0, "c3": 3.0},
    }

    compiled = model.compile()
    assert compiled.num("T1", "r1", "c1") == pytest.approx(5.0)
    assert compiled.num("T1", "r2", "c1") == pytest.approx(5.0)
    assert compiled.inspect_cell("T1", "r1", "c2")["shade"] == "alt"
    assert compiled.inspect_cell("T1", "r1", "c2")["note"] == "patched"
    assert compiled.inspect_cell("T1", "r2", "c2")["shade"] == "alt"
    assert compiled.inspect_cell("T1", "r2", "c2")["note"] == "patched"


def test_set_cells_scenario_3_selector_validation_and_atomic_failure() -> None:
    table = Table("T1", title="Validation Table")
    table.add_column("c1", header="C1")
    table.add_column("c2", header="C2")
    table.add_row("r1", cells={"c1": 10.0})
    table.add_row("r2", cells={"c1": 20.0, "c2": 30.0})

    # Unknown row or column ID (via str, Iterable, or slice endpoint) raises KeyError
    with pytest.raises(KeyError, match="Row 'missing_row' does not exist"):
        table.set_cells("missing_row", "c1", value=1.0)
    with pytest.raises(KeyError, match="Row 'missing_row' does not exist"):
        table.set_cells(["r1", "missing_row"], "c1", value=1.0)
    with pytest.raises(KeyError, match="Row 'missing_row' does not exist"):
        table.set_cells(slice("missing_row", "r2"), "c1", value=1.0)
    with pytest.raises(KeyError, match="Row 'missing_row' does not exist"):
        table.set_cells(slice("r1", "missing_row"), "c1", value=1.0)

    with pytest.raises(KeyError, match="Column 'missing_col' does not exist"):
        table.set_cells("r1", "missing_col", value=1.0)
    with pytest.raises(KeyError, match="Column 'missing_col' does not exist"):
        table.set_cells("r1", ["c1", "missing_col"], value=1.0)
    with pytest.raises(KeyError, match="Column 'missing_col' does not exist"):
        table.set_cells("r1", slice("missing_col", "c2"), value=1.0)
    with pytest.raises(KeyError, match="Column 'missing_col' does not exist"):
        table.set_cells("r1", slice("c1", "missing_col"), value=1.0)

    # Inverted slice raises ValueError
    with pytest.raises(ValueError, match="Inverted column slice"):
        table.set_cells("r1", slice("c2", "c1"), value=1.0)
    with pytest.raises(ValueError, match="Inverted row slice"):
        table.set_cells(slice("r2", "r1"), "c1", value=1.0)

    # Open-ended slice raises ValueError
    with pytest.raises(ValueError, match="non-None string 'start' and 'stop'"):
        table.set_cells("r1", slice(None, "c2"), value=1.0)
    with pytest.raises(ValueError, match="non-None string 'start' and 'stop'"):
        table.set_cells("r1", slice("c1", None), value=1.0)
    with pytest.raises(ValueError, match="non-None string 'start' and 'stop'"):
        table.set_cells(slice(None, "r2"), "c1", value=1.0)
    with pytest.raises(ValueError, match="non-None string 'start' and 'stop'"):
        table.set_cells(slice("r1", None), "c1", value=1.0)

    # Stepped slice raises ValueError
    with pytest.raises(ValueError, match="cannot specify a step"):
        table.set_cells("r1", slice("c1", "c2", 1), value=1.0)
    with pytest.raises(ValueError, match="cannot specify a step"):
        table.set_cells(slice("r1", "r2", 1), "c1", value=1.0)

    # Empty iterable raises ValueError
    with pytest.raises(ValueError, match="Row selector iterable cannot be empty"):
        table.set_cells([], "c1", value=1.0)
    with pytest.raises(ValueError, match="Column selector iterable cannot be empty"):
        table.set_cells("r1", [], value=1.0)

    # Atomic failure: ("r1", "c1") has 10.0 and ("r1", "c2") is unpopulated
    with pytest.raises(ValueError, match="must specify either 'value' or 'expr'"):
        table.set_cells("r1", ["c1", "c2"], note="missing payload")

    # Verify ("r1", "c1") was left completely unmutated without the note
    spec = table.__f8_spec__()
    rows_spec = [r for r in spec.get("rows", []) if r.get("kind") == "data"]
    assert rows_spec[0] == {
        "kind": "data",
        "id": "r1",
        "cells": {"c1": 10.0},
    }


def test_active_table_selector_dsl() -> None:
    model = Model(name="ActiveTableDSL")
    sheet = model.add_sheet("Model")
    table = sheet.add_table("IncomeStatement", title="Income Statement")
    table.add_column("y1", role="measure", header="Y1")
    table.add_column("y2", role="measure", header="Y2")

    table.add_row("rev", cells={"y1": 100.0, "y2": 200.0})
    table.add_row("cogs", cells={"y1": 40.0, "y2": 70.0})
    table.add_row("gp", expr='@table["rev", @col] - @table["cogs", @col]')

    compiled = model.compile()
    insp = compiled.inspect_cell("IncomeStatement", "gp", "y1")
    assert insp["formula_expr"] == '@table["rev", @col] - @table["cogs", @col]'
    assert insp["evaluated_value"] == {"Number": 60.0}

    insp_y2 = compiled.inspect_cell("IncomeStatement", "gp", "y2")
    assert insp_y2["formula_expr"] == '@table["rev", @col] - @table["cogs", @col]'
    assert insp_y2["evaluated_value"] == {"Number": 130.0}


def test_table_subtitle_normalization_and_validation() -> None:
    model = Model(name="SubtitlesModel")
    sheet = model.add_sheet("Summary")

    t1 = sheet.add_table("T1", title="T1", subtitle="Single")
    t2 = sheet.add_table("T2", title="T2", subtitle=["Line 1", "Line 2"])
    t3 = sheet.add_table("T3", title="T3", subtitle=[])
    t4 = sheet.add_table("T4", title="T4", subtitle=None)

    for t in (t1, t2, t3, t4):
        t.add_column("c1", header="Col 1")
        t.add_row("r1", cells={"c1": 1.0})

    assert t1.subtitles == ["Single"]
    assert t2.subtitles == ["Line 1", "Line 2"]
    assert t3.subtitles == []
    assert t4.subtitles == []

    assert t1.__f8_spec__().get("subtitles") == ["Single"]
    assert t2.__f8_spec__().get("subtitles") == ["Line 1", "Line 2"]
    assert "subtitles" not in t3.__f8_spec__()
    assert "subtitles" not in t4.__f8_spec__()

    compiled = model.compile()
    assert compiled.table("T1").subtitles == ["Single"]
    assert compiled.table("T2").subtitles == ["Line 1", "Line 2"]
    assert compiled.table("T3").subtitles == []
    assert compiled.table("T4").subtitles == []

    sheet_insp = compiled.inspect_sheet("Summary")
    subtitle_cells = [
        cell
        for row in sheet_insp["rows"]
        for cell in row["cells"]
        if cell["table_id"] == "T2" and cell["kind"] == "table_subtitle"
    ]
    assert len(subtitle_cells) == 2
    assert [c["formatted_value"] for c in subtitle_cells] == ["Line 1", "Line 2"]

    # Reject empty or whitespace subtitle lines
    for bad_sub in ("", ["Valid", "   "]):
        bad_model = Model(name="BadSubtitle")
        bad_sheet = bad_model.add_sheet("S1")
        bad_t = bad_sheet.add_table("Bad", title="Bad", subtitle=bad_sub)
        bad_t.add_column("c1", header="Col 1")
        bad_t.add_row("r1", cells={"c1": 1.0})
        with pytest.raises(SpecificationError, match="subtitle must not be empty"):
            bad_model.compile()


def test_detailed_empty_cell_with_metadata_and_merge() -> None:
    model = Model(name="PeerComps")
    sheet = model.add_sheet("Comps")
    table = sheet.add_table("PeerComps", title="Peer Comps")
    table.add_column("company", role="descriptor", header="Company")
    table.add_column("ev_fcf", header="EV / FCF", format="multiple", decimals=1)
    table.add_column("ev_ebitda", header="EV / EBITDA", format="multiple", decimals=1)

    table.add_row("crm", cells={"company": "Salesforce", "ev_fcf": 18.5, "ev_ebitda": 14.0})
    table.add_row(
        "orcl",
        shade="muted",
        highlight=True,
        cells={
            "company": "Oracle",
            "ev_fcf": Cell(
                value=None,
                note="Excluded: negative FCF",
                shade="none",
                highlight=False,
            ),
            "ev_ebitda": None,
        },
    )
    # Merge metadata onto an existing bare None cell via set_cell
    table.set_cell("orcl", "ev_ebitda", note="Merged note on empty cell")

    table.add_row(
        "raw_dict",
        cells={
            "company": "Raw Dict",
            "ev_fcf": {"value": None, "note": "Raw dict null note"},
            "ev_ebitda": {"value": None},
        },
    )
    table.add_row(
        "batch",
        cells={"company": "Batch Null"},
    )
    table.set_cells("batch", ["ev_fcf", "ev_ebitda"], value=None, note="Batch excluded")

    table.add_row(
        "avg",
        cells={"company": "Average"},
        expr='AVERAGE(PeerComps["crm".."batch", @col])',
    )

    compiled = model.compile()

    orcl_fcf = compiled.inspect_cell("PeerComps", "orcl", "ev_fcf")
    assert orcl_fcf["evaluated_value"] == "Empty"
    assert orcl_fcf["note"] == "Excluded: negative FCF"
    assert orcl_fcf["shade"] is None
    assert orcl_fcf["is_highlight"] is False

    orcl_ebitda = compiled.inspect_cell("PeerComps", "orcl", "ev_ebitda")
    assert orcl_ebitda["evaluated_value"] == "Empty"
    assert orcl_ebitda["note"] == "Merged note on empty cell"

    raw_fcf = compiled.inspect_cell("PeerComps", "raw_dict", "ev_fcf")
    assert raw_fcf["evaluated_value"] == "Empty"
    assert raw_fcf["note"] == "Raw dict null note"

    raw_ebitda = compiled.inspect_cell("PeerComps", "raw_dict", "ev_ebitda")
    assert raw_ebitda["evaluated_value"] == "Empty"
    assert raw_ebitda["note"] is None

    batch_fcf = compiled.inspect_cell("PeerComps", "batch", "ev_fcf")
    assert batch_fcf["evaluated_value"] == "Empty"
    assert batch_fcf["note"] == "Batch excluded"

    assert compiled.value("PeerComps", "orcl", "ev_fcf") is None
    assert compiled.num("PeerComps", "avg", "ev_fcf") == 18.5
    assert compiled.num("PeerComps", "avg", "ev_ebitda") == 14.0
    assert compiled.emit_workbook().startswith(b"PK\x03\x04")

    # 0D explicit null with note overrides 1D column and row vector formulas
    override_model = Model(name="VectorOverride")
    override_sheet = override_model.add_sheet("Comps")
    col_t = override_sheet.add_table("ColFormulaComps", title="Column Formula Comps")
    col_t.add_column("company", role="descriptor", header="Company")
    col_t.add_column("ev", header="EV", format="currency", decimals=1)
    col_t.add_column("fcf", header="FCF", format="currency", decimals=1)
    col_t.add_column(
        "ev_fcf",
        header="EV / FCF",
        format="multiple",
        decimals=1,
        expr='ColFormulaComps[@row, "ev"] / ColFormulaComps[@row, "fcf"]',
    )
    col_t.add_row("crm", cells={"company": "Salesforce", "ev": 185.0, "fcf": 10.0})
    col_t.add_row(
        "orcl",
        cells={
            "company": "Oracle",
            "ev": 400.0,
            "fcf": -5.0,
            "ev_fcf": Cell(value=None, note="Excluded: negative FCF"),
        },
    )
    col_t.add_row(
        "avg",
        cells={
            "company": "Average",
            "ev": None,
            "fcf": None,
            "ev_fcf": Cell(expr='AVERAGE(ColFormulaComps["crm".."orcl", @col])'),
        },
    )

    row_t = override_sheet.add_table("RowFormulaOverride", title="Row Formula Override")
    row_t.add_column("c1", header="C1")
    row_t.add_column("c2", header="C2")
    row_t.add_row(
        "r1",
        expr="10 + 5",
        cells={"c1": {"value": None, "note": "Excluded from row formula"}},
    )

    override_compiled = override_model.compile()
    crm_calc = override_compiled.inspect_cell("ColFormulaComps", "crm", "ev_fcf")
    assert crm_calc["evaluated_value"] == {"Number": 18.5}
    assert crm_calc["is_formula"] is True

    orcl_override = override_compiled.inspect_cell("ColFormulaComps", "orcl", "ev_fcf")
    assert orcl_override["evaluated_value"] == "Empty"
    assert orcl_override["is_formula"] is False
    assert orcl_override["formula_expr"] is None
    assert orcl_override["note"] == "Excluded: negative FCF"
    assert override_compiled.num("ColFormulaComps", "avg", "ev_fcf") == 18.5

    row_c1 = override_compiled.inspect_cell("RowFormulaOverride", "r1", "c1")
    assert row_c1["evaluated_value"] == "Empty"
    assert row_c1["is_formula"] is False
    assert row_c1["note"] == "Excluded from row formula"
    assert override_compiled.num("RowFormulaOverride", "r1", "c2") == 15.0

    # Raw dict with both explicit value=None and expr must fail with mutual exclusivity error
    conflict_model = Model(name="ConflictNullExpr")
    conflict_sheet = conflict_model.add_sheet("S1")
    conflict_t = conflict_sheet.add_table("T1", title="T1")
    conflict_t.add_column("c1", header="C1")
    conflict_t.add_row("r1", cells={"c1": {"value": None, "expr": "1 + 1"}})
    with pytest.raises(SpecificationError, match="both 'value' and 'expr'"):
        conflict_model.compile()

    # Raw dict omitting both value and expr must still fail with EmptyCellPayload
    empty_model = Model(name="MissingPayload")
    empty_sheet = empty_model.add_sheet("S1")
    empty_t = empty_sheet.add_table("T1", title="T1")
    empty_t.add_column("c1", header="C1")
    empty_t.add_row("r1", cells={"c1": {"note": "Missing payload"}})
    with pytest.raises(SpecificationError, match="neither 'value' nor 'expr'"):
        empty_model.compile()


def test_aligned_title_banners_cross_sdk_parity() -> None:
    model = Model(name="AlignedBannerParity")
    sheet = model.add_sheet("Summary")

    top = sheet.add_table("TopTable", title="Top Table Summary Banner")
    top.add_column("item", role="descriptor", header="Item")
    top.add_column("v1", header="Val 1")
    top.add_row("r1", cells={"item": "Metric A", "v1": 100.0})

    bottom = sheet.add_table("WideTable", title="Wide Forecast Table")
    bottom.add_column("item", role="descriptor", header="Item")
    bottom.add_column("y1", header="2024E")
    bottom.add_column("y2", header="2025E")
    bottom.add_column("y3", header="2026E")
    bottom.add_row(
        "r1",
        cells={"item": "Revenue", "y1": 10.0, "y2": 20.0, "y3": 30.0},
    )

    compiled = model.compile()
    sheet_insp = compiled.inspect_sheet("Summary")

    assert sheet_insp["min_col"] == 1
    assert sheet_insp["max_col"] == 4
    assert sheet_insp["min_row"] == 1

    # Row 0 (physical row 1, Excel row 2): TopTable title spans all 4 columns (B2..E2)
    top_title_row = sheet_insp["rows"][0]
    assert top_title_row["physical_row"] == 1
    assert top_title_row["excel_row"] == 2
    assert [
        (c["physical_col"], c["physical_a1"], c["kind"], c["table_id"])
        for c in top_title_row["cells"]
    ] == [
        (1, "B2", "table_title", "TopTable"),
        (2, "C2", "table_title", "TopTable"),
        (3, "D2", "table_title", "TopTable"),
        (4, "E2", "table_title", "TopTable"),
    ]
    assert [c["formatted_value"] for c in top_title_row["cells"]] == [
        "Top Table Summary Banner",
        None,
        None,
        None,
    ]

    # TopTable header row (row 4) and data row (row 5) stay strictly on B..C
    top_header_row = sheet_insp["rows"][2]
    assert [(c["physical_a1"], c["kind"], c["table_id"]) for c in top_header_row["cells"]] == [
        ("B4", "column_header", "TopTable"),
        ("C4", "column_header", "TopTable"),
        ("D4", "empty", None),
        ("E4", "empty", None),
    ]
    top_data_row = sheet_insp["rows"][3]
    assert [(c["physical_a1"], c["kind"], c["table_id"]) for c in top_data_row["cells"]] == [
        ("B5", "data", "TopTable"),
        ("C5", "data", "TopTable"),
        ("D5", "empty", None),
        ("E5", "empty", None),
    ]

    # Cell A1 addresses and col_widths remain unaffected by title banner alignment
    assert compiled.table("TopTable").address("r1", "item") == "B5"
    assert compiled.table("TopTable").address("r1", "v1") == "C5"
    assert compiled.table("WideTable").address("r1", "y3") == "E11"
    assert sheet_insp["col_widths"][1] == pytest.approx(12.0)
    assert sheet_insp["col_widths"][2] == pytest.approx(12.0)
    assert sheet_insp["col_widths"][3] == pytest.approx(12.0)
    assert sheet_insp["col_widths"][4] == pytest.approx(12.0)

    # OpenXML workbook emits styled title cells across B2..E2 while keeping headers/data on B..C
    xlsx_bytes = compiled.emit_workbook()
    assert xlsx_bytes.startswith(b"PK\x03\x04")
    with zipfile.ZipFile(io.BytesIO(xlsx_bytes), "r") as zf:
        sheet_xml = zf.read("xl/worksheets/sheet1.xml").decode("utf-8")

    title_styles: list[str] = []
    for ref in ("B2", "C2", "D2", "E2"):
        match = re.search(rf'<c r="{ref}"[^>]*\bs="(\d+)"', sheet_xml)
        assert match is not None, f"Missing aligned title cell {ref} in sheet1.xml"
        title_styles.append(match.group(1))
    assert len(set(title_styles)) == 1
    assert '<c r="D4"' not in sheet_xml
    assert '<c r="E4"' not in sheet_xml
    assert '<c r="D5"' not in sheet_xml
    assert '<c r="E5"' not in sheet_xml


def test_sheet_gridlines_default_false_and_opt_in_true() -> None:
    assert Sheet("Direct").show_gridlines is False
    assert Sheet("DirectVisible", show_gridlines=True).show_gridlines is True

    model = Model(name="GridlineDefaults")
    hidden_sheet = model.add_sheet("DefaultHidden")
    visible_sheet = model.add_sheet("ExplicitVisible", show_gridlines=True)

    assert hidden_sheet.show_gridlines is False
    assert visible_sheet.show_gridlines is True

    for sheet, table_id in ((hidden_sheet, "T1"), (visible_sheet, "T2")):
        tbl = sheet.add_table(table_id, title=table_id)
        tbl.add_column("c1", header="C1")
        tbl.add_row("r1", cells={"c1": 1.0})

    manifest, _ = model.__f8_spec__()
    assert manifest["sheets"] == [
        {
            "name": "DefaultHidden",
            "show_gridlines": False,
            "bands": [{"lanes": [{"tables": ["T1"]}]}],
        },
        {
            "name": "ExplicitVisible",
            "show_gridlines": True,
            "bands": [{"lanes": [{"tables": ["T2"]}]}],
        },
    ]

    xlsx_bytes = model.compile().emit_workbook()
    with zipfile.ZipFile(io.BytesIO(xlsx_bytes), "r") as zf:
        hidden_xml = zf.read("xl/worksheets/sheet1.xml").decode("utf-8")
        visible_xml = zf.read("xl/worksheets/sheet2.xml").decode("utf-8")

    assert 'showGridLines="0"' in hidden_xml
    assert 'showGridLines="0"' not in visible_xml

    # Raw f8.compile() with omitted show_gridlines also defaults to False
    raw_compiled = f8.compile(
        {
            "model_name": "RawDefaults",
            "theme": "corporate_navy",
            "sheets": [{"name": "RawSheet", "bands": [{"lanes": [{"tables": ["T1"]}]}]}],
        },
        [
            {
                "id": "T1",
                "title": "T1",
                "columns": [{"kind": "data", "id": "c1", "headers": ["C1"]}],
                "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 1.0}}],
            }
        ],
    )
    with zipfile.ZipFile(io.BytesIO(raw_compiled.emit_workbook()), "r") as zf:
        raw_xml = zf.read("xl/worksheets/sheet1.xml").decode("utf-8")
    assert 'showGridLines="0"' in raw_xml


def test_sheet_grid_layout_validation_and_diagnostics() -> None:
    # Public export check
    assert "SheetLayoutMode" in f8.__all__
    mode: SheetLayoutMode = "grid"
    assert mode == "grid"

    # Default Sheet layout is "auto" and omits "layout" in serialized spec
    default_sheet = Sheet("AutoDefault")
    assert default_sheet.layout == "auto"
    assert default_sheet.col_width is None
    assert "layout" not in default_sheet.__f8_spec__()

    # Valid grid sheets with and without col_width
    grid_default = Sheet("GridDefault", layout="grid")
    assert grid_default.__f8_spec__().get("layout") == {"kind": "grid"}
    grid_sized = Sheet("GridSized", layout="grid", col_width=14)
    assert grid_sized.col_width == 14.0
    assert grid_sized.__f8_spec__().get("layout") == {"kind": "grid", "col_width": 14.0}

    # Scenario 7: SDK constructor validation
    with pytest.raises(ValueError, match="Invalid sheet layout"):
        Sheet("BadLayout", layout=cast(SheetLayoutMode, "fixed"))
    with pytest.raises(ValueError, match="col_width can only be specified when layout='grid'"):
        Sheet("AutoWithWidth", layout="auto", col_width=12.0)
    with pytest.raises(TypeError, match="col_width must be a finite number"):
        Sheet("BoolWidth", layout="grid", col_width=True)
    with pytest.raises(TypeError, match="col_width must be a finite number"):
        Sheet("BoolWidthAuto", layout="auto", col_width=False)
    for bad_w in (
        0.0,
        0.5,
        -5.0,
        float("nan"),
        float("inf"),
        float("-inf"),
        256.0,
        10**400,
        -(10**400),
    ):
        with pytest.raises(ValueError, match="col_width must be a finite number in"):
            Sheet("BadWidth", layout="grid", col_width=bad_w)

    # Scenario 7: Raw compile rejects unknown fields on layout and out-of-bounds Pass 1 col_width
    minimal_table = {
        "id": "T1",
        "title": "T1",
        "columns": [{"kind": "data", "id": "c1", "headers": ["C1"]}],
        "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 1.0}}],
    }
    for bad_layout in (
        {"kind": "grid", "col_width": 14.0, "extra": 1},
        {"kind": "auto", "col_width": 12.0},
        {"kind": "grid", "col_width": 0.5},
        {"kind": "grid", "col_width": 256.0},
    ):
        with pytest.raises(SpecificationError):
            f8.compile(
                {
                    "model_name": "BadRawLayout",
                    "theme": "corporate_navy",
                    "sheets": [
                        {
                            "name": "S1",
                            "layout": bad_layout,
                            "bands": [{"lanes": [{"tables": ["T1"]}]}],
                        }
                    ],
                },
                [minimal_table],
            )

    # Scenario 5: Static base_w measure header rejection raises SpecificationError
    hdr_model = Model(name="WideHeader")
    hdr_sheet = hdr_model.add_sheet("Dash", layout="grid", col_width=12.0)
    hdr_t = hdr_sheet.add_table("Val", title="Valuation")
    hdr_t.add_column("item", role="descriptor", header="Item")
    hdr_t.add_column("ev", header="Enterprise Value")
    hdr_t.add_row("r1", cells={"item": "Base", "ev": 100.0})
    with pytest.raises(SpecificationError, match="Enterprise Value") as hdr_exc:
        hdr_model.compile()
    assert hdr_exc.value.table == "Val"

    # Scenario 6: Newline in data cell, header, and group banner raises SpecificationError
    nl_cell_model = Model(name="NewlineCell")
    nl_sheet = nl_cell_model.add_sheet("Dash", layout="grid")
    nl_t = nl_sheet.add_table("T1", title="T1")
    nl_t.add_column("item", role="descriptor", header="Item")
    nl_t.add_row("r1", cells={"item": "Line 1\nLine 2"})
    with pytest.raises(SpecificationError, match="multiline cell") as cell_exc:
        nl_cell_model.compile()
    assert cell_exc.value.table == "T1"

    nl_hdr_model = Model(name="NewlineHdr")
    nl_h_sheet = nl_hdr_model.add_sheet("Dash", layout="grid")
    nl_ht = nl_h_sheet.add_table("T2", title="T2")
    nl_ht.add_column("m", header="Line 1\nLine 2")
    nl_ht.add_row("r1", cells={"m": 1.0})
    with pytest.raises(SpecificationError, match="multiline column header") as h_exc:
        nl_hdr_model.compile()
    assert h_exc.value.table == "T2"

    nl_grp_model = Model(name="NewlineGrp")
    nl_g_sheet = nl_grp_model.add_sheet("Dash", layout="grid")
    nl_gt = nl_g_sheet.add_table("T3", title="T3")
    nl_gt.add_column("m", group="Forecast\rYears", header="Y1")
    nl_gt.add_row("r1", cells={"m": 1.0})
    with pytest.raises(SpecificationError, match="multiline column header") as g_exc:
        nl_grp_model.compile()
    assert g_exc.value.table == "T3"


def test_sheet_grid_layout_scenario_10_and_12_e2e_parity() -> None:
    model = Model(name="CrossSdkGridParity")
    dash = model.add_sheet("Dashboard", layout="grid", col_width=14.0)

    summary = dash.add_table("Summary", title="Executive Summary")
    summary.add_column("item", role="descriptor", header="Line Item")
    summary.add_column("y1", header="2024E", format="currency", decimals=1)
    summary.add_column("y2", header="2025E", format="currency", decimals=1)
    summary.add_row(
        "rev",
        cells={
            "item": "Total Consolidated Operating Revenue",  # 36+1=37 -> span 3 at W=14
            "y1": 100.0,
            "y2": 120.0,
        },
    )
    summary.add_row(
        "ebitda",
        style="total",
        cells={"item": "Adjusted EBITDA"},
        expr='Summary["rev", @col] * 0.25',
    )

    kpis = dash.add_table("KPIs", title="Key Metrics")
    kpis.add_column("metric", role="descriptor", header="Metric")
    kpis.add_column("val", header="Value", format="percentage", decimals=1)
    kpis.add_row(
        "margin",
        cells={"metric": "2025E Consolidated Margin"},  # 25+1=26 -> span 2 at W=14
        expr='Summary["ebitda", "y2"] / Summary["rev", "y2"]',
    )
    dash.set_bands([Band([Lane([summary]), Lane([kpis])])])

    notes_sheet = model.add_sheet("Notes")
    notes = notes_sheet.add_table("Commentary", title="Notes")
    notes.add_column("topic", role="descriptor", header="Topic")
    notes.add_column("detail", role="descriptor", header="Detail")
    notes.add_row("n1", cells={"topic": "Basis", "detail": "Line 1\nLine 2"})

    manifest, _ = model.__f8_spec__()
    assert manifest["sheets"][0].get("layout") == {"kind": "grid", "col_width": 14.0}
    assert "layout" not in manifest["sheets"][1]

    compiled = model.compile()
    assert compiled.table("Summary").address("rev", "item") == "B5"
    assert compiled.table("Summary").address("rev", "y1") == "E5"
    assert compiled.table("Summary").address("rev", "y2") == "F5"
    assert compiled.table("Summary").address("ebitda", "y2") == "F6"
    assert compiled.table("KPIs").address("margin", "metric") == "H5"
    assert compiled.table("KPIs").address("margin", "val") == "J5"
    assert compiled.num("KPIs", "margin", "val") == pytest.approx(0.25)

    insp = compiled.inspect_sheet("Dashboard")
    assert insp["min_col"] == 1
    assert insp["max_col"] == 9
    assert insp["col_widths"] == {
        0: 2.0,
        1: 14.0,
        2: 14.0,
        3: 14.0,
        4: 14.0,
        5: 14.0,
        6: 2.0,
        7: 14.0,
        8: 14.0,
        9: 14.0,
    }

    row5 = next(r for r in insp["rows"] if r["excel_row"] == 5)
    assert [
        (c["physical_a1"], c["kind"], c["table_id"], c["is_descriptor"]) for c in row5["cells"]
    ] == [
        ("B5", "data", "Summary", True),
        ("C5", "overflow", "Summary", True),
        ("D5", "overflow", "Summary", True),
        ("E5", "data", "Summary", False),
        ("F5", "data", "Summary", False),
        ("G5", "empty", None, False),
        ("H5", "data", "KPIs", True),
        ("I5", "overflow", "KPIs", True),
        ("J5", "data", "KPIs", False),
    ]
    c5 = row5["cells"][1]
    assert c5["row_id"] == "rev"
    assert c5["col_id"] == "item"
    assert c5["value"] == "Empty"
    assert c5["formatted_value"] is None

    xlsx = compiled.emit_workbook()
    assert xlsx.startswith(b"PK\x03\x04")
    with zipfile.ZipFile(io.BytesIO(xlsx), "r") as zf:
        dash_xml = zf.read("xl/worksheets/sheet1.xml").decode("utf-8")
    assert "<mergeCells" not in dash_xml


def test_sheet_print_settings_sdk_cascade_validation_and_e2e_parity() -> None:
    # Public export checks
    for sym in (
        "HeaderFooter",
        "PrintSetup",
        "PaperSize",
        "PageOrientation",
        "PrintFitMode",
        "HeaderFooterDict",
        "SheetPrintDict",
        "SheetPrintOverrideDict",
    ):
        assert sym in f8.__all__

    # Type alias sanity
    paper: PaperSize = "a4"
    orient: PageOrientation = "landscape"
    fit: PrintFitMode = "fit_width"
    hf_dict: HeaderFooterDict = {"left": "{model}"}
    sp_dict: SheetPrintDict = {"paper_size": paper, "orientation": orient, "fit": fit}
    spo_dict: SheetPrintOverrideDict = {"footer": "none"}
    assert hf_dict["left"] == "{model}"
    assert sp_dict["paper_size"] == "a4"
    assert spo_dict["footer"] == "none"

    # HeaderFooter eager validation
    with pytest.raises(ValueError, match="at least one"):
        HeaderFooter()
    with pytest.raises(TypeError, match="must be a str or None"):
        HeaderFooter(left=cast(str, 123))

    # PrintSetup eager validation & dict mutation isolation
    with pytest.raises(TypeError, match="center_horizontally"):
        PrintSetup(paper_size="a4", center_horizontally=cast(bool, 1))
    with pytest.raises(ValueError, match="Unknown print header key"):
        PrintSetup(paper_size="a4", header=cast(HeaderFooterDict, {"bogus": "x"}))
    with pytest.raises(ValueError, match="Invalid paper_size"):
        PrintSetup(paper_size=cast(PaperSize, "a3"))
    with pytest.raises(ValueError, match="Invalid orientation"):
        PrintSetup(paper_size="a4", orientation=cast(PageOrientation, "diagonal"))
    with pytest.raises(ValueError, match="Invalid fit"):
        PrintSetup(paper_size="a4", fit=cast(PrintFitMode, "fit_height"))

    mutable_hf: HeaderFooterDict = {"left": "Orig"}
    ps_cloned = PrintSetup(paper_size="a4", header=mutable_hf, footer=HeaderFooter(right="R"))
    mutable_hf["left"] = "Mutated"
    assert ps_cloned.header == HeaderFooter(left="Orig")
    assert isinstance(ps_cloned.header, HeaderFooter)
    assert isinstance(ps_cloned.footer, HeaderFooter)
    assert ps_cloned == PrintSetup(
        paper_size="a4",
        header=HeaderFooter(left="Orig"),
        footer={"right": "R"},
    )
    assert hash(ps_cloned) == hash(
        PrintSetup(
            paper_size="a4",
            header=HeaderFooter(left="Orig"),
            footer=HeaderFooter(right="R"),
        )
    )
    with pytest.raises(ValueError, match="at least one"):
        PrintSetup(paper_size="a4", header={})
    with pytest.raises(ValueError, match="at least one"):
        PrintSetup(paper_size="a4", header=cast(HeaderFooterDict, {"left": None}))

    # Model-level eager validation
    with pytest.raises(ValueError, match="paper_size"):
        Model(print=PrintSetup(orientation="portrait"))
    with pytest.raises(ValueError, match="cannot be 'none'"):
        Model(print=PrintSetup(paper_size="a4", header="none"))
    with pytest.raises(ValueError, match="cannot be 'none'"):
        Model(print=cast(SheetPrintDict, {"paper_size": "a4", "footer": "none"}))
    with pytest.raises(ValueError, match="Unknown print configuration key"):
        Model(print=cast(SheetPrintDict, {"paper_size": "a4", "scale": 80}))
    with pytest.raises(ValueError, match="Unknown print footer key"):
        Model(print=cast(SheetPrintDict, {"paper_size": "a4", "footer": {"bad": "x"}}))
    with pytest.raises(ValueError, match="Invalid paper_size"):
        Model(print=cast(SheetPrintDict, {"paper_size": "a3"}))
    with pytest.raises(ValueError, match="expected PrintSetup or mapping"):
        Model(print=cast(SheetPrintDict, "none"))

    # Sheet-level eager validation & no-op "none" opt-outs
    no_print_sheet = Sheet("NoPrint", print="none")
    assert no_print_sheet.print is None
    assert "print" not in no_print_sheet.__f8_spec__()

    noop_clear_hf = Sheet("NoopHF", print={"paper_size": "a4", "header": "none", "footer": "none"})
    assert noop_clear_hf.print == {"paper_size": "a4"}

    with pytest.raises(ValueError, match="paper_size"):
        Sheet("MissingPaper", print={"orientation": "portrait"})
    with pytest.raises(ValueError, match="Unknown print configuration key"):
        Sheet("UnknownKey", print=cast(SheetPrintOverrideDict, {"paper_size": "a4", "extra": 1}))
    with pytest.raises(ValueError, match="Invalid orientation"):
        Sheet(
            "BadOrient",
            print=cast(SheetPrintOverrideDict, {"paper_size": "a4", "orientation": "diagonal"}),
        )
    with pytest.raises(ValueError, match="Invalid fit"):
        Sheet(
            "BadFit",
            print=cast(SheetPrintOverrideDict, {"paper_size": "a4", "fit": "fit_height"}),
        )
    with pytest.raises(ValueError, match="PrintSetup, mapping, or 'none'"):
        Sheet("BadStr", print=cast(SheetPrintOverrideDict, "bogus"))
    with pytest.raises(TypeError, match="center_horizontally"):
        Sheet(
            "BadCenter",
            print=cast(SheetPrintOverrideDict, {"paper_size": "a4", "center_horizontally": "yes"}),
        )

    # Band.page_break_before eager validation
    with pytest.raises(TypeError, match="page_break_before"):
        Band(["T1"], page_break_before=cast(bool, 1))

    band0_sheet = Sheet("Band0Check", print={"paper_size": "a4"})
    band0_t = band0_sheet.add_table("T0", title="T0")
    band0_t.add_column("c1", header="C1")
    band0_t.add_row("r1", cells={"c1": 1.0})
    with pytest.raises(ValueError, match="band 0 cannot specify 'page_break_before=True'"):
        band0_sheet.set_bands([Band([band0_t], page_break_before=True)])

    # Scenario 8: Model print cascade + shallow overrides + wholesale header/footer replacement +
    # "none" opt-out + Band.page_break_before + XML + Layout Markdown + determinism + values_only
    model = Model(
        name="Acme & Co",
        print=PrintSetup(
            paper_size="a4",
            header=HeaderFooter(left="{model}", center="2026 Forecast {{Rev}}", right="{sheet}"),
            footer=HeaderFooter(left="M&A Confidential", right="Page {page} of {pages} ({date})"),
        ),
    )

    # Sheet 1: Inherits model.print (a4 landscape fit_width, header & footer),
    # 2 bands with page_break_before
    s_inherit = model.add_sheet("Summary")
    t1 = s_inherit.add_table("SummaryTop", title="Executive Summary")
    t1.add_column("item", role="descriptor", header="Item")
    t1.add_column("y1", header="2024E", format="currency", decimals=0)
    t1.add_column("y2", header="2025E", format="currency", decimals=0)
    t1.add_row("rev", cells={"item": "Revenue", "y1": 100.0}, expr='SummaryTop["rev", "y1"] * 1.1')
    t2 = s_inherit.add_table("SummaryBot", title="Supporting Detail")
    t2.add_column("item", role="descriptor", header="Item")
    t2.add_column("y1", header="2024E")
    t2.add_row("note", cells={"item": "Line 1\nLine 2", "y1": 50.0})
    s_inherit.set_bands([Band([t1]), Band([t2], page_break_before=True)])
    assert s_inherit.bands[1].page_break_before is True

    # Sheet 2: Overrides fit="fit_page", orientation="portrait", paper_size="letter",
    # center_horizontally=False, header="none", footer="none"
    s_fitpage = model.add_sheet(
        "Dashboard",
        print=PrintSetup(
            paper_size="letter",
            orientation="portrait",
            fit="fit_page",
            center_horizontally=False,
            header="none",
            footer="none",
        ),
    )
    t3 = s_fitpage.add_table("Dash", title="Dashboard")
    t3.add_column("k", role="descriptor", header="KPI")
    t3.add_column("v", header="Value")
    t3.add_row("m", cells={"k": "IRR", "v": 0.25})

    # Sheet 3: Wholesale footer replacement (replaces model footer's left+right with center-only)
    s_replace = model.add_sheet(
        "Appendix",
        print={"footer": {"center": "Appendix Only"}},
    )
    t4 = s_replace.add_table("App", title="Appendix")
    t4.add_column("k", role="descriptor", header="Key")
    t4.add_column("v", header="Value")
    t4.add_row("r1", cells={"k": "Note", "v": 1.0})

    # Sheet 4: Opts out of model.print via print="none"
    s_optout = model.add_sheet("Scratch", print="none")
    t5 = s_optout.add_table("Work", title="Scratch")
    t5.add_column("v", header="Val")
    t5.add_row("r1", cells={"v": 42.0})

    # Sheet 5: Empty sheet inheriting print (Scenario 5)
    model.add_sheet("EmptyPrint")

    manifest, _tables = model.__f8_spec__()
    assert manifest["sheets"][0].get("print") == {
        "paper_size": "a4",
        "header": {"left": "{model}", "center": "2026 Forecast {{Rev}}", "right": "{sheet}"},
        "footer": {"left": "M&A Confidential", "right": "Page {page} of {pages} ({date})"},
    }
    sheet0_bands = manifest["sheets"][0].get("bands")
    assert sheet0_bands is not None
    assert sheet0_bands[1].get("page_break_before") is True
    assert manifest["sheets"][1].get("print") == {
        "paper_size": "letter",
        "orientation": "portrait",
        "fit": "fit_page",
        "center_horizontally": False,
    }
    assert manifest["sheets"][2].get("print") == {
        "paper_size": "a4",
        "header": {"left": "{model}", "center": "2026 Forecast {{Rev}}", "right": "{sheet}"},
        "footer": {"center": "Appendix Only"},
    }
    assert "print" not in manifest["sheets"][3]
    sheet4_print = manifest["sheets"][4].get("print")
    assert sheet4_print is not None
    assert sheet4_print["paper_size"] == "a4"

    compiled = model.compile()

    # Verify Layout Markdown (Scenarios 1, 5, 6)
    layout_md = compiled.to_markdown(detail="layout")
    assert "Print: a4 landscape | Fit: fit_width (1 x auto) | Area: B2:D11" in layout_md
    assert ", page_break):" in layout_md
    assert "Print: letter portrait | Fit: fit_page (1 x 1) | Area: B2:C5" in layout_md
    scratch_section = layout_md.split("## Sheet: Scratch\n")[1].split("## Sheet: ")[0]
    assert "Print:" not in scratch_section
    empty_section = layout_md.split("## Sheet: EmptyPrint\n")[1]
    assert "_(no tables)_" in empty_section
    assert "Print:" not in empty_section

    # Verify OpenXML emission, byte determinism, and values_only parity
    buf1 = compiled.emit_workbook()
    buf2 = compiled.emit_workbook()
    assert buf1 == buf2
    buf_frozen = compiled.emit_workbook({"values_only": True})

    for label, buf in (("live", buf1), ("values_only", buf_frozen)):
        with zipfile.ZipFile(io.BytesIO(buf), "r") as zf:
            wb_xml = zf.read("xl/workbook.xml").decode("utf-8")
            sh1_xml = zf.read("xl/worksheets/sheet1.xml").decode("utf-8")
            sh2_xml = zf.read("xl/worksheets/sheet2.xml").decode("utf-8")
            sh3_xml = zf.read("xl/worksheets/sheet3.xml").decode("utf-8")
            sh4_xml = zf.read("xl/worksheets/sheet4.xml").decode("utf-8")
            sh5_xml = zf.read("xl/worksheets/sheet5.xml").decode("utf-8")

        assert 'name="_xlnm.Print_Area" localSheetId="0">Summary!$B$2:$D$11<' in wb_xml, label
        assert 'name="_xlnm.Print_Area" localSheetId="3"' not in wb_xml, label
        assert 'name="_xlnm.Print_Area" localSheetId="4"' not in wb_xml, label

        # Sheet 1 (inherited a4 landscape fit_width + header/footer + row break)
        assert '<pageSetUpPr fitToPage="1"/>' in sh1_xml
        assert '<printOptions horizontalCentered="1"/>' in sh1_xml
        assert '<pageSetup paperSize="9" fitToHeight="0" orientation="landscape"' in sh1_xml
        assert '<rowBreaks count="1" manualBreakCount="1">' in sh1_xml
        assert '<headerFooter scaleWithDoc="0">' in sh1_xml
        assert "Acme &amp;&amp; Co" in sh1_xml
        assert "2026 Forecast {Rev}" in sh1_xml
        assert "Page &amp;P of &amp;N (&amp;D)" in sh1_xml

        # Sheet 2 (letter portrait fit_page, center_horizontally=False, no header/footer)
        assert '<pageSetUpPr fitToPage="1"/>' in sh2_xml
        assert '<pageSetup paperSize="1" orientation="portrait"' in sh2_xml
        assert "fitToWidth=" not in sh2_xml
        assert "fitToHeight=" not in sh2_xml
        assert "<printOptions" not in sh2_xml
        assert "<headerFooter" not in sh2_xml

        # Sheet 3 (wholesale footer replacement: no M&A Confidential or Page &P)
        assert "Appendix Only" in sh3_xml
        assert "Confidential" not in sh3_xml
        assert "Page &amp;P" not in sh3_xml

        # Sheet 4 (opted out via print="none": zero print elements)
        assert "<pageSetUpPr" not in sh4_xml
        assert "<pageSetup" not in sh4_xml
        assert "<printOptions" not in sh4_xml
        assert "<headerFooter" not in sh4_xml
        assert "<rowBreaks" not in sh4_xml

        # Sheet 5 (empty sheet with print: pageSetup present, no rowBreaks or Print_Area)
        assert '<pageSetup paperSize="9" fitToHeight="0" orientation="landscape"' in sh5_xml
        assert "<rowBreaks" not in sh5_xml

    # Scenario 7: Raw f8.compile Pass 1 rejections
    # (empty header {}, smuggling, page_break rules, unknown fields)
    min_tbl: dict[str, object] = {
        "id": "T1",
        "title": "T1",
        "columns": [{"kind": "data", "id": "c1", "headers": ["C1"]}],
        "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 1.0}}],
    }
    min_tbl2: dict[str, object] = {**min_tbl, "id": "T2"}

    raw_invalid_cases: list[tuple[dict[str, object], list[dict[str, object]]]] = [
        # Empty header {}
        (
            {
                "model_name": "M",
                "sheets": [
                    {
                        "name": "S1",
                        "print": {"paper_size": "a4", "header": {}},
                        "bands": [{"lanes": [{"tables": ["T1"]}]}],
                    }
                ],
            },
            [min_tbl],
        ),
        # page_break_before without print
        (
            {
                "model_name": "M",
                "sheets": [
                    {
                        "name": "S1",
                        "bands": [
                            {"lanes": [{"tables": ["T1"]}]},
                            {"page_break_before": True, "lanes": [{"tables": ["T2"]}]},
                        ],
                    }
                ],
            },
            [min_tbl, min_tbl2],
        ),
        # page_break_before with fit_page
        (
            {
                "model_name": "M",
                "sheets": [
                    {
                        "name": "S1",
                        "print": {"paper_size": "a4", "fit": "fit_page"},
                        "bands": [
                            {"lanes": [{"tables": ["T1"]}]},
                            {"page_break_before": True, "lanes": [{"tables": ["T2"]}]},
                        ],
                    }
                ],
            },
            [min_tbl, min_tbl2],
        ),
        # {model} newline smuggling
        (
            {
                "model_name": "Bad\nName",
                "sheets": [
                    {
                        "name": "S1",
                        "print": {"paper_size": "a4", "header": {"left": "{model}"}},
                        "bands": [{"lanes": [{"tables": ["T1"]}]}],
                    }
                ],
            },
            [min_tbl],
        ),
        # Cross-boundary A&{model} macro smuggling
        (
            {
                "model_name": "[Page] Trick",
                "sheets": [
                    {
                        "name": "S1",
                        "print": {"paper_size": "a4", "header": {"left": "A&{model}"}},
                        "bands": [{"lanes": [{"tables": ["T1"]}]}],
                    }
                ],
            },
            [min_tbl],
        ),
        # Unknown key on print
        (
            {
                "model_name": "M",
                "sheets": [
                    {
                        "name": "S1",
                        "print": {"paper_size": "a4", "bogus": True},
                        "bands": [{"lanes": [{"tables": ["T1"]}]}],
                    }
                ],
            },
            [min_tbl],
        ),
    ]
    for bad_manifest, bad_tables in raw_invalid_cases:
        with pytest.raises(SpecificationError) as exc_info:
            f8.compile(bad_manifest, bad_tables)
        assert exc_info.value.table is None
