import math
from pathlib import Path

import f8
import pytest
from f8 import (
    CompiledModel,
    CycleDetectedError,
    EmissionError,
    F8Error,
    FormulaParseError,
    InspectionError,
    ReferenceResolutionError,
    SpecificationError,
)
from f8._core import compile as core_compile
from f8.types import (
    ProjectManifestDict,
    TableSpecDict,
)

type SampleModelFixture = tuple[dict[str, object], list[dict[str, object]]]


@pytest.fixture
def sample_model() -> SampleModelFixture:
    manifest: dict[str, object] = {
        "model_name": "TestModel",
        "theme": "corporate_navy",
        "sheets": [
            {
                "name": "Operations",
                "tables": ["OPERATIONS"],
            }
        ],
    }

    tables: list[dict[str, object]] = [
        {
            "id": "OPERATIONS",
            "title": "Operations Summary",
            "columns": [
                {
                    "kind": "data",
                    "id": "line_item",
                    "role": "descriptor",
                    "headers": ["Line Item"],
                },
                {
                    "kind": "data",
                    "id": "2024E",
                    "headers": ["2024E"],
                },
            ],
            "rows": [
                {
                    "kind": "data",
                    "id": "revenue",
                    "format": "currency",
                    "cells": {
                        "line_item": "Total Revenue",
                        "2024E": 100.0,
                    },
                },
                {
                    "kind": "data",
                    "id": "cogs",
                    "format": "currency",
                    "cells": {
                        "line_item": "Cost of Goods Sold",
                        "2024E": 40.0,
                    },
                },
                {
                    "kind": "data",
                    "id": "gross_profit",
                    "format": "currency",
                    "expr": 'OPERATIONS["revenue", @col] - OPERATIONS["cogs", @col]',
                    "cells": {
                        "line_item": "Gross Profit",
                    },
                },
                {
                    "kind": "data",
                    "id": "ebitda",
                    "format": "currency",
                    "expr": 'OPERATIONS["gross_profit", @col]',
                    "cells": {
                        "line_item": "EBITDA",
                    },
                },
            ],
        }
    ]

    return manifest, tables


def test_scenario_1_in_memory_compilation_and_emission(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = core_compile(manifest, tables)

    assert isinstance(compiled, CompiledModel)
    buffer = compiled.emit_workbook()

    assert isinstance(buffer, bytes)
    assert buffer.startswith(b"PK\x03\x04")
    assert len(buffer) > 1000


def test_scenario_2_emission_options_values_only(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = core_compile(manifest, tables)

    buffer = compiled.emit_workbook({"values_only": True})
    assert isinstance(buffer, bytes)
    assert buffer.startswith(b"PK\x03\x04")
    assert len(buffer) > 1000


def test_scenario_3_cell_inspection_flexible_coordinates(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = core_compile(manifest, tables)

    cell_3args = compiled.inspect_cell("OPERATIONS", "gross_profit", "2024E")
    cell_1arg = compiled.inspect_cell('OPERATIONS["gross_profit", "2024E"]')

    assert cell_3args == cell_1arg
    assert cell_3args["physical_address"] == "Operations!C7"
    assert cell_3args["evaluated_value"] == {"Number": 60.0}
    assert cell_3args["formatted_value"] == "$60"
    assert cell_3args["is_formula"] is True
    assert cell_3args["excel_formula"] is not None
    assert len(cell_3args["precedents"]) == 2


def test_scenario_4_dependency_tree_direct_and_recursive(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = core_compile(manifest, tables)

    tree = compiled.precedents_of("OPERATIONS", "ebitda", "2024E", recursive=True)
    assert tree["root"] == {"table": "OPERATIONS", "row": "ebitda", "col": "2024E"}
    assert tree["direct_count"] == 1
    assert tree["transitive_count"] == 3
    assert len(tree["nodes"]) == 1
    assert tree["nodes"][0]["cell_id"] == {
        "table": "OPERATIONS",
        "row": "gross_profit",
        "col": "2024E",
    }

    deps = compiled.dependents_of("OPERATIONS", "revenue", "2024E", recursive=True)
    assert deps["root"] == {"table": "OPERATIONS", "row": "revenue", "col": "2024E"}
    assert deps["direct_count"] == 1


def test_scenario_5_model_and_sheet_summary_inspection(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = core_compile(manifest, tables)

    summary = compiled.summary()
    assert summary["model_name"] == "TestModel"
    assert summary["total_sheets"] == 1
    assert summary["total_tables"] == 1
    assert summary["total_cells"] == 8
    assert summary["total_drivers"] == 6
    assert summary["total_formulas"] == 2

    sheet = compiled.inspect_sheet("Operations")
    assert sheet["sheet_id"] == "Operations"
    assert len(sheet["tables"]) == 1
    assert len(sheet["rows"]) > 0

    table_insp = compiled.inspect_table("OPERATIONS")
    assert table_insp["table_id"] == "OPERATIONS"
    assert len(table_insp["rows"]) == 4
    assert len(table_insp["columns"]) == 2
    assert table_insp["columns"][0]["style"] == "data"
    assert table_insp["rows"][0]["style"] == "data"
    rev_cell = table_insp["cells"]['OPERATIONS["revenue", "2024E"]']
    assert rev_cell["formatted_value"] == "$100"
    assert rev_cell["value"] == {"Number": 100.0}
    assert rev_cell["format"] == "currency"


def test_scenario_6_cyclic_dependency_error() -> None:
    manifest: dict[str, object] = {
        "model_name": "CycleModel",
        "theme": "corporate_navy",
        "sheets": [{"name": "S", "tables": ["T"]}],
    }
    tables: list[dict[str, object]] = [
        {
            "id": "T",
            "title": "Cycle Table",
            "columns": [
                {"kind": "data", "id": "desc", "role": "descriptor", "headers": ["Desc"]},
                {"kind": "data", "id": "val", "headers": ["Val"]},
            ],
            "rows": [
                {
                    "kind": "data",
                    "id": "a",
                    "expr": 'T["b", @col]',
                    "cells": {"desc": "Row A"},
                },
                {
                    "kind": "data",
                    "id": "b",
                    "expr": 'T["a", @col]',
                    "cells": {"desc": "Row B"},
                },
            ],
        }
    ]

    with pytest.raises(CycleDetectedError) as exc_info:
        core_compile(manifest, tables)

    err = exc_info.value
    assert isinstance(err, CycleDetectedError)
    assert isinstance(err, F8Error)
    assert hasattr(err, "cycle")
    assert isinstance(err.cycle, list)
    assert len(err.cycle) >= 2
    assert hasattr(err, "message")
    assert "Circular dependency" in err.message or "cycle" in err.message.lower()


def test_scenario_7_reference_resolution_error() -> None:
    manifest: dict[str, object] = {
        "model_name": "RefModel",
        "theme": "corporate_navy",
        "sheets": [{"name": "S", "tables": ["T"]}],
    }
    tables: list[dict[str, object]] = [
        {
            "id": "T",
            "title": "Table",
            "columns": [
                {"kind": "data", "id": "desc", "role": "descriptor", "headers": ["Desc"]},
                {"kind": "data", "id": "val", "headers": ["Val"]},
            ],
            "rows": [
                {
                    "kind": "data",
                    "id": "a",
                    "expr": 'NONEXISTENT["row", @col]',
                    "cells": {"desc": "Row A"},
                }
            ],
        }
    ]

    with pytest.raises(ReferenceResolutionError) as exc_info:
        core_compile(manifest, tables)

    err = exc_info.value
    assert isinstance(err, ReferenceResolutionError)
    assert isinstance(err, F8Error)
    assert hasattr(err, "coordinate")
    assert hasattr(err, "target_table")
    assert err.target_table == "NONEXISTENT"


def test_scenario_8_formula_syntax_parse_error() -> None:
    manifest: dict[str, object] = {
        "model_name": "ParseErrorModel",
        "theme": "corporate_navy",
        "sheets": [{"name": "S", "tables": ["T"]}],
    }
    tables: list[dict[str, object]] = [
        {
            "id": "T",
            "title": "Table",
            "columns": [
                {"kind": "data", "id": "desc", "role": "descriptor", "headers": ["Desc"]},
                {"kind": "data", "id": "val", "headers": ["Val"]},
            ],
            "rows": [
                {
                    "kind": "data",
                    "id": "a",
                    "expr": "(100 + ",
                    "cells": {"desc": "Row A"},
                }
            ],
        }
    ]

    with pytest.raises(FormulaParseError) as exc_info:
        core_compile(manifest, tables)

    err = exc_info.value
    assert isinstance(err, FormulaParseError)
    assert isinstance(err, F8Error)
    assert hasattr(err, "table")
    assert err.table == "T"
    assert hasattr(err, "row")
    assert hasattr(err, "col")
    assert hasattr(err, "token_offset")
    assert hasattr(err, "message")


def test_scenario_9_specification_error() -> None:
    # Duplicate row ID
    manifest: dict[str, object] = {
        "model_name": "SpecErrorModel",
        "theme": "corporate_navy",
        "sheets": [{"name": "S", "tables": ["T"]}],
    }
    tables: list[dict[str, object]] = [
        {
            "id": "T",
            "title": "Table",
            "columns": [
                {"kind": "data", "id": "desc", "role": "descriptor", "headers": ["Desc"]},
                {"kind": "data", "id": "val", "headers": ["Val"]},
            ],
            "rows": [
                {
                    "kind": "data",
                    "id": "dup",
                    "cells": {"desc": "1", "val": 1.0},
                },
                {
                    "kind": "data",
                    "id": "dup",
                    "cells": {"desc": "2", "val": 2.0},
                },
            ],
        }
    ]

    with pytest.raises(SpecificationError) as exc_info:
        core_compile(manifest, tables)

    err = exc_info.value
    assert isinstance(err, SpecificationError)
    assert isinstance(err, F8Error)
    assert hasattr(err, "table")
    assert hasattr(err, "reason")
    assert "duplicate" in err.reason.lower()

    # Empty column group raises SpecificationError with table attribute
    empty_group_tables: list[dict[str, object]] = [
        {
            "id": "T",
            "title": "Table",
            "columns": [
                {"kind": "data", "id": "desc", "role": "descriptor", "headers": ["Desc"]},
                {"kind": "data", "id": "val", "group": "   ", "headers": ["Val"]},
            ],
            "rows": [
                {"kind": "data", "id": "r1", "cells": {"desc": "1", "val": 1.0}},
            ],
        }
    ]
    with pytest.raises(SpecificationError) as exc_info_group:
        core_compile(manifest, empty_group_tables)
    assert exc_info_group.value.table == "T"
    assert "group must not be empty" in exc_info_group.value.reason


def test_scenario_10_inspection_entity_not_found(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = core_compile(manifest, tables)

    with pytest.raises(InspectionError) as exc_info:
        compiled.inspect_cell("NONEXISTENT", "r", "c")
    err = exc_info.value
    assert isinstance(err, InspectionError)
    assert isinstance(err, F8Error)
    assert hasattr(err, "entity")
    assert "NONEXISTENT" in err.entity

    with pytest.raises(InspectionError) as exc_info:
        compiled.inspect_table("UNKNOWN_TABLE")
    err = exc_info.value
    assert isinstance(err, InspectionError)
    assert isinstance(err, F8Error)
    assert hasattr(err, "entity")
    assert "UNKNOWN_TABLE" in err.entity


def test_scenario_11_save_workbook_str_and_path(
    sample_model: SampleModelFixture,
    tmp_path: Path,
) -> None:
    manifest, tables = sample_model
    compiled = core_compile(manifest, tables)

    # Save via str path
    str_target = str(tmp_path / "model_str.xlsx")
    compiled.save(str_target)
    assert Path(str_target).exists()
    file_bytes = Path(str_target).read_bytes()
    assert file_bytes.startswith(b"PK\x03\x04")
    assert len(file_bytes) > 1000

    # Save via Path with automatic parent directory creation
    nested_target = tmp_path / "nested" / "deeply" / "dir" / "model_path.xlsx"
    assert not nested_target.parent.exists()
    compiled.save(nested_target)
    assert nested_target.exists()
    nested_bytes = nested_target.read_bytes()
    assert nested_bytes.startswith(b"PK\x03\x04")
    assert len(nested_bytes) > 1000

    # Save with options
    values_only_target = tmp_path / "values_only.xlsx"
    compiled.save(values_only_target, options={"values_only": True})
    assert values_only_target.exists()

    # Emission failure with non-writable location raises EmissionError
    if not (tmp_path / "dir_blocked").exists():
        (tmp_path / "dir_blocked").mkdir()
    with pytest.raises(EmissionError):
        # Trying to save to a directory as a file path
        compiled.save(tmp_path / "dir_blocked")


def test_scenario_12_typeddict_contracts_and_schema_compatibility() -> None:
    manifest: ProjectManifestDict = {
        "model_name": "TypedModel",
        "theme": "corporate_navy",
        "sheets": [
            {
                "name": "Sheet1",
                "tables": ["T1"],
                "show_gridlines": True,
                "freeze_panes": {"row": 2, "col": 1},
            }
        ],
    }

    table: TableSpecDict = {
        "id": "T1",
        "title": "Typed Table",
        "subtitle": "Subtitle",
        "currency": "usd",
        "collapsible": False,
        "placement": {"kind": "stack", "gap": 2},
        "columns": [
            {
                "kind": "data",
                "id": "line_item",
                "role": "descriptor",
                "headers": ["Line Item"],
            },
            {
                "kind": "data",
                "id": "c1",
                "role": "measure",
                "headers": ["Col 1"],
                "format": "currency",
                "decimals": 2,
                "currency": "usd",
            },
            {"kind": "gutter"},
        ],
        "rows": [
            {
                "kind": "section_header",
                "title": "Revenue Section",
                "indent": 0,
                "note": "Section note",
            },
            {
                "kind": "data",
                "id": "r1",
                "format": "currency",
                "cells": {
                    "line_item": {
                        "value": "Total Revenue",
                        "indent": 1,
                        "note": "Audited",
                    },
                    "c1": 100.0,
                },
            },
            {"kind": "gutter"},
        ],
    }

    compiled = core_compile(manifest, [table])
    assert isinstance(compiled, CompiledModel)
    summary = compiled.summary()
    assert summary["model_name"] == "TypedModel"
    assert summary["total_tables"] == 1
    assert summary["total_cells"] == 2


def test_scenario_13_fast_path_scalar_num_and_formatted_lookups() -> None:
    model = f8.Model(name="ScalarLookupModel", theme="corporate_navy")
    sheet = model.add_sheet("Main")
    table = sheet.add_table("Metrics", title="Metrics", currency="USD")
    table.add_column("line_item", role="descriptor", header="Item")
    table.add_column("val", role="measure", header="Value")

    table.add_row(
        "rev",
        cells={"line_item": "Revenue", "val": 115.0},
        format="currency",
        decimals=1,
    )
    table.add_row(
        "loss",
        cells={"line_item": "Net Loss", "val": -50.0},
        format="currency",
        decimals=1,
    )
    table.add_row(
        "flag",
        cells={"line_item": "Is Active", "val": True},
    )
    table.add_row(
        "empty_row",
        cells={"line_item": "Unassigned", "val": None},
        format="currency",
        decimals=1,
    )
    table.add_row(
        "err_row",
        cells={"line_item": "Div Zero"},
        expr="1.0 / 0.0",
    )

    compiled = model.compile()

    # 1. compiled.value (1-arg and 3-arg)
    assert compiled.value('Metrics["rev", "val"]') == pytest.approx(115.0)
    assert compiled.value("Metrics", "rev", "val") == pytest.approx(115.0)
    assert compiled.value("Metrics", "rev", "line_item") == "Revenue"
    assert compiled.value("Metrics", "flag", "val") is True
    assert compiled.value("Metrics", "empty_row", "val") is None
    assert compiled.value("Metrics", "err_row", "val") == "#DIV/0!"

    # 2. compiled.num (1-arg and 3-arg)
    num_1arg = compiled.num('Metrics["rev", "val"]')
    num_3arg = compiled.num("Metrics", "rev", "val")
    assert isinstance(num_1arg, float)
    assert num_1arg == pytest.approx(115.0)
    assert num_3arg == pytest.approx(115.0)

    with pytest.raises(TypeError):
        compiled.num("Metrics", "rev", "line_item")
    with pytest.raises(TypeError):
        compiled.num("Metrics", "flag", "val")
    with pytest.raises(TypeError):
        compiled.num("Metrics", "empty_row", "val")
    with pytest.raises(TypeError):
        compiled.num("Metrics", "err_row", "val")

    # 3. compiled.formatted (1-arg and 3-arg, with and without unit)
    assert compiled.formatted('Metrics["rev", "val"]') == "$115.0"
    assert compiled.formatted("Metrics", "rev", "val", unit="M") == "$115.0M"
    assert compiled.formatted("Metrics", "loss", "val") == "($50.0)"
    assert compiled.formatted("Metrics", "loss", "val", unit="M") == "($50.0M)"
    assert compiled.formatted("Metrics", "empty_row", "val", unit="M") == ""

    # 4. Unknown coordinates raise InspectionError
    with pytest.raises(InspectionError):
        compiled.value("Metrics", "nonexistent", "val")
    with pytest.raises(InspectionError):
        compiled.num("Metrics", "nonexistent", "val")
    with pytest.raises(InspectionError):
        compiled.formatted("Metrics", "nonexistent", "val")


def test_scenario_14_manifest_theme_resolution_and_validation(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model

    # 1. Omitted theme defaults to corporate_navy
    omitted_manifest: dict[str, object] = {
        "model_name": "OmittedThemeModel",
        "sheets": manifest["sheets"],
    }
    compiled_omitted = core_compile(omitted_manifest, tables)
    assert compiled_omitted.summary()["theme_name"] == "corporate_navy"

    # 2. Unknown built-in theme raises SpecificationError
    unknown_manifest: dict[str, object] = {
        **manifest,
        "theme": "nonexistent_theme",
    }
    with pytest.raises(SpecificationError, match="nonexistent_theme"):
        core_compile(unknown_manifest, tables)

    # 3. Inline custom theme in manifest is used and validated
    custom_theme: dict[str, object] = {
        "name": "manifest_custom",
        "typography": {
            "font_family": "Aptos",
            "title_size": 14.0,
            "subtitle_size": 9.0,
            "super_header_size": 10.0,
            "col_header_size": 10.0,
            "section_header_size": 10.0,
            "data_size": 10.0,
        },
        "metrics": {
            "title_row_height": 24.0,
            "header_row_height": 18.0,
            "section_row_height": 18.0,
            "data_row_height": 15.0,
            "gutter_row_height": 9.0,
            "gutter_col_width": 3.0,
            "title_gutter_row_height": 8.0,
            "char_width_multiplier": 1.0,
            "col_padding": 1.0,
            "min_col_width": 12.0,
        },
        "palette": {
            "table_title_fill": "#1B365D",
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
            "defaults": {
                "currency_decimals": 2,
                "number_decimals": 0,
                "percentage_decimals": 1,
                "multiple_decimals": 1,
                "basis_points_decimals": 0,
            },
        },
    }
    custom_manifest: dict[str, object] = {
        **manifest,
        "theme": custom_theme,
    }
    compiled_custom = core_compile(custom_manifest, tables)
    assert compiled_custom.summary()["theme_name"] == "manifest_custom"
    assert compiled_custom.formatted("OPERATIONS", "revenue", "2024E") == "$100.00"

    # 4. Invalid custom theme palette color in manifest raises SpecificationError
    palette_dict: dict[str, object] = {
        "table_title_fill": "not_a_hex",
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
    }
    bad_theme: dict[str, object] = {**custom_theme, "palette": palette_dict}
    bad_manifest: dict[str, object] = {**manifest, "theme": bad_theme}
    with pytest.raises(SpecificationError, match="table_title_fill"):
        core_compile(bad_manifest, tables)

    # 5. max_col_width < min_col_width in custom theme raises SpecificationError
    base_metrics = custom_theme["metrics"]
    assert isinstance(base_metrics, dict)
    inverted_metrics: dict[str, object] = {
        **base_metrics,
        "min_col_width": 12.0,
        "max_col_width": 10.0,
    }
    inverted_theme: dict[str, object] = {**custom_theme, "metrics": inverted_metrics}
    inverted_manifest: dict[str, object] = {**manifest, "theme": inverted_theme}
    with pytest.raises(SpecificationError, match="max_col_width"):
        core_compile(inverted_manifest, tables)

    # 6. Rejection of removed shade_accent_fill in theme palette
    accent_palette = {
        **palette_dict,
        "table_title_fill": "#1B365D",
        "shade_accent_fill": "#EFF6FF",
    }
    accent_theme = {**custom_theme, "palette": accent_palette}
    accent_manifest = {**manifest, "theme": accent_theme}
    with pytest.raises(SpecificationError, match="shade_accent_fill"):
        core_compile(accent_manifest, tables)


def test_scenario_15_contiguous_and_discontinuous_range_slice_validation() -> None:
    # 1. Contiguous sub-block and single-descriptor vertical slice succeed
    ok_model = f8.Model(name="ContiguousOk", theme="corporate_navy")
    ok_sheet = ok_model.add_sheet("Main")
    ok_table = ok_sheet.add_table("T", title="Summary")
    ok_table.add_column("line_item", role="descriptor", header="Line Item")
    ok_table.add_column_gutter()
    ok_table.add_column("2024E", role="measure", header="2024E")
    ok_table.add_column("2025E", role="measure", header="2025E")
    ok_table.add_section_header("Revenue")
    ok_table.add_row("r1", cells={"line_item": "Product", "2024E": 10.0, "2025E": 20.0})
    ok_table.add_row("r2", cells={"line_item": "Rev", "2024E": 30.0, "2025E": 40.0})
    ok_table.add_row_gutter()
    ok_table.add_row(
        "tot",
        cells={"line_item": f8.Cell(expr='COUNTA(@table["r1".."r2", @col])')},
        expr=(
            'SUM(@table["r1".."r2", "2024E".."2025E"]) + '
            'XLOOKUP("Rev", @table["r1".."r2", "line_item"], @table["r1".."r2", @col])'
        ),
    )
    compiled = ok_model.compile()
    assert compiled.num("T", "tot", "2024E") == pytest.approx(130.0)
    assert compiled.num("T", "tot", "2025E") == pytest.approx(140.0)
    assert compiled.num("T", "tot", "line_item") == pytest.approx(2.0)

    # 2. Row range spanning RowGutter -> ReferenceResolutionError (col=None, row=None)
    rg_model = f8.Model(name="RowGutterFail")
    rg_table = rg_model.add_sheet("Main").add_table("T", title="T")
    rg_table.add_column("2024E", role="measure", header="2024E")
    rg_table.add_row("r1", cells={"2024E": 10.0})
    rg_table.add_row_gutter()
    rg_table.add_row("r2", cells={"2024E": 20.0})
    rg_table.add_row("tot", expr='SUM(T["r1".."r2", @col])')
    with pytest.raises(ReferenceResolutionError) as exc_rg:
        rg_model.compile()
    assert exc_rg.value.coordinate == 'T["tot", "2024E"]'
    assert exc_rg.value.target_table == "T"
    assert exc_rg.value.row is None
    assert exc_rg.value.col is None
    assert "a gutter row" in exc_rg.value.message

    # 3. Row range spanning SectionHeader -> ReferenceResolutionError (col=None, row=None)
    sh_model = f8.Model(name="SectionHeaderFail")
    sh_table = sh_model.add_sheet("Main").add_table("T", title="T")
    sh_table.add_column("2024E", role="measure", header="2024E")
    sh_table.add_row("r1", cells={"2024E": 10.0})
    sh_table.add_section_header("OpEx")
    sh_table.add_row("r2", cells={"2024E": 20.0})
    sh_table.add_row("tot", expr='SUM(T["r1".."r2", @col])')
    with pytest.raises(ReferenceResolutionError) as exc_sh:
        sh_model.compile()
    assert exc_sh.value.coordinate == 'T["tot", "2024E"]'
    assert exc_sh.value.target_table == "T"
    assert exc_sh.value.row is None
    assert exc_sh.value.col is None
    assert "section header 'OpEx'" in exc_sh.value.message

    # 4. Column range spanning ColumnGutter -> ReferenceResolutionError (col=None, row=None)
    cg_model = f8.Model(name="ColGutterFail")
    cg_table = cg_model.add_sheet("Main").add_table("T", title="T")
    cg_table.add_column("2024E", role="measure", header="2024E")
    cg_table.add_column_gutter()
    cg_table.add_column("2025E", role="measure", header="2025E")
    cg_table.add_row(
        "r1",
        cells={"2024E": 10.0, "2025E": f8.Cell(expr='SUM(T["r1", col.prev..@col])')},
    )
    with pytest.raises(ReferenceResolutionError) as exc_cg:
        cg_model.compile()
    assert exc_cg.value.coordinate == 'T["r1", "2025E"]'
    assert exc_cg.value.target_table == "T"
    assert exc_cg.value.row is None
    assert exc_cg.value.col is None
    assert "a gutter column" in exc_cg.value.message

    # 5. Column range spanning DescriptorColumn -> ReferenceResolutionError with col populated
    dc_model = f8.Model(name="DescriptorColFail")
    dc_table = dc_model.add_sheet("Main").add_table("T", title="T")
    dc_table.add_column("m1", role="measure", header="M1")
    dc_table.add_column("notes", role="descriptor", header="Notes")
    dc_table.add_column("m2", role="measure", header="M2")
    dc_table.add_row(
        "r1",
        cells={
            "m1": 10.0,
            "notes": "memo",
            "m2": f8.Cell(expr='SUM(T["r1", col.prev..@col])'),
        },
    )
    with pytest.raises(ReferenceResolutionError) as exc_dc:
        dc_model.compile()
    assert exc_dc.value.coordinate == 'T["r1", "m2"]'
    assert exc_dc.value.target_table == "T"
    assert exc_dc.value.row is None
    assert exc_dc.value.col == "notes"
    assert "descriptor column 'notes'" in exc_dc.value.message


def test_scenario_16_cross_table_relative_offsets_and_diagnostics() -> None:
    # 1. Valid cross-table column & row offsets and range slices (Scenarios 1, 2, 3)
    model = f8.Model(name="CrossTableOffsets", theme="corporate_navy")
    sheet = model.add_sheet("Main")

    history = sheet.add_table("History", title="History")
    for col_id in ("2022A", "2023A", "2024E"):
        history.add_column(col_id, role="measure", header=col_id)
    history.add_row(
        "rev",
        cells={
            "2022A": 100.0,
            "2023A": 150.0,
            "2024E": f8.Cell(expr='Forecast["rev", col.next]'),
        },
    )

    detail = sheet.add_table("Detail", title="Detail")
    detail.add_column("2022A", role="measure", header="2022A")
    detail.add_column("notes", role="descriptor", header="Notes")
    detail.add_column_gutter()
    detail.add_column("2023A", role="measure", header="2023A")
    detail.add_column("2024E", role="measure", header="2024E")
    detail.add_row(
        "rev",
        cells={"2022A": 80.0, "notes": "audited", "2023A": 120.0, "2024E": 160.0},
    )

    forecast = sheet.add_table("Forecast", title="Forecast")
    forecast.add_column("2024E", role="measure", header="2024E")
    forecast.add_column("2025E", role="measure", header="2025E")
    forecast.add_row(
        "rev",
        cells={
            "2024E": f8.Cell(expr='History["rev", col.prev]'),
            "2025E": 300.0,
        },
    )
    forecast.add_row(
        "cum_hist",
        cells={
            "2024E": f8.Cell(expr='SUM(History["rev", "2022A"..col.prev])'),
            "2025E": 0.0,
        },
    )
    forecast.add_row(
        "detail_lag",
        cells={
            "2024E": f8.Cell(expr='Detail["rev", col.prev]'),
            "2025E": 0.0,
        },
    )
    forecast.add_row(
        "detail_lag2",
        cells={
            "2024E": f8.Cell(expr='Detail["rev", col.prev(2)]'),
            "2025E": 0.0,
        },
    )

    schedule = sheet.add_table("Schedule", title="Schedule")
    schedule.add_column("2024E", role="measure", header="2024E")
    schedule.add_row("r0", cells={"2024E": 10.0})
    schedule.add_section_header("Sec")
    schedule.add_row("r1", cells={"2024E": 20.0})
    schedule.add_row_gutter()
    schedule.add_row("r2", cells={"2024E": 30.0})
    schedule.add_row("r3", cells={"2024E": 40.0})

    driver = sheet.add_table("DriverTable", title="Drivers")
    driver.add_column("2024E", role="measure", header="2024E")
    driver.add_row("r1", expr="Schedule[row.prev, @col]")
    driver.add_row("r3", expr='Schedule[row.prev, @col] + SUM(Schedule["r2"..row.prev, @col])')

    compiled = model.compile()
    assert compiled.num("Forecast", "rev", "2024E") == pytest.approx(150.0)
    assert compiled.num("Forecast", "cum_hist", "2024E") == pytest.approx(250.0)
    assert compiled.num("History", "rev", "2024E") == pytest.approx(300.0)
    assert compiled.num("Forecast", "detail_lag", "2024E") == pytest.approx(120.0)
    assert compiled.num("Forecast", "detail_lag2", "2024E") == pytest.approx(80.0)
    assert compiled.num("DriverTable", "r1", "2024E") == pytest.approx(10.0)
    assert compiled.num("DriverTable", "r3", "2024E") == pytest.approx(60.0)

    fcst_cum = compiled.inspect_cell("Forecast", "cum_hist", "2024E")
    assert fcst_cum["excel_formula"] == "=SUM($B$5:C$5)"

    # 2. Cross-table active column mismatch (scalar and range endpoint) -> ReferenceResolutionError
    for expr_str in (
        'Assumptions["tax", col.prev]',
        'SUM(Assumptions["tax", "val"..col.prev])',
    ):
        col_mm = f8.Model(name="ColMismatch")
        mm_sheet = col_mm.add_sheet("Main")
        assump = mm_sheet.add_table("Assumptions", title="Assumptions")
        assump.add_column("val", role="measure", header="Val")
        assump.add_row("tax", cells={"val": 0.21})
        fcst = mm_sheet.add_table("Forecast", title="Forecast")
        fcst.add_column("2024E", role="measure", header="2024E")
        fcst.add_row("ebitda", expr=expr_str)
        with pytest.raises(ReferenceResolutionError) as exc_col:
            col_mm.compile()
        assert exc_col.value.coordinate == 'Forecast["ebitda", "2024E"]'
        assert exc_col.value.target_table == "Assumptions"
        assert exc_col.value.col == "2024E"
        assert exc_col.value.row is None

    # 3. Cross-table active row mismatch (scalar and range endpoint) -> ReferenceResolutionError
    for expr_str in (
        'Assumptions[row.prev, "val"]',
        'SUM(Assumptions["tax"..row.prev, "val"])',
    ):
        row_mm = f8.Model(name="RowMismatch")
        mm_sheet = row_mm.add_sheet("Main")
        assump = mm_sheet.add_table("Assumptions", title="Assumptions")
        assump.add_column("val", role="measure", header="Val")
        assump.add_row("tax", cells={"val": 0.21})
        fcst = mm_sheet.add_table("Forecast", title="Forecast")
        fcst.add_column("2024E", role="measure", header="2024E")
        fcst.add_row("ebitda", expr=expr_str)
        with pytest.raises(ReferenceResolutionError) as exc_row:
            row_mm.compile()
        assert exc_row.value.coordinate == 'Forecast["ebitda", "2024E"]'
        assert exc_row.value.target_table == "Assumptions"
        assert exc_row.value.row == "ebitda"
        assert exc_row.value.col is None

    # 4. Same-table and cross-table target out-of-bounds, target descriptor anchor,
    # multi-table disambiguation, and inverted range (Scenarios 3, 4, 5)
    same_oob_model = f8.Model(name="SameTableOob")
    same_oob_tbl = same_oob_model.add_sheet("Main").add_table("Origin", title="Origin")
    same_oob_tbl.add_column("2024E", role="measure", header="2024E")
    same_oob_tbl.add_row("r1", cells={"2024E": f8.Cell(expr='@table["r1", col.prev]')})
    with pytest.raises(ReferenceResolutionError) as exc_same_oob:
        same_oob_model.compile()
    same_target_table: str = exc_same_oob.value.target_table
    assert same_target_table.upper() == "ORIGIN"
    assert same_target_table == "Origin"
    assert exc_same_oob.value.row is None
    assert exc_same_oob.value.col is None
    assert (
        'Formula in cell \'Origin["r1", "2024E"]\' has temporal offset -1 '
        "which exceeds axis boundaries in table 'Origin'"
    ) in exc_same_oob.value.message

    for origin_col, expr_str, expected_offset_str in (
        ("2024E", 'Target["r1", col.prev]', "-1"),
        ("2024E", 'Target[row.prev, "2024E"]', "-1"),
        ("label", 'Target["r1", col.prev]', "-1"),
        ("2024E", 'HistoryA["r1", col.prev] + Target["r1", col.prev]', "-1"),
        ("2025E", 'HistoryA["r1", col.next] + Target["r1", col.next]', "+1"),
        ("2024E", 'HistoryA[row.prev, "2024E"] + Target[row.prev, "2024E"]', "-1"),
        ("2024E", 'HistoryA[row.next, "2024E"] + Target[row.next, "2024E"]', "+1"),
        ("label", 'HistoryA["r1", col.prev] + Target["r1", col.prev]', "-1"),
    ):
        oob_model = f8.Model(name="TargetOob")
        oob_sheet = oob_model.add_sheet("Main")

        history_a = oob_sheet.add_table("HistoryA", title="History A")
        for col_id in ("2023A", "label", "2024E", "2025E", "2026E"):
            history_a.add_column(col_id, role="measure", header=col_id)
        for row_id, base in (("r0", 1.0), ("r1", 10.0), ("r2", 20.0)):
            history_a.add_row(
                row_id,
                cells={
                    "2023A": base,
                    "label": base + 1.0,
                    "2024E": base + 2.0,
                    "2025E": base + 3.0,
                    "2026E": base + 4.0,
                },
            )

        target_tbl = oob_sheet.add_table("Target", title="Target")
        target_tbl.add_column("label", role="descriptor", header="Label")
        target_tbl.add_column("2024E", role="measure", header="2024E")
        target_tbl.add_column("2025E", role="measure", header="2025E")
        target_tbl.add_row("r1", cells={"label": "Item", "2024E": 10.0, "2025E": 20.0})

        origin_tbl = oob_sheet.add_table("Origin", title="Origin")
        origin_tbl.add_column("2023A", role="measure", header="2023A")
        origin_tbl.add_column("label", role="descriptor", header="Label")
        origin_tbl.add_column("2024E", role="measure", header="2024E")
        origin_tbl.add_column("2025E", role="measure", header="2025E")
        origin_tbl.add_row(
            "r0",
            cells={"2023A": 1.0, "label": "Zero", "2024E": 2.0, "2025E": 3.0},
        )
        origin_cells: dict[str, float | str | f8.Cell] = {
            "2023A": 4.0,
            "label": "One",
            "2024E": 5.0,
            "2025E": 6.0,
        }
        origin_cells[origin_col] = f8.Cell(expr=expr_str)
        origin_tbl.add_row("r1", cells=origin_cells)

        with pytest.raises(ReferenceResolutionError) as exc_oob:
            oob_model.compile()
        target_table_id: str = exc_oob.value.target_table
        assert target_table_id.upper() == "TARGET"
        assert target_table_id == "Target"
        assert exc_oob.value.coordinate == f'Origin["r1", "{origin_col}"]'
        assert exc_oob.value.row is None
        assert exc_oob.value.col is None
        assert (
            f'Formula in cell \'Origin["r1", "{origin_col}"]\' has temporal offset '
            f"{expected_offset_str} which exceeds axis boundaries in table 'Target'"
        ) in exc_oob.value.message

    inv_model = f8.Model(name="TargetInverted")
    inv_sheet = inv_model.add_sheet("Main")
    inv_target = inv_sheet.add_table("Target", title="Target")
    inv_target.add_column("label", role="descriptor", header="Label")
    inv_target.add_column("2024E", role="measure", header="2024E")
    inv_target.add_column("2025E", role="measure", header="2025E")
    inv_target.add_row("r1", cells={"label": "Item", "2024E": 10.0, "2025E": 20.0})
    inv_origin = inv_sheet.add_table("Origin", title="Origin")
    inv_origin.add_column("label", role="descriptor", header="Label")
    inv_origin.add_column("2024E", role="measure", header="2024E")
    inv_origin.add_column("2025E", role="measure", header="2025E")
    inv_origin.add_row(
        "r1",
        cells={
            "label": "Item",
            "2024E": 10.0,
            "2025E": f8.Cell(expr='SUM(Target["r1", col.prev.."label"])'),
        },
    )
    with pytest.raises(ReferenceResolutionError) as exc_inv:
        inv_model.compile()
    assert exc_inv.value.coordinate == 'Origin["r1", "2025E"]'
    assert exc_inv.value.target_table == "Target"
    assert exc_inv.value.row is None
    assert exc_inv.value.col is None
    assert "inverted range" in exc_inv.value.message.lower()


def test_scenario_17_static_function_validation_and_call_arg_confinement() -> None:
    invalid_call_cases: tuple[tuple[str, str], ...] = (
        ("IF(TRUE, 10, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"),
        ("IF(FALSE, SUM(), 10)", "too few arguments for 'SUM'"),
        ("IF(TRUE, 10, ABS(1, 2))", "too many arguments for 'ABS'"),
        ("IF(TRUE, 10, IFS(TRUE, 1, FALSE))", "variadic pairs in 'IFS'"),
        (
            "IF(TRUE, 10, ABS(_))",
            "argument 'number' at position 1 of 'ABS' cannot be omitted",
        ),
        ("IFS(TRUE, 1, FALSE, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"),
        ("IFERROR(10, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"),
        ("IFERROR(ABS(1, 2, 3), 0)", "too many arguments for 'ABS'"),
        ("IFNA(10, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"),
        ("CHOOSE(1, 10, SUM())", "too few arguments for 'SUM'"),
        ("SWITCH(1, 1, 10, ABS())", "too few arguments for 'ABS'"),
        (
            'XLOOKUP(1, @table["k1".."k2", @col], @table["v1".."v2", @col], UNKNOWN_FN())',
            "unknown function 'UNKNOWN_FN'",
        ),
        ("IF(1 / 0, UNKNOWN_FN(), 10)", "unknown function 'UNKNOWN_FN'"),
        ("IF(TRUE, 10, -(1 + UNKNOWN_FN()))", "unknown function 'UNKNOWN_FN'"),
        (
            "IF(FALSE, 10, _)",
            "trailing argument 'value_if_false' at position 3 of 'IF' cannot use '_'",
        ),
        (
            "IF(TRUE, 10, _)",
            "trailing argument 'value_if_false' at position 3 of 'IF' cannot use '_'",
        ),
        (
            "IF(TRUE, 10, IF(FALSE, 1, _))",
            "trailing argument 'value_if_false' at position 3 of 'IF' cannot use '_'",
        ),
        (
            'INDEX(@table["k1".."k2", @col], 2, _)',
            "trailing argument 'col_num' at position 3 of 'INDEX' cannot use '_'",
        ),
        (
            "YEARFRAC(45292, 45474, _)",
            "trailing argument 'basis' at position 3 of 'YEARFRAC' cannot use '_'",
        ),
        (
            'IRR(@table["k1".."k2", @col], _)',
            "trailing argument 'guess' at position 2 of 'IRR' cannot use '_'",
        ),
        (
            'XIRR(@table["v1".."v2", @col], @table["k1".."k2", @col], _)',
            "trailing argument 'guess' at position 3 of 'XIRR' cannot use '_'",
        ),
        (
            "PMT(0.05, 10, -1000, _)",
            "trailing argument 'fv' at position 4 of 'PMT' cannot use '_'",
        ),
        (
            "PMT(0.05, 10, -1000, _, _)",
            "trailing argument 'fv' at position 4 of 'PMT' cannot use '_'",
        ),
        (
            'XMATCH(1, @table["k1".."k2", @col], _)',
            "trailing argument 'match_mode' at position 3 of 'XMATCH' cannot use '_'",
        ),
        (
            'XLOOKUP(1, @table["k1".."k2", @col], @table["v1".."v2", @col], _)',
            "trailing argument 'if_not_found' at position 4 of 'XLOOKUP' cannot use '_'",
        ),
        (
            'XLOOKUP(1, @table["k1".."k2", @col], @table["v1".."v2", @col], _, 0, _)',
            "trailing argument 'search_mode' at position 6 of 'XLOOKUP' cannot use '_'",
        ),
        (
            "ROUND(1.5, _)",
            "argument 'num_digits' at position 2 of 'ROUND' cannot be omitted",
        ),
        (
            "IFS(TRUE, 1, FALSE, _)",
            "argument 'value_if_true1' at position 4 of 'IFS' cannot be omitted",
        ),
        (
            "IF(_, 10, _)",
            "argument 'logical_test' at position 1 of 'IF' cannot be omitted",
        ),
        (
            'INDEX(@table["k1".."k2", @col], _, 2)',
            "argument 'row_num' at position 2 of 'INDEX' cannot be omitted",
        ),
    )

    for formula, expected_substr in invalid_call_cases:
        err_model = f8.Model(name="InvalidCallModel")
        sheet = err_model.add_sheet("Main")
        tbl = sheet.add_table("T", title="Table")
        tbl.add_column("c1", role="measure", header="C1")
        tbl.add_row("k1", cells={"c1": 1.0})
        tbl.add_row("k2", cells={"c1": 2.0})
        tbl.add_row("v1", cells={"c1": 100.0})
        tbl.add_row("v2", cells={"c1": 200.0})
        tbl.add_row("target", expr=formula)

        with pytest.raises(F8Error) as exc_call:
            err_model.compile()
        assert type(exc_call.value) is F8Error
        assert expected_substr in exc_call.value.message

    # Valid IFERROR domain error trapping, 2-arg IF, and intermediate CallArg::Omitted lowering
    ok_model = f8.Model(name="ValidCallArgModel")
    ok_sheet = ok_model.add_sheet("Main")
    ok_tbl = ok_sheet.add_table("T", title="Table")
    ok_tbl.add_column("c1", role="measure", header="C1")
    ok_tbl.add_row("k1", cells={"c1": 100.0})
    ok_tbl.add_row("k2", cells={"c1": 200.0})
    ok_tbl.add_row("v1", cells={"c1": 10.0})
    ok_tbl.add_row("v2", cells={"c1": 20.0})
    ok_tbl.add_row("iferror_ok", expr="IFERROR(10 / 0, 42)")
    ok_tbl.add_row("if_2arg", expr="IF(FALSE, 10)")
    ok_tbl.add_row("pmt_skip", expr="PMT(0.05, 10, -1000, _, 1)")
    ok_tbl.add_row("pmt_explicit", expr="PMT(0.05, 10, -1000, 0, 1)")
    ok_tbl.add_row(
        "xlookup_omitted",
        expr='XLOOKUP(100, @table["k1".."k2", @col], @table["v1".."v2", @col], _, 0, 1)',
    )
    ok_tbl.add_row(
        "xlookup_2skip",
        expr='XLOOKUP(100, @table["k1".."k2", @col], @table["v1".."v2", @col], _, _, -1)',
    )

    compiled = ok_model.compile()
    assert compiled.num("T", "iferror_ok", "c1") == 42.0
    assert compiled.num("T", "pmt_skip", "c1") == compiled.num("T", "pmt_explicit", "c1")
    assert compiled.num("T", "xlookup_omitted", "c1") == 10.0
    assert compiled.num("T", "xlookup_2skip", "c1") == 10.0

    if_cell = compiled.inspect_cell("T", "if_2arg", "c1")
    assert if_cell["evaluated_value"] == {"Boolean": False}
    assert if_cell["excel_formula"] == "=IF(FALSE,10)"

    pmt_cell = compiled.inspect_cell("T", "pmt_skip", "c1")
    assert pmt_cell["excel_formula"] == "=PMT(0.05,10,-1000,,1)"

    xlookup_cell = compiled.inspect_cell("T", "xlookup_omitted", "c1")
    xlookup_excel = xlookup_cell["excel_formula"]
    assert xlookup_excel is not None
    assert xlookup_excel.startswith("=XLOOKUP(100,")
    assert xlookup_excel.endswith(",,0,1)")

    xlookup_2skip_cell = compiled.inspect_cell("T", "xlookup_2skip", "c1")
    xlookup_2skip_excel = xlookup_2skip_cell["excel_formula"]
    assert xlookup_2skip_excel is not None
    assert xlookup_2skip_excel.startswith("=XLOOKUP(100,")
    assert xlookup_2skip_excel.endswith(",,,-1)")

    # Invalid bare `_` outside direct function argument positions raises FormulaParseError
    for bad_syntax in ("_", "1 + _", "IF(TRUE, 10, _ + 1)", "IF(TRUE, 10, (_))"):
        syntax_model = f8.Model(name="InvalidUnderscoreSyntax")
        syntax_sheet = syntax_model.add_sheet("Main")
        syntax_tbl = syntax_sheet.add_table("T", title="Table")
        syntax_tbl.add_column("c1", role="measure", header="C1")
        syntax_tbl.add_row("r1", expr=bad_syntax)

        with pytest.raises(FormulaParseError):
            syntax_model.compile()


def test_scenario_18_formula_cell_blank_coercion_and_empty_boolean_comparisons() -> None:
    model = f8.Model(name="BlankCoercionModel", theme="corporate_navy")
    sheet = model.add_sheet("Main")
    tbl = sheet.add_table("T", title="Blank Coercion", currency="USD")
    tbl.add_column("c1", role="measure", header="C1")

    tbl.add_row("a", cells={"c1": None}, format="currency", decimals=1)
    tbl.add_row("k", cells={"c1": "match"})
    tbl.add_row("b", expr='@table["a", @col]', format="currency", decimals=1)
    tbl.add_row("c", expr='INDEX(@table["a".."a", @col], 1)')
    tbl.add_row("d", expr='XLOOKUP("match", @table["k".."k", @col], @table["a".."a", @col])')
    tbl.add_row("e", expr='IF(TRUE, @table["a", @col], 1)')
    tbl.add_row("f", expr='IFS(TRUE, @table["a", @col])')
    tbl.add_row("g", expr='CHOOSE(1, @table["a", @col])')
    tbl.add_row("h", expr='SWITCH(1, 1, @table["a", @col])')

    tbl.add_row("a_eq_empty", expr='(@table["a", @col] = "")')
    tbl.add_row(
        "xlookup_a_eq_empty",
        expr='(XLOOKUP("match", @table["k".."k", @col], @table["a".."a", @col]) = "")',
    )
    tbl.add_row(
        "if_guard_a",
        expr='IF(@table["a", @col] = "", "", @table["a", @col])',
        format="currency",
        decimals=1,
    )
    tbl.add_row("b_eq_empty", expr='(@table["b", @col] = "")')
    tbl.add_row("b_eq_zero", expr='(@table["b", @col] = 0)')
    tbl.add_row("count_a", expr='COUNT(@table["a", @col])')
    tbl.add_row("count_b", expr='COUNT(@table["b", @col])')
    tbl.add_row("counta_a", expr='COUNTA(@table["a", @col])')
    tbl.add_row("counta_b", expr='COUNTA(@table["b", @col])')
    tbl.add_row("avg_a", expr='AVERAGE(@table["a", @col], 10)')
    tbl.add_row("avg_b", expr='AVERAGE(@table["b", @col], 10)')
    tbl.add_row(
        "xlookup_zero_a",
        expr='XLOOKUP(0, @table["a".."a", @col], @table["k".."k", @col])',
    )
    tbl.add_row(
        "xlookup_zero_b",
        expr='XLOOKUP(0, @table["b".."b", @col], @table["k".."k", @col])',
    )
    tbl.add_row("countifs_a_empty", expr='COUNTIFS(@table["a".."a", @col], "")')
    tbl.add_row("countifs_b_empty", expr='COUNTIFS(@table["b".."b", @col], "")')

    tbl.add_row("a_eq_false", expr='@table["a", @col] = FALSE')
    tbl.add_row("a_neq_false", expr='@table["a", @col] <> FALSE')
    tbl.add_row("a_lt_false", expr='@table["a", @col] < FALSE')
    tbl.add_row("a_lte_false", expr='@table["a", @col] <= FALSE')
    tbl.add_row("a_gt_false", expr='@table["a", @col] > FALSE')
    tbl.add_row("a_gte_false", expr='@table["a", @col] >= FALSE')
    tbl.add_row("a_lt_true", expr='@table["a", @col] < TRUE')
    tbl.add_row("a_eq_true", expr='@table["a", @col] = TRUE')
    tbl.add_row("empty_str_lt_false", expr='"" < FALSE')
    tbl.add_row("switch_a_bool", expr='SWITCH(@table["a", @col], FALSE, 10, TRUE, 20, 30)')
    tbl.add_row("switch_a_zero", expr='SWITCH(@table["a", @col], "", 10, FALSE, 20, 0, 40, 50)')

    compiled = model.compile()

    # Literal null cell remains Empty
    assert compiled.value("T", "a", "c1") is None
    with pytest.raises(TypeError):
        compiled.num("T", "a", "c1")
    assert compiled.formatted("T", "a", "c1") == ""
    assert compiled.inspect_cell("T", "a", "c1")["evaluated_value"] == "Empty"

    # Formula cells referencing null materialize to 0.0 at cell boundary
    b_val = compiled.value("T", "b", "c1")
    assert isinstance(b_val, float) and b_val == 0.0 and math.copysign(1.0, b_val) == 1.0
    assert compiled.num("T", "b", "c1") == 0.0
    assert compiled.formatted("T", "b", "c1") == "$0.0"
    assert compiled.inspect_cell("T", "b", "c1")["evaluated_value"] == {"Number": 0.0}

    for row_id in ("c", "d", "e", "f", "g", "h"):
        assert compiled.num("T", row_id, "c1") == 0.0
        assert compiled.formatted("T", row_id, "c1") == "0"

    # Mid-expression blank preservation vs downstream formula cell materialization
    assert compiled.value("T", "a_eq_empty", "c1") is True
    assert compiled.value("T", "xlookup_a_eq_empty", "c1") is True
    assert compiled.value("T", "if_guard_a", "c1") == ""
    assert compiled.formatted("T", "if_guard_a", "c1") == ""
    assert compiled.value("T", "b_eq_empty", "c1") is False
    assert compiled.value("T", "b_eq_zero", "c1") is True
    assert compiled.num("T", "count_a", "c1") == 0.0
    assert compiled.num("T", "count_b", "c1") == 1.0
    assert compiled.num("T", "counta_a", "c1") == 0.0
    assert compiled.num("T", "counta_b", "c1") == 1.0
    assert compiled.num("T", "avg_a", "c1") == 10.0
    assert compiled.num("T", "avg_b", "c1") == 5.0
    assert compiled.value("T", "xlookup_zero_a", "c1") == "#N/A"
    assert compiled.value("T", "xlookup_zero_b", "c1") == "match"
    assert compiled.num("T", "countifs_a_empty", "c1") == 1.0
    assert compiled.num("T", "countifs_b_empty", "c1") == 0.0

    # Empty vs Boolean comparisons coerce Empty to FALSE; SWITCH coerces Empty operands to 0.0
    assert compiled.value("T", "a_eq_false", "c1") is True
    assert compiled.value("T", "a_neq_false", "c1") is False
    assert compiled.value("T", "a_lt_false", "c1") is False
    assert compiled.value("T", "a_lte_false", "c1") is True
    assert compiled.value("T", "a_gt_false", "c1") is False
    assert compiled.value("T", "a_gte_false", "c1") is True
    assert compiled.value("T", "a_lt_true", "c1") is True
    assert compiled.value("T", "a_eq_true", "c1") is False
    assert compiled.value("T", "empty_str_lt_false", "c1") is True
    assert compiled.num("T", "switch_a_bool", "c1") == 30.0
    assert compiled.num("T", "switch_a_zero", "c1") == 40.0
