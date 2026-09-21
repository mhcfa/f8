---
title: Transparent Fills & Gridline Preservation
date: 2026-09-21
status: completed
target-living-spec:
  - specs/living-specs/model/theme.md
  - specs/living-specs/compiler/format-synthesis.md
---

## Proposal

### What

Support transparent (no-fill) cell backgrounds across Finx themes and format synthesis by accepting `"none"` and `"transparent"` (case-insensitive) for all palette fill attributes (`data_fill`, `subtotal_fill`, `total_fill`, `col_header_fill`, `section_header_fill`, `super_header_fill`, `table_title_fill`, `zebra_fill`, and `highlight_fill`). When a fill is `"none"` or `"transparent"`, OpenXML format synthesis omits background color assignment, emitting native transparent cells (`patternType="none"`) that preserve underlying Excel worksheet gridlines.

### Why

- **Worksheet gridline visibility**: Applying an explicit white fill (`#FFFFFF`) to data cells in OpenXML suppresses Excel's default gray worksheet gridlines within tables. While suitable for presentation decks and printed cards, financial modeling workflows frequently require transparent cell backgrounds where native worksheet gridlines remain visible across data matrices.
- **Selective styling flexibility**: Financial modelers can choose which components have background fills (e.g. bold navy headers with transparent data cells and accounting-bordered totals) without being forced into solid opaque backgrounds across the entire table.
- **Standardized keyword convention**: Aligning with CSS, SVG, and presentation standards (`"none"`, `"transparent"`) provides an intuitive, declarative contract without requiring schema breaks or nullable types.

### Scope

- **In-scope**:
  - Semantic validation in `finx::model::theme::PaletteConfig` permitting `"none"` and `"transparent"` (case-insensitive) for all fill attributes, while maintaining strict hex validation for text and border colors.
  - Fill parsing in `finx::compiler::emission::format` via `parse_fill_color(field, s) -> Result<Option<Color>, EmissionError>`, returning `Ok(None)` for `"none"` / `"transparent"`.
  - Optional `Color` storage in `ParsedPalette` for all fill fields (`Option<Color>`).
  - Conditional background color emission in `FormatCache` for table titles, super-headers, column headers, section headers, and data/summary cells.
  - Comprehensive unit and integration tests verifying validation, parsing, transparent format synthesis, and border preservation.

- **Out-of-scope**:
  - Transparent text or border colors (text requires readable color; border omission is governed by `BorderStyle::None`).
  - Modifying the default palette values of `Theme::corporate_navy` (`#FFFFFF` remains default; transparent is opt-in via theme configuration or custom themes).

### What Changes

- **Affected modules**:
  - `src/model/theme.rs`: Update `PaletteConfig::validate` to distinguish fill fields from text/border color fields; allow `"none"` and `"transparent"` for fills.
  - `src/compiler/emission/format.rs`: Implement `parse_fill_color`, update `ParsedPalette` fill fields to `Option<Color>`, and apply conditional `format.set_background_color(fill)` only when `fill.is_some()`.
  - `src/compiler/emission/mod.rs`: Re-export `parse_fill_color`.
  - `tests/theme_test.rs`: Add tests verifying `"none"` / `"transparent"` validation for fills and rejection for text colors.
  - `tests/format_cache_test.rs`: Add tests verifying `parse_fill_color` and transparent format generation in `FormatCache`.

### Capabilities

- **New capabilities**:
  - Themes can specify `"none"` or `"transparent"` for `data_fill`, `subtotal_fill`, `total_fill`, `col_header_fill`, `section_header_fill`, `super_header_fill`, `table_title_fill`, `zebra_fill`, and `highlight_fill`.
  - Cells configured with transparent fills render without `<patternFill>` in Excel, allowing native gridlines to show through cleanly while preserving borders and typography.
- **Modified capabilities**:
  - `FormatCache::get_cell_format` conditionally applies background fills only when the resolved semantic fill color is `Some(Color)`.

---

## Design

### Goals

1. **Native Excel transparency**: Transparent cells emit without background fills so native worksheet gridlines show through.
2. **Zero breakage**: Existing themes using `#FFFFFF` or other hex colors continue behaving identically.
3. **Type safety and validation**: Strict hex validation remains intact for text and borders; fills accept only valid hex strings or `"none"` / `"transparent"`.

### Invariants

- Text and border colors must always be valid hex colors; `"none"` is prohibited for text and borders.
- Precedence rules (`is_highlight > total > subtotal > data`) remain identical, resolving to `Option<Color>`.
- Format deduplication via `IndexMap` remains fully deterministic.

### Decisions

- **String keywords (`"none"` / `"transparent"`) vs `Option<String>` in `PaletteConfig`**:
  - *Choice*: Keep `String` fields in `PaletteConfig` and accept `"none"` / `"transparent"` (case-insensitive).
  - *Rationale*: Preserves JSON schema backward compatibility, prevents breaking existing serialization contracts or requiring `None` / `null` handling across serde configs, and aligns with standard CSS/SVG conventions.
  - *Alternatives considered*: Making all fill fields `Option<String>`. Rejected because it would break existing serde schemas and require migration of existing theme files.

- **Option<Color> in `ParsedPalette`**:
  - *Choice*: Store `Option<Color>` for all fill fields in `ParsedPalette`.
  - *Rationale*: Upfront resolution during cache initialization avoids string comparisons on every cell lookup, ensuring high compiler performance.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Theme validation accepts transparent fill keywords**
  - **Given** a `PaletteConfig` with `data_fill: "none"` and `subtotal_fill: "transparent"` (any casing)
  - **When** calling `palette.validate()`
  - **Then** validation succeeds without error.

- [x] **Scenario 2: Theme validation rejects transparent keyword for text or borders**
  - **Given** a `PaletteConfig` with `data_text: "none"` or `border_color: "transparent"`
  - **When** calling `palette.validate()`
  - **Then** returns `ModelError::InvalidThemeColor`.

- [x] **Scenario 3: Fill parser resolves keywords to None and hex to Some(Color)**
  - **When** calling `parse_fill_color` with `"none"`, `"transparent"`, `"NONE"`, `"TRANSPARENT"`
  - **Then** returns `Ok(None)`.
  - **When** calling `parse_fill_color` with `"#1B365D"`
  - **Then** returns `Ok(Some(Color::RGB(0x1B365D)))`.

- [x] **Scenario 4: FormatCache emits transparent cells without background color**
  - **Given** a theme with `data_fill: "none"`
  - **When** synthesizing a data cell format via `cache.get_cell_format`
  - **Then** the resulting `Format` has no background color set (does not contain `pattern_type: Solid`).

- [x] **Scenario 5: Summary borders and typography preserved on transparent fills**
  - **Given** a theme with `total_fill: "none"`
  - **When** synthesizing a total cell format
  - **Then** format contains bolding and accounting borders (thin top, double bottom) but no background fill.

---

## Tasks

- [x] **Phase 1: Theme Schema & Validation**
  - [x] Add `is_valid_fill_color` in `src/model/theme.rs`.
  - [x] Update `PaletteConfig::validate` to check fill fields with `is_valid_fill_color` and text/border fields with `is_valid_hex_color`.
  - [x] Add unit tests in `tests/theme_test.rs` covering `"none"` and `"transparent"` validation.

- [x] **Phase 2: Format Synthesis & Emission**
  - [x] Implement `parse_fill_color` in `src/compiler/emission/format.rs`.
  - [x] Update `ParsedPalette` to hold `Option<Color>` for all fill fields.
  - [x] Update `FormatCache::new` to parse fills with `parse_fill_color`.
  - [x] Update structural header formats (title, super-header, column headers, section header) and `get_cell_format` to conditionally set background color only when fill is `Some`.
  - [x] Re-export `parse_fill_color` in `src/compiler/emission/mod.rs`.
  - [x] Add unit tests in `tests/format_cache_test.rs` covering transparent cell and header formatting.

- [x] **Phase 3: Verification & SDD Reconciliation**
  - [x] Run verification suite (`cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`).
  - [x] Reconcile living specs `specs/living-specs/model/theme.md` and `specs/living-specs/compiler/format-synthesis.md`.
  - [x] Archive draft spec to `specs/history/2026-09-21-transparent-fills-and-gridline-preservation.md`.
