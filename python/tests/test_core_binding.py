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
            "table_subtitle_text": "#4B5563",
            "super_header_fill": "#ced4da",
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
            "shade_accent_fill": "#EFF6FF",
            "shade_alt_fill": "#F1F5F9",
            "highlight_fill": "#FEF3C7",
            "highlight_text": "#000000",
            "gridline_color": "#E5E7EB",
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
        "table_subtitle_text": "#4B5563",
        "super_header_fill": "#ced4da",
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
        "shade_accent_fill": "#EFF6FF",
        "shade_alt_fill": "#F1F5F9",
        "highlight_fill": "#FEF3C7",
        "highlight_text": "#000000",
        "gridline_color": "#E5E7EB",
        "border_color": "#000000",
        "total_border_color": "#1B365D",
    }
    bad_theme: dict[str, object] = {**custom_theme, "palette": palette_dict}
    bad_manifest: dict[str, object] = {**manifest, "theme": bad_theme}
    with pytest.raises(SpecificationError, match="table_title_fill"):
        core_compile(bad_manifest, tables)
