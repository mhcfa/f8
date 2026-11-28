# f8

**Build, verify, and automate financial models in code with AI, compiled into live Excel.**

I'm a CFA charterholder working in private M&A and project finance. I built `f8` because I believe **code is the best way to leverage AI for financial modeling**. As a bonus, moving your models into code gives you 40 years of software engineering infrastructure (version control, automated tests, reusable modules, and live data pipelines) out of the box.

## The problem

Excel is still the non-negotiable deliverable across finance, but today's ways of bringing AI or code into financial modeling each hit a wall:

- **AI inside Excel (add-ins, Office.js, GUI agents):** Still inherits everything painful about Excel — fragile grid edits, silent copy-paste errors, no version control or diffs, no automated tests, and awkward links to external data. Making AI click cells faster doesn't fix the foundation.
- **Low-level workbook libraries (`openpyxl`, `xlsxwriter`, `exceljs`):** You're in code, but you (and your AI agent) are still babysitting raw cell addresses (C42:G42) where inserting one row breaks downstream formulas, more importantly, none of these libraries evaluate formulas, so your agent is flying blind.
- **DataFrames (`pandas`, `polars`):** Great for data pipelines, awkward for row-by-row 2D schedules (3-statement links, corkscrews, debt waterfalls), and they can't give your stakeholders a live, formula-driven .xlsx file.

## How f8 works

`f8` makes code your single source of truth, and turns .xlsx into a compiled output:

- **Semantic 2D modeling:** Write formulas using readable names (`PPE["end_balance", col.prev]`). `f8` handles the grid layout and compiles your logic into native Excel A1 formulas.
- **Compiler-enforced rigor:** A 6-pass Rust compiler checks types, shapes, grid completeness (no forgotten cells or formula overlaps), and blocks circular references via Tarjan cycle detection.
- **Closed-loop verification:** Evaluates every formula in memory in milliseconds, so your AI agent knows what went wrong immediately.
- **Verified Excel parity:** Matches Excel across all 37 supported functions, including error propagation and the 1900 leap-day quirk. Our uncached [parity workbook](https://github.com/mhcfa/f8/releases/latest/download/excel_parity_verification.xlsx) lets Excel recalculate and grade every test case itself when you open it.

## Who is f8 for

- **Anyone who wants AI to build real financial models:** Whether you're doing M&A and LBOs, fundamental equity research, project finance, FP&A, credit, or real estate — give your AI agent a real language and compiler so you get models you can actually diff, test, and hand to a client or IC as a live `.xlsx`.
- **Quants and code-first analysts:** Keep your financial statements and 2D schedules right next to live data feeds (SEC filings, market APIs, databases) and quant models in Python or TypeScript — with `compiled.save("model.xlsx")` ready whenever someone asks for the spreadsheet.
- **Complex modeling beyond finance:** Pricing models, operational forecasts, scenario planning — anything where you want software-grade reliability without giving up Excel as the deliverable.

## Getting started

Install `f8-engine` in Python (imported as `f8`) or TypeScript / JavaScript:

```bash
# Python
pip install f8-engine   # or: uv add f8-engine

# TypeScript / JavaScript
npm install f8-engine   # or: pnpm add f8-engine
```

Using an AI coding agent? `f8` ships with a bundled modeling skill and CLI reference you can drop right into your workspace:

```bash
f8 agent init    # scaffolds .agents/skills/f8-modeling/
f8 theme init    # generates a customizable theme.json in your workspace root
f8 guide --all   # streams the full DSL, layout, function, and auditing guide to stdout
```

## Quick look

```python
from f8 import Model

years = ["2025A", "2026E", "2027E"]
capex = {"2025A": 25.0, "2026E": 30.0, "2027E": 35.0}
depr = {"2025A": -15.0, "2026E": -18.0, "2027E": -22.0}

model = Model("Operating Forecast")
sheet = model.add_sheet("Summary")

ppe = sheet.add_table("PPE", "PP&E Corkscrew Schedule", "($ in millions)")
ppe.add_column("item", role="descriptor", header="Line Item")
for year in years:
    ppe.add_column(year, header=year, format="currency", decimals=1)

ppe.add_row(
    "beg_balance",
    cells={"item": "Beginning Balance", "2025A": 100.0},
    expr='PPE["end_balance", col.prev]',
)
ppe.add_row("capex", cells={"item": "Capital Expenditures", **capex})
ppe.add_row("depreciation", cells={"item": "Depreciation", **depr})
ppe.add_row(
    "end_balance",
    cells={"item": "Ending Balance"},
    expr='SUM(PPE["beg_balance".."depreciation", @col])',
    style="total",
)

compiled = model.compile()

# Audit evaluated values in memory (or pass view="formulas" to inspect lowered Excel formulas)
print(compiled.table("PPE").to_markdown())

# Emit a live .xlsx workbook with native Excel formulas and styling (you don't have to)
compiled.save("forecast.xlsx")
```

<details>
<summary>TypeScript equivalent</summary>

```typescript
import { Model } from "f8-engine"

const years = ["2025A", "2026E", "2027E"] as const
const capex = { "2025A": 25.0, "2026E": 30.0, "2027E": 35.0 }
const depr = { "2025A": -15.0, "2026E": -18.0, "2027E": -22.0 }

const model = new Model({ name: "Operating Forecast" })
const sheet = model.addSheet("Summary")

const ppe = sheet.addTable("PPE", "PP&E Corkscrew Schedule", {
  subtitle: "($ in millions)",
})
ppe.addColumn("item", { role: "descriptor", header: "Line Item" })
for (const year of years) {
  ppe.addColumn(year, { header: year, format: "currency", decimals: 1 })
}

ppe.addRow("beg_balance", {
  cells: { item: "Beginning Balance", "2025A": 100.0 },
  expr: 'PPE["end_balance", col.prev]',
})
ppe.addRow("capex", { cells: { item: "Capital Expenditures", ...capex } })
ppe.addRow("depreciation", { cells: { item: "Depreciation", ...depr } })
ppe.addRow("end_balance", {
  cells: { item: "Ending Balance" },
  expr: 'SUM(PPE["beg_balance".."depreciation", @col])',
  style: "total",
})

const compiled = model.compile()

// Audit evaluated values in memory (or pass { view: "formulas" } to inspect lowered Excel formulas)
console.log(compiled.table("PPE").toMarkdown())

// Emit a live .xlsx workbook with native Excel formulas and styling (you don't have to)
compiled.save("forecast.xlsx")
```

</details>
<details>
<summary>Output</summary>

```markdown
| Line Item            |      2025A |      2026E |      2027E |
| :------------------- | ---------: | ---------: | ---------: |
| Beginning Balance    |     $100.0 |     $110.0 |     $122.0 |
| Capital Expenditures |      $25.0 |      $30.0 |      $35.0 |
| Depreciation         |    ($15.0) |    ($18.0) |    ($22.0) |
| **Ending Balance**   | **$110.0** | **$122.0** | **$135.0** |
```

</details>

## Sample deliverables

### Autonomous equity research models ([mhcfa/faac](https://github.com/mhcfa/faac))

Full TypeScript financial models, audited SEC/macro data bundles, compiled `.xlsx` workbooks, and valuation decks generated by autonomous multi-agent workflows live in [mhcfa/faac](https://github.com/mhcfa/faac):

| Engagement             | Model                                                                                          | Excel Model                                                                                                                                        | Valuation Deck                                                                                                                         |
| :--------------------- | :--------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------- |
| **Costco (`COST`)**    | [`public-equity/costco`](https://github.com/mhcfa/faac/tree/main/public-equity/costco)         | [`costco-valuation.xlsx`](https://github.com/mhcfa/faac/blob/main/public-equity/costco/deliverables/costco-valuation.xlsx)                         | [`costco-equity-valuation.pdf`](https://github.com/mhcfa/faac/blob/main/public-equity/costco/deliverables/costco-equity-valuation.pdf) |
| **Boeing (`BA`)**      | [`public-equity/boeing`](https://github.com/mhcfa/faac/tree/main/public-equity/boeing)         | [`boeing-valuation-model.xlsx`](https://github.com/mhcfa/faac/blob/main/public-equity/boeing/deliverables/boeing-valuation-model.xlsx)             | [`boeing-initiation-deck.pdf`](https://github.com/mhcfa/faac/blob/main/public-equity/boeing/deliverables/boeing-initiation-deck.pdf)   |
| **Meta (`META`)**      | [`public-equity/meta`](https://github.com/mhcfa/faac/tree/main/public-equity/meta)             | [`meta_model.xlsx`](https://github.com/mhcfa/faac/blob/main/public-equity/meta/deliverables/meta_model.xlsx)                                       | [`meta_presentation.pdf`](https://github.com/mhcfa/faac/blob/main/public-equity/meta/deliverables/meta_presentation.pdf)               |
| **Salesforce (`CRM`)** | [`public-equity/salesforce`](https://github.com/mhcfa/faac/tree/main/public-equity/salesforce) | [`salesforce_financial_model.xlsx`](https://github.com/mhcfa/faac/blob/main/public-equity/salesforce/deliverables/salesforce_financial_model.xlsx) | [`salesforce-initiation.pdf`](https://github.com/mhcfa/faac/blob/main/public-equity/salesforce/deliverables/salesforce-initiation.pdf) |

- Costco write-up: [Running an Autonomous Agent on Costco Equity Research: Why It Called a Short](https://mhcfa.github.io/blog/autonomous-costco-equity-research/) (4-hour, ~$70 autonomous run across 50 SEC and macro sources with zero human intervention)

### LBO model preview

![LBO Executive Dashboard](https://github.com/mhcfa/f8/releases/latest/download/lbo-dashboard.png)

![LBO Transaction Balance Sheet Bridge](https://github.com/mhcfa/f8/releases/latest/download/lbo-transaction.png)

> Disclaimer: Sample deliverables are AI-generated technical demonstrations and do not constitute financial analysis or investment advice.

## Trade-offs

Because `f8` treats code as the source of truth and `.xlsx` as just an output, there are a few trade-offs to keep in mind:

1. **Structured layout only:** You can't scatter ad-hoc cells wherever you want in the emitted workbook — everything is verified by strict schemas. (You wouldn't want a messy spreadsheet anyway, right?)
2. **Basic Python or TypeScript:** You'll be working in code, though AI largely takes care of that for you.
3. **No circular references:** Per FAST Standard Section 2.01. Revolver circularity has a clean closed-form formula anyway (and don't worry, AI knows it).
4. **No native charts or Pivot Tables:** Charts are on the roadmap, and since you're already in Python or TypeScript, your language ecosystem handles aggregation far better than Pivot Tables.
5. **One-way flow (`code -> Excel`):** Manual edits to the emitted `.xlsx` won't sync back and will be overwritten on the next compile. By design, `.xlsx` is a build artifact, not a source file.

## Architecture

Under the hood, `f8` pairs Python (`python/f8/`) and TypeScript/WebAssembly (`ts/`, `crates/f8-wasm/`) SDKs with a 6-pass Rust compiler (`crates/f8-core/`):

`Spec -> IR Expansion -> DAG & Cycle Check -> Topological Evaluation -> Spatial Layout -> Formula Lowering -> .xlsx Emission`

See `docs/architecture.md` for the full pipeline and invariants.

## License

- Licensed under the Apache License, Version 2.0 (`Apache-2.0`); see `LICENSE`.
- Copyright (C) 2026 Michael (Chencheng) Hu, CFA (`@mhcfa`).
