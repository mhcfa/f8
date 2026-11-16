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
    assert f8.compile is core_compile
    assert f8.default_theme()["name"] == "corporate_navy"
    compiled = f8.compile(manifest, tables)

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
    assert cell_3args["physical_address"] == "'Operations'!C7"
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
            "gutter_col_width": 3.0,
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
        cells={"line_item": f8.Cell(expr='INDEX(@table["r1".."r2", @col], 2)')},
        expr=(
            'SUM(@table["r1".."r2", "2024E".."2025E"]) + '
            'XLOOKUP("Rev", @table["r1".."r2", "line_item"], @table["r1".."r2", @col])'
        ),
    )
    compiled = ok_model.compile()
    assert compiled.num("T", "tot", "2024E") == pytest.approx(130.0)
    assert compiled.num("T", "tot", "2025E") == pytest.approx(140.0)
    assert compiled.value("T", "tot", "line_item") == "Rev"

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
        ("IF(TRUE, 10, IFS(TRUE, 1, FALSE, 2))", "unknown function 'IFS'"),
        (
            "IF(TRUE, 10, ABS(_))",
            "argument 'number' at position 1 of 'ABS' cannot be omitted",
        ),
        ("IFS(TRUE, 1, FALSE, 2)", "unknown function 'IFS'"),
        ("IFERROR(10, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"),
        ("IFERROR(ABS(1, 2, 3), 0)", "too many arguments for 'ABS'"),
        ("IFNA(10, 0)", "unknown function 'IFNA'"),
        ('COUNTA(@table["k1".."k2", @col])', "unknown function 'COUNTA'"),
        ("DAYS(45474, 45292)", "unknown function 'DAYS'"),
        ('ISBLANK(@table["k1", @col])', "unknown function 'ISBLANK'"),
        ('ISNA(@table["k1", @col])', "unknown function 'ISNA'"),
        ("CHOOSE(1, 10, SUM())", "too few arguments for 'SUM'"),
        ("SWITCH(1, 1, 10, 20)", "unknown function 'SWITCH'"),
        ("IF(FALSE, SWITCH(1, 1, 10, 20), 0)", "unknown function 'SWITCH'"),
        (
            'SUMIFS(@table["v1".."v2", @col], @table["k1".."k2", @col], 1)',
            "unknown function 'SUMIFS'",
        ),
        ('COUNTIFS(@table["k1".."k2", @col], 1)', "unknown function 'COUNTIFS'"),
        (
            'AVERAGEIFS(@table["v1".."v2", @col], @table["k1".."k2", @col], 1)',
            "unknown function 'AVERAGEIFS'",
        ),
        (
            'MINIFS(@table["v1".."v2", @col], @table["k1".."k2", @col], 1)',
            "unknown function 'MINIFS'",
        ),
        (
            'MAXIFS(@table["v1".."v2", @col], @table["k1".."k2", @col], 1)',
            "unknown function 'MAXIFS'",
        ),
        (
            'XLOOKUP(1, @table["k1".."k2", @col], @table["v1".."v2", @col], UNKNOWN_FN())',
            "unknown function 'UNKNOWN_FN'",
        ),
        ("IF((1 / 0) > 0, UNKNOWN_FN(), 10)", "unknown function 'UNKNOWN_FN'"),
        ("IF(TRUE, 10, -(1 + UNKNOWN_FN()))", "unknown function 'UNKNOWN_FN'"),
        (
            "IF(FALSE, 10)",
            "too few arguments for 'IF'",
        ),
        (
            "IF(FALSE, 10, _)",
            "argument 'value_if_false' at position 3 of 'IF' cannot be omitted",
        ),
        (
            "IF(TRUE, 10, _)",
            "argument 'value_if_false' at position 3 of 'IF' cannot be omitted",
        ),
        (
            "IF(TRUE, 10, IF(FALSE, 1, _))",
            "argument 'value_if_false' at position 3 of 'IF' cannot be omitted",
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

    # Valid IFERROR domain error trapping, 2-arg XMATCH, and intermediate CallArg::Omitted lowering
    ok_model = f8.Model(name="ValidCallArgModel")
    ok_sheet = ok_model.add_sheet("Main")
    ok_tbl = ok_sheet.add_table("T", title="Table")
    ok_tbl.add_column("c1", role="measure", header="C1")
    ok_tbl.add_row("k1", cells={"c1": 100.0})
    ok_tbl.add_row("k2", cells={"c1": 200.0})
    ok_tbl.add_row("v1", cells={"c1": 10.0})
    ok_tbl.add_row("v2", cells={"c1": 20.0})
    ok_tbl.add_row("iferror_ok", expr="IFERROR(10 / 0, 42)")
    ok_tbl.add_row("xmatch_2arg", expr='XMATCH(200, @table["k1".."k2", @col])')
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
    assert compiled.num("T", "xmatch_2arg", "c1") == 2.0
    assert compiled.num("T", "pmt_skip", "c1") == compiled.num("T", "pmt_explicit", "c1")
    assert compiled.num("T", "xlookup_omitted", "c1") == 10.0
    assert compiled.num("T", "xlookup_2skip", "c1") == 10.0

    xmatch_cell = compiled.inspect_cell("T", "xmatch_2arg", "c1")
    assert xmatch_cell["evaluated_value"] == {"Number": 2.0}
    xmatch_excel = xmatch_cell["excel_formula"]
    assert xmatch_excel is not None
    assert xmatch_excel.startswith("=XMATCH(200,")

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


def test_scenario_18_cluster_a1_strict_types_slices_iserror_and_na() -> None:
    # 1. Grammar strictness: unary `+`, chained comparisons, invalid offsets -> FormulaParseError
    for bad_parse in (
        "+10",
        "+TRUE",
        "1 = 1 = TRUE",
        "1 < 2 < 3",
        "@table[row.prev(0), @col]",
        "@table[@row, col.next(0)]",
        "@table[@row, col.prev(5000000000)]",
    ):
        parse_model = f8.Model(name="ParseStrictModel")
        parse_tbl = parse_model.add_sheet("Main").add_table("T", title="Parse Strict")
        parse_tbl.add_column("c1", role="measure", header="C1")
        parse_tbl.add_row("r1", expr=bad_parse)
        with pytest.raises(FormulaParseError) as parse_exc:
            parse_model.compile()
        assert type(parse_exc.value) is FormulaParseError

    # 2. Type, EmptyFormulaResult, EmptyArgument, SliceInScalarContext, and Range -> exact F8Error
    for bad_f8, expected_substr in (
        ('@table["a", @col]', "formula cannot evaluate to an empty cell"),
        ('INDEX(@table["a".."a", @col], 1)', "evaluated to an empty cell"),
        (
            'XLOOKUP("match", @table["k".."k", @col], @table["a".."a", @col])',
            "evaluated to an empty cell",
        ),
        (
            'XLOOKUP("miss", @table["k".."k", @col], @table["a".."a", @col], @table["a", @col])',
            "evaluated to an empty cell",
        ),
        ('IF(TRUE, @table["a", @col], 1)', "evaluated to an empty cell"),
        ('CHOOSE(1, @table["a", @col], 1)', "evaluated to an empty cell"),
        ('IFERROR(@table["a", @col], 1)', "evaluated to an empty cell"),
        ('ISERROR(@table["a", @col])', "evaluated to an empty cell"),
        ('"5" + 1', "expected number, found text"),
        ("TRUE * 10", "expected number, found boolean"),
        ('@table["a", @col] + 1', "evaluated to an empty cell"),
        ("IF(1, 10, 20)", "expected boolean, found number"),
        ('AND(@table["a", @col], TRUE)', "evaluated to an empty cell"),
        ('100 < "1"', "cannot compare '<' operands of types number and text"),
        ('"" < FALSE', "cannot compare '<' operands of types text and boolean"),
        ('@table["a", @col] = ""', "evaluated to an empty cell"),
        ('@table["a", @col] = FALSE', "evaluated to an empty cell"),
        ('IFERROR(@table["a", @col] + 1, 0)', "evaluated to an empty cell"),
        ('@table["r1".."r2", @col]', "slice"),
        ('@table["r1".."r1", @col]', "slice"),
        ('ISERROR(@table["r1".."r1", @col])', "slice"),
        ("COUNT(10)", "expected range"),
        ('COUNT(@table["a", @col])', "evaluated to an empty cell"),
        ('SUMPRODUCT("abc")', "expected range"),
        ("SWITCH(1, 1, 10, 20)", "unknown function 'SWITCH'"),
    ):
        err_model = f8.Model(name="StrictErrorModel")
        err_tbl = err_model.add_sheet("Main").add_table("T", title="Strict Error")
        err_tbl.add_column("c1", role="measure", header="C1")
        err_tbl.add_row("a", cells={"c1": None})
        err_tbl.add_row("k", cells={"c1": "match"})
        err_tbl.add_row("r1", cells={"c1": 10.0})
        err_tbl.add_row("r2", cells={"c1": 20.0})
        err_tbl.add_row("target", expr=bad_f8)
        with pytest.raises(F8Error) as f8_exc:
            err_model.compile()
        assert type(f8_exc.value) is F8Error
        assert not isinstance(f8_exc.value, TypeError)
        assert expected_substr in f8_exc.value.message

    # 3. Valid ISERROR, NA(), dormant empty branches, parenthesized comparisons, sparse Slice AND
    model = f8.Model(name="IserrorAndNaModel", theme="corporate_navy")
    sheet = model.add_sheet("Main")
    tbl = sheet.add_table("T", title="Blank Guard & Information", currency="USD")
    tbl.add_column("c1", role="measure", header="C1")

    tbl.add_row("a", cells={"c1": None}, format="currency", decimals=1)
    tbl.add_row("k", cells={"c1": "match"})
    tbl.add_row("f1", cells={"c1": None})
    tbl.add_row("f2", cells={"c1": True})
    tbl.add_row(
        "b",
        expr='IF(FALSE, @table["a", @col], 0)',
        format="currency",
        decimals=1,
    )
    tbl.add_row(
        "if_guard_a",
        expr='IF(FALSE, @table["a", @col], "")',
        format="currency",
        decimals=1,
    )
    tbl.add_row("ie_guarded", expr='ISERROR(@table["b", @col])')
    tbl.add_row("ie_empty_str", expr='ISERROR("")')
    tbl.add_row("ie_div0", expr="ISERROR(1 / 0)")
    tbl.add_row("na_cell", expr="NA()")
    tbl.add_row("na_trapped", expr="IFERROR(IF(FALSE, 10, NA()), 0)")
    tbl.add_row("paren_cmp", expr="(1 = 1) = TRUE")
    tbl.add_row("and_sparse", expr='AND(@table["f1".."f2", @col])')
    tbl.add_row("b_eq_zero", expr='(@table["b", @col] = 0)')
    tbl.add_row("count_a", expr='COUNT(@table["a".."a", @col])')
    tbl.add_row("count_b", expr='COUNT(@table["b", @col])')

    compiled = model.compile()

    # Literal null cell remains Empty; .num() still raises host TypeError
    assert compiled.value("T", "a", "c1") is None
    with pytest.raises(TypeError):
        compiled.num("T", "a", "c1")
    assert compiled.formatted("T", "a", "c1") == ""
    assert compiled.inspect_cell("T", "a", "c1")["evaluated_value"] == "Empty"

    # Guarded formula cell evaluates to +0.0
    b_val = compiled.value("T", "b", "c1")
    assert isinstance(b_val, float) and b_val == 0.0 and math.copysign(1.0, b_val) == 1.0
    assert compiled.num("T", "b", "c1") == 0.0
    assert compiled.formatted("T", "b", "c1") == "$0.0"

    assert compiled.value("T", "if_guard_a", "c1") == ""
    assert compiled.formatted("T", "if_guard_a", "c1") == ""
    assert compiled.value("T", "ie_guarded", "c1") is False
    assert compiled.value("T", "ie_empty_str", "c1") is False
    assert compiled.value("T", "ie_div0", "c1") is True

    na_info = compiled.inspect_cell("T", "na_cell", "c1")
    assert compiled.value("T", "na_cell", "c1") == "#N/A"
    assert na_info["excel_formula"] == "=NA()"
    with pytest.raises(TypeError):
        compiled.num("T", "na_cell", "c1")
    assert compiled.num("T", "na_trapped", "c1") == 0.0

    assert compiled.value("T", "paren_cmp", "c1") is True
    assert compiled.value("T", "and_sparse", "c1") is True
    assert compiled.value("T", "b_eq_zero", "c1") is True
    assert compiled.num("T", "count_a", "c1") == 0.0
    assert compiled.num("T", "count_b", "c1") == 1.0


def test_scenario_19_neg_zero_normalization() -> None:
    model = f8.Model(name="NegZeroModel")
    tbl = model.add_sheet("Main").add_table("T", title="Neg Zero")
    tbl.add_column("num", role="measure", header="Number")

    tbl.add_row("r1", cells={"num": 42.0})
    tbl.add_row("r2", cells={"num": -0.0})

    compiled = model.compile()
    neg_zero_val = compiled.value("T", "r2", "num")
    assert isinstance(neg_zero_val, float) and math.copysign(1.0, neg_zero_val) == 1.0


def test_scenario_20_cluster_a2_monomorphic_branches_dormant_validation_and_lookup_strictness() -> (
    None
):
    # 1. Stage A & Stage B rejections raise exact F8Error (never host TypeError or 'dormant')
    for bad_a2 in (
        "IF(TRUE, 10)",
        "SWITCH(1, 1, 10, 20)",
        "IF(FALSE, SWITCH(1, 1, 10, 20), 0)",
        'IF(TRUE, 10, "NM")',
        'IFERROR(10 / 0, "N/A")',
        'CHOOSE(1, 10, "NM")',
        'IF(FALSE, IF(TRUE, "a", "b") = IF(FALSE, 1, 2), TRUE)',
        'ROUND(10, IF(TRUE, "a", "b"))',
        'ROUND(10, IF(TRUE, 2, "bad"))',
        '1 + INDEX(@table["txt1".."txt2", @col], @table["dyn_oob_idx", @col])',
        '1 = XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col])',
        'ABS(XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col]))',
        (
            'IF(XLOOKUP("Missing", @table["cat1".."cat2", @col], '
            '@table["txt1".."txt2", @col]), 10, 20)'
        ),
        'XMATCH(IF(FALSE, "Alpha", NA()), @table["num1".."num2", @col])',
        (
            'XMATCH(INDEX(@table["txt1".."txt2", @col], @table["dyn_oob_idx", @col]), '
            '@table["num1".."num2", @col])'
        ),
        (
            'XLOOKUP(XLOOKUP("Missing", @table["cat1".."cat2", @col], '
            '@table["txt1".."txt2", @col]), '
            '@table["num1".."num2", @col], @table["num1".."num2", @col])'
        ),
        '1 + XLOOKUP("Beta", @table["cat1".."cat2", @col], @table["num1".."num2", @col], "NM")',
        'IFERROR(@table["null_cell", @col], 1)',
        'IFERROR(1 / 0, @table["null_cell", @col])',
        'ISERROR(IFERROR(@table["null_cell", @col], 1))',
        'IF(TRUE, @table["num1", @col], @table["txt1", @col])',
        'IF(@table["flag_false", @col], @table["txt1", @col] + 5, 0)',
        'IF(@table["flag_false", @col], @table["num1", @col] < @table["txt1", @col], FALSE)',
        'IF(TRUE, 10, XLOOKUP("Beta", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col]))',
        'IF(TRUE, 10, INDEX(@table["txt1".."txt2", @col], @table["dyn_idx", @col]))',
        'IFERROR(INDEX(@table["txt1".."txt2", @col], @table["dyn_oob_idx", @col]), 10)',
        (
            'IFERROR(XLOOKUP("Missing", @table["cat1".."cat2", @col], '
            '@table["txt1".."txt2", @col]), 10)'
        ),
        'IFERROR(@table["err_txt", @col], 10)',
        'IFERROR(@table["err_oob_txt", @col], 10)',
        '1 + @table["err_txt", @col]',
        '@table["err_txt", @col] = 10',
        'IFERROR(@table["err_div0", @col], "fallback")',
        'AND(@table["c1".."err_txt", @col])',
        'AND(@table["err_txt".."err_txt", @col])',
        'IF(FALSE, AND(@table["c1".."err_txt", @col]), FALSE)',
        'XLOOKUP(10, @table["num_adj".."err_txt", @col], @table["txt1".."txt2", @col])',
        'XMATCH(10, @table["err_txt".."err_txt", @col])',
        'XMATCH(10, @table["err_txt".."err_txt", @col], 0, 2)',
        'INDEX(@table["num_adj".."err_txt", @col], 1)',
        'XLOOKUP("Alpha", @table["cat1".."cat2", @col], @table["num_adj".."err_txt", @col])',
        'IFERROR(INDEX(@table["err_div0".."err_div0_2", @col], 1), "fallback")',
        'XMATCH("Alpha", @table["txt1".."txt2", @col], "abc")',
        'INDEX(@table["num1".."txt1", @col], 1)',
    ):
        err_model = f8.Model(name="ClusterA2ErrorModel")
        err_tbl = err_model.add_sheet("Main").add_table("T", title="Cluster A2 Error")
        err_tbl.add_column("c1", role="measure", header="C1")
        err_tbl.add_row("null_cell", cells={"c1": None})
        err_tbl.add_row("num1", cells={"c1": 10.0})
        err_tbl.add_row("num2", cells={"c1": 20.0})
        err_tbl.add_row("txt1", cells={"c1": "Alpha"})
        err_tbl.add_row("txt2", cells={"c1": "Beta"})
        err_tbl.add_row("cat1", cells={"c1": "Alpha"})
        err_tbl.add_row("cat2", cells={"c1": "Beta"})
        err_tbl.add_row("flag_false", cells={"c1": False})
        err_tbl.add_row("flt_cell", cells={"c1": 2.5})
        err_tbl.add_row("dyn_idx", expr="1 + 0")
        err_tbl.add_row("dyn_oob_idx", expr="5 + 0")
        if '"c1".."err_txt"' in bad_a2:
            err_tbl.add_row("num_adj", cells={"c1": 10.0})
            err_tbl.add_row("c1", cells={"c1": True})
        else:
            err_tbl.add_row("c1", cells={"c1": True})
            err_tbl.add_row("num_adj", cells={"c1": 10.0})
        err_tbl.add_row(
            "err_txt",
            expr='XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col])',
        )
        err_tbl.add_row(
            "err_oob_txt",
            expr='INDEX(@table["txt1".."txt2", @col], @table["dyn_oob_idx", @col])',
        )
        err_tbl.add_row("err_div0", expr="1 / 0")
        err_tbl.add_row("err_div0_2", expr="2 / 0")
        err_tbl.add_row("unsort1", cells={"c1": 10.0})
        err_tbl.add_row("unsort2", cells={"c1": 30.0})
        err_tbl.add_row("unsort3", cells={"c1": 20.0})
        err_tbl.add_row("errk1", cells={"c1": 10.0})
        err_tbl.add_row("errk2", expr="1 / 0")
        err_tbl.add_row("target", expr=bad_a2)
        with pytest.raises(F8Error) as a2_exc:
            err_model.compile()
        assert type(a2_exc.value) is F8Error
        assert not isinstance(a2_exc.value, TypeError)
        assert "dormant" not in a2_exc.value.message

    # 2. Valid monomorphic branches, taken integer arm, error provenance, and binary search
    ok_model = f8.Model(name="ClusterA2ValidModel")
    ok_tbl = ok_model.add_sheet("Main").add_table("T", title="Cluster A2 Valid")
    ok_tbl.add_column("c1", role="measure", header="C1")
    ok_tbl.add_row("null_cell", cells={"c1": None})
    ok_tbl.add_row("flt_cell", cells={"c1": 2.5})
    ok_tbl.add_row("cat1", cells={"c1": "Alpha"})
    ok_tbl.add_row("cat2", cells={"c1": "Beta"})
    ok_tbl.add_row("txt1", cells={"c1": "one"})
    ok_tbl.add_row("txt2", cells={"c1": "two"})
    ok_tbl.add_row("sp1", cells={"c1": 10.0})
    ok_tbl.add_row("sp2", cells={"c1": None})
    ok_tbl.add_row("sp3", cells={"c1": 30.0})
    ok_tbl.add_row("asc1", cells={"c1": 10.0})
    ok_tbl.add_row("asc2", cells={"c1": 20.0})
    ok_tbl.add_row("asc3", cells={"c1": 30.0})
    ok_tbl.add_row("asc4", cells={"c1": 40.0})
    ok_tbl.add_row("desc1", cells={"c1": 40.0})
    ok_tbl.add_row("desc2", cells={"c1": 30.0})
    ok_tbl.add_row("desc3", cells={"c1": 20.0})
    ok_tbl.add_row("desc4", cells={"c1": 10.0})
    ok_tbl.add_row("if_text", expr='IF(TRUE, "PASS", "FAIL")')
    ok_tbl.add_row("if_na_bottom", expr="IF(10 > 0, 100 / 10, NA())")
    ok_tbl.add_row("round_taken_lit", expr="ROUND(10, IF(TRUE, 2, 2.5))")
    ok_tbl.add_row("round_taken_cell", expr='ROUND(10, IF(TRUE, 2, @table["flt_cell", @col]))')
    ok_tbl.add_row("round_guard_cell", expr='IF(FALSE, ROUND(10, @table["flt_cell", @col]), 0)')
    ok_tbl.add_row(
        "helper_guard",
        expr='IF(TRUE, 0, @table["null_cell", @col] + 5)',
    )
    ok_tbl.add_row("index_sparse", expr='INDEX(@table["sp1".."sp3", @col], 3)')
    ok_tbl.add_row("xmatch_bin_asc", expr='XMATCH(30, @table["asc1".."asc4", @col], 0, 2)')
    ok_tbl.add_row(
        "xlookup_bin_desc",
        expr=(
            'XLOOKUP(25, @table["desc1".."desc4", @col], @table["asc1".."asc4", @col], -1, -1, -2)'
        ),
    )
    ok_tbl.add_row("add_na", expr="1 + IF(FALSE, 10, NA())")
    ok_tbl.add_row("xmatch_na", expr='XMATCH(NA(), @table["asc1".."asc2", @col])')
    ok_tbl.add_row(
        "err_txt",
        expr='XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col])',
    )
    ok_tbl.add_row("err_div0", expr="1 / 0")
    ok_tbl.add_row("err_div0_2", expr="2 / 0")
    ok_tbl.add_row("c1", cells={"c1": True})
    ok_tbl.add_row("err_na", expr="NA()")
    ok_tbl.add_row("ok_iferr_txt", expr='IFERROR(@table["err_txt", @col], "fallback")')
    ok_tbl.add_row("ok_iferr_div0", expr='IFERROR(@table["err_div0", @col], 0)')
    ok_tbl.add_row(
        "ok_iferr_idx_div0",
        expr='IFERROR(INDEX(@table["err_div0".."err_div0_2", @col], 1), 0)',
    )
    ok_tbl.add_row("ok_add_div0", expr='1 + @table["err_div0", @col]')
    ok_tbl.add_row("ok_iferr_na_num", expr='IFERROR(@table["err_na", @col], 10)')
    ok_tbl.add_row("ok_iferr_na_txt", expr='IFERROR(@table["err_na", @col], "fallback")')
    ok_tbl.add_row("ok_and_na_slice", expr='AND(@table["c1".."err_na", @col])')

    compiled = ok_model.compile()
    assert compiled.value("T", "if_text", "c1") == "PASS"
    assert compiled.num("T", "if_na_bottom", "c1") == 10.0
    assert compiled.num("T", "round_taken_lit", "c1") == 10.0
    assert compiled.num("T", "round_taken_cell", "c1") == 10.0
    assert compiled.num("T", "round_guard_cell", "c1") == 0.0
    assert compiled.num("T", "helper_guard", "c1") == 0.0
    assert compiled.num("T", "index_sparse", "c1") == 30.0
    assert compiled.num("T", "xmatch_bin_asc", "c1") == 3.0
    assert compiled.num("T", "xlookup_bin_desc", "c1") == 30.0
    assert compiled.value("T", "add_na", "c1") == "#N/A"
    assert compiled.value("T", "xmatch_na", "c1") == "#N/A"
    assert compiled.value("T", "ok_iferr_txt", "c1") == "fallback"
    assert compiled.num("T", "ok_iferr_div0", "c1") == 0.0
    assert compiled.num("T", "ok_iferr_idx_div0", "c1") == 0.0
    assert compiled.value("T", "ok_add_div0", "c1") == "#DIV/0!"
    assert compiled.num("T", "ok_iferr_na_num", "c1") == 10.0
    assert compiled.value("T", "ok_iferr_na_txt", "c1") == "fallback"
    assert compiled.value("T", "ok_and_na_slice", "c1") == "#N/A"


def test_scenario_21_cluster_b_aggregation_roles_sparse_slices_and_date_domains() -> None:
    # 1. Stage A & Stage B rejections raise exact F8Error (never host TypeError or 'dormant')
    bad_cluster_b: tuple[tuple[str, str], ...] = (
        # Scenario 1: Descriptor references in numeric/financial aggregations
        ('SUM(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"),
        ('SUM(@table["r1", "line_item"])', "cannot reference descriptor column"),
        ('AVERAGE(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"),
        ('MIN(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"),
        ('MAX(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"),
        ('MEDIAN(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"),
        ('COUNT(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"),
        (
            'SUMPRODUCT(@table["r1".."r3", "line_item"], @table["r1".."r3", "c1"])',
            "cannot reference descriptor column",
        ),
        ('NPV(0.1, @table["r1".."r3", "line_item"])', "cannot reference descriptor column"),
        ('IRR(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"),
        (
            'XNPV(0.1, @table["r1".."r3", "line_item"], @table["d1".."d3", "c1"])',
            "cannot reference descriptor column",
        ),
        (
            'XIRR(@table["cf1".."cf3", "c1"], @table["r1".."r3", "line_item"])',
            "cannot reference descriptor column",
        ),
        (
            'IF(FALSE, SUM(@table["r1".."r3", "line_item"]), 0)',
            "cannot reference descriptor column",
        ),
        # Scenario 2: Non-numeric cells, SingleCell Empty refs, and NPV/IRR/XNPV/XIRR Empty holes
        ('SUM(@table["txt1".."txt2", "c1"])', "expected number, found text"),
        ('SUM(@table["bool1".."bool2", "c1"])', "expected number, found boolean"),
        ("SUM(TRUE, 5)", "expected number, found boolean"),
        ('SUM(@table["null1", "c1"])', "evaluated to an empty cell"),
        ('AVERAGE(@table["r1", "c1"], @table["null1", "c1"])', "evaluated to an empty cell"),
        ('MIN(@table["txt1".."txt2", "c1"])', "expected number, found text"),
        ('MAX(@table["null1", "c1"])', "evaluated to an empty cell"),
        ('MEDIAN(@table["bool1".."bool2", "c1"])', "expected number, found boolean"),
        ('COUNT(@table["txt1".."txt2", "c1"])', "expected number, found text"),
        ('COUNT(@table["null1", "c1"])', "evaluated to an empty cell"),
        (
            'SUMPRODUCT(@table["txt1".."txt2", "c1"], @table["r1".."r2", "c1"])',
            "expected number, found text",
        ),
        (
            'SUMPRODUCT(@table["bool1".."bool2", "c1"], @table["r1".."r2", "c1"])',
            "expected number, found boolean",
        ),
        ('IFERROR(SUM(@table["null1", "c1"]), 0)', "evaluated to an empty cell"),
        ('NPV(0.1, @table["cf_hole1".."cf_hole3", "c1"])', "evaluated to an empty cell"),
        ('IRR(@table["cf_hole1".."cf_hole3", "c1"])', "evaluated to an empty cell"),
        (
            'XNPV(0.1, @table["cf_hole1".."cf_hole3", "c1"], @table["d1".."d3", "c1"])',
            "evaluated to an empty cell",
        ),
        (
            'XIRR(@table["cf1".."cf3", "c1"], @table["d_hole1".."d_hole3", "c1"])',
            "evaluated to an empty cell",
        ),
        # Scenario 5: ShapePolicy order
        (
            'XNPV(-2.0, @table["r1".."r2", "c1".."c2"], @table["r1".."r2", "c1".."c2"])',
            "requires a 1D slice, found 2x2",
        ),
        (
            'XNPV(0.1, @table["cf1".."cf3", "c1"], @table["zero_serial".."zero_serial", "c1"])',
            "length mismatch in 'XNPV'",
        ),
    )

    for formula, expected_substr in bad_cluster_b:
        err_model = f8.Model(name="ClusterBErrorModel")
        err_tbl = err_model.add_sheet("Main").add_table("T", title="Cluster B Error")
        err_tbl.add_column("line_item", role="descriptor", header="Item")
        err_tbl.add_column("c1", role="measure", header="C1")
        err_tbl.add_column("c2", role="measure", header="C2")
        err_tbl.add_row("r1", cells={"line_item": "Rev", "c1": 10.0, "c2": 1.0})
        err_tbl.add_row("r2", cells={"line_item": "Cost", "c1": 20.0, "c2": 2.0})
        err_tbl.add_row("r3", cells={"line_item": "OpEx", "c1": 30.0, "c2": 3.0})
        err_tbl.add_row("txt1", cells={"line_item": "T1", "c1": "Alpha", "c2": 0.0})
        err_tbl.add_row("txt2", cells={"line_item": "T2", "c1": "Beta", "c2": 0.0})
        err_tbl.add_row("bool1", cells={"line_item": "B1", "c1": True, "c2": 0.0})
        err_tbl.add_row("bool2", cells={"line_item": "B2", "c1": False, "c2": 0.0})
        err_tbl.add_row("null1", cells={"line_item": "N1", "c1": None, "c2": 0.0})
        err_tbl.add_row("cf1", cells={"line_item": "CF1", "c1": -1000.0, "c2": 0.0})
        err_tbl.add_row("cf2", cells={"line_item": "CF2", "c1": 600.0, "c2": 0.0})
        err_tbl.add_row("cf3", cells={"line_item": "CF3", "c1": 600.0, "c2": 0.0})
        err_tbl.add_row("cf_hole1", cells={"line_item": "CFH1", "c1": -1000.0, "c2": 0.0})
        err_tbl.add_row("cf_hole2", cells={"line_item": "CFH2", "c1": None, "c2": 0.0})
        err_tbl.add_row("cf_hole3", cells={"line_item": "CFH3", "c1": 1200.0, "c2": 0.0})
        err_tbl.add_row("d1", cells={"line_item": "D1", "c1": 45292.0, "c2": 0.0})
        err_tbl.add_row("d2", cells={"line_item": "D2", "c1": 45658.0, "c2": 0.0})
        err_tbl.add_row("d3", cells={"line_item": "D3", "c1": 46023.0, "c2": 0.0})
        err_tbl.add_row("d_hole1", cells={"line_item": "DH1", "c1": 45292.0, "c2": 0.0})
        err_tbl.add_row("d_hole2", cells={"line_item": "DH2", "c1": None, "c2": 0.0})
        err_tbl.add_row("d_hole3", cells={"line_item": "DH3", "c1": 46023.0, "c2": 0.0})
        err_tbl.add_row("yr_ok", cells={"line_item": "YOK", "c1": 2024.0, "c2": 0.0})
        err_tbl.add_row("yr_24", cells={"line_item": "Y24", "c1": 24.0, "c2": 0.0})
        err_tbl.add_row("day_29", cells={"line_item": "D29", "c1": 29.0, "c2": 0.0})
        err_tbl.add_row("zero_serial", cells={"line_item": "ZS", "c1": 0.0, "c2": 0.0})
        err_tbl.add_row(
            "target",
            cells={"line_item": "Target", "c1": f8.Cell(expr=formula), "c2": 0.0},
        )
        with pytest.raises(F8Error) as b_exc:
            err_model.compile()
        assert type(b_exc.value) is F8Error
        assert not isinstance(b_exc.value, TypeError)
        assert "dormant" not in b_exc.value.message
        assert expected_substr in b_exc.value.message

    # 2. Valid descriptor lookups, sparse slices, COUNT error skipping, overflow #NUM!, date parity
    ok_model = f8.Model(name="ClusterBValidModel", theme="corporate_navy")
    ok_tbl = ok_model.add_sheet("Main").add_table("T", title="Cluster B Valid")
    ok_tbl.add_column("line_item", role="descriptor", header="Item")
    ok_tbl.add_column("c1", role="measure", header="C1")
    ok_tbl.add_column("w", role="measure", header="Weight")

    ok_tbl.add_row("r1", cells={"line_item": "Rev", "c1": 10.0, "w": 1.0})
    ok_tbl.add_row("r2", cells={"line_item": "Cost", "c1": 20.0, "w": 2.0})
    ok_tbl.add_row("r3", cells={"line_item": "OpEx", "c1": 30.0, "w": 3.0})

    ok_tbl.add_row("sp1", cells={"line_item": "S1", "c1": None, "w": 5.0})
    ok_tbl.add_row("sp2", cells={"line_item": "S2", "c1": 0.0, "w": 2.0})
    ok_tbl.add_row("sp3", cells={"line_item": "S3", "c1": 20.0, "w": 4.0})
    ok_tbl.add_row("sp4", cells={"line_item": "S4", "c1": None, "w": 9.0})

    ok_tbl.add_row("an1", cells={"line_item": "AN1", "c1": None, "w": 0.0})
    ok_tbl.add_row("an2", cells={"line_item": "AN2", "c1": None, "w": 0.0})

    ok_tbl.add_row("er1", cells={"line_item": "E1", "c1": 10.0, "w": 0.0})
    ok_tbl.add_row("er2", cells={"line_item": "E2", "c1": f8.Cell(expr="1 / 0"), "w": 0.0})
    ok_tbl.add_row("er3", cells={"line_item": "E3", "c1": 20.0, "w": 0.0})

    ok_tbl.add_row("big1", cells={"line_item": "BG1", "c1": 1e308, "w": 1e200})
    ok_tbl.add_row("big2", cells={"line_item": "BG2", "c1": 1e308, "w": 1e200})

    ok_tbl.add_row("yr_ok", cells={"line_item": "Y", "c1": 2024.0, "w": 0.0})
    ok_tbl.add_row("mo_feb", cells={"line_item": "M", "c1": 2.0, "w": 0.0})
    ok_tbl.add_row("day_29", cells={"line_item": "D", "c1": 29.0, "w": 0.0})
    ok_tbl.add_row("type_1", cells={"line_item": "T", "c1": 1.0, "w": 0.0})
    ok_tbl.add_row("basis_3", cells={"line_item": "B", "c1": 3.0, "w": 0.0})
    ok_tbl.add_row("zero_date", cells={"line_item": "ZD", "c1": 0.0, "w": 0.0}, format="date")

    def add_check(row_id: str, formula: str) -> None:
        ok_tbl.add_row(
            row_id,
            cells={"line_item": row_id, "c1": f8.Cell(expr=formula), "w": 0.0},
        )

    add_check("desc_xmatch", 'XMATCH("OpEx", @table["r1".."r3", "line_item"])')
    add_check(
        "desc_xlookup", 'XLOOKUP("Rev", @table["r1".."r3", "line_item"], @table["r1".."r3", "c1"])'
    )
    add_check("desc_index", 'INDEX(@table["r1".."r3", "line_item"], 2)')
    add_check("guard_sum", 'IF(TRUE, 0, SUM(@table["sp1", "c1"]))')

    add_check("sum_sparse", 'SUM(@table["sp1".."sp4", "c1"])')
    add_check("avg_sparse", 'AVERAGE(@table["sp1".."sp4", "c1"])')
    add_check("cnt_sparse", 'COUNT(@table["sp1".."sp4", "c1"])')
    add_check("sp_sparse", 'SUMPRODUCT(@table["sp1".."sp4", "c1"], @table["sp1".."sp4", "w"])')

    add_check("sum_all_null", 'SUM(@table["an1".."an2", "c1"])')
    add_check("avg_all_null", 'AVERAGE(@table["an1".."an2", "c1"])')
    add_check("med_all_null", 'MEDIAN(@table["an1".."an2", "c1"])')
    add_check("min_all_null", 'MIN(@table["an1".."an2", "c1"])')
    add_check("max_all_null", 'MAX(@table["an1".."an2", "c1"])')
    add_check("cnt_all_null", 'COUNT(@table["an1".."an2", "c1"])')

    add_check("cnt_err_slice", 'COUNT(@table["er1".."er3", "c1"])')
    add_check("sum_err_slice", 'SUM(@table["er1".."er3", "c1"])')

    add_check("sum_ovf", 'SUM(@table["big1".."big2", "c1"])')
    add_check("sp_ovf", 'SUMPRODUCT(@table["big1".."big1", "w"], @table["big2".."big2", "w"])')
    add_check("max_big", 'MAX(@table["big1".."big2", "c1"])')

    add_check("date_1900_leap", "DATE(1900, 2, 29)")
    add_check(
        "date_dyn",
        'DATE(@table["yr_ok", "c1"], @table["mo_feb", "c1"], @table["day_29", "c1"])',
    )
    add_check("pmt_dyn", 'PMT(0.05, 10, -1000, 0, @table["type_1", "c1"])')
    add_check("yf_dyn", 'YEARFRAC(45292, 45658, @table["basis_3", "c1"])')
    add_check("yr_min", "YEAR(1)")
    add_check("yr_max", "YEAR(2958465.9)")
    add_check(
        "yr_guard",
        'IF(@table["zero_date", "c1"] >= 1, YEAR(@table["zero_date", "c1"]), 2024)',
    )
    add_check("edate_udf", "EDATE(DATE(1900, 1, 15), -2)")
    add_check("eomonth_ovf", "EOMONTH(DATE(9999, 12, 1), 2)")
    add_check("yr_edate_bubble", "YEAR(EDATE(DATE(1900, 1, 15), -2))")
    add_check("round_float", "ROUND(10, 2.5)")
    add_check("date_yr24", "DATE(24, 1, 1)")
    add_check("idx_oob", 'INDEX(@table["r1".."r3", "c1"], 5)')
    add_check("choose_zero", "CHOOSE(0, 10, 20)")
    add_check("xmatch_bad_mode", 'XMATCH(10, @table["r1".."r3", "c1"], 9)')

    compiled = ok_model.compile()
    assert compiled.num("T", "desc_xmatch", "c1") == 3.0
    assert compiled.num("T", "desc_xlookup", "c1") == 10.0
    assert compiled.value("T", "desc_index", "c1") == "Cost"
    assert compiled.num("T", "guard_sum", "c1") == 0.0

    assert compiled.num("T", "sum_sparse", "c1") == 20.0
    assert compiled.num("T", "avg_sparse", "c1") == 10.0
    assert compiled.num("T", "cnt_sparse", "c1") == 2.0
    assert compiled.num("T", "sp_sparse", "c1") == 80.0

    assert compiled.num("T", "sum_all_null", "c1") == 0.0
    assert compiled.value("T", "avg_all_null", "c1") == "#DIV/0!"
    assert compiled.value("T", "med_all_null", "c1") == "#NUM!"
    assert compiled.num("T", "min_all_null", "c1") == 0.0
    assert compiled.num("T", "max_all_null", "c1") == 0.0
    assert compiled.num("T", "cnt_all_null", "c1") == 0.0

    assert compiled.num("T", "cnt_err_slice", "c1") == 2.0
    assert compiled.value("T", "sum_err_slice", "c1") == "#DIV/0!"

    assert compiled.value("T", "sum_ovf", "c1") == "#NUM!"
    assert compiled.value("T", "sp_ovf", "c1") == "#NUM!"
    assert compiled.num("T", "max_big", "c1") == 1e308

    assert compiled.num("T", "date_1900_leap", "c1") == 60.0
    assert compiled.num("T", "date_dyn", "c1") == 45351.0
    assert compiled.num("T", "pmt_dyn", "c1") > 120.0
    assert compiled.num("T", "yf_dyn", "c1") == pytest.approx(366.0 / 365.0)
    assert compiled.num("T", "yr_min", "c1") == 1900.0
    assert compiled.num("T", "yr_max", "c1") == 9999.0
    assert compiled.num("T", "yr_guard", "c1") == 2024.0
    assert compiled.formatted("T", "zero_date", "c1") == "1900-01-00"
    assert compiled.value("T", "edate_udf", "c1") == "#NUM!"
    assert compiled.value("T", "eomonth_ovf", "c1") == "#NUM!"
    assert compiled.value("T", "yr_edate_bubble", "c1") == "#NUM!"
    assert compiled.value("T", "round_float", "c1") == "#NUM!"
    assert compiled.value("T", "date_yr24", "c1") == "#NUM!"
    assert compiled.value("T", "idx_oob", "c1") == "#REF!"
    assert compiled.value("T", "choose_zero", "c1") == "#VALUE!"
    assert compiled.value("T", "xmatch_bad_mode", "c1") == "#VALUE!"


def test_scenario_22_error_introspection() -> None:
    # 1. Dotted function grammar rejected with FormulaParseError
    for bad_parse in (
        "ERROR.TYPE(1 / 0)",
        "ERROR . TYPE(1 / 0)",
        "error.type(1 / 0)",
    ):
        m = f8.Model(name="ParseCheck")
        t = m.add_sheet("Main").add_table("T", title="Parse")
        t.add_column("c1", role="measure", header="Val")
        t.add_row("bad", expr=bad_parse)
        with pytest.raises(FormulaParseError):
            m.compile()

    # 2. Compile-time type error and Empty argument inside ISERROR raise F8Error
    for bad_iserror in ('ISERROR("5" + 1)', 'ISERROR(@table["n", "c1"])'):
        m_err = f8.Model(name="IserrorTypeCheck")
        t_err = m_err.add_sheet("Main").add_table("T", title="Type Check")
        t_err.add_column("c1", role="measure", header="Val")
        t_err.add_row("n", cells={"c1": None})
        t_err.add_row("target", expr=bad_iserror)
        with pytest.raises(F8Error) as f8_exc:
            m_err.compile()
        assert type(f8_exc.value) is F8Error

    # 3. Error introspection function (ISERROR)
    ok_model = f8.Model(name="ErrorIntrospectionOk")
    ok_tbl = ok_model.add_sheet("Main").add_table("T", title="Error Introspection")
    for col_id in ("cat", "res"):
        ok_tbl.add_column(col_id, role="measure", header=col_id)

    ok_tbl.add_row("r1", cells={"cat": "Alpha", "res": 0.0})
    ok_tbl.add_row("r2", cells={"cat": "Beta", "res": 0.0})
    ok_tbl.add_row("r3", cells={"cat": "Alpha", "res": 0.0})
    ok_tbl.add_row("r4", cells={"cat": "May", "res": 0.0})

    def add_res(row_id: str, formula: str) -> None:
        ok_tbl.add_row(
            row_id,
            cells={
                "cat": None,
                "res": f8.Cell(expr=formula),
            },
        )

    add_res("iserror_div0", "ISERROR(1 / 0)")
    add_res("iserror_na", 'ISERROR(XMATCH("Missing", @table["r1".."r4", "cat"]))')
    add_res("iserror_non_err", "ISERROR(42)")

    compiled = ok_model.compile()
    assert compiled.value("T", "iserror_div0", "res") is True
    assert compiled.value("T", "iserror_na", "res") is True
    assert compiled.value("T", "iserror_non_err", "res") is False
    iserr_cell = compiled.inspect_cell("T", "iserror_div0", "res")
    assert "ISERROR(" in (iserr_cell["excel_formula"] or "")
