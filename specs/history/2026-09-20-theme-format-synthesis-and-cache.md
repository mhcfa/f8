---
title: Theme-to-OpenXML Format Synthesis & Style Cache
date: 2026-09-20
status: completed
target-living-spec: specs/living-specs/compiler/format-synthesis.md
---

## Proposal

### What

Implement Finx's Theme-to-OpenXML Format Translation and Style Deduplication Engine in `finx::compiler::emission`:
1. Add `rust_xlsxwriter` (0.99) dependency to `Cargo.toml`.
2. Add `Hash` derive to `StyleRole` in `src/compiler/layout/mod.rs` to permit caching by semantic role.
3. Implement structured emission diagnostics in `src/compiler/emission/error.rs`: `EmissionError`.
4. Implement `src/compiler/emission/format.rs` with `FormatCache`:
   - Hex color parser converting theme color strings (`#RGB`, `#RRGGBB`, `#RRGGBBAA`) into `rust_xlsxwriter::Color`.
   - Border style translator mapping `finx::model::theme::BorderStyle` to `rust_xlsxwriter::FormatBorder`.
   - Structural header format generators: Table Title, Subtitle, Super-Header, Column Header (descriptor/measure), and Section Header.
   - Data cell format synthesizer translating `(StyleRole, Option<FormatPreset>, Decimals, Currency, is_input, is_descriptor)` into `rust_xlsxwriter::Format`.
   - Financial modeling conventions: color hardcoded measure input literals with `theme.palette.input_text` (classic blue `#002060`) and formulas/descriptors with `theme.palette.formula_text` or `theme.palette.data_text` (black `#000000`).
   - Summary borders, colors, and emphasis: thin top borders for subtotals with `theme.palette.border_color`, thin top + double bottom borders for totals with `theme.palette.total_border_color`, bolding, and summary fills.
   - Deterministic format deduplication cache keyed by `CellFormatKey` using `IndexMap` to prevent OpenXML `<cellXfs>` style table bloat.
5. Provide a comprehensive unit test suite in `tests/format_cache_test.rs` verifying format masks, typography, colors, borders, alignments, and deduplication efficiency.

### Why

- **Prerequisite for workbook emission (Pass 6b)**: OpenXML files require concrete styling records (`<xf>`) that fuse typography, number format strings, text colors, background fills, and borders into a single index.
- **Separation of concerns**: Isolating formatting and style synthesis from worksheet assembly and cell streaming keeps the codebase modular, clean, and testable without the overhead of generating complete `.xlsx` workbook files.
- **Style count limit mitigation**: Excel enforces a hard limit of 64,000 unique cell formats. Models with tens of thousands of cells will corrupt or bloat workbooks unless formats are deduplicated through an immutable cache.
- **Institutional aesthetics**: Professional financial models strictly enforce visual standards (blue inputs, black formulas, thin/double accounting underlines, and currency alignment).

### Scope

- **In-scope**:
  - `rust_xlsxwriter = "0.99"` dependency integration in `Cargo.toml`.
  - Adding `Hash` derive to `StyleRole` in `src/compiler/layout/mod.rs`.
  - `EmissionError` diagnostic enum in `src/compiler/emission/error.rs` and `CompilerError::Emission` in `src/compiler/error.rs`.
  - `FormatCache` and `CellFormatKey` in `src/compiler/emission/format.rs`.
  - Parsing hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`) into `rust_xlsxwriter::Color`.
  - Mapping `Theme` typography (font family, point sizes) and borders to `rust_xlsxwriter::Format`.
  - Formatting data cells using `Theme::resolve_number_format` (`{dec}`, `{sym}`) when preset is present, and preserving unformatted `General` when preset is omitted.
  - Financial modeling color rules (inputs blue, formulas black, highlight, subtotal/total fills).
  - Deduplication cache using `IndexMap` preventing duplicate `Format` instantiations.
  - Comprehensive unit test suite in `tests/format_cache_test.rs`.

- **Out-of-scope**:
  - Workbook/Worksheet assembly, column widths, row heights, and freeze panes (deferred to Spec 2: Workbook Assembly).
  - Cell streaming, formula result caching, and values-only freezing (deferred to Spec 2).
  - Disk / buffer serialization of `.xlsx` files (deferred to Spec 2).

### What Changes

- **Affected modules**:
  - `Cargo.toml`: Add `rust_xlsxwriter = "0.99"`.
  - `src/compiler/layout/mod.rs`: Add `Hash` to `#[derive(...)]` for `StyleRole`.
  - `src/compiler/emission/mod.rs`: Declare `error` and `format` modules, re-export `FormatCache`, `CellFormatKey`, and `EmissionError`.
  - `src/compiler/emission/error.rs`: Define `EmissionError`.
  - `src/compiler/emission/format.rs`: Implement `FormatCache` and format synthesis helpers.
  - `src/compiler/error.rs`: Add `CompilerError::Emission(#[from] EmissionError)`.
  - `tests/format_cache_test.rs`: New integration test suite for formatting and caching.

### Capabilities

- **New capabilities**:
  - Convert any `Theme` and semantic cell formatting tuple into a fully configured `rust_xlsxwriter::Format`.
  - Dynamically synthesize format masks with precision and currency symbols, or emit unformatted cells when preset is omitted.
  - Apply institutional accounting borders and financial hardcode color coding.
  - Transparently deduplicate format instances across millions of cell evaluations deterministically.

- **Modified capabilities**:
  - Compiler error hierarchy gains structured `EmissionError`.
  - `StyleRole` gains `Hash` conformance.

---

## Design

### Goals

1. **Exact OpenXML styling**: Every generated `Format` correctly encodes number format, typography, fills, and borders.
2. **Zero allocation bloat**: Cache formats by semantic signature; duplicate queries return cloned references to identical formats.
3. **Deterministic styling**: Given identical `Theme` and cell attributes, format synthesis produces 100% identical OpenXML representations with stable `<cellXfs>` indices.

### Invariants

- **Zero production panics**: Invalid color codes or formatting failures bubble up via `EmissionError`.
- **Institutional colors**: Hardcoded measure literals must never emit black text; formulas and descriptor labels must never emit blue text.
- **Border alignment**: Horizontal summary styles (`Subtotal`, `Total`) only set top/bottom borders; vertical summary styles only set left/right borders.
- **Deterministic ordering**: Format cache storage uses `IndexMap` to preserve insertion order deterministically.

### Decisions

- **Color parsing and validation**:
  - *Choice*: Parse `#RGB`, `#RRGGBB`, and `#RRGGBBAA` strings into `rust_xlsxwriter::Color::RGB(0x00RRGGBB)` during `FormatCache::new`.
  - *Rationale*: Pre-parsing all palette colors once during cache initialization guarantees zero runtime parsing failures and avoids repetitive string conversions on every cell lookup.
  - *Alternatives considered*: Lazy parsing on each cell formatting query. Rejected because it introduces error handling into tight cell emission loops and repeats identical parsing thousands of times.

- **Optional format preset in cache key**:
  - *Choice*: Use `pub preset: Option<FormatPreset>` in `CellFormatKey`.
  - *Rationale*: Matches `CellNode.format: Option<FormatPreset>`. Cells with no preset declared receive standard Excel General formatting without synthesizing an artificial mask.
  - *Alternatives considered*: Mandatory `FormatPreset`. Rejected because forcing `FormatPreset::Number` or `Text` onto unformatted cells corrupts general number and boolean formatting in Excel.

- **Color and fill precedence hierarchy**:
  - *Choice*: Enforce strict precedence rules:
    - **Fill**: `is_highlight` (`highlight_fill`) > `Total` (`total_fill`) > `Subtotal` (`subtotal_fill`) > `Data` (`data_fill`).
    - **Text**: `is_highlight` (`highlight_text`) > `Total` (`total_text`) > `Subtotal` (`subtotal_text`) > (`is_input && !is_descriptor` ? `input_text` : `formula_text`).
    - **Descriptor protection**: Descriptor cells (`is_descriptor: true`) never receive `input_text` (blue), even when backed by literal strings.
  - *Rationale*: Wall Street modeling conventions reserve blue text exclusively for measure inputs; descriptor labels must remain neutral black/navy.

- **Borders, summary emphasis, and bolding**:
  - *Choice*:
    - **Subtotals**: Top border from `theme.borders.subtotal_border_top` using `theme.palette.border_color`, bold text, and `subtotal_fill`.
    - **Totals**: Top border from `theme.borders.total_border_top` and bottom border from `theme.borders.total_border_bottom` using `theme.palette.total_border_color`, bold text, and `total_fill`.
    - **Vertical summary styles**: Left border from `col_subtotal_border_left` / `col_total_border_left`, right border from `col_total_border_right`, using respective border colors.
    - **Structural headers**: Table title (bold, left), subtitle (regular, left), super-header (bold, center), column header (bold, descriptor left / measure right), section header (bold, left).
  - *Rationale*: Exactly matches institutional Excel models and financial reporting standards.

- **Format cache structure**:
  - *Choice*: Key data cell formats by `CellFormatKey` stored in `IndexMap<CellFormatKey, Format>`:
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct CellFormatKey {
        pub role: StyleRole,
        pub preset: Option<FormatPreset>,
        pub decimals: Option<u32>,
        pub currency: Option<CurrencyCode>,
        pub is_input: bool,
        pub is_descriptor: bool,
    }
    ```
  - *Rationale*: Minimal semantic attribute tuple for high cache hit rates and deterministic XML serialization.

### Data Contracts

#### Public Types in `src/compiler/emission/`

| Struct / Enum | Role | Key Fields / Methods |
| :--- | :--- | :--- |
| `CellFormatKey` | Cache key for data cell formatting | `role: StyleRole`, `preset: Option<FormatPreset>`, `decimals: Option<u32>`, `currency: Option<CurrencyCode>`, `is_input: bool`, `is_descriptor: bool` |
| `FormatCache` | Format synthesis and deduplication cache | `new(theme: &Theme) -> Result<Self, EmissionError>`, `get_cell_format(&mut self, key: &CellFormatKey) -> Result<Format, EmissionError>`, `title_format(&self) -> Format`, `subtitle_format(&self) -> Format`, `super_header_format(&self) -> Format`, `col_header_format(&self, is_descriptor: bool) -> Format`, `section_header_format(&self) -> Format` |
| `EmissionError` | Structured diagnostic enum | `Xlsx(#[from] rust_xlsxwriter::XlsxError)`, `InvalidColor { field: String, color: String }`, `Model(#[from] ModelError)` |

### Risks & Trade-offs

- **Format cloning**: In `rust_xlsxwriter`, `Format` is cheap to clone. Returning cloned formats from the cache avoids lifetime gymnastics when streaming cells across multiple sheets.
- **IndexMap memory**: `IndexMap` incurs minor overhead over standard `HashMap`, but guarantees deterministic style table ordering in output OpenXML files.

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Dynamic Number Format Mask Synthesis**
  - **Given** a `Theme` and a `CellFormatKey` with `preset: Some(FormatPreset::Currency)`, `decimals: Some(2)`, and `currency: Some(CurrencyCode::Usd)`
  - **When** querying `FormatCache::get_cell_format`
  - **Then** returns a `Format` configured with number format string `_($* #,##0.00_);_($* (#,##0.00);_($* "-"_);_(@_)`.

- [ ] **Scenario 2: Hardcode Input vs Formula Text Color Differentiation**
  - **Given** identical measure cell attributes (`is_descriptor: false`) except `is_input: true` vs `is_input: false`
  - **When** querying `FormatCache::get_cell_format` for both keys
  - **Then** the input format uses `theme.palette.input_text` (blue) and the formula format uses `theme.palette.formula_text` (black).

- [ ] **Scenario 3: Descriptor Labels Suppress Input Blue Text**
  - **Given** a cell format key with `is_descriptor: true` and `is_input: true`
  - **When** querying `FormatCache::get_cell_format`
  - **Then** returns a format with `theme.palette.data_text` (not `input_text`) and left alignment.

- [ ] **Scenario 4: Subtotal and Total Accounting Borders and Colors**
  - **Given** keys with `StyleRole` having `horizontal: SemanticStyle::Subtotal` and `horizontal: SemanticStyle::Total`
  - **When** querying `FormatCache::get_cell_format`
  - **Then** subtotal format has a thin top border with `theme.palette.border_color` and bolding, and total format has a thin top border and double bottom border with `theme.palette.total_border_color` and bolding.

- [ ] **Scenario 5: Highlight Style Role Emphasis**
  - **Given** a key with `StyleRole { is_highlight: true, .. }`
  - **When** querying `FormatCache::get_cell_format`
  - **Then** returns a format configured with `theme.palette.highlight_fill` and `theme.palette.highlight_text`.

- [ ] **Scenario 6: Structural Header Formats and Alignments**
  - **Given** the active theme
  - **When** querying title, subtitle, super-header, column header (descriptor/measure), and section header formats
  - **Then** each format matches the corresponding theme typography, fills, text colors, borders, bolding, and alignment rules (super-header centered, column descriptor left, column measure right).

- [ ] **Scenario 7: Format Deduplication Cache Hits**
  - **Given** a `FormatCache` instance
  - **When** requesting formats for identical `CellFormatKey` instances repeatedly
  - **Then** the internal cache size remains 1 and cached instances are reused.

- [ ] **Scenario 8: Unformatted General Preset Handling**
  - **Given** a `CellFormatKey` with `preset: None`
  - **When** querying `FormatCache::get_cell_format`
  - **Then** returns a valid `Format` with typography, fills, and borders applied, but without setting an explicit number format mask.

- [ ] **Scenario 9: Invalid Hex Color Diagnostics**
  - **Given** a theme with an invalid hex color string
  - **When** initializing `FormatCache::new`
  - **Then** returns an `EmissionError::InvalidColor` without panicking.

---

## Tasks

### Phase 1: Dependencies & Error Hierarchy
- [x] Add `rust_xlsxwriter = "0.99"` to `Cargo.toml`.
- [x] Add `Hash` derive to `StyleRole` in `src/compiler/layout/mod.rs`.
- [x] Implement `src/compiler/emission/error.rs` with `EmissionError`.
- [x] Add `CompilerError::Emission(#[from] EmissionError)` to `src/compiler/error.rs`.
- [x] Update `src/compiler/emission/mod.rs` to expose error types.

### Phase 2: Color and Border Translation Helpers
- [x] Implement hex color parser (`parse_hex_color`) converting `#RGB`, `#RRGGBB`, `#RRGGBBAA` strings to `rust_xlsxwriter::Color`.
- [x] Implement border style translator mapping `BorderStyle` to `rust_xlsxwriter::FormatBorder`.

### Phase 3: Format Cache & Structural Header Synthesis
- [x] Define `CellFormatKey` with `preset: Option<FormatPreset>` in `src/compiler/emission/format.rs`.
- [x] Implement `FormatCache::new` with upfront palette validation and color pre-parsing.
- [x] Implement structural header format generators: `title_format`, `subtitle_format`, `super_header_format`, `col_header_format`, `section_header_format`.
- [x] Implement data cell format synthesis with number format mask expansion, color/fill precedence, accounting borders, bolding, and alignment.
- [x] Implement caching and deduplication map using `IndexMap<CellFormatKey, Format>` in `FormatCache`.

### Phase 4: Integration Test Suite & Verification
- [x] Create `tests/format_cache_test.rs` covering all 9 acceptance scenarios.
- [x] Run verification suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [x] Update living specification `specs/living-specs/compiler/format-synthesis.md` and archive draft to `specs/history/`.
