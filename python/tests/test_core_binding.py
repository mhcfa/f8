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
    compiled = f8.compile(manifest, tables)

    buffer = compiled.emit_workbook({"values_only": True})
    assert isinstance(buffer, bytes)
    assert buffer.startswith(b"PK\x03\x04")
    assert len(buffer) > 1000


def test_scenario_3_cell_inspection_flexible_coordinates(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = f8.compile(manifest, tables)

    cell_3args = compiled.inspect_cell("OPERATIONS", "gross_profit", "2024E")
    cell_1arg = compiled.inspect_cell('OPERATIONS["gross_profit", "2024E"]')

    assert cell_3args == cell_1arg
    assert cell_3args["physical_address"] == "Operations!C7"
    assert cell_3args["evaluated_value"] == {"Number": 60.0}
    assert cell_3args["formatted_value"] == "$ 60.0"
    assert cell_3args["is_formula"] is True
    assert cell_3args["excel_formula"] is not None
    assert len(cell_3args["precedents"]) == 2


def test_scenario_4_dependency_tree_direct_and_recursive(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = f8.compile(manifest, tables)

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
    compiled = f8.compile(manifest, tables)

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
    rev_cell = table_insp["cells"]['OPERATIONS["revenue", "2024E"]']
    assert rev_cell["formatted_value"] == "$ 100.0"
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
        f8.compile(manifest, tables)

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
        f8.compile(manifest, tables)

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
        f8.compile(manifest, tables)

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
        f8.compile(manifest, tables)

    err = exc_info.value
    assert isinstance(err, SpecificationError)
    assert isinstance(err, F8Error)
    assert hasattr(err, "table")
    assert hasattr(err, "reason")
    assert "duplicate" in err.reason.lower()


def test_scenario_10_inspection_entity_not_found(
    sample_model: SampleModelFixture,
) -> None:
    manifest, tables = sample_model
    compiled = f8.compile(manifest, tables)

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
    compiled = f8.compile(manifest, tables)

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

    compiled = f8.compile(manifest, [table])
    assert isinstance(compiled, CompiledModel)
    summary = compiled.summary()
    assert summary["model_name"] == "TypedModel"
    assert summary["total_tables"] == 1
    assert summary["total_cells"] == 2
