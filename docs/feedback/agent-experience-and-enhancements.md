# f8 Agent Experience & Enhancement Proposals

> **Document Status**: Production Feedback & Architectural Enhancement Proposals  
> **Source**: Antigravity AI Agent (Pair Programming on PKT Group Financial Model)  
> **Target Audience**: `f8` Core Compiler & Tooling Engineering Team  
> **Context**: Real-world stress testing across a 15-sheet, 26-table, 1,926-cell cross-linked M&A statutory consolidation model.

---

## 1. Executive Summary & Praise

As an AI agent acting as a first-class user of `f8`, the core developer experience is **exceptionally strong**:

- **Predictable, Deterministic Compiler**: The topological sort and DAG cycle detector validated 1,926 cross-sheet dependencies in under 40ms. When cycles or syntax errors occur, the compiler halts immediately with clear diagnostics rather than silently emitting broken spreadsheets.
- **True Orthogonal Matrix IR**: Decoupling logical coordinates (`Table["row", @col]`) from physical grid cell addresses (`Revenue Bridge!C8`) allows complex multi-entity consolidation models to be built without manual grid layout arithmetic.
- **High-Fidelity OpenXML Emission**: The generated `.xlsx` workbooks feature clean typography, cohesive theme palettes, proper number formatting, and native Excel formulas that update dynamically upon editing.

During a complex refactoring task (replacing hardcoded non-store adjustments with an entity-by-entity dynamic reconciliation bridge), the agent verified the compiled output by unzipping `PKT_Group_Historical_Financial_Model.xlsx` with a Python script. 

This document outlines **why that external verification was performed**, identifies **gaps in the current CLI inspection tooling**, and proposes concrete **CLI enhancements and a Python SDK design** to make `f8` entirely self-contained for agents and human analysts.

---

## 2. Why the Agent Unzipped the `.xlsx` (The Verification Gap)

The decision to unzip the compiled `.xlsx` and inspect `xl/worksheets/sheet4.xml` via Python was driven by two specific limitations:

### A. Lack of Batch Inspection for Lowered Excel Formulas
- **`f8 inspect table <Table>`** displays the 2D table grid, but only shows **evaluated numerical values** in table coordinate space (`[line_item, fy2023]`). It does not show the lowered Excel formulas (e.g., `=C7+C8` or `='PKT Wholesale'!$C$7-'Store Sales & SSSG'!$H$8*1000000`).
- **`f8 inspect cell <Table> <Row> <Col>`** displays both the physical sheet address (`Revenue Bridge!C8`) and the lowered Excel formula (`=C35`), but it operates **strictly on a 0D point (one single cell at a time)**.
- **The Gap**: When refactoring 18 dynamic rows across two stacked tables (`REV_RECON` and `REV_RECON_ENTITY`), running `f8 inspect cell` 18 separate times was prohibitive for agent token consumption and latency. A single Python script reading the worksheet XML verified all formula strings in a single pass.

### B. Emitted OpenXML Package Parity (`<f>` vs `<v>`)
- `f8 inspect` runs on the in-memory compiler AST / IR representation.
- In financial due diligence, workbooks are often opened by third-party spreadsheet viewers, Google Sheets, web previewers, or Excel instances configured with *Manual Calculation*.
- For a spreadsheet to render correctly in all environments without requiring the user to press `F9` (Calculate Now), every formula cell `<c>` must contain:
  1. `<f>`: The formula string for native recalculation.
  2. `<v>`: The pre-calculated cached value emitted by the compiler.
- Unzipping the archive confirmed that `f8` correctly emits both `<f>` and `<v>`, avoiding `#VALUE!` or blank displays for external parties.

---

## 3. Proposed CLI Enhancements

Implementing the following commands and flags will make external Python verification completely redundant:

### Proposal 1: Formula & Address Flags for `f8 inspect table`

Extend `f8 inspect table` with view mode flags:

```bash
# 1. View lowered Excel formulas across the table
f8 inspect table REV_RECON --formulas
# Outputs the table grid displaying `=C7+C8`, `='Store Sales & SSSG'!H29*1000000`, etc.

# 2. View physical Excel grid coordinates
f8 inspect table REV_RECON --addresses
# Outputs the table grid displaying `[C7]`, `[C8]`, `[C9]`, etc.

# 3. Combined audit view
f8 inspect table REV_RECON --view full
# Shows a structured breakdown per cell: Coord | Address | Evaluated Value | Formula
```

### Proposal 2: Sheet-Level Physical Grid Inspection

In `f8`, a single sheet frequently hosts **multiple stacked tables** (e.g., `REV_RECON` + `REV_RECON_ENTITY`, or `PL_WW` + `BS_WW`), separated by blank rows and headers.

Add a subcommand to inspect the sheet as rendered in the physical grid:

```bash
f8 inspect sheet "Revenue Bridge" [--values | --formulas | --addresses]
```

**Expected Output**:
```text
Sheet: Revenue Bridge (Physical Range: B4:E37)
-----------------------------------------------------------------------------------------
Row | Col B (Line Item)                    | Col C (FY2023)        | Col D (FY2024)
-----------------------------------------------------------------------------------------
 7  | Store-Level POS Counter Sales        | ='Store Sales'!H29*1M | ='Store Sales'!I29*1M
 8  | Audited Non-Store Adjustments        | =C35                  | =D35
 9  | Audited Gross Statutory Revenue      | =C7+C8                | =D7+D8
...
 25 | --- Table: REV_RECON_ENTITY ---      |                       |
 27 | PKT Wholesale Warehouse (PT)         | ='PKT Wholesale'!$C$7 | ='PKT Wholesale'!$D$7
...
 35 | Total Non-Store Adjustments          | =SUM(C$27:C$34)       | =SUM(D$27:D$34)
-----------------------------------------------------------------------------------------
```

### Proposal 3: OpenXML Artifact Linter (`f8 verify`)

Add an automated verification subcommand that inspects the compiled `.xlsx` archive directly:

```bash
f8 verify PKT_Group_Historical_Financial_Model.xlsx
```

**Checks Performed**:
1. **Formula / Value Parity**: Confirms 100% of `<f>` formula cells have a corresponding cached `<v>` value.
2. **Error Scan**: Scans all cached values for `#REF!`, `#VALUE!`, `#NAME?`, `#DIV/0!`, or `#N/A`.
3. **OpenXML Integrity**: Verifies sheet relationship IDs (`xl/_rels/workbook.xml.rels`), shared string indexing, and theme style references.
4. **Flag Option**: Enable automatic verification on compile via `f8 compile -p model -o model.xlsx --verify`.

### Proposal 4: Cell Classification Map (`--audit`)

Financial model auditors and investment bankers always demand an "Input vs. Calculation Map":

```bash
f8 inspect table REV_RECON --audit
```

**Output**:
- `[Input]`: Hardcoded driver literal (e.g., `im_turnover`).
- `[Formula]`: Intra-table calculation (e.g., `gross_audited = pos_sales + non_store_adj`).
- `[Cross-Link]`: Reference to another sheet/table (e.g., `pos_sales = STORE_SALES_SSSG[...]`).
- `[Subtotal]`: Range aggregation (e.g., `SUM(...)`).

---

## 4. Python SDK Ergonomics (The Authoring Layer)

The authoring model currently relies on manual JSON authoring across `model/f8.json` and `model/tables/*.json`. While JSON is an ideal compiler intermediate representation, it introduces friction for AI agents and human analysts:

### A. Pain Points of Raw JSON Authoring
1. **Quote Escaping in Formulas**: Formula expressions require heavy escaping, e.g.:
   ```json
   "expr": "PL_WB[\"revenue\", @col] - (STORE_SALES_SSSG[\"store_wb\", @col] * 1000000)"
   ```
   A single unescaped quote or missing bracket invalidates the JSON payload.
2. **Boilerplate Overhead**: Repeating schema keys (`kind`, `id`, `role`, `headers`, `format`, `decimals`, `cells`) burns substantial LLM token context.
3. **No Dynamic Autocompletion**: Renaming an upstream table or column requires global grep/replace across disparate JSON files.

### B. Proposed Python SDK Design

A lightweight, fluent Python SDK (e.g., `pyf8` or `f8-python`) will dramatically improve authoring speed and reliability:

```python
from f8 import Model, Sheet, Table, Expr, Col, Row

# 1. Initialize Model
model = Model(name="PKT_Group_Historical_Financial_Model", theme="corporate_navy")

# 2. Add Sheet
sheet = model.add_sheet("Revenue Bridge", color="#385D8A")

# 3. Add Master Bridge Table
rev_recon = sheet.add_table(
    id="REV_RECON",
    title="PKT Group - POS Store Sales to Audited Revenue Reconciliation Bridge",
    columns=[
        Col.descriptor("line_item", "Reconciliation Layer / Step"),
        Col.measure("fy2023", ["Audited Actuals", "FY2023 (RM)"]),
        Col.measure("fy2024", ["Audited Actuals", "FY2024 (RM)"]),
        Col.measure("fy2025", ["Audited Actuals", "FY2025 (RM)"]),
    ]
)

# 4. Declarative Dynamic Rows (Natural Python expressions)
rev_recon.add_row(
    id="pos_sales",
    label="Store-Level POS Counter Sales (Front-End Till Register)",
    expr=Expr("STORE_SALES_SSSG['total_group_sales', @col] * 1000000"),
    format="number",
    decimals=2
)

rev_recon.add_row(
    id="non_store_adj",
    label="Audited Non-Store Adjustments (Sum of 8 Entity Variances)",
    expr=Expr("REV_RECON_ENTITY['total_non_store', @col]"),
    format="number",
    decimals=2
)

rev_recon.add_row(
    id="gross_audited",
    label="Audited Gross Statutory Revenue (Sum of 8 Entity Reports)",
    expr=Expr("REV_RECON['pos_sales', @col] + REV_RECON['non_store_adj', @col]"),
    style="subtotal",
    format="number",
    decimals=2
)

# 5. Compile directly to OpenXML or emit canonical f8.json project
model.compile("PKT_Group_Historical_Financial_Model.xlsx")
# OR: model.export_project("./model")
```

#### Key Benefits of Python SDK for AI Agents:
- **Zero Quote Escaping**: Formulas can be passed as raw strings or python expressions.
- **Type Checking & Validation**: Pydantic/dataclasses catch missing parameters before disk writes.
- **Programmatic Generation**: Repeating loops (e.g., generating 8 identical entity P&L tables) take 10 lines of Python instead of 8 separate 200-line JSON files.

---

## 5. Diagnostic & Error Feedback Enhancements

During testing, one compiler diagnostic was encountered:
```text
Error: Intermediate representation error: Table 'PL_CONSOL': format conflict at row 'gross_margin' (percentage) and column 'fy2023a' (number)
```

### Recommendation on Formatting Specificity:
In the f8 documentation ([`docs/authoring-guide.md`](file:///home/mhcfa/Work/f8/docs/authoring-guide.md)), the three-tier specificity lattice is defined as:
$$\text{Cell override (0D point)} > \text{Vector rule (1D row or column)} > \text{Explicit null}$$

When a row defines `format: percentage` and a column defines `format: number`, this currently triggers an unresolvable format conflict error. 

**Suggested Enhancement**:
1. **Row-Priority Resolution**: In financial statement modeling, columns almost universally represent periods (defaulting to currency/number), while specific rows (margins, growth rates, tax rates) represent percentages. Allowing row format to cleanly take precedence over column format—or providing an explicit priority rule—would prevent authoring friction.
2. **Actionable Remediation Hints**: When returning the error, suggest the exact fix:
   ```text
   Hint: Remove 'format' from column 'fy2023a' or override specific cells to resolve format collision.
   ```

---

## 6. Conclusion

`f8` is on track to become an industry-defining standard for AI-driven financial engineering. Addressing the physical grid inspection gap, adding OpenXML artifact validation, and introducing a Python SDK will make `f8` the uncontested best-in-class tool for autonomous agents building institutional-grade financial models.
