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
   - Deserializes JSON specifications into strongly typed AST structures, enforcing `#[serde(deny_unknown_fields)]` on `SheetSpec`, `BandSpec`, `LaneSpec`, `TableSpec`, `SheetLayoutSpec`, `SheetPrintSpec`, `HeaderFooterSpec`, and `DetailedCellInput`.
   - Validates schema invariants: non-empty worksheet list, Excel sheet naming rules (`<= 31` UTF-16 code units, forbidden `: \ / ? * [ ]`, no leading/trailing `'`, non-`"History"`, case-insensitive uniqueness), `FreezePanes` grid bounds (`row < 1_048_576`, `col < 16_384`), `SheetLayoutSpec` (`auto` default | `grid { col_width }`, requiring finite `col_width` in `1.0..=255.0` via `ModelError::InvalidSheetColWidth`), `SheetSpec.bands` layout invariants (non-empty `BandSpec.lanes` via `ModelError::EmptyBand`, non-empty `LaneSpec.tables` via `ModelError::EmptyLane`, no `gap` on Band `0` via `ModelError::InvalidFirstBandGap`, no `lane_gap` on 1-lane bands via `ModelError::InvalidSingleLaneGap`, no `table_gap` on 1-table lanes via `ModelError::InvalidSingleTableGap`, and globally unique `TableId` placement across all bands, lanes, and sheets via `ModelError::DuplicateSheetTable`), opt-in worksheet print invariants (`SheetSpec.print: Option<SheetPrintSpec>` with required `paper_size` (`a4` | `letter`), `orientation` (`landscape` default | `portrait`), `fit` (`fit_width` default | `fit_page`), `center_horizontally` (`true` default), optional 3-zone `header`/`footer` (`HeaderFooterSpec`), and `BandSpec.page_break_before`: rejecting `page_break_before` on Band `0`, without `sheet.print`, with `fit_page`, or `> 1,023` breaks per sheet; rejecting all-`None` `HeaderFooterSpec`, unknown/unmatched `{...}` placeholders, post-expansion empty/whitespace zones, control characters, or raw `rust_xlsxwriter` `&[...]` macros via shared `expand_header_footer_zone`; and via `ProjectManifest::validate_print_with_theme` rejecting invalid `theme.typography.font_family` or compiled headers/footers exceeding `255` UTF-16 code units), valid identifier `TableId`s, non-empty and unique table/axis IDs, non-empty dimensions, non-empty subtitles, and finite `ValueLiteral::Number` inputs.
   - Validates formatting invariants: maximum 14 decimal places (`decimals <= 14`), precision allowed only on decimal-supporting numeric presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`), currency symbols allowed only on `currency` preset, valid hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA` on `tab_color` and palette colors, plus `"none"`/`"transparent"` on fills), and positive finite layout metrics (`col_padding >= 0.0`; `min_col_width <= max_col_width`).
2. Grid Expansion & Formula Parsing (`f8_core::compiler::ir`):
   - Registers symbols and expands `CellGrid` in canonical `manifest.sheets -> sheet.table_ids()` (`bands -> lanes -> tables`) order into discrete cells identified by `CellId { table, row, col }`.
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
   - Excel domain error semantics: division by zero, `0 ^ -n`, and `MOD(number, 0)` return `#DIV/0!`, zero logical slice inputs, out-of-bounds `CHOOSE` `index_num`, invalid `XLOOKUP`/`XMATCH` `match_mode`/`search_mode`, or `Empty`/unsorted binary-search lookup arrays return `#VALUE!` (while `Error` cells in binary-search lookup arrays bubble their `ExcelError`), out-of-bounds `INDEX` `row_num`/`col_num` returns `#REF!`, `0 ^ 0`, negative-base non-integer exponentiation, `LN(number <= 0)`, `EXP` overflow, `MOD` with `|number / divisor| > MAX_SAFE_INTEGER` or non-finite quotient, non-integer arguments at discrete integer parameters (`strict_integer`), invalid `DATE(year, month, day)` (`1900..=9999`, `1..=12`, `1..=days_in_month` with `1900-02-29` preserved and 2-digit years/rollovers rejected), out-of-range date serials (`< 1` or `> 2958465`), invalid annuity `type` (`!= 0, 1`) or `YEARFRAC` `basis` (`< 0` or `> 4`), and arithmetic/domain/stepping overflow (`EDATE`/`EOMONTH` outside `1900..=9999`) return `#NUM!`, and `NA()` or failed lookups return `#N/A`; `ISERROR` inspects runtime `CellValue::Error` values without suppressing compile-time `F8Error`s.
   - Lazy evaluation: short-circuits dormant branches in `IF`, `IFERROR`, `CHOOSE`, and `XLOOKUP` (`IFERROR` traps only `CellValue::Error` domain errors while propagating `FunctionError` / `EvalError`). Because Stage A validates static types, roles, and shapes across all branches, Stage B evaluates only taken branches without re-walking dormant AST subtrees, enabling `ISERROR` and dynamic value-domain branch guards (`IF(serial >= 1, YEAR(serial), 0)`).
   - Error tolerance: runtime domain errors stored as `CellValue::Error` without aborting compiler; downstream formulas bubble or handle them via `IFERROR`.
5. Spatial Layout Planning (`f8_core::compiler::layout`):
   - Arranges tables into 0-indexed physical grid coordinates (`LayoutMap`).
   - Sheet margins: Col 0 (Excel Col A) and Row 0 (Excel Row 1) reserved as blank gutters.
   - Band & lane placement (`SheetSpec.bands -> BandSpec.lanes -> LaneSpec.tables`): Band `0` starts at `row = 1` (Excel Row `2`) and Band `b > 0` starts at `prev_band_max_end_row + 1 + band.gap.unwrap_or(2)`; Lane `0` in every band starts at `col = 1` (Excel Col `B`) and Lane `l > 0` starts at `prev_lane_max_end_col + 1 + band.lane_gap.unwrap_or(1)`; Table `0` in every lane starts at `band_start_row` and Table `t > 0` starts at `prev_table_end_row + 1 + lane.table_gap.unwrap_or(2)`. Structural non-overlap is guaranteed by construction while enforcing Excel grid bounds (`LayoutError::GridLimitExceeded`).
   - Vertical table anatomy: optional title banner (`title_row` / `title_end_col`), optional multi-row subtitles, title gutter, multi-tier column headers (Tier 0 `group` banners + bottom-aligned per-column `headers`, height `H = max(header_depth())`), body rows.
   - Post-placement title banner alignment: `TablePlacement.title_end_col` resolves `Some` iff `title_row` is `Some` — titled tables in the last lane of any band (including single-lane bands) extend to `sheet_max_end_col`, while titled tables in any non-last lane extend to that lane's `lane_max_end_col` (preserving `band.lane_gap` cleanly across lanes and bands). Subtitles, headers, super-headers, section headers, body formatting, and `col_widths` remain strictly bounded by `start_col..=end_col`.
   - Sheet-wide dimension arbitration & `TablePlacement.col_spans`: Every `TablePlacement` records 1:1 inclusive physical column spans `col_spans: Vec<(u32, u32)>` (single source of truth shared across Pass 5, Pass 6b, and `inspect_sheet`):
     - `SheetLayoutSpec::Auto` (default): `col_spans[i] = (start_col + i, start_col + i)`; `col_widths` computed per physical column (`1..=sheet_max_end_col`) as `clamp(chars * char_width_multiplier + col_padding, min_col_width, max_col_width)` across all tables spanning that column, measuring the longest line in `headers` (`max_header_chars()`, excluding `group` banners) and `\n`-delimited cell segments with `indent * 2` character padding, `+1` trailing alignment-space padding on non-empty `measure` headers, `Date`/numeric/unformatted `measure` numbers not ending in `)`, and non-empty text cells under formats emitting trailing `_)` (leaving `descriptor` headers and unformatted `descriptor` cells unpadded), and `gutter_col_width` for horizontal gap/gutter columns; cells spanning multiple lines (`lines > 1`, either from exceeding `max_col_width` or explicit `\n` newlines) set `PhysicalCoordinate.wrap_text = true`, while `.xlsx` row heights remain automatic for Excel font auto-fitting.
     - `SheetLayoutSpec::Grid { col_width }`: Rejects `\n`/`\r` in data cells (`LayoutError::GridMultilineCell`) and in column `headers` or `group` banners (`LayoutError::GridMultilineHeader`); enforces static base width `base_w = round2(col_width.unwrap_or(theme.metrics.min_col_width))` in `1.0..=255.0` (`LayoutError::GridColWidthOutOfBounds`); validates every non-empty `measure` header line against static `base_w` (`LayoutError::GridMeasureHeaderTooWide`); widens uniform sheet width `W = round2(max(base_w, max_measure_cell_width, max_non_text_descriptor_cell_width))` (ignoring `max_col_width`, rejecting `W > 255.0` via `LayoutError::GridColWidthOutOfBounds`, short-circuiting zero-length strings/headers/descriptors from adding `col_padding`); computes closed-form `descriptor` overflow spans (`span >= 1` from non-empty descriptor headers/text with `+1` breathing space and `indent * 2`, while `measure` and `gutter` columns stay span `1`); resolves vertical rows in one band/lane pass, then finalizes horizontal `col_spans` lane-by-lane (`0..=max_lane_idx`), computing Lane `l > 0`'s `start_col` from `max(end_col of Band b, Lane l-1) + 1 + band.lane_gap.unwrap_or(1)` and aligning leading descriptor prefix spans `sum(raw_span[0..=last_leading_desc_idx])` (keeping trailing gutters after the last leading descriptor as period/column spacers and excluding descriptor-only tables) within each `(lane_idx, start_col)` group so descriptor-span growth in Lane `k` shifts Lane `k+1` automatically; enforces `PhysicalCoordinate.wrap_text = false` unconditionally; and assigns `W` to all physical columns covered by any `ColumnSpec::Data` span and `gutter_col_width` to column `0` and gutter/gap-only columns.
   - Opt-in print layout resolution (`SheetLayout.print: Option<SheetPrintLayout>`): When `SheetSpec.print` is `Some`, resolves content `print_area: Option<PrintArea>` trimming the blank Col `0` / Row `0` gutter (`B2:<last_col><last_row>`, `None` on empty sheets) and manual `row_breaks: Vec<u32>` (`band_start_row` of each band with `page_break_before = true`) identically across `auto` and `grid` modes for Pass 6b and `to_markdown("layout")`.
6. Formula Lowering & Emission (`f8_core::compiler::lowering` & `f8_core::compiler::emission`):
   - Pass 6a (Lowering): catalog-agnostic AST-to-Excel-string emitter (`FormulaLowerer::lower_cell` / `FormulaLowerer::lower_grid`) prefixed with `=`. Finite `Literal::Number` values normalize `-0.0` to `"0"`, emit plain decimal on `[1e-6, 1e15)`, and emit lossless uppercase `E` notation outside `[1e-6, 1e15)` (`format_number_literal`); dynamic coordinates (`@col`, `@row`, `prev`, `next`) lowered to relative A1 coordinates; static keys lowered to `$` absolute coordinates; range slices lowered with per-endpoint `$` pinning (e.g. expanding slices `"start"..@col` -> `$C$6:C$6`); intermediate `CallArg::Omitted` slots lowered to empty positional slots (`,,`) while trailing `CallArg::Omitted` is rejected (`LoweringError::SyntaxError`). Cross-sheet references formatted as `'Sheet'!Address`.
   - Pass 6b (Emission): writes `.xlsx` byte buffers via `rust_xlsxwriter`, applying OpenXML `quotePrefix="1"` (`Format::set_quote_prefix`) on emitted literal string cells (data cells, `values_only` text cells, and structural headers/titles/subtitles/section headers) starting with Excel formula or quote-prefix triggers (`=`, `+`, `-`, `@` after optional leading ASCII whitespace, or leading `'`) without mutating stored string text or tagging live formula cells.
   - Format deduplication: OpenXML `<cellXfs>` synthesized and cached via `FormatCache` with `FormatAlign::VerticalCenter` on data cells, `Format::set_text_wrap()` (`wrapText="1"`) on wrapped cells, `SpanEdge` (`Single | Start | Middle | End`) gating vertical left borders (`col_subtotal_border_left`, `col_total_border_left`) to `Single | Start` and vertical right borders (`descriptor_border_right`, `col_total_border_right`) to `Single | End`, and uniform trailing alignment-space (`_)`) number formats across right-aligned `measure` elements (`theme.number_formats.text` = `"@_)"` on upper/bottom measure headers, 4-section `;@_)` numeric defaults, `date` = `"yyyy-mm-dd_)"`, and `theme.number_formats.general` = `"General_);(General);0_);@_)"` on unformatted `format: None` measure cells; empty or case-insensitive `"General"` templates omit `set_num_format` to emit native `numFmtId="0"`, and left-aligned `descriptor` headers / unformatted `descriptor` cells remain on native `numFmtId="0"`).
   - Color precedence: literal inputs styled with `input_text` (blue) overriding highlight/total fills; formula cells styled with `formula_text` (black); descriptors styled with `data_text`.
   - Presentation layout: title banners emit across `start_col..=title_end_col` (matching `inspect_sheet`); super-headers centered across runs (`col_spans[start_c].0..=col_spans[end_c].1`) via `FormatAlign::CenterAcross` (zero merged cells); spanned descriptor headers and data runways emit styled `write_blank` cells across `(phys_start + 1)..=phys_end` (normalized blank `CellFormatKey` mapping `Bold -> Data`, preserving Excel horizontal text overflow with continuous fills and borders); opt-in table-level row outline grouping (`table.collapsible`); opt-in worksheet print OpenXML when `SheetLayout.print` is `Some` (`normal` view, `paperSize`, `orientation`, `<pageSetUpPr fitToPage="1"/>` with `1x0` `fit_width` or `1x1` `fit_page`, fixed `0.4"/0.6"/0.25"` margins, print gridlines off, optional `horizontalCentered="1"`, trimmed `_xlnm.Print_Area`, manual `<rowBreaks>`, and gated `<headerFooter scaleWithDoc="0">` with `&8&K<RRGGBB>&"<font>,Regular"` zone styling; zero OpenXML changes when `print` is `None`, and preserved identically under `values_only = true`); pre-populated formula `<v>` value cache tags (configurable via `EmissionOptions`).

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
  - `errors()`: Deterministic workbook-wide audit of runtime `CellValue::Error` cells in canonical `sheet -> band -> lane -> table -> row -> column` order (`Vec<CellError>`, with canonical Excel error strings and sheet-qualified `physical_address`).
  - `to_markdown(options)`: Pure in-memory whole-workbook compact GFM summary (`ModelMarkdownOptions`) supporting `ModelMarkdownDetail::{Values, Index, Layout}` (`values` compact GFM tables, `index` coordinate catalog, and `layout` pure-geometry 2D worksheet presentation map rendered directly from authored `SheetSpec.bands` / `lanes` with `Columns:` RLE widths, optional `Print:` boundary feedback using Normal-style column width points and typography-/wrapped-line-aware row heights with `Est. scale:`, `FitWidth` `Pages:` fills/`!`/`wrap:N`, `FitPage` `(h:~..%, v:~..%)`/`wrap:N`, and `[low scale <60%]`, inter-band `gap:Nr`, `, page_break`, inter-lane `gap:Nc`, non-default `table_gap:Nr`, `right_empty` / `empty_below` slack, title overhangs, subtitle width-overflow indicators `sub:{n}({chars}ch>w)`, physical column role runs with Pass 5 `req:{w}` demand on multi-column `grid` descriptors and widened gutters, and `wrap:N` counts), strict union `sheets`/`tables` allowlist filtering (rendering full sheet geometry with `(context)` tags on unselected siblings under partial `tables` filters in `layout` mode), scoped `## Runtime Errors`, and inline `[id]` annotations (`show_ids`, ignored in `index` and `layout` modes).
  - `cell_value(table, row, col)`: Fast `O(1)` runtime scalar lookup.
  - `cell_formatted(table, row, col)`: Fast `O(1)` formatted display string lookup.
  - `inspect_cell(table, row, col)`: Detailed cell card (values, styles, AST, Excel formula, A1 coordinate, precedents, dependents).
  - `inspect_table(table)`: 2D table grid with column headers, row descriptors, and cell records.
  - `inspect_sheet(sheet)`: 2D worksheet grid compositing stacked/side-by-side tables, aligned `TableTitle` banner spans (`start_col..=title_end_col`), physical `col_spans` across headers/section headers/gutters, `SheetCellKind::Overflow` (`"overflow"`) descriptor runway cells (keeping `SheetCellKind::Empty` strictly unowned canvas), bounding boxes, and column widths.
  - `precedents_of(cell, recursive)`: Direct or transitive upstream dependency tree (`DependencyTree`).
  - `dependents_of(cell, recursive)`: Direct or transitive downstream blast-radius tree (`DependencyTree`).
  - `parse_coordinate(target, row, col)`: Resolves 3 positional tokens or canonical IR string (`Table["row", "col"]`) into `CellId`.

## 3. Structural Hierarchy

- Workbook: Root manifest defining global model metadata, themes, and sheet ordering.
- Sheets: Worksheet containers managing tab color, freeze panes, gridlines, layout mode (`auto` | `grid`), opt-in print settings (`SheetPrintSpec`), and 2D band/lane table arrangement (`SheetSpec.bands -> BandSpec.lanes -> LaneSpec.tables`, including `BandSpec.page_break_before`).
- Tables: Named 2D matrices indexed by `[RowId, ColId]`. Globally unique IDs across workbook.
- Cells: Discrete calculation nodes uniquely addressed by `CellId { table, row, col }`.

## 4. 3-Tier Separation of Concerns

Strict decoupling of semantic identity, visual presentation, and physical grid location:

| Tier                   | Table Axis                  | Row Axis                                 | Column Axis                    | Responsibility                             | Mutability                                 |
| :--------------------- | :-------------------------- | :--------------------------------------- | :----------------------------- | :----------------------------------------- | :----------------------------------------- |
| **1. Identity (`id`)** | `id: "IncomeStatement"`     | `id: "revenue"`                          | `id: "2024E"`                  | Logic, formulas, AST, DAG                  | Immutable programmatic ID                  |
| **2. Presentation**    | `title: "Income Statement"` | Descriptor cell (`line_item: "Revenue"`) | `headers: ["2024 (Forecast)"]` | Visual display labels                      | Editable without breaking formulas         |
| **3. Physical Grid**   | Sheet bands & lanes         | Row index                                | Column index                   | Concrete Excel coordinates (`C10`, `$B$4`) | Deterministically planned by layout engine |

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
  - Releases Python GIL (`py.allow_threads`) during compilation, emission, and full-model error/Markdown scans.
  - Fluent builder DSL (`Model`, `Sheet`, `Band`, `Lane`, `Table`, `Cell`, `PrintSetup`, `HeaderFooter`), resolving workbook-level `Model.print` defaults with shallow per-sheet overrides and `"none"` opt-outs into canonical `SheetSpec.print`.
  - Table auditor (`TableAuditor`): dictionary conversions (`to_dict(orient="records" | "dict" | "index" | "split")`), rich GitHub-Flavored Markdown tables (`to_markdown(..., compact=False)`), Jupyter display protocol (`_repr_markdown_`).
- TypeScript / WebAssembly adapter layer (`ts/` + `crates/f8-wasm`):
  - Compiles `f8-core` to `wasm32-unknown-unknown` via `wasm-bindgen` and `serde-wasm-bindgen` with zero filesystem I/O in WASM, using a strict JS-object struct deserializer (`crates/f8-wasm/src/de.rs`) to enforce `#[serde(deny_unknown_fields)]` while preserving non-finite numeric values for Pass 1 validation.
  - Synchronous WASM initialization in server/CLI runtimes (Node.js, Bun, Deno) and async `init()` for browser main threads.
  - Fluent builder DSL (`Model`, `Sheet`, `Table`, `Cell` with `Sheet.setBands` and `Model`->`Sheet` print cascade / `"none"` opt-out in `ts/src/builder.ts`).
  - Table auditor (`TableAuditor` in `ts/src/audit.ts`): dictionary conversions (`toDict("records" | "dict" | "index" | "split")`), rich GitHub-Flavored Markdown tables (`toMarkdown({ compact })`), and scalar/vector coordinate accessors.
  - Idiomatic `camelCase` `CompiledModel` inspection API (`summary()`, `errors()`, `toMarkdown()`, cell/table/sheet inspection, dependency tracing), `Uint8Array` workbook emission (`emitWorkbook()`), `node:fs` disk persistence (`save()`), and structured `F8Error` hierarchy (`ts/src/errors.ts`).
  - CLI runner (`ts/src/cli.ts`) and bundled TypeScript modeling skill (`ts/skills/f8-modeling/`): `f8 guide`, `f8 agent init`, and `f8 theme init`.

## 5. Technology Stack

| Component             | Technology                                     | Purpose                                                                      |
| :-------------------- | :--------------------------------------------- | :--------------------------------------------------------------------------- |
| **Serialization**     | `serde`, `serde_json`                          | Type-safe JSON specification parsing and validation                          |
| **Formula parser**    | Custom lexer & Pratt parser                    | Formula expression tokenization and AST parsing                              |
| **Graph engine**      | `petgraph`                                     | Directed dependency graph, Tarjan SCC cycle detection, Kahn topological sort |
| **Evaluation engine** | Native Rust                                    | Topological evaluation with Excel parity and lazy branch dispatch            |
| **Function catalog**  | Extensible trait registry                      | Built-in Excel function implementations                                      |
| **Workbook emitter**  | `rust_xlsxwriter`                              | Native OpenXML `.xlsx` generation with live formulas and deduplicated styles |
| **Python bridge**     | `pyo3`, `maturin`, `pythonize`                 | Native C-extension (`abi3-py312`), GIL release, exception mapping            |
| **Python SDK**        | Python 3.12+ (`uv`)                            | Fluent builder DSL, table auditor, CLI, bundled agent skills                 |
| **WASM bridge**       | `wasm-bindgen`, `js-sys`, `serde-wasm-bindgen` | Universal `wasm32-unknown-unknown` compiler and inspection bindings          |
| **TypeScript SDK**    | TypeScript 7 (`pnpm`, `oxlint`, `vitest`)      | Fluent builder DSL, table auditor, CLI, bundled agent skills                 |

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
