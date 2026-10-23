"""Tests for the pure Python modeling DSL and builder classes."""

from __future__ import annotations

from pathlib import Path

import pytest
from f8 import Cell, CellInputValue, CompiledModel, Model, Shade, Sheet, SpecificationError, Table


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

    # Non-existent row raises KeyError
    with pytest.raises(KeyError, match="Row 'missing' does not exist"):
        table.set_cell(row="missing", col="c1", value=10.0)

    # Creating a new cell without value or expr raises ValueError
    with pytest.raises(ValueError, match="either 'value' or 'expr'"):
        table.set_cell(row="rev", col="c_new", note="Invalid")


def test_scenario_6_header_specification_validation() -> None:
    table = Table("T1", title="Table 1")
    with pytest.raises(ValueError, match="both 'header' and 'headers'"):
        table.add_column("rev", header="Revenue", headers=["Financials", "Revenue"])


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
                "show_gridlines": True,
                "freeze_panes": {"row": 4, "col": 2},
                "tables": ["T1"],
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
            "title_row_height": 26.0,
            "header_row_height": 20.0,
            "section_row_height": 20.0,
            "data_row_height": 16.0,
            "gutter_row_height": 10.0,
            "gutter_col_width": 4.0,
            "title_gutter_row_height": 9.0,
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
            "shade_accent_fill": "#EFF6FF",
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


def test_scenario_11_semantic_background_shade_dsl_and_conflict() -> None:
    model = Model()
    sheet = model.add_sheet("Sheet1")
    table = sheet.add_table("T1", title="Shaded Table")
    table.add_column("label", role="descriptor", header="Item")
    table.add_column("2023a", header="2023A", shade="muted")
    table.add_column("stub", header="Stub", shade="accent")
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
            "stub": {"value": 12.0, "shade": "accent"},
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
                "shade": "accent",
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
                    "stub": {"value": 12.0, "shade": "accent"},
                    "2024e": 15.0,
                },
            },
        ],
    }

    compiled = model.compile()
    table_insp = compiled.inspect_table("T1")
    assert table_insp["columns"][0]["shade"] is None
    assert table_insp["columns"][1]["shade"] == "muted"
    assert table_insp["columns"][2]["shade"] == "accent"
    assert table_insp["rows"][0]["shade"] is None
    assert table_insp["rows"][1]["shade"] == "alt"

    assert compiled.inspect_cell("T1", "rev", "2023a")["shade"] == "muted"
    assert compiled.inspect_cell("T1", "rev", "stub")["shade"] == "accent"
    assert compiled.inspect_cell("T1", "rev", "2024e")["shade"] == "muted"
    assert compiled.inspect_cell("T1", "kpi", "label")["shade"] == "alt"
    assert compiled.inspect_cell("T1", "kpi", "2023a")["shade"] is None
    assert compiled.inspect_cell("T1", "kpi", "stub")["shade"] == "accent"
    assert compiled.inspect_cell("T1", "kpi", "2024e")["shade"] == "alt"

    # Symmetrical vector shade conflict raises SpecificationError
    conflict_model = Model()
    cs = conflict_model.add_sheet("S")
    ct = cs.add_table("Conflict", title="Conflict")
    shade_val: Shade = "accent"
    ct.add_column("c1", header="C1", shade=shade_val)
    ct.add_row("r1", shade="muted", cells={"c1": 1.0})
    with pytest.raises(SpecificationError, match="shade conflict") as exc_info:
        conflict_model.compile()
    assert exc_info.value.table == "Conflict"
