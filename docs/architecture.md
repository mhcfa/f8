# Architecture Overview

Deterministic financial model compiler treating spreadsheets as compiled binary artifacts.

## 1. Core Philosophy & Architectural Invariants

- Code as single source of truth: Declarative Python/TypeScript SDKs and structured specifications.
- Explicit core data structures: Exactly one canonical representation per concept. Zero implicit fallbacks, hidden defaults, or syntactic magic in core engine.
- Agent-first intermediate representation (IR): Orthogonal 2D matrix IR optimized for AI agent synthesis and deterministic verification over typing brevity. Ergonomic scaffolding isolated to outer SDKs (`python/f8`, `ts/`).
- Pure in-memory calculation: Engine (`crates/f8-core`) operates 100% in-memory with in-memory `.xlsx` generation. Zero filesystem I/O in core and WebAssembly bridge (`crates/f8-wasm`); disk writes (`.xlsx` emission via `save()`) isolated to outer language bindings (`crates/f8-python`, `ts/src/model.ts`).
- Orthogonal 2D matrix paradigm: General `R x C` table matrix. Columns represent time series, categories, or metrics. Every cell `Table[row, col]` is a first-class calculation node.
- Excel as compile artifact: Emitted `.xlsx` workbooks contain live, auditable formulas and formatting, functioning like compiled binaries or PDF reports.
- Strict input grammar & type safety, exact Excel calculation parity: Enforces an explicit, unambiguous DSL and two-stage compile-time type/shape/role + runtime `Empty` validation system (`F8Error` prior to `.xlsx` emission, decoupled from Tier-2 `FormatPreset`), rejecting permissive Excel/Lotus syntax and weak-typing quirks (e.g. unary `+`, unparenthesized chained comparisons, `prev(0)`/`next(0)`, dotted function names, implicit scalar/logical/relational/aggregate coercions, `AxisRole::Descriptor` references in numeric/financial aggregations, `Empty` on arithmetic/relational operands, `EmptyPolicy::Forbid` arguments, `EmptyPolicy::AllowInSlice` `1x1` single-cell arguments, `NPV`/`IRR`/`XNPV`/`XIRR` cash-flow/date slices, evaluated `IF`/`CHOOSE`/`IFERROR`/`XLOOKUP` branch arms, selected `XLOOKUP`/`INDEX` elements, `XLOOKUP`/`XMATCH` lookup keys, and top-level formula `Empty`, 2-arg `IF`, retired `COUNTA`, `IFS`, `IFNA`, `DAYS`, `ISBLANK`, `ISNA`, `ERROR.TYPE`, `SWITCH`, `IPMT`, `PPMT`, and `*IFS` string-encoded criteria functions (`SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS`), heterogeneous branch result arms, heterogeneous `lookup_array`/`return_array`/`INDEX` slices, syntactic scalar slices, non-reference scalars in range-only slots, static slice dimension/rank mismatches, trailing omitted arguments, discontinuous range slices across structural dividers), while evaluating numeric and lookup value-domain checks (`strict_integer` at discrete integer parameters, `DATE` 4-digit years `1900..=9999` and valid calendar days without 2-digit expansion or rollover, input date serials `1..=2958465` rejecting `0` = `1900-01-00`, annuity `type` `{0, 1}`, `YEARFRAC` `basis` `0..=4`, `CHOOSE`/`INDEX` bounds, `XLOOKUP`/`XMATCH` mode codes excluding wildcard `match_mode = 2`, and binary-search array sorting / `Empty` / `Error` handling) to standard runtime `CellValue::Error(ExcelError)` values. For every accepted construct, Pass 4 evaluation matches live Excel calculation results 100% (IEEE-754 `-0.0` normalization, finite overflow `#NUM!` guards, homogeneous case-insensitive ASCII text comparison, operator precedence, Excel domain error bubbling, `ISERROR`/`NA()`, and 1900 leap day bug reproduction).
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
   - Validates schema invariants: non-empty worksheet list, Excel sheet naming rules (`<= 31` UTF-16 code units, forbidden `: \ / ? * [ ]`, no leading/trailing `'`, non-`"History"`, case-insensitive uniqueness), `FreezePanes` grid bounds (`row < 1_048_576`, `col < 16_384`), valid identifier `TableId`s, non-empty and unique table/axis IDs, non-empty dimensions, non-empty subtitles, and finite `ValueLiteral::Number` inputs.
   - Validates formatting invariants: maximum 14 decimal places (`decimals <= 14`), precision allowed only on decimal-supporting numeric presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`), currency symbols allowed only on `currency` preset, valid hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA` on `tab_color` and palette colors, plus `"none"`/`"transparent"` on fills), and positive finite layout metrics (`col_padding >= 0.0`; `min_col_width <= max_col_width`).
2. Grid Expansion & Formula Parsing (`f8_core::compiler::ir`):
   - Expands tables into discrete cells identified by `CellId { table, row, col }`.
   - Parses formula strings into round-trippable `Expr` AST nodes via recursive-descent Pratt parser (supporting finite decimal and scientific-notation numeric literals `[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?` with `-0.0 -> 0.0` normalization and `ParseError::InvalidNumber` on overflow or malformed numeric tokens, `\\`/`\"`-escaped string and coordinate-key literals, and single-identifier uppercase function names `[A-Z][A-Z0-9_]*`), rejecting dotted function names, unary `+`, and unparenthesized chained relational operators (`ParseError::ChainedComparison`), enforcing positive integer `[1..=i32::MAX]` `prev(k)`/`next(k)` offsets, and confining positional argument omission (`_`) structurally to `CallArg` inside `Expr::FunctionCall`.
   - Enforces 3-tier specificity lattice:
     - Specificity: `0D Cell Override` > (`1D Row Rule` XOR `1D Column Rule`) > `Explicit Null / Error`.
     - Conflicting row and column expressions halt compilation (`IrError::AmbiguousExpressionConflict`).
     - Unassigned calculable cells halt compilation (`IrError::UnassignedCell`).
   - Isolates `descriptor` columns: vector formulas never broadcast into descriptor columns.
   - Resolves cell formatting, indentation, notes, visual highlights, and background zone shades (`row.shade` vs `col.shade`; conflicting active shades halt with `IrError::ShadeConflict`, while `shade: "none"` on a cell clears inherited shades).
   - Resolves cell currency: Point override > Vector rules (`row.currency` vs `col.currency`; conflicting codes halt with `IrError::CurrencyConflict`) > `table.currency` > default `usd`.
3. Dependency Graph & SCC (`f8_core::compiler::graph`):
   - Resolves cell references: table-qualified keys (`Table[row, col]`), active table selector (`@table[row, col]`), active coordinate selectors (`@col`, `@row`), temporal offsets (`col.prev(k)`, `col.next(k)`, `row.prev(k)`, `row.next(k)`), and static or dynamic range slices (`start..end` with key, active, or offset endpoints), populating `ResolvedSlice::axis_role` (`Measure` or `Descriptor`) from `TableMetadata::col_role` at origin while keeping Pass 3 function-agnostic.
   - Range contiguity: row range slices (`RowSelector::Range`) require physically contiguous `Data` rows (rejecting intervening `RowSpec::Gutter` and `RowSpec::SectionHeader` dividers via `ResolveError::DiscontinuousRange`), and horizontal column range slices (`ColSelector::Range`) require physically contiguous `Measure` columns (rejecting `ColumnSpec::Gutter` and `Descriptor` columns via `ResolveError::DiscontinuousRange`), guaranteeing a 1-to-1 correspondence with physical A1 rectangles in Pass 6a while preserving 1D vertical slices on single `Descriptor` columns (`ColSelector::Key` / `ColSelector::Active`).
   - Column offset semantics: anchors `origin.col` in `target_table` (failing with `ResolveError::ActiveColumnMismatch` if undeclared in `target_table` or `ResolveError::TemporalOffsetOutOfBounds` if declared as `Descriptor` in `target_table`) and steps strictly across `target_table` data measure columns, skipping gutters and descriptor columns.
   - Row offset semantics: anchors `origin.row` in `target_table` (failing with `ResolveError::ActiveRowMismatch` if undeclared in `target_table`) and steps strictly across `target_table` data rows, skipping gutters and section headers.
   - Cycle detection: executes Tarjan's Strongly Connected Components (SCC) on `petgraph::DiGraph`. Cycles halt compilation with exact cycle trace (`StrictDagHandler`).
   - Topological sorting: Kahn's algorithm with declaration-order tie-breaking for deterministic execution order.
4. Topological Evaluation (`f8_core::compiler::eval`):
   - Stage A static AST & slice validation (`Evaluator::validate_grid` / `FunctionRegistry::validate_and_synth_expr`): executes a single bottom-up topological pass over `graph.topological_order()`, seeding `cell_types: IndexMap<CellId, Option<ValueType>>` from literal payloads (`Number | String | Boolean -> Some(vt)`, `Null | Error -> None`) and resolving `TableRef`s to `StaticRef { rows, cols, axis_role, elem_types }`. Driven by `ValidationPolicy` metadata (`Standard`, `ShapeCoupled`, `Branch`, `Lookup`) and `ParamType { Any, Number, Boolean }`, enforces signature/arity bounds (including 3-arg `IF`), intermediate-only `_` omission (`CannotOmitRequiredArgument` before `CannotOmitTrailingArgument`), static scalar/boolean/relational `ValueType` rules (`InvalidOperandType`, `IncompatibleComparison`, `InvalidArgumentType`), `RolePolicy::MeasureOnly` (`DescriptorReferenceNotAllowed`), monomorphic branch return-type unification across `IF`/`CHOOSE`/`IFERROR` and `XLOOKUP` `return_array` vs `if_not_found` (`IncompatibleBranchTypes`), homogeneous `lookup_array`/`return_array`/`INDEX` slice element types (`InvalidArgumentType`), syntactic scalar slice exclusion (`r.is_slice()` -> `SliceInScalarContext`), `ParamKind::Range` (`Expr::Reference` required -> `InvalidArgumentKind`), slice dimension/length equality (`DimensionMismatch`, `LengthMismatch`), 1D rank constraints (`ExpectedOneDimensionalSlice`), and 2D `INDEX` 3-arg arity (`MissingColumnIndexFor2DSlice`) across both active and dormant branches without inspecting AST literal values (preserving static `ValueType`s on formulas that produce runtime `ExcelError`).
   - Stage B active topological evaluation: evaluates cells in topological order over `IndexMap<CellId, CellValue>` using `FunctionRegistry` (`bind_arguments` with `ReferenceKind { SingleCell, Slice }`, `RangeGrid`, and parameter-level `EmptyPolicy { Forbid, AllowInSlice }`), raising `EvalError` (`F8Error`) strictly on executed code paths when encountering `CellValue::Empty` on arithmetic/relational operands (`TypeError::EmptyOperand`), `EmptyPolicy::Forbid` scalar/single-cell/slice arguments, evaluated `IF`/`CHOOSE`/`IFERROR`/`XLOOKUP` branch arms, or selected `XLOOKUP`/`INDEX` elements (`TypeError::EmptyArgument`), `EmptyPolicy::AllowInSlice` single-cell (`1x1`) or scalar arguments (`TypeError::EmptyArgument`), or root formula results (`TypeError::EmptyFormulaResult`). All numeric value-domain checks (`strict_integer`, `DATE` year/month/day, date serial `1..=2958465`, annuity `type` `{0, 1}`, `YEARFRAC` `basis` `0..=4`, `CHOOSE`/`INDEX` bounds, `XLOOKUP`/`XMATCH` mode codes and binary-search ordering) execute in Stage B and produce standard `CellValue::Error(ExcelError)`.
   - IEEE-754 & overflow normalizations: `-0.0` normalized to `0.0`; non-finite float results (`NaN`, `+/-inf`) mapped to `CellValue::Error(ExcelError::Num)` via `CellValue::number(...)`.
   - Formula empty exclusion: `CellPayload::Formula` expressions and standalone `eval_expr` calls evaluating to `CellValue::Empty` at the root fail with `TypeError::EmptyFormulaResult` (`"formula cannot evaluate to an empty cell"`; `EvaluatedModel` stores `CellValue::Empty` on grid cells exclusively for `ValueLiteral::Null`). Every scalar parameter, `1x1` single-cell reference, evaluated lazy branch arm (`IF`, `CHOOSE`, `IFERROR`, `XLOOKUP` `if_not_found`), and selected lookup/index element (`XLOOKUP` `return_array`, `INDEX` `array`) rejects `CellValue::Empty` with `TypeError::EmptyArgument`; only syntactic `start..end` slices on `EmptyPolicy::AllowInSlice` parameters (`SUM`, `AVERAGE`, `MIN`, `MAX`, `MEDIAN`, `COUNT`, `SUMPRODUCT`, `AND`, `OR`, `XLOOKUP`, `XMATCH`, `INDEX`) tolerate unselected `CellValue::Empty` cells.
   - Homogeneous relational comparison: `=`, `<>`, `<`, `<=`, `>`, `>=` require non-`Empty` operands (`TypeError::EmptyOperand` on `Empty`) of identical static `ValueType` (`Number`, `Text` via case-insensitive ASCII folding, or `Boolean`).
   - Excel domain error semantics: division by zero and `0 ^ -n` return `#DIV/0!`, zero logical slice inputs, out-of-bounds `CHOOSE` `index_num`, invalid `XLOOKUP`/`XMATCH` `match_mode`/`search_mode`, or `Empty`/unsorted binary-search lookup arrays return `#VALUE!` (while `Error` cells in binary-search lookup arrays bubble their `ExcelError`), out-of-bounds `INDEX` `row_num`/`col_num` returns `#REF!`, `0 ^ 0`, negative-base non-integer exponentiation, non-integer arguments at discrete integer parameters (`strict_integer`), invalid `DATE(year, month, day)` (`1900..=9999`, `1..=12`, `1..=days_in_month` with `1900-02-29` preserved and 2-digit years/rollovers rejected), out-of-range date serials (`< 1` or `> 2958465`), invalid annuity `type` (`!= 0, 1`) or `YEARFRAC` `basis` (`< 0` or `> 4`), and arithmetic/domain/stepping overflow (`EDATE`/`EOMONTH` outside `1900..=9999`) return `#NUM!`, and `NA()` or failed lookups return `#N/A`; `ISERROR` inspects runtime `CellValue::Error` values without suppressing compile-time `F8Error`s.
   - Lazy evaluation: short-circuits dormant branches in `IF`, `IFERROR`, `CHOOSE`, and `XLOOKUP` (`IFERROR` traps only `CellValue::Error` domain errors while propagating `FunctionError` / `EvalError`). Because Stage A validates static types, roles, and shapes across all branches, Stage B evaluates only taken branches without re-walking dormant AST subtrees, enabling `ISERROR` and dynamic value-domain branch guards (`IF(serial >= 1, YEAR(serial), 0)`).
   - Error tolerance: runtime domain errors stored as `CellValue::Error` without aborting compiler; downstream formulas bubble or handle them via `IFERROR`.
5. Spatial Layout Planning (`f8_core::compiler::layout`):
   - Arranges tables into 0-indexed physical grid coordinates (`LayoutMap`).
   - Sheet margins: Col 0 (Excel Col A) and Row 0 (Excel Row 1) reserved as blank gutters.
   - Table placement (`PlacementSpec`): primary table placed at `(col: 1, row: 1)` (Excel `B2`); subsequent tables placed via `stack` (`start_col = 1`, `gap = 2` gutter rows below sheet max row), `below` (`start_col = target.start_col`, `gap = 2` gutter rows below `target`), or `right_of` (`start_row = target.start_row`, `gap = 1` gutter column right of `target`). Validates declaration-order same-sheet targets (`LayoutError::InvalidPlacementTarget`) and disjoint 2D bounding boxes (`LayoutError::TableOverlap`).
   - Vertical table anatomy: optional title banner, optional subtitle, title gutter, multi-tier column headers (Tier 0 `group` banners + bottom-aligned per-column `headers`, height `H = max(header_depth())`), body rows.
   - Sheet-wide dimension arbitration: `col_widths` computed as `clamp(chars * char_width_multiplier + col_padding, min_col_width, max_col_width)` across all tables spanning each physical column (`1..=sheet_max_end_col`), measuring the longest line in `headers` (`max_header_chars()`, excluding `group` banners) and `\n`-delimited cell segments with `indent * 2` character padding and `gutter_col_width` for horizontal gap/gutter columns; cells spanning multiple lines (`lines > 1`, either from exceeding `max_col_width` or explicit `\n` newlines) set `PhysicalCoordinate.wrap_text = true`, while `.xlsx` row heights remain automatic for Excel font auto-fitting.
6. Formula Lowering & Emission (`f8_core::compiler::lowering` & `f8_core::compiler::emission`):
   - Pass 6a (Lowering): catalog-agnostic AST-to-Excel-string emitter (`FormulaLowerer::lower_cell` / `FormulaLowerer::lower_grid`) prefixed with `=`. Finite `Literal::Number` values normalize `-0.0` to `"0"`, emit plain decimal on `[1e-6, 1e15)`, and emit lossless uppercase `E` notation outside `[1e-6, 1e15)` (`format_number_literal`); dynamic coordinates (`@col`, `@row`, `prev`, `next`) lowered to relative A1 coordinates; static keys lowered to `$` absolute coordinates; range slices lowered with per-endpoint `$` pinning (e.g. expanding slices `"start"..@col` -> `$C$6:C$6`); intermediate `CallArg::Omitted` slots lowered to empty positional slots (`,,`) while trailing `CallArg::Omitted` is rejected (`LoweringError::SyntaxError`). Cross-sheet references formatted as `'Sheet'!Address`.
   - Pass 6b (Emission): writes `.xlsx` byte buffers via `rust_xlsxwriter`.
   - Format deduplication: OpenXML `<cellXfs>` synthesized and cached via `FormatCache` with `FormatAlign::VerticalCenter` on data cells and `Format::set_text_wrap()` (`wrapText="1"`) on wrapped cells.
   - Color precedence: literal inputs styled with `input_text` (blue) overriding highlight/total fills; formula cells styled with `formula_text` (black); descriptors styled with `data_text`.
   - Presentation layout: super-headers centered across runs via `FormatAlign::CenterAcross` (zero merged cells); opt-in table-level row outline grouping (`table.collapsible`); pre-populated formula `<v>` value cache tags (configurable via `EmissionOptions`).

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
- Python adapter / tooling layer (`python/f8` + `crates/f8-python`):
  - Bridge between human/agent workflows and the core compiler.
  - Handles operating system and filesystem I/O (`save()`).
  - Releases Python GIL (`py.allow_threads`) during compilation and emission.
  - Fluent builder DSL (`Model`, `Sheet`, `Table`, `Cell`).
  - Table auditor (`TableAuditor`): dictionary conversions (`to_dict(orient="records" | "dict" | "index" | "split")`), rich GitHub-Flavored Markdown tables (`to_markdown()`), Jupyter display protocol (`_repr_markdown_`).
- TypeScript / WebAssembly adapter layer (`ts/` + `crates/f8-wasm`):
  - Compiles `f8-core` to `wasm32-unknown-unknown` via `wasm-bindgen` and `serde-wasm-bindgen` with zero filesystem I/O in WASM.
  - Synchronous WASM initialization in server/CLI runtimes (Node.js, Bun, Deno) and async `init()` for browser main threads.
  - Fluent builder DSL (`Model`, `Sheet`, `Table`, `Cell` in `ts/src/builder.ts`).
  - Table auditor (`TableAuditor` in `ts/src/audit.ts`): dictionary conversions (`toDict("records" | "dict" | "index" | "split")`), rich GitHub-Flavored Markdown tables (`toMarkdown()`), and scalar/vector coordinate accessors.
  - Idiomatic `camelCase` `CompiledModel` inspection API, `Uint8Array` workbook emission (`emitWorkbook()`), `node:fs` disk persistence (`save()`), and structured `F8Error` hierarchy (`ts/src/errors.ts`).
  - CLI runner (`ts/src/cli.ts`) and bundled TypeScript modeling skill (`ts/skills/f8-modeling/`): `f8 guide`, `f8 agent init`, and `f8 theme init`.

## 5. Technology Stack

| Component             | Technology                                | Purpose                                                                      |
| :-------------------- | :---------------------------------------- | :--------------------------------------------------------------------------- |
| **Serialization**     | `serde`, `serde_json`                     | Type-safe JSON specification parsing and validation                          |
| **Formula parser**    | Custom lexer & Pratt parser               | Formula expression tokenization and AST parsing                              |
| **Graph engine**      | `petgraph`                                | Directed dependency graph, Tarjan SCC cycle detection, Kahn topological sort |
| **Evaluation engine** | Native Rust                               | Topological evaluation with Excel parity and lazy branch dispatch            |
| **Function catalog**  | Extensible trait registry                 | Built-in Excel function implementations                                      |
| **Workbook emitter**  | `rust_xlsxwriter`                         | Native OpenXML `.xlsx` generation with live formulas and deduplicated styles |
| **Python bridge**     | `pyo3`, `maturin`, `pythonize`            | Native C-extension (`abi3-py312`), GIL release, exception mapping            |
| **Python SDK**        | Python 3.12+ (`uv`)                       | Fluent builder DSL, table auditor, CLI, bundled agent skills                 |
| **WASM bridge**       | `wasm-bindgen`, `serde-wasm-bindgen`      | Universal `wasm32-unknown-unknown` compiler and inspection bindings          |
| **TypeScript SDK**    | TypeScript 7 (`pnpm`, `oxlint`, `vitest`) | Fluent builder DSL, table auditor, CLI, bundled agent skills                 |

## 6. Workspace Architecture

Cargo workspace separating core compiler from Python native bindings, WebAssembly bindings, and distribution packages:

```text
f8/
├── Cargo.toml                  # Workspace root: shared dependencies and lints
├── package.json                # Root TypeScript package manifest (f8-engine)
├── pyproject.toml              # Root Python package manifest (f8-engine)
├── crates/
│   ├── f8-core/                # Pure in-memory calculation compiler & IR
│   │   └── src/
│   │       ├── model/          # Specification schemas & validation invariants
│   │       ├── expr/           # Expression AST & Pratt parser
│   │       ├── value/          # Runtime values (CellValue, ExcelError) & parity rules
│   │       ├── functions/      # Function registry, traits, & catalog
│   │       └── compiler/       # 6 sequential compiler passes & CompiledModel facade
│   │
│   ├── f8-python/              # Native PyO3 C-extension bindings (abi3-py312)
│   │   └── src/                # Compiler bridge, PyCompiledModel, Serde marshaling, & error mapping
│   │
│   └── f8-wasm/                # WebAssembly bindings (wasm32-unknown-unknown)
│       └── src/                # WasmCompiledModel, JSON-compatible Serde bridge, & WasmErrorPayload
│
├── python/                     # Python SDK package
│   └── f8/                     # Fluent builder, TableAuditor, CLI, & skills
│
├── ts/                         # TypeScript SDK package
│   ├── src/                    # Fluent builder, TableAuditor, CLI, CompiledModel, & F8Error hierarchy
│   ├── skills/                 # Bundled TypeScript f8-modeling agent skill
│   └── tests/                  # Vitest integration test suite
│
└── docs/                       # Architecture overview & active task specs
    ├── architecture.md         # Compiler pipeline, invariants, & inspection facade
    └── specs/drafts/           # Ephemeral task specifications
```
