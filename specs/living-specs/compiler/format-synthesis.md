---
title: Theme-to-OpenXML Format Synthesis & Style Cache
last_updated: 2026-09-20
domain: compiler
status: active
---

## Purpose

The `compiler::emission::format` subsystem translates Finx model themes and semantic cell formatting tuples into fully configured `rust_xlsxwriter::Format` instances, providing Pass 6b of the compiler pipeline with deduplicated, deterministic, and institutional-grade OpenXML styling. It prevents OpenXML `<cellXfs>` style table explosion through an `IndexMap` deduplication cache and enforces Wall Street presentation standards (input blue, formula black, accounting underline/double underline, and currency alignment).

## Architecture & Boundaries

- **Modules & components**:
  - `finx::compiler::emission::format`: Core structures (`FormatCache`, `CellFormatKey`), hex color parser (`parse_hex_color`), border style translator (`translate_border_style`), and header format generators.
  - `finx::compiler::emission::error`: Diagnostic domain errors (`EmissionError`).
  - `finx::compiler::emission::mod`: Module declarations and public API re-exports.
  - `finx::compiler::mod`: Compiler facade re-exports (`CellFormatKey`, `FormatCache`, `EmissionError`).
- **Data flow & interfaces**:
  1. Input: Active `Theme` passed to `FormatCache::new(&theme)`.
  2. Validation & pre-parsing: Upfront validation of all palette colors into `rust_xlsxwriter::Color::RGB(0x00RRGGBB)`, pre-generating structural header formats.
  3. Query: Pass 6b queries `cache.get_cell_format(&key)` with `CellFormatKey { role, preset, decimals, currency, is_input, is_descriptor }`.
  4. Cache lookup: If matching key exists in `IndexMap<CellFormatKey, Format>`, returns cloned format.
  5. Synthesis: If cache miss, constructs `Format` with typography, bolding, fills, text colors, borders, alignments, and synthesized number format mask via `theme.resolve_number_format`, then inserts into cache.
- **Dependencies**:
  - `rust_xlsxwriter`: OpenXML formatting primitives (`Format`, `Color`, `FormatAlign`, `FormatBorder`).
  - `indexmap`: Preserves deterministic insertion order for XML format tables.
  - `thiserror`: Structured diagnostic error reporting.

## Requirements & Invariants

### 1. Hex Color Parsing and Upfront Validation

Hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`) are strictly validated and parsed into 24-bit `rust_xlsxwriter::Color::RGB(0x00RRGGBB)` during `FormatCache::new`.

#### Scenarios

- **Valid hex color string**:
  - **When** parsing `#1B365D`, `#123`, or `#1B365DFF`
  - **Then** resolves to the corresponding RGB 24-bit integer color.
- **Invalid hex color string**:
  - **When** parsing a malformed or non-hex string during `FormatCache::new`
  - **Then** returns `EmissionError::InvalidColor` without panicking.

### 2. Semantic Number Format Mask Synthesis

Data cell number format strings are dynamically generated via `theme.resolve_number_format(preset, decimals, currency)` when `preset` is present (`Some`). When `preset` is `None`, no custom mask is applied, preserving Excel's default `General` formatting.

#### Scenarios

- **Formatted currency cell**:
  - **When** `CellFormatKey` has `preset: Some(Currency)`, `decimals: Some(2)`, and `currency: Some(Usd)`
  - **Then** synthesized `Format` carries mask `_($* #,##0.00_);_($* (#,##0.00);_($* "-"_);_(@_)`.
- **Unformatted general cell**:
  - **When** `CellFormatKey` has `preset: None`
  - **Then** typography and borders are configured, but no custom number format mask is set.

### 3. Financial Modeling Color Coding Rules

Model presentation strictly enforces financial modeling conventions:
- Measure literal inputs (`is_input: true && !is_descriptor`) use `palette.input_text` (classic blue `#002060`).
- Formula cells (`!is_input`) use `palette.formula_text` (black `#000000`).
- Descriptor cells (`is_descriptor: true`) always use neutral text (`palette.data_text`), never input blue.
- Highlights (`role.is_highlight: true`) take precedence, applying `palette.highlight_fill` and `palette.highlight_text`.

#### Scenarios

- **Measure input literal vs formula text color**:
  - **When** querying format for measure input vs formula
  - **Then** input cell uses blue `input_text` and formula cell uses black `formula_text`.
- **Descriptor label protection**:
  - **When** querying format for a string literal cell in a descriptor column
  - **Then** text color remains `palette.data_text` and alignment is left.

### 4. Orthogonal 2D Summary Accounting Borders and Emphasis

Horizontal summary styles (`Subtotal`, `Total`) govern top and bottom borders; vertical summary styles govern left and right borders:
- Subtotals: Thin top border with `palette.border_color`, bold text, and `palette.subtotal_fill`.
- Totals: Thin top border, double bottom border with `palette.total_border_color`, bold text, and `palette.total_fill`.
- Matrix intersection total box: Cells with both `horizontal: Total` and `vertical: Total` receive a 4-sided total box (thin top, double bottom, thin left, double right).

#### Scenarios

- **Subtotal accounting border**:
  - **When** `role.horizontal == Subtotal`
  - **Then** format has thin top border with `border_color` and bold text.
- **Total accounting double border**:
  - **When** `role.horizontal == Total`
  - **Then** format has thin top border, double bottom border with `total_border_color`, and bold text.
- **2D total box**:
  - **When** `role.horizontal == Total` and `role.vertical == Total`
  - **Then** format has top (thin), bottom (double), left (thin), and right (double) borders.

### 5. Deterministic Format Deduplication

Identical `CellFormatKey` queries return cached `Format` clones without creating duplicate formatting records. The cache uses `IndexMap` to guarantee deterministic `<cellXfs>` record ordering across builds.

#### Scenarios

- **Deduplication hit**:
  - **When** querying identical `CellFormatKey` instances repeatedly
  - **Then** cache size remains constant and identical formats are reused.
