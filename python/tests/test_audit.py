"""Tests for TableAuditor, unboxed cell values, and tabular export protocols."""

from __future__ import annotations

import pytest
from f8 import (
    Cell,
    CompiledModel,
    InspectionError,
    MarkdownTable,
    Model,
    SplitDict,
    TableAuditor,
    unbox_cell_value,
)


def _build_test_model() -> CompiledModel:
    """Builds a test model with drivers, formulas, section headers, and gutters."""
    model = Model(name="PerformanceModel", theme="corporate_navy")
    sheet = model.add_sheet("Financials")
    table = sheet.add_table(
        "Performance",
        title="Performance",
        subtitle="($ in Millions)",
        currency="USD",
    )
    table.add_column("line_item", role="descriptor", header="Line Item")
    table.add_column("2023A", role="measure", header="2023A", format="currency", decimals=1)
    table.add_column("2024E", role="measure", header="2024E", format="currency", decimals=1)
    table.add_column("2025E", role="measure", header="2025E", format="currency", decimals=1)

    table.add_section_header("Revenue & Operations")
    table.add_row(
        "revenue",
        cells={"line_item": "Total Revenue", "2023A": 100.0},
        expr='Performance["revenue", col.prev] * 1.15',
        format="currency",
        decimals=1,
    )
    table.add_row(
        "cogs",
        cells={"line_item": Cell("Cost of Goods Sold", indent=1), "2023A": 40.0},
        expr='Performance["cogs", col.prev] * 1.10',
        format="currency",
        decimals=1,
    )
    table.add_row(
        "gross_profit",
        cells={"line_item": "Gross Profit"},
        expr='Performance["revenue", @col] - Performance["cogs", @col]',
        format="currency",
        decimals=1,
    )

    table.add_section_header("Other Metrics")
    table.add_row(
        "div_zero",
        cells={"line_item": "Error Row"},
        expr="1.0 / 0.0",
    )
    table.add_row(
        "sparse_row",
        cells={"line_item": "Sparse Line", "2023A": 10.0, "2024E": None, "2025E": None},
        format="currency",
        decimals=1,
    )

    return model.compile()


def test_scenario_1_table_accessor_on_compiled_model() -> None:
    compiled = _build_test_model()

    table = compiled.table("Performance")
    assert isinstance(table, TableAuditor)
    assert table.id == "Performance"
    assert table.sheet == "Financials"

    with pytest.raises(InspectionError):
        compiled.table("NonExistent")


def test_scenario_2_unboxed_scalar_cell_lookups() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    # Evaluated formula cell
    rev_2024 = table.value("revenue", "2024E")
    assert isinstance(rev_2024, float)
    assert pytest.approx(rev_2024) == 115.0

    # __getitem__ tuple indexing
    assert pytest.approx(table["revenue", "2024E"]) == 115.0

    # Formula lookup
    formula_rev_2024 = table.formula("revenue", "2024E")
    assert formula_rev_2024 is not None
    assert formula_rev_2024.startswith("=")

    # Driver cell has no excel formula
    assert table.formula("revenue", "2023A") is None

    # DSL expression lookup
    assert table.expr("revenue", "2024E") == 'Performance["revenue", col.prev] * 1.15'

    # Physical address lookup
    addr = table.address("revenue", "2023A")
    assert isinstance(addr, str)
    assert len(addr) > 0

    # Raw TableCellInfo metadata lookup
    cell_info = table.cell("revenue", "2024E")
    assert isinstance(cell_info, dict)
    assert cell_info.get("is_formula") is True
    assert cell_info.get("formula_expr") == 'Performance["revenue", col.prev] * 1.15'
    assert cell_info.get("excel_formula") == "=C7*1.15"
    assert cell_info.get("physical_a1") == "D7"
    assert cell_info.get("formatted_value") == "$115.0"

    # Division by zero evaluation produces canonical #DIV/0!
    assert table.value("div_zero", "2024E") == "#DIV/0!"

    # Sparse row has None for unassigned cell
    assert table.value("sparse_row", "2024E") is None

    # Unknown coordinate lookup raises KeyError
    with pytest.raises(KeyError, match=r"Coordinate \('unknown_row', '2024E'\) not found"):
        table.value("unknown_row", "2024E")

    # Non-tuple indexing raises TypeError
    with pytest.raises(TypeError, match="requires a \\(row, col\\) tuple"):
        _ = table["revenue"]  # pyright: ignore[reportArgumentType]

    with pytest.raises(TypeError, match="requires a \\(row, col\\) tuple"):
        _ = table[1, 2]  # pyright: ignore[reportArgumentType]


def test_scenario_3_tabular_export_records() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    records = table.to_dict(orient="records")
    assert isinstance(records, list)
    assert len(records) == 5  # 5 calculable data rows, section headers excluded

    # Every record contains 100% of table.columns keys
    for row_dict in records:
        assert list(row_dict.keys()) == table.columns

    # First row is revenue
    rev_row = records[0]
    assert rev_row["line_item"] == "Total Revenue"
    assert rev_row["2023A"] == 100.0
    assert pytest.approx(rev_row["2024E"]) == 115.0

    # Sparse row unassigned cell is None
    sparse_row = records[4]
    assert sparse_row["line_item"] == "Sparse Line"
    assert sparse_row["2023A"] == 10.0
    assert sparse_row["2024E"] is None
    assert sparse_row["2025E"] is None


def test_scenario_4_tabular_export_dict() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    col_dict = table.to_dict(orient="dict")
    assert isinstance(col_dict, dict)
    assert list(col_dict.keys()) == ["line_item", "2023A", "2024E", "2025E"]

    # Each column matches length of rows
    for col_name in table.columns:
        assert len(col_dict[col_name]) == len(table.rows)

    assert col_dict["line_item"][0] == "Total Revenue"
    assert col_dict["2023A"][0] == 100.0
    assert pytest.approx(col_dict["2024E"][0]) == 115.0


def test_scenario_5_tabular_export_index() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    index_dict = table.to_dict(orient="index")
    assert isinstance(index_dict, dict)
    assert list(index_dict.keys()) == ["revenue", "cogs", "gross_profit", "div_zero", "sparse_row"]

    assert index_dict["revenue"]["line_item"] == "Total Revenue"
    assert pytest.approx(index_dict["revenue"]["2024E"]) == 115.0
    assert index_dict["sparse_row"]["2025E"] is None


def test_scenario_6_tabular_export_split() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    split_res: SplitDict = table.to_dict(orient="split")
    assert isinstance(split_res, dict)
    assert split_res["columns"] == ["line_item", "2023A", "2024E", "2025E"]
    assert split_res["index"] == ["revenue", "cogs", "gross_profit", "div_zero", "sparse_row"]

    data = split_res["data"]
    assert len(data) == 5
    assert len(data[0]) == 4
    assert data[0][0] == "Total Revenue"
    assert data[0][1] == 100.0
    assert pytest.approx(data[0][2]) == 115.0


def test_scenario_7_visual_markdown_grid_rendering() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    # Values mode
    from operator import methodcaller

    repr_markdown = methodcaller("_repr_markdown_")
    md_values = table.to_markdown(view="values")
    assert isinstance(md_values, MarkdownTable)
    assert isinstance(md_values, str)
    assert md_values.endswith("\n")
    assert repr_markdown(md_values) == str(md_values)
    assert repr_markdown(table) == str(md_values)

    # Check header and alignment rows
    lines = [line for line in md_values.split("\n") if line]
    assert "Line Item" in lines[0]
    assert "2023A" in lines[0]
    assert "2024E" in lines[0]
    assert ":---" in lines[1]  # Descriptor left-aligned
    assert "---:" in lines[1]  # Measure right-aligned

    # Check section divider row
    assert any("**Revenue & Operations**" in line for line in lines)
    assert any("**Other Metrics**" in line for line in lines)

    # Check indentation for cogs line item
    assert any("  Cost of Goods Sold" in line for line in lines)

    # Check pre-formatted values with currency (no internal space) and em-dash for empty cells
    assert any("$100.0" in line for line in lines)
    assert any("$115.0" in line for line in lines)
    assert any("Sparse Line" in line and "—" in line for line in lines)

    # Formulas mode
    md_formulas = table.to_markdown(view="formulas")
    assert any("=C" in line or "=B" in line for line in md_formulas.split("\n"))

    # Expr mode
    md_expr = table.to_markdown(view="expr")
    assert any("Performance" in line and "col.prev" in line for line in md_expr.split("\n"))

    # Addresses mode
    md_addrs = table.to_markdown(view="addresses")
    assert any("C" in line for line in md_addrs.split("\n"))


def test_scenario_8_dimension_and_metadata_introspection() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    assert table.title == "Performance"
    assert table.subtitle == "($ in Millions)"
    assert table.sheet == "Financials"
    assert table.columns == ["line_item", "2023A", "2024E", "2025E"]
    assert table.measure_columns == ["2023A", "2024E", "2025E"]
    assert table.rows == ["revenue", "cogs", "gross_profit", "div_zero", "sparse_row"]
    assert table.shape == (5, 4)
    assert len(table) == 5


def test_unbox_cell_value_scalar_variants_and_errors() -> None:
    assert unbox_cell_value({"Number": 100.0}) == 100.0
    assert unbox_cell_value({"Number": 42}) == 42.0
    assert unbox_cell_value({"Text": "Hello"}) == "Hello"
    assert unbox_cell_value({"Boolean": True}) is True
    assert unbox_cell_value({"Boolean": False}) is False
    assert unbox_cell_value("Empty") is None
    assert unbox_cell_value("empty") is None
    assert unbox_cell_value({"Empty": None}) is None
    assert unbox_cell_value(None) is None
    assert unbox_cell_value(123.45) == 123.45
    assert unbox_cell_value("raw text") == "raw text"
    assert unbox_cell_value(True) is True

    # Error code mapping
    assert unbox_cell_value({"Error": "Div0"}) == "#DIV/0!"
    assert unbox_cell_value({"Error": "Value"}) == "#VALUE!"
    assert unbox_cell_value({"Error": "Ref"}) == "#REF!"
    assert unbox_cell_value({"Error": "Num"}) == "#NUM!"
    assert unbox_cell_value({"Error": "Na"}) == "#N/A"
    assert unbox_cell_value({"Error": "Name"}) == "#NAME?"


def test_invalid_orient_and_view_mode_validation() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    with pytest.raises(ValueError, match="Invalid orient 'invalid'"):
        table.to_dict(orient="invalid")  # pyright: ignore[reportCallIssue,reportArgumentType]

    with pytest.raises(ValueError, match="Invalid view mode 'invalid'"):
        table.to_markdown(view="invalid")  # pyright: ignore[reportArgumentType]


def test_scenario_9_multi_tier_header_and_section_auditing() -> None:
    model = Model(name="ThreeStatementModel", theme="corporate_navy")
    sheet = model.add_sheet("Model")
    inc = sheet.add_table("IncomeStatement", title="Income Statement", currency="USD")
    inc.add_column("line_item", role="descriptor", header="Line Item")
    inc.add_column(
        "y2023a",
        role="measure",
        group="Historical Actuals",
        header="2023A",
        format="currency",
        decimals=1,
    )
    inc.add_column(
        "y2024e",
        role="measure",
        group="Forecast Projections",
        header="2024E",
        format="currency",
        decimals=1,
    )
    inc.add_column(
        "y2025e",
        role="measure",
        group="Forecast Projections",
        header="2025E",
        format="currency",
        decimals=1,
    )

    inc.add_section_header("Operating Expenses")
    inc.add_row(
        "revenue",
        cells={"line_item": "Revenue", "y2023a": 100.0},
        expr='IncomeStatement["revenue", col.prev] * 1.10',
        format="currency",
        decimals=1,
    )
    inc.add_row(
        "sga",
        cells={"line_item": Cell("Salaries & SG&A Overhead", indent=1), "y2023a": 30.0},
        expr='IncomeStatement["sga", col.prev] * 1.05',
        format="currency",
        decimals=1,
    )
    inc.add_row(
        "net_income",
        cells={"line_item": "Net Income"},
        expr='IncomeStatement["revenue", @col] - IncomeStatement["sga", @col]',
        style="total",
        format="currency",
        decimals=1,
    )

    compiled = model.compile()
    is_table = compiled.table("IncomeStatement")
    assert isinstance(is_table, TableAuditor)
    assert is_table.sheet == "Model"
    assert is_table.shape == (3, 4)
    assert is_table.columns == ["line_item", "y2023a", "y2024e", "y2025e"]
    assert is_table.measure_columns == ["y2023a", "y2024e", "y2025e"]
    assert is_table.rows == ["revenue", "sga", "net_income"]

    records = is_table.to_dict("records")
    assert len(records) == 3
    assert all(list(r.keys()) == is_table.columns for r in records)
    assert records[0]["line_item"] == "Revenue"
    assert records[0]["y2023a"] == 100.0

    split_data = is_table.to_dict("split")
    assert split_data["columns"] == is_table.columns
    assert len(split_data["data"]) == 3
    assert len(split_data["data"][0]) == 4

    md = is_table.to_markdown("values")
    assert isinstance(md, MarkdownTable)
    lines = [line for line in md.split("\n") if line]
    # Single flattened GFM header row before alignment row
    assert "Historical Actuals: 2023A" in lines[0]
    assert "Forecast Projections: 2024E" in lines[0]
    assert ":---" in lines[1] and "---:" in lines[1]
    assert "[y2023a]" not in md
    assert "[net_income]" not in md
    assert "**Operating Expenses**" in md
    assert "  Salaries & SG&A Overhead" in md
    assert "**Net Income**" in md
    assert "**$70.0**" in md

    # Subsetting rows and cols (retaining descriptor column and omitting section headers by default)
    md_sub = is_table.to_markdown(rows=["revenue", "net_income"], cols=["y2023a", "y2025e"])
    sub_lines = [line for line in md_sub.split("\n") if line]
    assert len(sub_lines) == 4  # header + align + 2 rows
    assert "Line Item" in sub_lines[0]
    assert "Historical Actuals: 2023A" in sub_lines[0]
    assert "Forecast Projections: 2025E" in sub_lines[0]
    assert "2024E" not in md_sub
    assert "**Operating Expenses**" not in md_sub
    assert "Salaries" not in md_sub

    # Subsetting rows with explicit include_section_headers=True
    md_sub_sec = is_table.to_markdown(
        rows=["revenue", "net_income"],
        cols=["y2023a", "y2025e"],
        include_section_headers=True,
    )
    assert "**Operating Expenses**" in md_sub_sec

    # Unknown row or col in to_markdown raises KeyError
    with pytest.raises(KeyError):
        is_table.to_markdown(rows=["unknown_row"])
    with pytest.raises(KeyError):
        is_table.to_markdown(cols=["unknown_col"])

    # show_ids=True appends [col_id] and [row_id] when display text differs from ID
    md_ids = is_table.to_markdown(show_ids=True)
    assert "Historical Actuals: 2023A [y2023a]" in md_ids

    # Verify group + multi-line stacked headers and ungrouped multi-line headers in to_markdown
    lbo_model = Model(name="LboHeaders", theme="corporate_navy")
    lbo_sheet = lbo_model.add_sheet("LBO")
    lbo_tbl = lbo_sheet.add_table("Schedule", title="LBO Schedule")
    lbo_tbl.add_column("label", role="descriptor", header="($ in millions)")
    lbo_tbl.add_column("mrq", group="Historical", headers=["MRQ", "2012-03-31"])
    lbo_tbl.add_column("standalone", headers=["Standalone", "at Close", "2012-12-31"])
    lbo_tbl.add_column("group_only", group="Summary")
    lbo_tbl.add_row(
        "rev",
        cells={"label": "Revenue", "mrq": 100.0, "standalone": 110.0, "group_only": 120.0},
    )
    lbo_compiled = lbo_model.compile()
    lbo_insp = lbo_compiled.inspect_table("Schedule")
    assert lbo_insp["columns"][0]["group"] is None
    assert lbo_insp["columns"][1]["group"] == "Historical"
    assert lbo_insp["columns"][1]["headers"] == ["MRQ", "2012-03-31"]
    assert lbo_insp["columns"][2]["group"] is None
    assert lbo_insp["columns"][2]["headers"] == ["Standalone", "at Close", "2012-12-31"]
    lbo_md = lbo_compiled.table("Schedule").to_markdown()
    lbo_header_line = lbo_md.split("\n")[0]
    assert "Historical: MRQ 2012-03-31" in lbo_header_line
    assert "Standalone at Close 2012-12-31" in lbo_header_line
    assert "Summary" in lbo_header_line
    assert "Net Income [net_income]" in md_ids


def test_scenario_10_auditor_scalar_and_vector_accessors() -> None:
    compiled = _build_test_model()
    table = compiled.table("Performance")

    assert table.measure_columns == ["2023A", "2024E", "2025E"]

    # Scalar num and formatted
    assert table.num("revenue", "2024E") == pytest.approx(115.0)
    assert table.formatted("revenue", "2024E") == "$115.0"
    assert table.formatted("revenue", "2024E", unit="M") == "$115.0M"
    assert table.formatted("sparse_row", "2024E", unit="M") == ""

    with pytest.raises(TypeError):
        table.num("revenue", "line_item")
    with pytest.raises(TypeError):
        table.num("sparse_row", "2024E")
    with pytest.raises(TypeError):
        table.num("div_zero", "2024E")
    with pytest.raises(KeyError):
        table.num("unknown", "2024E")
    with pytest.raises(KeyError):
        table.formatted("unknown", "2024E")

    # 1D row_values and row_nums
    assert table.row_values("revenue") == pytest.approx([100.0, 115.0, 132.25])
    assert table.row_values("sparse_row") == [10.0, None, None]
    assert table.row_nums("revenue") == pytest.approx([100.0, 115.0, 132.25])
    assert table.row_nums("revenue", cols=["2023A", "2024E"]) == pytest.approx([100.0, 115.0])

    with pytest.raises(TypeError):
        table.row_nums("sparse_row")
    with pytest.raises(TypeError):
        table.row_nums("div_zero")
    with pytest.raises(KeyError):
        table.row_values("unknown_row")
    with pytest.raises(KeyError):
        table.row_nums("unknown_row")
    with pytest.raises(KeyError):
        table.row_nums("revenue", cols=["unknown_col"])
