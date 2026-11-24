"""Tests for TableAuditor, unboxed cell values, and tabular export protocols."""

from __future__ import annotations

import pytest
from f8 import (
    Cell,
    CellError,
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
    assert table.subtitles == ["($ in Millions)"]
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

    # Multi-section subsetting with include_section_headers=True omits orphaned section headers
    multi_sec_model = Model(name="MultiSec", theme="corporate_navy")
    multi_sec_sheet = multi_sec_model.add_sheet("Sheet1")
    multi_sec_tbl = multi_sec_sheet.add_table("MultiSecTable", title="Multi Section")
    multi_sec_tbl.add_column("line_item", role="descriptor", header="Metric")
    multi_sec_tbl.add_column("val", role="measure", header="Value")
    multi_sec_tbl.add_section_header("Section 1: Empty Match")
    multi_sec_tbl.add_row("r1", cells={"line_item": "Row 1", "val": 10.0})
    multi_sec_tbl.add_section_header("Section 2: Active Match")
    multi_sec_tbl.add_row("r2", cells={"line_item": "Row 2", "val": 20.0})
    multi_sec_tbl.add_section_header("Section 3: Trailing Empty Match")
    multi_sec_tbl.add_row("r3", cells={"line_item": "Row 3", "val": 30.0})
    multi_sec_compiled = multi_sec_model.compile()
    md_filtered_sec = multi_sec_compiled.table("MultiSecTable").to_markdown(
        rows=["r2"],
        include_section_headers=True,
    )
    assert "**Section 1: Empty Match**" not in md_filtered_sec
    assert "**Section 2: Active Match**" in md_filtered_sec
    assert "Row 2" in md_filtered_sec
    assert "**Section 3: Trailing Empty Match**" not in md_filtered_sec

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


def test_scenario_11_row_col_ids_with_comma_quote_and_markdown_pipe_newline_escaping() -> None:
    model = Model(name="SpecialChars", theme="corporate_navy")
    sheet = model.add_sheet("Sheet1")
    tbl = sheet.add_table("T", title="Special Table")
    tbl.add_column("label", role="descriptor", header="Item | Name\nLine 2")
    tbl.add_column('val", "c', role="measure", header="Value | Col")
    tbl.add_section_header("Sec | Header\nSub")
    row_key = 'rev", "sub'
    col_key = 'val", "c'
    tbl.add_row(
        row_key,
        cells={
            "label": "A | B\nC",
            col_key: 42.0,
        },
    )
    compiled = model.compile()
    auditor = compiled.table("T")

    cell_info = auditor.cell(row_key, col_key)
    assert cell_info["row"] == row_key
    assert cell_info["col"] == col_key
    assert auditor.value(row_key, col_key) == 42.0
    assert auditor.to_dict("index")[row_key][col_key] == 42.0

    md = str(auditor.to_markdown())
    lines = [line for line in md.splitlines() if line]
    assert len(lines) == 4  # header, align, section header, 1 data row (no raw embedded newlines)
    assert r"Item \| Name Line 2" in lines[0]
    assert r"Value \| Col" in lines[0]
    assert r"**Sec \| Header Sub**" in lines[2]
    assert r"A \| B C" in lines[3]

    # Formula cell without lowered excel_formula renders empty instead of value fallback
    raw_insp = compiled.inspect_table("T")
    for info in raw_insp["cells"].values():
        if info["col"] == col_key:
            info["is_formula"] = True
            info["excel_formula"] = None
    missing_formula_md = str(TableAuditor(raw_insp).to_markdown("formulas"))
    assert "42" not in missing_formula_md.splitlines()[-1]


def _build_parity_model() -> CompiledModel:
    """Builds the shared cross-language parity workbook."""
    model = Model(name="ParityModel", theme="corporate_navy")

    fin = model.add_sheet("Financials")
    inc = fin.add_table(
        "IncomeStatement",
        title="Income Statement",
        subtitle=["($ in Millions)", "Fiscal Year"],
        currency="USD",
    )
    inc.add_column("line_item", role="descriptor", header="Line Item")
    inc.add_column(
        "y2023a",
        role="measure",
        group="Historical",
        header="2023A",
        format="currency",
        decimals=1,
    )
    inc.add_column_gutter()
    inc.add_column("2024E", role="measure", header="2024E", format="currency", decimals=1)

    inc.add_section_header("Revenue & Margin")
    inc.add_row(
        "revenue",
        cells={
            "line_item": "Revenue",
            "y2023a": 100.0,
            "2024E": Cell(expr='IncomeStatement["revenue", col.prev] * 1.15'),
        },
    )
    inc.add_row(
        "cogs",
        cells={
            "line_item": Cell("Cost | Goods\nSold", indent=1),
            "y2023a": 40.0,
            "2024E": 44.0,
        },
    )
    inc.add_row("sparse", cells={"line_item": "Memo", "y2023a": 5.0, "2024E": None})
    inc.add_row_gutter()
    inc.add_row(
        "gross_profit",
        style="subtotal",
        cells={"line_item": "Gross Profit"},
        expr='IncomeStatement["revenue", @col] - IncomeStatement["cogs", @col]',
    )
    inc.add_section_header("Checks | Errors")
    inc.add_row(
        "div_err",
        style="total",
        cells={
            "line_item": "Net | Check",
            "y2023a": 10.0,
            "2024E": Cell(expr="1 / 0"),
        },
    )

    pure = fin.add_table("PureMeasures", title="PureMeasures")
    pure.add_column("c1", role="measure", header="c1")
    pure.add_column("c2", role="measure", header="Col Two")
    pure.add_row("r1", cells={"c1": 1.0, "c2": Cell(expr="NA()")})
    pure.add_row("r2", style="total", cells={"c1": 2.0, "c2": 3.0})

    val = model.add_sheet("Valuation")
    comps = val.add_table("Comps", title="Trading Comps", currency="USD")
    comps.add_column("ticker", role="descriptor", header="Ticker")
    comps.add_column("company", role="descriptor", header="Company Name")
    comps.add_column("ev", role="measure", header="EV", format="currency", decimals=1)
    comps.add_row("aapl", cells={"ticker": "AAPL", "company": "Apple Inc.", "ev": 3000.0})
    comps.add_row(
        "msft_row",
        cells={"ticker": "MSFT", "company": "Microsoft", "ev": Cell(expr="LN(-1)")},
    )

    model.add_sheet("Notes")
    return model.compile()


EXPECTED_PARITY_VALUES_MD = (
    "# ParityModel\n"
    "Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3\n"
    "\n"
    "## Runtime Errors\n"
    "\n"
    '- `IncomeStatement["div_err", "2024E"]` (`\'Financials\'!E15`): `#DIV/0!` (`=1/0`)\n'
    '- `PureMeasures["r1", "c2"]` (`\'Financials\'!C21`): `#N/A` (`=NA()`)\n'
    '- `Comps["msft_row", "ev"]` (`\'Valuation\'!D6`): `#NUM!` (`=LN(-1)`)\n'
    "\n"
    "## Sheet: Financials\n"
    "\n"
    "### `IncomeStatement` — Income Statement\n"
    "_($ in Millions)_\n"
    "_Fiscal Year_\n"
    "\n"
    "| Line Item [line_item] | Historical: 2023A [y2023a] | 2024E |\n"
    "| :--- | ---: | ---: |\n"
    "| **Revenue & Margin** | | |\n"
    "| Revenue | $100.0 | $115.0 |\n"
    r"|   Cost \| Goods Sold [cogs] | $40.0 | $44.0 |"
    "\n"
    "| Memo [sparse] | $5.0 | — |\n"
    "| **Gross Profit [gross_profit]** | **$60.0** | **$71.0** |\n"
    r"| **Checks \| Errors** | | |"
    "\n"
    r"| **Net \| Check [div_err]** | **$10.0** | **#DIV/0!** |"
    "\n"
    "\n"
    "### `PureMeasures`\n"
    "\n"
    "| Row | c1 | Col Two [c2] |\n"
    "| :--- | ---: | ---: |\n"
    "| r1 | 1 | #N/A |\n"
    "| **r2** | **2** | **3** |\n"
    "\n"
    "## Sheet: Valuation\n"
    "\n"
    "### `Comps` — Trading Comps\n"
    "\n"
    "| Ticker | Company Name [company] | EV |\n"
    "| :--- | :--- | ---: |\n"
    "| AAPL | Apple Inc. | $3,000.0 |\n"
    "| MSFT [msft_row] | Microsoft | #NUM! |\n"
    "\n"
    "## Sheet: Notes\n"
    "\n"
    "_(no tables)_\n"
)

EXPECTED_PARITY_INDEX_MD = (
    "# ParityModel\n"
    "Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3\n"
    "\n"
    "## Runtime Errors\n"
    "\n"
    '- `IncomeStatement["div_err", "2024E"]` (`\'Financials\'!E15`): `#DIV/0!` (`=1/0`)\n'
    '- `PureMeasures["r1", "c2"]` (`\'Financials\'!C21`): `#N/A` (`=NA()`)\n'
    '- `Comps["msft_row", "ev"]` (`\'Valuation\'!D6`): `#NUM!` (`=LN(-1)`)\n'
    "\n"
    "## Sheet: Financials\n"
    "\n"
    "### `IncomeStatement` — Income Statement\n"
    "_($ in Millions)_\n"
    "_Fiscal Year_\n"
    "\n"
    "Columns: `line_item` (Line Item, descriptor), `y2023a` (Historical: 2023A), `2024E`\n"
    "Rows:\n"
    "- **Revenue & Margin**\n"
    "  - `revenue`\n"
    "  - `cogs`: Cost | Goods Sold\n"
    "  - `sparse`: Memo\n"
    "  - **`gross_profit`**: Gross Profit (subtotal)\n"
    "- **Checks | Errors**\n"
    "  - **`div_err`**: Net | Check (total)\n"
    "\n"
    "### `PureMeasures`\n"
    "\n"
    "Columns: `c1`, `c2` (Col Two)\n"
    "Rows:\n"
    "- `r1`\n"
    "- **`r2`** (total)\n"
    "\n"
    "## Sheet: Valuation\n"
    "\n"
    "### `Comps` — Trading Comps\n"
    "\n"
    "Columns: `ticker` (descriptor), `company` (Company Name, descriptor), `ev`\n"
    "Rows:\n"
    "- `aapl`: AAPL | Apple Inc.\n"
    "- `msft_row`: MSFT | Microsoft\n"
    "\n"
    "## Sheet: Notes\n"
    "\n"
    "_(no tables)_\n"
)


EXPECTED_PARITY_LAYOUT_MD = (
    "# ParityModel\n"
    "Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3\n"
    "\n"
    "## Runtime Errors\n"
    "\n"
    '- `IncomeStatement["div_err", "2024E"]` (`\'Financials\'!E15`): `#DIV/0!` (`=1/0`)\n'
    '- `PureMeasures["r1", "c2"]` (`\'Financials\'!C21`): `#N/A` (`=NA()`)\n'
    '- `Comps["msft_row", "ev"]` (`\'Valuation\'!D6`): `#NUM!` (`=LN(-1)`)\n'
    "\n"
    "## Sheet: Financials\n"
    "Layout: auto | Bounds: B2:E22 (21r x 4c, w:41)\n"
    "Columns: A:3, B:14, C:12, D:3, E:12\n"
    "\n"
    "- Band 1 (R2:R15, 14r x cols B..E, w:41):\n"
    "  - `IncomeStatement` [B2:E15] 14r x 4c (w:41) | stack"
    " | sub:2 hdr:R6:R7(grouped) body:R8:R15"
    " | cols: desc B(1c), measure C(1c), gutter D(1c), measure E(1c) | wrap:1\n"
    "- Band 2 (R18:R22, 5r x cols B..C, w:26, gap:2r, right_empty:D..E (2c, w:15)):\n"
    "  - `PureMeasures` [B18:C22] 5r x 2c (w:26) | stack"
    " | title->E (+2 empty) hdr:R20 body:R21:R22 | cols: measure B..C(2c)\n"
    "\n"
    "## Sheet: Valuation\n"
    "Layout: auto | Bounds: B2:D6 (5r x 3c, w:36)\n"
    "Columns: A:3, B..D:12\n"
    "\n"
    "- Band 1 (R2:R6, 5r x cols B..D, w:36):\n"
    "  - `Comps` [B2:D6] 5r x 3c (w:36) | stack"
    " | hdr:R4 body:R5:R6 | cols: desc B..C(2c), measure D(1c)\n"
    "\n"
    "## Sheet: Notes\n"
    "\n"
    "_(no tables)_\n"
)


def test_compiled_model_errors_and_workbook_markdown() -> None:
    # Scenario 1: Clean workbook returns []
    clean_model = Model(name="CleanModel")
    clean_sheet = clean_model.add_sheet("Summary")
    clean_tbl = clean_sheet.add_table("KPIs", title="KPIs")
    clean_tbl.add_column("metric", role="descriptor", header="Metric")
    clean_tbl.add_column("val", role="measure", header="Value")
    clean_tbl.add_row("rev", cells={"metric": "Revenue", "val": 100.0})
    clean = clean_model.compile()
    assert clean.errors() == []
    clean_md = clean.to_markdown()
    assert type(clean_md) is str
    assert "| Errors: 0\n" in clean_md
    assert "## Runtime Errors" not in clean_md

    # Scenario 2: Deterministic error list across sheets/tables
    compiled = _build_parity_model()
    errs: list[CellError] = compiled.errors()
    assert errs == [
        {
            "coordinate": {"table": "IncomeStatement", "row": "div_err", "col": "2024E"},
            "sheet": "Financials",
            "physical_address": "'Financials'!E15",
            "error": "#DIV/0!",
            "formula_expr": "1 / 0",
            "excel_formula": "=1/0",
        },
        {
            "coordinate": {"table": "PureMeasures", "row": "r1", "col": "c2"},
            "sheet": "Financials",
            "physical_address": "'Financials'!C21",
            "error": "#N/A",
            "formula_expr": "NA()",
            "excel_formula": "=NA()",
        },
        {
            "coordinate": {"table": "Comps", "row": "msft_row", "col": "ev"},
            "sheet": "Valuation",
            "physical_address": "'Valuation'!D6",
            "error": "#NUM!",
            "formula_expr": "LN(-1)",
            "excel_formula": "=LN(-1)",
        },
    ]

    # Scenarios 3, 4, 5, 10: Byte-identical cross-language parity in values and index modes
    md_values = compiled.to_markdown()
    assert type(md_values) is str
    assert md_values == EXPECTED_PARITY_VALUES_MD
    assert compiled.to_markdown("values", show_ids=True) == EXPECTED_PARITY_VALUES_MD

    md_index = compiled.to_markdown("index")
    assert md_index == EXPECTED_PARITY_INDEX_MD
    assert compiled.to_markdown("index", show_ids=False) == EXPECTED_PARITY_INDEX_MD

    md_layout = compiled.to_markdown("layout")
    assert md_layout == EXPECTED_PARITY_LAYOUT_MD
    assert compiled.to_markdown("layout", show_ids=False) == EXPECTED_PARITY_LAYOUT_MD
    assert compiled.to_markdown("layout", show_ids=True) == EXPECTED_PARITY_LAYOUT_MD

    # Scenario 5: show_ids=False omits synthetic Row column & inline ID annotations
    md_no_ids = compiled.to_markdown("values", show_ids=False)
    assert "| Row |" not in md_no_ids
    assert "| c1 | Col Two |" in md_no_ids
    assert "[msft_row]" not in md_no_ids
    assert "| MSFT | Microsoft | #NUM! |" in md_no_ids

    # Scenarios 6 & 7: Filtering, union, deduplication, empty allowlists, empty sheet, strict errors
    md_val_only = compiled.to_markdown(sheets=("Valuation",))
    assert "| Errors: 3\n" in md_val_only
    assert "## Runtime Errors" in md_val_only
    assert 'Comps["msft_row", "ev"]' in md_val_only
    assert "IncomeStatement" not in md_val_only

    md_notes_only = compiled.to_markdown(sheets=["Notes"])
    assert "| Errors: 3\n" in md_notes_only
    assert "## Runtime Errors" not in md_notes_only
    assert "## Sheet: Notes\n\n_(no tables)_\n" in md_notes_only

    md_union = compiled.to_markdown(
        sheets=["Valuation", "Valuation"],
        tables=["Comps", "PureMeasures", "PureMeasures"],
    )
    assert md_union.count("### `Comps`") == 1
    assert md_union.count("### `PureMeasures`") == 1
    assert "### `IncomeStatement`" not in md_union
    assert md_union.index("## Sheet: Financials") < md_union.index("## Sheet: Valuation")

    expected_header_only = (
        "# ParityModel\nSheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3\n"
    )
    assert compiled.to_markdown(sheets=[]) == expected_header_only
    assert compiled.to_markdown(tables=[]) == expected_header_only
    assert compiled.to_markdown(sheets=[], tables=[]) == expected_header_only
    assert compiled.to_markdown("layout", sheets=[]) == expected_header_only
    assert compiled.to_markdown("layout", tables=[]) == expected_header_only

    with pytest.raises(InspectionError):
        compiled.to_markdown(sheets=["UnknownSheet"])
    with pytest.raises(InspectionError):
        compiled.to_markdown(tables=["UnknownTable"])
    with pytest.raises(InspectionError):
        compiled.to_markdown("layout", sheets=["UnknownSheet"])
    with pytest.raises(InspectionError):
        compiled.to_markdown("layout", tables=["UnknownTable"])

    # Scenario 8: Invalid detail raises ValueError (not F8Error)
    from collections.abc import Callable
    from typing import cast

    untyped_to_markdown = cast(Callable[[str], str], compiled.to_markdown)
    with pytest.raises(
        ValueError,
        match=r"Invalid detail mode 'summary'\. Expected one of 'values', 'index', 'layout'\.",
    ):
        untyped_to_markdown("summary")


def test_compiled_model_layout_markdown_scenarios() -> None:
    # Partial tables filter in layout mode tags unselected siblings as (context) and scopes errors
    parity = _build_parity_model()
    md_partial = parity.to_markdown("layout", tables=["PureMeasures"])
    assert "## Sheet: Financials\n" in md_partial
    assert "## Sheet: Valuation" not in md_partial
    assert "## Sheet: Notes" not in md_partial
    assert "  - `IncomeStatement` (context) [B2:E15]" in md_partial
    assert "  - `PureMeasures` [B18:C22]" in md_partial
    assert "  - `PureMeasures` (context)" not in md_partial
    assert 'PureMeasures["r1", "c2"]' in md_partial
    assert 'IncomeStatement["div_err", "2024E"]' not in md_partial

    # Grid layout with multi-lane geometry, uniform width inflation, and auto chrome metadata
    model = Model("LayoutEndToEnd")
    dash = model.add_sheet("Dashboard", layout="grid", col_width=12.0)

    t1 = dash.add_table("T1", "Top Left")
    t1.add_column("item", role="descriptor", header="Item")
    t1.add_column("v1", header="V1", format="currency", decimals=1)
    t1.add_row("r1", cells={"item": "Revenue", "v1": 123_456_789.0})

    t2 = dash.add_table("T2", "Bottom Left", below=t1)
    t2.add_column("item", role="descriptor", header="Item")
    t2.add_column("v1", header="V1", format="currency", decimals=1)
    t2.add_row("r1", cells={"item": "Cost", "v1": 50.0})

    t3 = dash.add_table("T3", "Right Tall", right_of=t1)
    t3.add_column("item", role="descriptor", header="Item")
    t3.add_column("v1", header="V1")
    for idx in range(1, 12):
        t3.add_row(f"r{idx}", cells={"item": f"Row {idx}", "v1": float(idx)})

    auto_sheet = model.add_sheet(
        "Detail",
        freeze_panes=(6, 2),
        show_gridlines=True,
        tab_color="#1F4E79",
    )
    tbl = auto_sheet.add_table(
        "Notes",
        "Notes Table",
        subtitle=["Line 1", "Line 2"],
        collapsible=True,
    )
    tbl.add_column("item", role="descriptor", header="Item")
    tbl.add_column("val", group="Grp", header="Val")
    tbl.add_row("r1", cells={"item": "Multi\nLine", "val": 10.0})

    one_row = auto_sheet.add_table("Single", "", collapsible=True)
    one_row.add_column("v")
    one_row.add_row("r1", cells={"v": 1.0})

    md = model.compile().to_markdown("layout")
    assert "Layout: grid (W=15, base=12)" in md
    assert "- Band 1 (" in md
    assert "  - Lane B..C (" in md
    assert "empty_below:" in md
    assert "    - `T1` " in md
    assert "    - `T2` " in md
    assert "    - `T3` " in md
    assert "Layout: auto |" in md
    assert "| Freeze: C7 (R1:R6, A..B) | Gridlines: on | Tab: #1F4E79\n" in md
    assert "sub:2 hdr:" in md
    assert "(grouped)" in md
    assert "collapsible | cols:" in md
    assert "| wrap:1\n" in md
    assert "`Single`" in md
    assert "untitled hdr:none body:" in md
    single_line = next(line for line in md.splitlines() if "`Single`" in line)
    assert "collapsible" not in single_line


def test_table_auditor_to_markdown_compact_mode() -> None:
    # Scenario 9: compact=False preserves padded output; compact=True emits unpadded GFM
    compiled = _build_parity_model()
    inc = compiled.table("IncomeStatement")

    for view in ("values", "formulas", "expr", "addresses"):
        padded = inc.to_markdown(view)
        compact = inc.to_markdown(view, compact=True)
        assert isinstance(compact, MarkdownTable)
        assert padded == inc.to_markdown(view, compact=False)
        assert len(compact) < len(padded)

        compact_lines = compact.splitlines()
        assert compact_lines[1] == "| :--- | ---: | ---: |"
        assert padded.splitlines()[1] != compact_lines[1]

    compact_values = str(inc.to_markdown("values", show_ids=True, compact=True))
    assert compact_values == (
        "| Line Item [line_item] | Historical: 2023A [y2023a] | 2024E |\n"
        "| :--- | ---: | ---: |\n"
        "| **Revenue & Margin** | | |\n"
        "| Revenue | $100.0 | $115.0 |\n"
        r"|   Cost \| Goods Sold [cogs] | $40.0 | $44.0 |"
        "\n"
        "| Memo [sparse] | $5.0 | — |\n"
        "| **Gross Profit [gross_profit]** | **$60.0** | **$71.0** |\n"
        r"| **Checks \| Errors** | | |"
        "\n"
        r"| **Net \| Check [div_err]** | **$10.0** | **#DIV/0!** |"
        "\n"
    )

    compact_formulas = str(inc.to_markdown("formulas", compact=True))
    assert "| Revenue | $100.0 | =C9*1.15 |" in compact_formulas
    assert "| Memo | $5.0 | |" in compact_formulas


def test_bold_style_values_index_and_table_auditor_markdown() -> None:
    model = Model("BoldModel")
    sheet = model.add_sheet("Summary")
    tbl = sheet.add_table("KPIs", "Key Metrics", currency="USD")
    tbl.add_column("item", role="descriptor", header="Metric")
    tbl.add_column("base", header="Base Case", style="bold")
    tbl.add_column("upside", header="Upside")

    tbl.add_row(
        "rev",
        format="currency",
        cells={"item": "Revenue", "base": Cell(100.0, style="bold"), "upside": 120.0},
    )
    tbl.add_row(
        "margin",
        style="bold",
        format="percentage",
        cells={"item": "EBITDA Margin", "base": 0.25, "upside": 0.28},
    )
    tbl.add_row(
        "ebitda",
        style="subtotal",
        format="currency",
        cells={"item": "EBITDA", "base": 25.0, "upside": 34.0},
    )
    tbl.add_row(
        "net_income",
        style="total",
        format="currency",
        cells={"item": "Net Income", "base": 18.0, "upside": 24.0},
    )

    compiled = model.compile()
    inspected = compiled.inspect_table("KPIs")
    assert inspected["columns"][1]["style"] == "bold"
    assert inspected["rows"][1]["style"] == "bold"

    # Whole-workbook Markdown: bold row bolded in values (plain in index); bold col unbolded
    md_values = compiled.to_markdown("values", show_ids=False)
    assert "| Revenue | $100 | $120 |\n" in md_values
    assert "| **EBITDA Margin** | **25.0%** | **28.0%** |\n" in md_values
    assert "| **EBITDA** | **$25** | **$34** |\n" in md_values
    assert "| **Net Income** | **$18** | **$24** |\n" in md_values

    md_index = compiled.to_markdown("index")
    assert "- `margin`: EBITDA Margin\n" in md_index
    assert "(bold)" not in md_index
    assert "**`margin`**" not in md_index
    assert "- **`ebitda`** (subtotal)\n" in md_index
    assert "- **`net_income`**: Net Income (total)\n" in md_index

    # TableAuditor Markdown matches whole-workbook values behavior
    auditor_compact = str(compiled.table("KPIs").to_markdown("values", compact=True))
    assert "| Revenue | $100 | $120 |\n" in auditor_compact
    assert "| **EBITDA Margin** | **25.0%** | **28.0%** |\n" in auditor_compact

    # Workbook emission succeeds cleanly
    xlsx = compiled.emit_workbook()
    assert xlsx[:2] == b"PK"
