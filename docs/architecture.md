# Architecture Overview

Deterministic financial model compiler treating spreadsheets as compiled binary artifacts.

## 1. Core Philosophy & Architectural Invariants

- Code as single source of truth: Declarative Python DSL and structured specifications.
- Explicit core data structures: Exactly one canonical representation per concept. Zero implicit fallbacks, hidden defaults, or syntactic magic in core engine.
- Agent-first intermediate representation (IR): Orthogonal 2D matrix IR optimized for AI agent synthesis and deterministic verification over typing brevity. Ergonomic scaffolding isolated to Python SDK.
- Pure in-memory calculation: Engine (`crates/f8-core`) operates 100% in-memory with in-memory `.xlsx` generation. Zero filesystem I/O in core; disk writes (`.xlsx` emission via `save()`) isolated to outer Python bindings (`crates/f8-python`).
- Orthogonal 2D matrix paradigm: General `R x C` table matrix. Columns represent time series, categories, or metrics. Every cell `Table[row, col]` is a first-class calculation node.
- Excel as compile artifact: Emitted `.xlsx` workbooks contain live, auditable formulas and formatting, functioning like compiled binaries or PDF reports.
- 100% Excel calculation and syntax parity: Exact calculation parity for supported and future functions, IEEE-754 normalization, mixed-type comparison ranking (`Number < Text < Boolean`), Excel error bubbling, and 1900 leap day bug reproduction.
- No volatile or dynamic array functions: Excludes volatile functions (`NOW`, `TODAY`, `RAND`, `OFFSET`, `INDIRECT`) to guarantee pure determinism. Excludes runtime dynamic array spilling (`FILTER`, `SORT`, `UNIQUE`, `#` spill operator) to maintain static 2D layout planning and cell-level DAG verification.
- Pure DAG compliance: Zero circular dependencies permitted; strict FAST Standard Section 2.01 compliance.
- Tarjan SCC firewall: Cycles fail fast with semantic trace (`CellA -> CellB -> CellA`).
- Closed-form modeling: Simultaneous equations (e.g. debt revolving interest sweeps) require closed-form algebraic formulations rather than iterative spreadsheet calculation.
- Bit-for-bit determinism: Identical JSON specifications produce identical formula grids, cell formats, and OpenXML byte buffers.

## 2. Compilation & Evaluation Pipeline

6-pass sequential pipeline:

```text
Pass 1: Ingest & Parse       (crates/f8-core/src/model/)
  │
Pass 2: IR Grid Expansion    (crates/f8-core/src/compiler/ir/)
  │
Pass 3: Dependency DAG & SCC (crates/f8-core/src/compiler/graph/)
  │
Pass 4: Topological Eval     (crates/f8-core/src/compiler/eval/)
  │
Pass 5: Spatial Layout       (crates/f8-core/src/compiler/layout/)
  │
Pass 6: Lowering & Emission  (crates/f8-core/src/compiler/lowering/, crates/f8-core/src/compiler/emission/)
```

### Pass Details

1. Ingest & Parse (`f8_core::model`):
   - Deserializes JSON specifications into strongly typed AST structures.
   - Validates schema invariants: unique sheet/table/axis IDs, non-empty dimensions, non-empty subtitles.
   - Validates formatting invariants: maximum 14 decimal places (`decimals <= 14`), precision allowed only on decimal-supporting numeric presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`), currency symbols allowed only on `currency` preset, valid 6-character hex colors (`#RRGGBB`), and positive finite layout metrics.
2. Grid Expansion & Formula Parsing (`f8_core::compiler::ir`):
   - Expands tables into discrete cells identified by `CellId { table, row, col }`.
   - Parses formula strings into `Expr` AST nodes via recursive-descent Pratt parser.
   - Enforces 3-tier specificity lattice:
     - Specificity: `0D Cell Override` > (`1D Row Rule` XOR `1D Column Rule`) > `Explicit Null / Error`.
     - Conflicting row and column expressions halt compilation (`IrError::AmbiguousExpressionConflict`).
     - Unassigned calculable cells halt compilation (`IrError::UnassignedCell`).
   - Isolates `descriptor` columns: vector formulas never broadcast into descriptor columns.
   - Resolves cell formatting, indentation, notes, visual highlights, and background zone shades (`row.shade` vs `col.shade`; conflicting active shades halt with `IrError::ShadeConflict`, while `shade: "none"` on a cell clears inherited shades).
   - Resolves cell currency: Point override > Vector rules (`row.currency` vs `col.currency`; conflicting codes halt with `IrError::CurrencyConflict`) > `table.currency` > default `usd`.
3. Dependency Graph & SCC (`f8_core::compiler::graph`):
   - Resolves cell references: table-qualified keys (`Table[row, col]`), active coordinate selectors (`@col`, `@row`), and temporal offsets (`col.prev(k)`, `col.next(k)`, `row.prev(k)`, `row.next(k)`).
   - Column offset semantics: steps strictly across data measure columns, skipping gutters and descriptor columns.
   - Row offset semantics: steps strictly across data rows, skipping gutters and section headers.
   - Cycle detection: executes Tarjan's Strongly Connected Components (SCC) on `petgraph::DiGraph`. Cycles halt compilation with exact cycle trace (`StrictDagHandler`).
   - Topological sorting: Kahn's algorithm with declaration-order tie-breaking for deterministic execution order.
4. Topological Evaluation (`f8_core::compiler::eval`):
   - Topologically evaluates cells in topological order using `FunctionRegistry`.
   - IEEE-754 normalizations: `-0.0` normalized to `0.0`.
   - Excel type coercions: `Empty` coerces to `0.0` (numeric) or `""` (string); booleans coerce to `1.0`/`0.0`.
   - Excel error semantics: division by zero or empty returns `#DIV/0!`, invalid text arithmetic returns `#VALUE!`, invalid math arguments return `#NUM!`, failed lookups return `#N/A`.
   - Mixed-type comparison ranking: `Number < Text < Boolean`; case-insensitive ASCII string comparisons.
   - Lazy evaluation: short-circuits dormant branches in `IF`, `IFS`, `IFERROR`, `IFNA`, `CHOOSE`, `SWITCH`, and `XLOOKUP`.
   - Error tolerance: runtime errors stored as `CellValue::Error` without aborting compiler; downstream formulas bubble or handle them.
5. Spatial Layout Planning (`f8_core::compiler::layout`):
   - Arranges tables into 0-indexed physical grid coordinates (`LayoutMap`).
   - Sheet margins: Col 0 (Excel Col A) and Row 0 (Excel Row 1) reserved as blank gutters.
   - Table placement: primary table placed at `(col: 1, row: 1)` (Excel `B2`); stacked tables separated by 2 blank gutter rows.
   - Vertical table anatomy: optional title banner, optional subtitle, title gutter, multi-tier column headers (bottom-aligned, height `H = max(headers.len())`), body rows.
   - Sheet-wide column width arbitration: `max(chars * char_width_multiplier + col_padding, min_col_width)` computed across all stacked tables sharing each physical column. Indentation adds `indent * 2` character width padding.
6. Formula Lowering & Emission (`f8_core::compiler::lowering` & `f8_core::compiler::emission`):
   - Pass 6a (Lowering): translates AST `Expr` to Excel formula strings prefixed with `=`. Dynamic coordinates (`@col`, `@row`, `prev`, `next`) lowered to relative A1 coordinates; static keys lowered to `$` absolute coordinates. Cross-sheet references formatted as `'Sheet'!Address`.
   - Pass 6b (Emission): writes `.xlsx` byte buffers via `rust_xlsxwriter`.
   - Format deduplication: OpenXML `<cellXfs>` synthesized and cached via `FormatCache`.
   - Color precedence: literal inputs styled with `input_text` (blue) overriding highlight/total fills; formula cells styled with `formula_text` (black); descriptors styled with `data_text`.
   - Presentation layout: super-headers centered across runs via `FormatAlign::CenterAcross` (zero merged cells); opt-in row outline grouping up to level 7 (`table.collapsible`); pre-populated formula `<v>` value cache tags (configurable via `EmissionOptions`).

### Unified Inspection Facade (`CompiledModel`)

In-memory artifact consolidating all pass outputs for inspection without re-running passes:

- State:
  - `manifest`: Parsed `ProjectManifest`.
  - `tables`: `IndexMap<TableId, TableSpec>`.
  - `theme`: Resolved visual and typography `Theme`.
  - `symbol_table`: Registered table and sheet metadata.
  - `grid`: Expanded `CellGrid`.
  - `graph`: Validated `DependencyGraph`.
  - `evaluated`: Topologically evaluated `EvaluatedModel`.
  - `layout_map`: Physical 2D `LayoutMap`.
  - `lowered_map`: Lowered concrete Excel formula map (`LoweredFormulaMap`).
- Core inspection API:
  - `emit_workbook(options)`: Generates `.xlsx` byte buffer (`Vec<u8>`).
  - `summary()`: Model-, sheet-, and table-level cell, driver, and formula counts (`ModelSummary`).
  - `cell_value(table, row, col)`: Fast `O(1)` runtime scalar lookup.
  - `cell_formatted(table, row, col)`: Fast `O(1)` formatted display string lookup.
  - `inspect_cell(table, row, col)`: Detailed cell card (values, styles, AST, Excel formula, A1 coordinate, precedents, dependents).
  - `inspect_table(table)`: 2D table grid with column headers, row descriptors, and cell records.
  - `inspect_sheet(sheet)`: 2D worksheet grid compositing stacked tables, bounding boxes, and column widths.
  - `precedents_of(cell, recursive)`: Direct or transitive upstream dependency tree (`DependencyTree`).
  - `dependents_of(cell, recursive)`: Direct or transitive downstream blast-radius tree (`DependencyTree`).
  - `parse_coordinate(target, row, col)`: Resolves 3 positional tokens or canonical IR string (`Table["row", "col"]`) into `CellId`.

## 3. Structural Hierarchy

- Workbook: Root manifest defining global model metadata, themes, and sheet ordering.
- Sheets: Worksheet containers managing tab color, freeze panes, gridlines, and stacked table layout.
- Tables: Named 2D matrices indexed by `[RowId, ColId]`. Globally unique IDs across workbook.
- Cells: Discrete calculation nodes uniquely addressed by `CellId { table, row, col }`.

## 4. 3-Tier Separation of Concerns

Strict decoupling of semantic identity, visual presentation, and physical grid location:

| Tier                   | Table Axis                  | Row Axis                                 | Column Axis                    | Responsibility                             | Mutability                                 |
| :--------------------- | :-------------------------- | :--------------------------------------- | :----------------------------- | :----------------------------------------- | :----------------------------------------- |
| **1. Identity (`id`)** | `id: "IncomeStatement"`     | `id: "revenue"`                          | `id: "2024E"`                  | Logic, formulas, AST, DAG                  | Immutable programmatic ID                  |
| **2. Presentation**    | `title: "Income Statement"` | Descriptor cell (`line_item: "Revenue"`) | `headers: ["2024 (Forecast)"]` | Visual display labels                      | Editable without breaking formulas         |
| **3. Physical Grid**   | Sheet placement             | Row index                                | Column index                   | Concrete Excel coordinates (`C10`, `$B$4`) | Deterministically planned by layout engine |

### Core Compiler vs. Adapter / Tooling Layer

- Core compiler (`crates/f8-core`):
  - Pure in-memory calculation; zero filesystem access.
  - Emits `.xlsx` byte buffers directly via `CompiledModel::emit_workbook`.
  - Treats tables strictly as orthogonal `R x C` matrices.
  - Agnostic to financial semantics (3-statement, LBO, Cap Table, Comps, Waterfall share identical IR primitives).
  - Enforces explicit JSON schema invariants with zero dynamic fallbacks.
- Adapter / tooling layer (`python/f8` + `crates/f8-python`):
  - Bridge between human/agent workflows and the core compiler.
  - Handles operating system and filesystem I/O (`save()`).
  - Releases Python GIL (`py.allow_threads`) during compilation and emission.
  - Fluent builder DSL (`Model`, `Sheet`, `Table`, `Cell`).
  - Table auditor (`TableAuditor`): dictionary conversions (`to_dict(orient="records" | "dict" | "index" | "split")`), rich GitHub-Flavored Markdown tables (`to_markdown()`), Jupyter display protocol (`_repr_markdown_`).

## 5. Technology Stack

| Component             | Technology                     | Purpose                                                                      |
| :-------------------- | :----------------------------- | :--------------------------------------------------------------------------- |
| **Serialization**     | `serde`, `serde_json`          | Type-safe JSON specification parsing and validation                          |
| **Formula parser**    | Custom lexer & Pratt parser    | Formula expression tokenization and AST parsing                              |
| **Graph engine**      | `petgraph`                     | Directed dependency graph, Tarjan SCC cycle detection, Kahn topological sort |
| **Evaluation engine** | Native Rust                    | Topological evaluation with Excel parity and lazy branch dispatch            |
| **Function catalog**  | Extensible trait registry      | Built-in Excel function implementations                                      |
| **Workbook emitter**  | `rust_xlsxwriter`              | Native OpenXML `.xlsx` generation with live formulas and deduplicated styles |
| **Python bridge**     | `pyo3`, `maturin`, `pythonize` | Native C-extension (`abi3-py312`), GIL release, exception mapping            |
| **Python SDK**        | Python 3.12+ (`uv`)            | Fluent builder DSL, table auditor, bundled agent skills                      |

## 6. Workspace Architecture

Cargo workspace separating core compiler from Python native bindings and distribution packages:

```text
f8/
├── Cargo.toml                  # Workspace root: shared dependencies and lints
├── crates/
│   ├── f8-core/                # Pure in-memory calculation compiler & IR
│   │   └── src/
│   │       ├── model/          # Specification schemas & validation invariants
│   │       ├── expr/           # Expression AST & Pratt parser
│   │       ├── value/          # Runtime values (CellValue, ExcelError) & parity rules
│   │       ├── functions/      # Function registry, traits, & catalog
│   │       └── compiler/       # 6 sequential compiler passes & CompiledModel facade
│   │
│   └── f8-python/              # Native PyO3 C-extension bindings (abi3-py312)
│       └── src/                # Compiler bridge, PyCompiledModel, Serde marshaling, & error mapping
│
├── python/                     # Python SDK package
│   └── f8/                     # Fluent builder, TableAuditor, CLI, & skills
│
└── docs/                       # Architecture, DSL reference, catalog, & task specs
    ├── dsl/                    # Specification format, addressing, grammar, formatting, patterns
    ├── functions/              # Excel function catalog
    └── specs/                  # Task specifications
```
