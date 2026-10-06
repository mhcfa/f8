"""Tests for Python project export and direct disk compilation."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from f8 import Cell, CompiledModel, Model, compile_project


def _create_sample_model() -> Model:
    model = Model(name="ProjectTitan", theme="corporate_navy")
    sheet1 = model.add_sheet("Summary", tab_color="#1B365D", freeze_panes=(2, 1))
    table_a = sheet1.add_table("Assumptions", title="Key Assumptions", currency="USD")
    table_a.add_column("param", role="descriptor", header="Parameter")
    table_a.add_column("val", role="measure", header="Value", format="currency", decimals=0)
    table_a.add_row("entry_val", cells={"param": "Entry Valuation", "val": 1500.0})

    table_b = sheet1.add_table("Returns", title="Returns Summary", currency="USD")
    table_b.add_column("metric", role="descriptor", header="Metric")
    table_b.add_column("val", role="measure", header="Multiple", format="multiple", decimals=2)
    table_b.add_row("moic", cells={"metric": "MOIC", "val": 2.50})

    sheet2 = model.add_sheet("Operations", tab_color="#003366")
    table_c = sheet2.add_table("Operations", title="Operating Performance", currency="USD")
    table_c.add_column("line_item", role="descriptor", header="Line Item")
    table_c.add_column("2023A", role="measure", header="2023A", format="currency", decimals=0)
    table_c.add_column("2024E", role="measure", header="2024E", format="currency", decimals=0)
    table_c.add_row(
        "rev",
        cells={"line_item": "Revenue", "2023A": 1000.0},
        expr='Operations["rev", col.prev] * 1.10',
    )
    return model


def test_scenario_1_canonical_multi_file_json_export(tmp_path: Path) -> None:
    model = _create_sample_model()
    export_dir = tmp_path / "exported_project"

    model.export_project(export_dir)

    manifest_file = export_dir / "f8.json"
    tables_dir = export_dir / "tables"
    assert manifest_file.is_file()
    assert tables_dir.is_dir()

    manifest_content = manifest_file.read_text(encoding="utf-8")
    assert manifest_content.startswith('{\n  "model_name": "ProjectTitan",')
    manifest_data = json.loads(manifest_content)
    assert manifest_data["model_name"] == "ProjectTitan"
    assert manifest_data["theme"] == "corporate_navy"
    assert len(manifest_data["sheets"]) == 2
    assert manifest_data["sheets"][0]["tables"] == ["Assumptions", "Returns"]
    assert manifest_data["sheets"][1]["tables"] == ["Operations"]
    assert manifest_data["sheets"][0]["freeze_panes"] == {"row": 2, "col": 1}

    table_files = sorted([p.name for p in tables_dir.glob("*.json")])
    assert table_files == ["Assumptions.json", "Operations.json", "Returns.json"]

    assumptions_json = (tables_dir / "Assumptions.json").read_text(encoding="utf-8")
    assert assumptions_json.startswith('{\n  "id": "Assumptions",')
    assumptions_data = json.loads(assumptions_json)
    assert assumptions_data["id"] == "Assumptions"
    assert assumptions_data["title"] == "Key Assumptions"
    assert assumptions_data["currency"] == "usd"


def test_scenario_2_overwrite_protection(tmp_path: Path) -> None:
    model = _create_sample_model()
    export_dir = tmp_path / "protected_project"

    model.export_project(export_dir, overwrite=False)
    assert (export_dir / "f8.json").is_file()

    with pytest.raises(FileExistsError, match="Project manifest already exists"):
        model.export_project(export_dir, overwrite=False)

    # Calling with overwrite=True succeeds
    model.export_project(export_dir, overwrite=True)
    assert (export_dir / "f8.json").is_file()


def test_scenario_3_direct_project_compilation_from_disk(tmp_path: Path) -> None:
    model = _create_sample_model()
    export_dir = tmp_path / "project_to_compile"
    model.export_project(export_dir)

    compiled_from_dir = compile_project(export_dir)
    assert isinstance(compiled_from_dir, CompiledModel)
    summary_dir = compiled_from_dir.summary()
    assert summary_dir["model_name"] == "ProjectTitan"
    assert summary_dir["total_tables"] == 3

    # Also accepts direct path to f8.json
    compiled_from_manifest = compile_project(export_dir / "f8.json")
    assert isinstance(compiled_from_manifest, CompiledModel)
    assert compiled_from_manifest.summary() == summary_dir


def test_scenario_4_round_trip_evaluation_fidelity(tmp_path: Path) -> None:
    model = Model(name="ComprehensiveModel", theme="corporate_navy")
    sheet = model.add_sheet("Statements", freeze_panes=(3, 1))
    table = sheet.add_table("Financials", title="Income Statement", currency="USD")

    table.add_column("metric", role="descriptor", header="Metric")
    table.add_column_gutter()
    table.add_column("2023A", role="measure", header="2023A", format="currency", decimals=0)
    table.add_column("2024E", role="measure", header="2024E", format="currency", decimals=0)

    table.add_section_header("Topline Growth", note="Audited numbers")
    table.add_row_gutter()
    table.add_row(
        "rev",
        cells={"metric": "Revenue", "2023A": 5000.0},
        expr='Financials["rev", col.prev] * 1.20',
        format="currency",
    )
    table.add_row(
        "cogs",
        cells={
            "metric": Cell("COGS", indent=1),
            "2023A": Cell(2000.0, note="Historical cost base"),
        },
        expr='Financials["cogs", col.prev] * 1.10',
        format="currency",
    )
    table.add_row(
        "gp",
        cells={"metric": Cell("Gross Profit", style="subtotal")},
        expr='Financials["rev", @col] - Financials["cogs", @col]',
        style="subtotal",
        format="currency",
    )

    compiled_direct = model.compile()

    export_dir = tmp_path / "round_trip_project"
    model.export_project(export_dir)

    compiled_loaded = compile_project(export_dir)

    # 1. High-level summary metrics match
    assert compiled_direct.summary() == compiled_loaded.summary()

    # 2. Cell inspections match across values, formulas, and physical layout
    cell_keys = [
        ("Financials", "rev", "2023A"),
        ("Financials", "rev", "2024E"),
        ("Financials", "cogs", "2023A"),
        ("Financials", "cogs", "2024E"),
        ("Financials", "gp", "2023A"),
        ("Financials", "gp", "2024E"),
    ]

    for tbl, row, col in cell_keys:
        direct_insp = compiled_direct.inspect_cell(tbl, row, col)
        loaded_insp = compiled_loaded.inspect_cell(tbl, row, col)

        assert direct_insp["evaluated_value"] == loaded_insp["evaluated_value"]
        assert direct_insp["excel_formula"] == loaded_insp["excel_formula"]
        assert direct_insp["formula_expr"] == loaded_insp["formula_expr"]
        assert direct_insp["coordinate"] == loaded_insp["coordinate"]
        assert direct_insp["physical_address"] == loaded_insp["physical_address"]
        assert direct_insp["format"] == loaded_insp["format"]
        assert direct_insp["note"] == loaded_insp["note"]


def test_scenario_5_missing_project_manifest_error_handling(tmp_path: Path) -> None:
    empty_dir = tmp_path / "empty_folder"
    empty_dir.mkdir()

    with pytest.raises(FileNotFoundError, match="Project manifest 'f8.json' not found"):
        compile_project(empty_dir)

    non_existent = tmp_path / "non_existent_folder"
    with pytest.raises(FileNotFoundError, match="does not exist"):
        compile_project(non_existent)


def test_duplicate_table_id_detection(tmp_path: Path) -> None:
    project_dir = tmp_path / "dup_project"
    project_dir.mkdir()
    tables_dir = project_dir / "tables"
    tables_dir.mkdir()

    manifest = {
        "model_name": "DupModel",
        "theme": "corporate_navy",
        "sheets": [{"name": "Sheet1", "tables": ["T1"]}],
    }
    (project_dir / "f8.json").write_text(json.dumps(manifest), encoding="utf-8")

    t1 = {
        "id": "T1",
        "title": "First Table",
        "columns": [{"kind": "data", "id": "c1", "role": "measure", "headers": ["C1"]}],
        "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 10}}],
    }
    t2 = {
        "id": "T1",
        "title": "Second Table With Duplicate ID",
        "columns": [{"kind": "data", "id": "c1", "role": "measure", "headers": ["C1"]}],
        "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 20}}],
    }

    (tables_dir / "table_a.json").write_text(json.dumps(t1), encoding="utf-8")
    (tables_dir / "table_b.json").write_text(json.dumps(t2), encoding="utf-8")

    with pytest.raises(ValueError, match="Duplicate table id 'T1'"):
        compile_project(project_dir)


def test_compile_fixture_templates() -> None:
    repo_root = Path(__file__).resolve().parent.parent.parent
    templates_dir = repo_root / "crates" / "f8-cli" / "templates"

    if not templates_dir.is_dir():
        pytest.skip("Templates directory not found")

    for template_name in ["three-statement", "lbo", "trading-comps"]:
        template_path = templates_dir / template_name
        assert (template_path / "f8.json").is_file()

        compiled = compile_project(template_path)
        assert isinstance(compiled, CompiledModel)
        summary = compiled.summary()
        assert summary["total_tables"] > 0
        assert summary["total_cells"] > 0


def test_manifest_without_theme_defaults_cleanly(tmp_path: Path) -> None:
    project_dir = tmp_path / "no_theme_project"
    project_dir.mkdir()
    tables_dir = project_dir / "tables"
    tables_dir.mkdir()

    # Omit "theme" key in f8.json
    manifest: dict[str, object] = {
        "model_name": "NoThemeModel",
        "sheets": [{"name": "Sheet1", "tables": ["T1"]}],
    }
    (project_dir / "f8.json").write_text(json.dumps(manifest), encoding="utf-8")

    t1 = {
        "id": "T1",
        "title": "Valid Table",
        "columns": [{"kind": "data", "id": "c1", "role": "measure", "headers": ["C1"]}],
        "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 42}}],
    }
    (tables_dir / "T1.json").write_text(json.dumps(t1), encoding="utf-8")

    compiled = compile_project(project_dir)
    assert isinstance(compiled, CompiledModel)
    assert compiled.summary()["model_name"] == "NoThemeModel"


def test_malformed_table_file_detection(tmp_path: Path) -> None:
    project_dir = tmp_path / "malformed_project"
    project_dir.mkdir()
    tables_dir = project_dir / "tables"
    tables_dir.mkdir()

    manifest: dict[str, object] = {
        "model_name": "MalformedModel",
        "sheets": [{"name": "Sheet1", "tables": []}],
    }
    (project_dir / "f8.json").write_text(json.dumps(manifest), encoding="utf-8")

    # 1. Non-dict JSON in tables/
    (tables_dir / "bad_array.json").write_text("[1, 2, 3]", encoding="utf-8")
    with pytest.raises(ValueError, match="must contain a JSON object"):
        compile_project(project_dir)

    (tables_dir / "bad_array.json").unlink()

    # 2. Dict without 'id'
    (tables_dir / "no_id.json").write_text(json.dumps({"title": "Missing ID"}), encoding="utf-8")
    with pytest.raises(ValueError, match="must specify a non-empty string 'id'"):
        compile_project(project_dir)

    (tables_dir / "no_id.json").unlink()

    # 3. Dict with blank 'id'
    (tables_dir / "blank_id.json").write_text(json.dumps({"id": "   "}), encoding="utf-8")
    with pytest.raises(ValueError, match="must specify a non-empty string 'id'"):
        compile_project(project_dir)


def test_compile_project_with_path_theme(tmp_path: Path) -> None:
    project_dir = tmp_path / "theme_project"
    project_dir.mkdir()
    tables_dir = project_dir / "tables"
    tables_dir.mkdir()

    manifest = {
        "model_name": "ThemeModel",
        "sheets": [{"name": "Sheet1", "tables": ["T1"]}],
    }
    (project_dir / "f8.json").write_text(json.dumps(manifest), encoding="utf-8")

    t1 = {
        "id": "T1",
        "title": "Theme Table",
        "columns": [{"kind": "data", "id": "c1", "role": "measure", "headers": ["C1"]}],
        "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 100}}],
    }
    (tables_dir / "T1.json").write_text(json.dumps(t1), encoding="utf-8")

    # Create custom theme file at an external location
    external_theme = tmp_path / "external_theme.json"
    theme_spec: dict[str, object] = {
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
            "zebra_fill": "#FAFCFD",
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
    external_theme.write_text(json.dumps(theme_spec), encoding="utf-8")

    # 1. Pass custom theme file as Path object
    compiled = compile_project(project_dir, theme=external_theme)
    assert isinstance(compiled, CompiledModel)
    assert compiled.summary()["model_name"] == "ThemeModel"

    # 2. Pass built-in theme name as Path object
    compiled_builtin = compile_project(project_dir, theme=Path("corporate_navy"))
    assert isinstance(compiled_builtin, CompiledModel)
    assert compiled_builtin.summary()["model_name"] == "ThemeModel"
