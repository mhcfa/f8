"""End-to-end integration tests for the f8 Python SDK and CLI runner.

Validates the complete lifecycle:
- Autonomous agent skill scaffolding and CLI documentation streaming.
- Multi-sheet institutional financial model construction via pure Python DSL.
- Cross-table formula evaluation and temporal offset resolution.
- Table auditing, scalar unboxing, structured dictionary export, and 2D Markdown rendering.
- File system project export and 100% on-disk compilation parity.
- OpenXML (.xlsx) workbook binary emission with live formulas and cached results.
"""

from __future__ import annotations

import zipfile
from pathlib import Path

import pytest
from f8 import Cell, Model, compile_project
from f8.cli import main as cli_main


def _build_institutional_lbo_model() -> Model:
    """Builds a multi-sheet corporate financial model with cross-table formulas."""
    model = Model(name="AcmeCorpLBO", theme="corporate_navy")

    # Sheet 1: Operating Assumptions
    assumptions_sheet = model.add_sheet(
        "Assumptions",
        tab_color="#1F4E79",
        show_gridlines=True,
        freeze_panes=(3, 2),
    )
    assumptions_tbl = assumptions_sheet.add_table(
        "assumptions",
        title="Macro & Operating Assumptions",
        subtitle="Audited Forecast Drivers",
        currency="USD",
    )
    assumptions_tbl.add_column("driver", role="descriptor", header="Operating Driver")
    assumptions_tbl.add_column(
        "y2023a", role="measure", headers=["Historical", "2023A"], format="percentage", decimals=1
    )
    assumptions_tbl.add_column(
        "y2024e", role="measure", headers=["Forecast", "2024E"], format="percentage", decimals=1
    )
    assumptions_tbl.add_column(
        "y2025e", role="measure", headers=["Forecast", "2025E"], format="percentage", decimals=1
    )
    assumptions_tbl.add_column(
        "y2026e", role="measure", headers=["Forecast", "2026E"], format="percentage", decimals=1
    )

    assumptions_tbl.add_section_header("Top-Line Revenue Drivers")
    assumptions_tbl.add_row(
        "rev_growth",
        cells={
            "driver": "Revenue Growth Rate (%)",
            "y2023a": None,
            "y2024e": 0.15,
            "y2025e": 0.12,
            "y2026e": 0.10,
        },
        format="percentage",
        decimals=1,
    )

    assumptions_tbl.add_row_gutter()
    assumptions_tbl.add_section_header("Margin & Rate Benchmarks")
    assumptions_tbl.add_row(
        "cogs_margin",
        cells={
            "driver": "COGS Margin (% of Revenue)",
            "y2023a": 0.45,
            "y2024e": 0.42,
            "y2025e": 0.40,
            "y2026e": 0.39,
        },
        format="percentage",
        decimals=1,
    )
    assumptions_tbl.add_row(
        "sga_margin",
        cells={
            "driver": "SG&A Margin (% of Revenue)",
            "y2023a": 0.20,
            "y2024e": 0.18,
            "y2025e": 0.17,
            "y2026e": 0.16,
        },
        format="percentage",
        decimals=1,
    )
    assumptions_tbl.add_row(
        "tax_rate",
        cells={
            "driver": "Effective Income Tax Rate",
            "y2023a": 0.25,
            "y2024e": 0.25,
            "y2025e": 0.25,
            "y2026e": 0.25,
        },
        format="percentage",
        decimals=1,
    )

    # Sheet 2: Income Statement
    is_sheet = model.add_sheet(
        "Income Statement",
        tab_color="#2E75B6",
        show_gridlines=True,
        freeze_panes=(3, 2),
    )
    is_tbl = is_sheet.add_table(
        "income_statement",
        title="Consolidated Statement of Operations",
        subtitle="USD Thousands ($)",
        currency="USD",
    )
    is_tbl.add_column("line_item", role="descriptor", header="Financial Line Item")
    is_tbl.add_column(
        "y2023a", role="measure", headers=["Historical", "2023A"], format="currency", decimals=0
    )
    is_tbl.add_column(
        "y2024e", role="measure", headers=["Forecast", "2024E"], format="currency", decimals=0
    )
    is_tbl.add_column(
        "y2025e", role="measure", headers=["Forecast", "2025E"], format="currency", decimals=0
    )
    is_tbl.add_column(
        "y2026e", role="measure", headers=["Forecast", "2026E"], format="currency", decimals=0
    )

    is_tbl.add_section_header("Gross Revenue & Cost of Sales")
    is_tbl.add_row(
        "rev",
        cells={"line_item": "Total Gross Revenue", "y2023a": 125000.0},
        expr='income_statement["rev", col.prev] * (1 + assumptions["rev_growth", @col])',
        format="currency",
        decimals=0,
    )
    is_tbl.add_row(
        "cogs",
        cells={"line_item": Cell("Cost of Goods Sold (COGS)", indent=1), "y2023a": 56250.0},
        expr='income_statement["rev", @col] * assumptions["cogs_margin", @col]',
        format="currency",
        decimals=0,
    )
    is_tbl.add_row(
        "gp",
        cells={"line_item": Cell("Gross Operating Profit", style="subtotal")},
        expr='income_statement["rev", @col] - income_statement["cogs", @col]',
        style="subtotal",
        format="currency",
        decimals=0,
    )

    is_tbl.add_row_gutter()
    is_tbl.add_section_header("Operating Expenses & Pre-Tax Income")
    is_tbl.add_row(
        "sga",
        cells={
            "line_item": Cell("Selling, General & Administrative", indent=1),
            "y2023a": 25000.0,
        },
        expr='income_statement["rev", @col] * assumptions["sga_margin", @col]',
        format="currency",
        decimals=0,
    )
    is_tbl.add_row(
        "ebitda",
        cells={"line_item": Cell("Operating EBITDA", style="subtotal")},
        expr='income_statement["gp", @col] - income_statement["sga", @col]',
        style="subtotal",
        format="currency",
        decimals=0,
    )
    is_tbl.add_row(
        "tax",
        cells={"line_item": Cell("Provision for Income Taxes", indent=1)},
        expr='income_statement["ebitda", @col] * assumptions["tax_rate", @col]',
        format="currency",
        decimals=0,
    )
    is_tbl.add_row(
        "net_income",
        cells={"line_item": Cell("Net Consolidated Income", style="total")},
        expr='income_statement["ebitda", @col] - income_statement["tax", @col]',
        style="total",
        format="currency",
        decimals=0,
    )

    # Attach audit note to net income exit year milestone
    is_tbl.set_cell(
        "net_income",
        "y2026e",
        expr='income_statement["ebitda", @col] - income_statement["tax", @col]',
        note="5-Year Strategic Exit Target Milestone",
    )

    return model


def test_scenario_1_e2e_cli_agent_scaffolding_and_guide(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """Verifies that the CLI module runner cleanly lists guides and scaffolds agent skills."""
    # 1. Guide topic list
    exit_code = cli_main(["guide", "--list"])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "Available topics:" in captured.out
    assert "syntax" in captured.out
    assert "functions" in captured.out
    assert "audit" in captured.out
    assert "all" in captured.out

    # 2. Agent skill initialization
    dest_workspace = tmp_path / "agent_workspace"
    exit_code = cli_main(["agent", "init", "--dest", str(dest_workspace)])
    assert exit_code == 0

    skill_root = dest_workspace / ".agents" / "skills" / "f8-modeling"
    assert (skill_root / "SKILL.md").is_file()
    assert (skill_root / "references" / "syntax-and-grid.md").is_file()
    assert (skill_root / "references" / "functions.md").is_file()
    assert (skill_root / "references" / "auditing-and-repair.md").is_file()


def test_scenario_2_e2e_model_compilation_and_table_auditing() -> None:
    """Verifies end-to-end model authoring, compilation, unboxing, and table auditing."""
    model = _build_institutional_lbo_model()
    compiled = model.compile()

    # Verify model summary
    summary = compiled.summary()
    assert summary["model_name"] == "AcmeCorpLBO"
    assert summary["total_sheets"] == 2
    assert summary["total_tables"] == 2
    assert summary["total_cells"] == 55
    assert summary["total_drivers"] == 30
    assert summary["total_formulas"] == 25

    # Verify table auditor
    is_auditor = compiled.table("income_statement")
    assert is_auditor.id == "income_statement"
    assert is_auditor.title == "Consolidated Statement of Operations"
    assert is_auditor.subtitle == "USD Thousands ($)"
    assert is_auditor.columns == ["line_item", "y2023a", "y2024e", "y2025e", "y2026e"]
    assert is_auditor.rows == ["rev", "cogs", "gp", "sga", "ebitda", "tax", "net_income"]
    assert is_auditor.shape == (7, 5)
    assert len(is_auditor) == 7

    # Verify unboxed mathematical calculations
    # 2023A (Inputs)
    assert is_auditor["rev", "y2023a"] == 125000.0
    assert is_auditor["cogs", "y2023a"] == 56250.0
    assert is_auditor["gp", "y2023a"] == 68750.0
    assert is_auditor["sga", "y2023a"] == 25000.0
    assert is_auditor["ebitda", "y2023a"] == 43750.0
    assert is_auditor["tax", "y2023a"] == 10937.5
    assert is_auditor["net_income", "y2023a"] == 32812.5

    # 2024E (Compound Growth + Margins from Assumptions table)
    # Revenue: 125000 * 1.15 = 143750
    assert is_auditor["rev", "y2024e"] == 143750.0
    # COGS: 143750 * 0.42 = 60375
    assert is_auditor["cogs", "y2024e"] == 60375.0
    # Gross Profit: 143750 - 60375 = 83375
    assert is_auditor["gp", "y2024e"] == 83375.0
    # SG&A: 143750 * 0.18 = 25875
    assert is_auditor["sga", "y2024e"] == 25875.0
    # EBITDA: 83375 - 25875 = 57500
    assert is_auditor["ebitda", "y2024e"] == 57500.0
    # Tax: 57500 * 0.25 = 14375
    assert is_auditor["tax", "y2024e"] == 14375.0
    # Net Income: 57500 - 14375 = 43125
    assert is_auditor["net_income", "y2024e"] == 43125.0

    # 2026E Exit Year
    assert is_auditor["rev", "y2026e"] == pytest.approx(177100.0)
    assert is_auditor["net_income", "y2026e"] == pytest.approx(59771.25)

    # Verify physical coordinates and lowered Excel formulas
    assert is_auditor.address("rev", "y2024e") == "D8"
    assert is_auditor.formula("rev", "y2024e") == "=C8*(1+'Assumptions'!D8)"
    assert is_auditor.address("net_income", "y2026e") == "F16"
    assert is_auditor.formula("net_income", "y2026e") == "=F14-F15"

    # Verify cell audit note
    cell_info = compiled.inspect_cell("income_statement", "net_income", "y2026e")
    assert cell_info.get("note") == "5-Year Strategic Exit Target Milestone"

    # Verify Markdown rendering across views
    md_values = is_auditor.to_markdown(view="values")
    assert "$ 125,000" in md_values
    assert "$ 143,750" in md_values
    assert "**Gross Revenue & Cost of Sales**" in md_values

    md_formulas = is_auditor.to_markdown(view="formulas")
    assert "=C8*(1+'Assumptions'!D8)" in md_formulas
    assert "=F14-F15" in md_formulas

    md_addresses = is_auditor.to_markdown(view="addresses")
    assert "C8" in md_addresses
    assert "F16" in md_addresses

    # Verify structured record export
    records = is_auditor.to_dict(orient="records")
    assert len(records) == 7
    assert records[0]["line_item"] == "Total Gross Revenue"
    assert records[0]["y2023a"] == 125000.0


def test_scenario_3_e2e_disk_export_and_compilation_parity(tmp_path: Path) -> None:
    """Verifies that on-disk export and direct disk compilation match
    in-memory results identically.
    """
    model = _build_institutional_lbo_model()
    compiled_mem = model.compile()
    is_mem = compiled_mem.table("income_statement")

    # Export project to disk
    proj_dir = tmp_path / "exported_lbo_project"
    model.export_project(proj_dir)

    manifest_file = proj_dir / "f8.json"
    assert manifest_file.is_file()
    assert (proj_dir / "tables" / "assumptions.json").is_file()
    assert (proj_dir / "tables" / "income_statement.json").is_file()

    # Compile directly from disk
    compiled_disk = compile_project(proj_dir)
    is_disk = compiled_disk.table("income_statement")

    # 100% parity assertion across all rows, columns, values, and coordinates
    for r in is_mem.rows:
        for c in is_mem.columns:
            mem_val = is_mem[r, c]
            disk_val = is_disk[r, c]
            assert mem_val == disk_val, f"Value mismatch at ({r}, {c})"

            mem_addr = is_mem.address(r, c)
            disk_addr = is_disk.address(r, c)
            assert mem_addr == disk_addr, f"Address mismatch at ({r}, {c})"

            mem_formula = is_mem.formula(r, c)
            disk_formula = is_disk.formula(r, c)
            assert mem_formula == disk_formula, f"Formula mismatch at ({r}, {c})"


def test_scenario_4_e2e_openxml_workbook_emission_and_inspection(tmp_path: Path) -> None:
    """Verifies OpenXML binary workbook synthesis, structure, live formulas, and cache results."""
    model = _build_institutional_lbo_model()
    compiled = model.compile()

    xlsx_path = tmp_path / "acme_lbo_model.xlsx"
    compiled.save(xlsx_path)

    assert xlsx_path.is_file()
    assert xlsx_path.stat().st_size > 0

    with zipfile.ZipFile(xlsx_path, "r") as zf:
        namelist = set(zf.namelist())
        assert "[Content_Types].xml" in namelist
        assert "xl/workbook.xml" in namelist
        assert "xl/styles.xml" in namelist
        assert "xl/worksheets/sheet1.xml" in namelist
        assert "xl/worksheets/sheet2.xml" in namelist

        sheet2_xml = zf.read("xl/worksheets/sheet2.xml").decode("utf-8")
        # Live formulas and cached evaluated results
        assert "<f>" in sheet2_xml
        assert "<v>" in sheet2_xml
        assert "Assumptions" in sheet2_xml
