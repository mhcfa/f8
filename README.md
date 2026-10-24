# f8

Financial modeling engine with compiler verification and live Excel formulas.

## Who is f8 for

1. **People who want AI to reliably build complex Excel models (financial and beyond)**: Give AI agents a semantic 2D DSL and compile-time verification to produce clean and error-free `.xlsx` workbooks with live formulas.
2. **People doing financial analysis in pure code (no Excel required)**: Run row-by-row financial statements, corkscrew schedules, and cross-table dependency graphs in Python (where columnar DataFrames fall short), with `compiled.save("model.xlsx")` ready whenever a stakeholder asks for the spreadsheet.

## Why f8

I am a CFA charterholder working in private M&A and project finance, where complex Excel models remain the non-negotiable deliverable despite fragile manual work, silent copy-paste errors, and zero version control.

AI agents struggle to manipulate raw spreadsheets (`openpyxl`, Office.js, or GUI) for two reasons:

- Brittle coordinates: Agents must reason in spatial cell addresses (`C42:G42`) instead of domain semantics; inserting one row shifts every downstream reference.
- No verification loop: Spreadsheet libraries do not evaluate formulas, leaving agents blind until the `.xlsx` file is opened and inspected.

`f8` turns the `.xlsx` workbook into a compiled artifact rather than a hand-edited canvas:

- Semantic 2D modeling: Author models with readable table, row, and column identifiers; `f8` handles spatial layout and lowers logic into native Excel formulas.
- Compiler-enforced rigor: A 6-pass Rust compiler verifies grid completeness (zero unassigned cells or formula overlaps) and enforces a strict DAG via Tarjan cycle detection.
- Closed-loop verification: Evaluates formulas in memory with 100% Excel parity (including the 1900 leap day bug) so AI agents and `pytest` suites can audit tables, trace dependencies, and verify accounting checks in milliseconds.

## Getting started

Install from PyPI (package name is `f8-engine`, imported as `f8`):

```bash
pip install f8-engine
# or
uv add f8-engine
```

If you are working with an AI coding agent, `f8` ships with a bundled modeling skill and CLI reference you can drop straight into your workspace:

```bash
f8 agent init    # scaffolds .agents/skills/f8-modeling/
f8 theme init    # generates a customizable theme.json in the workspace root
f8 guide --all   # streams the full DSL, function, and auditing guide to stdout
```

## Quick look

```python
from f8 import Model

model = Model("Operating Forecast")
sheet = model.add_sheet("Summary")

pnl = sheet.add_table("PnL", "Three-Year Projection", "($ in millions)")
pnl.add_column("item", role="descriptor", header="Line Item")
for year in ["2025A", "2026E", "2027E"]:
    pnl.add_column(year, header=year, format="currency", decimals=1)

pnl.add_row(
    "revenue",
    cells={"item": "Revenue", "2025A": 100.0},
    expr='PnL["revenue", col.prev] * 1.25',
)
pnl.add_row(
    "cogs",
    cells={"item": "Cost of Revenue"},
    expr='PnL["revenue", @col] * 0.30',
)
pnl.add_row(
    "gross_profit",
    cells={"item": "Gross Profit"},
    expr='PnL["revenue", @col] - PnL["cogs", @col]',
    style="total",
)

compiled = model.compile()

# Audit evaluated values in memory (or pass view="formulas" to inspect lowered Excel formulas)
print(compiled.table("PnL").to_markdown())

# Emit a live .xlsx workbook with native Excel formulas and styling (you don't have to)
compiled.save("forecast.xlsx")
```

Output:

```markdown
| Line Item        |     2025A |     2026E |      2027E |
| :--------------- | --------: | --------: | ---------: |
| Revenue          |    $100.0 |    $125.0 |     $156.2 |
| Cost of Revenue  |     $30.0 |     $37.5 |      $46.9 |
| **Gross Profit** | **$70.0** | **$87.5** | **$109.4** |
```

## Sample deliverables

Inspect compiled deliverables generated from a real-world model:

- [costco_financial_model.xlsx](https://github.com/mhcfa/f8/releases/latest/download/costco_financial_model.xlsx): Compiled financial model with live Excel formulas.
- [costco_valuation_report.pdf](https://github.com/mhcfa/f8/releases/latest/download/costco_valuation_report.pdf): Valuation report rendered directly from the compiled `f8` model using Quarto, Typst, and Matplotlib.

> Disclaimer: Sample deliverables are AI-generated technical demonstrations and do not constitute financial analysis or investment advice.

## Trade-offs

`f8` treats code as the single source of truth, and `.xlsx` is just an output. So there are some trade-offs to consider:

| What you gain                                     | What you lose                                                       |
| ------------------------------------------------- | ------------------------------------------------------------------- |
| Clean structure and strict schema                 | Hidden mistakes no one notices                                      |
| Git and line-by-line diffs                        | `Model_v14_Final_(2).xlsx` and guessing what changed since v13      |
| Instant DAG and completeness verification in Rust | The opportunity to mess up your model right before a client meeting |
| Superior AI modeling                              | Being an Excel Ninja                                                |
| Readable semantic code and logic                  | The joy of hunting down a hidden `#REF!` at 2 AM                    |

Jokes aside, writing models in code gives you 40 years of software infrastructure out of the box, line-by-line `git diff` reviews, automated `pytest` checks, reusable modules, and parametric loops. The only real downsides I can think of are:

1. You can't do whatever you want when it comes to layout in the emitted workbook; everything has to be structured. But you wouldn't want your spreadsheet to be a mess anyway, right?
2. You have to learn some Python. But that is largely solved by AI now.
3. Constantly re-compiling just to peek at the Excel workbook can be a bit annoying, so we provide in-memory inspection methods in Python. Once you get used to `print()`, you don't need to open the `.xlsx` all the time (or just ask AI to do it).
4. There are no circular references or iterative solvers (as per FAST Standard Section 2.01). There is a closed-form formula that solves the revolver circularity problem without circular references anyway (and don't worry, AI knows it).
5. There is no native Excel chart emission yet (on roadmap), and no Pivot Tables. But since you are already in Python, `pandas` or `polars` handles data aggregation much better.

## Architecture

Under the hood, `f8` is a Python SDK (`python/f8/`) backed by a 6-pass Rust compiler (`crates/f8-core/`):

`Spec -> IR Expansion -> DAG & Cycle Check -> Topological Evaluation -> Spatial Layout -> Formula Lowering -> .xlsx Emission`

- Architecture overview: `docs/architecture.md`
- Python modeling skill: `python/f8/skills/f8-modeling/SKILL.md`
- Syntax, grid, and formatting reference: `python/f8/skills/f8-modeling/references/syntax-and-grid.md`
- Function reference: `python/f8/skills/f8-modeling/references/functions.md`
- Auditing and repair reference: `python/f8/skills/f8-modeling/references/auditing-and-repair.md`

## License

- Licensed under the GNU Affero General Public License v3.0 (`AGPL-3.0-only`); see `LICENSE`.
- Copyright (C) 2026 Michael (Chencheng) Hu, CFA (`@mhcfa`).
- Output ownership: `.xlsx` workbooks and financial models generated by `f8` are standalone user outputs and are not subject to `AGPL-3.0-only` copyleft terms.
