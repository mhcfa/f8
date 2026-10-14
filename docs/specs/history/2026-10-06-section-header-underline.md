---
title: Section Header Font Underline and Transparent Band Styling
date: 2026-10-06
status: completed
target-living-spec: specs/living-specs/model/theme.md
---

## Proposal

### Problem

Section headers (`RowSpec::SectionHeader`) lack text underline configurability in `TypographyConfig`. In institutional financial modeling, section headers frequently utilize text font underlines (e.g. <u>**Revenue & Gross Profit**</u>) on a clean, transparent background rather than full-width shaded banners, where the underline terminates cleanly at the end of the text characters instead of spanning across measure columns. Furthermore, the default `corporate_navy` theme currently applies a solid background fill (`section_header_fill: "#F7F9FC"`), which clashes aesthetically with text font underlines.

### Solution

1. Add `section_header_underline: bool` to `TypographyConfig` in `crates/f8-core/src/model/theme.rs` with `#[serde(default = "default_section_header_underline")]`, defaulting to `true`.
2. Update `TypographyConfig::default()` to set `section_header_underline: true`.
3. Update `corporate_navy` palette in `crates/f8-core/src/model/theme.rs`: change `section_header_fill` from `"#F7F9FC"` to `"none"`.
4. Synthesize `FormatUnderline::Single` on `section_header_format` in `crates/f8-core/src/compiler/emission/format.rs` when `theme.typography.section_header_underline` is `true`.
5. Update theme schema documentation in `docs/dsl/formatting.md`.
6. Add unit, format cache, and emission test coverage.

### Scope

- **In-scope**:
  - `crates/f8-core/src/model/theme.rs`: Add `section_header_underline: bool` (default `true`) to `TypographyConfig`, and update `corporate_navy` to `section_header_fill: "none"`.
  - `crates/f8-core/src/compiler/emission/format.rs`: Apply `FormatUnderline::Single` to `section_header_format` when enabled.
  - `docs/dsl/formatting.md`: Document `section_header_underline` in typography configuration table and update `corporate_navy` schema example.
  - Tests: Update and add test cases in `crates/f8-core/tests/theme_test.rs`, `crates/f8-core/tests/format_cache_test.rs`, and `crates/f8-core/tests/workbook_emission_test.rs`.
- **Out-of-scope**:
  - Double or accounting underline variations (single text underline is the standard institutional financial modeling convention for section dividers).
  - Point-level or row-level underline overrides on `SectionHeaderRowSpec` (typography remains governed uniformly by the active theme).

### Impacted components

- `crates/f8-core/src/model/theme.rs`: `TypographyConfig` struct, default constructor, serde default helper, and `Theme::corporate_navy`.
- `crates/f8-core/src/compiler/emission/format.rs`: `FormatCache::new` section header format generator.
- `docs/dsl/formatting.md`: Theme schema documentation and typography configuration table.

### Capabilities

- **New capabilities**: Themes can configure `typography.section_header_underline: bool` (defaulting to `true`) to toggle native Excel font underline (`FormatUnderline::Single`) on section header titles.
- **Modified capabilities**: `corporate_navy` renders section headers with clean transparent background (`section_header_fill: "none"`) and text-bounded font underline by default, omitting continuous blank band cells across measure columns.

---

## Design

### Goals & non-goals

- **Goals**:
  - Native Excel text underline (`<u val="single"/>`) on section header titles.
  - Default `corporate_navy` aligns with institutional standard: transparent background (`section_header_fill: "none"`) with text-bounded underline.
  - Clean boolean serde default (`true`) when omitted from custom theme JSON files.
  - Symmetrical format caching through `FormatCache`.
  - Gridline preservation across section header rows when fill is `"none"`.
- **Non-goals**:
  - Legacy backward compatibility shims or versioned migration adapters (f8 is greenfield v0.1.0).
  - Introducing cell bottom borders on section header rows.

### Invariants

- **Text-bounded underline**: Underline applies strictly as a font property on the title text; it does not introduce cell border lines or span across data columns.
- **Fill orthogonality**: Works cleanly whether `section_header_fill` is set to `"none"`, `"transparent"`, or a solid hex color.
- **Serde boolean default**: Deserializing a theme JSON omitting `section_header_underline` succeeds and yields `true`.

### Technical decisions

- **Decision**: Model as `pub section_header_underline: bool` in `TypographyConfig`, defaulting to `true`.
  - **Rationale**: Single underline is the institutional standard for section headers; boolean toggle cleanly integrates with existing typography settings.
- **Decision**: Set `section_header_fill: "none"` in `corporate_navy`.
  - **Rationale**: Replaces the full-width `#F7F9FC` shaded banner with the classic Wall Street / PE presentation standard: bold text with font underline on a clean background.
- **Decision**: Mark with `#[serde(default = "default_section_header_underline")]`.
  - **Rationale**: Follows the established codebase pattern used by `default_integer_format` in `NumberFormatsConfig`, defaulting omitted boolean fields to `true`.
- **Decision**: Apply via `format.set_underline(FormatUnderline::Single)` in `FormatCache::new`.
  - **Rationale**: Integrates directly with the format deduplication cache and `section_header_format(indent)` hierarchy without requiring changes to binary emission logic in `workbook.rs`.

### Data contracts

```rust
// In crates/f8-core/src/model/theme.rs

fn default_section_header_underline() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypographyConfig {
    pub font_family: String,
    pub title_size: f64,
    pub subtitle_size: f64,
    pub super_header_size: f64,
    pub col_header_size: f64,
    pub section_header_size: f64,
    pub data_size: f64,
    #[serde(default = "default_section_header_underline")]
    pub section_header_underline: bool,
}

impl Default for TypographyConfig {
    fn default() -> Self {
        Self {
            font_family: "Aptos".to_string(),
            title_size: 14.0,
            subtitle_size: 9.0,
            super_header_size: 10.0,
            col_header_size: 10.0,
            section_header_size: 10.0,
            data_size: 10.0,
            section_header_underline: true,
        }
    }
}
```

### Risks & mitigations

- **Risk**: Visual change in emitted models from removing `#F7F9FC` band and adding underline.
- **Mitigation**: Intended aesthetic alignment with institutional financial modeling standards; custom themes desiring shaded banners can explicitly configure `section_header_fill` to a hex color and `section_header_underline: false`.

---

### Acceptance criteria (Given / When / Then)

- [x] **Scenario 1: Default fallback and deserialization**
  - **Given** a theme JSON omitting `section_header_underline`
  - **When** parsed via `Theme::from_json_str`
  - **Then** deserialization succeeds and `theme.typography.section_header_underline` is `true`.

- [x] **Scenario 2: Explicit underline disabled in theme**
  - **Given** a theme JSON specifying `"section_header_underline": false` in `typography`
  - **When** parsed via `Theme::from_json_str`
  - **Then** `theme.typography.section_header_underline` is `false`.

- [x] **Scenario 3: Format synthesis produces single underline**
  - **Given** a theme with `typography.section_header_underline = true`
  - **When** `FormatCache::new(&theme)` is initialized
  - **Then** `format_cache.section_header_format(None)` has font underline set to `FormatUnderline::Single`.

- [x] **Scenario 4: Built-in corporate_navy attributes**
  - **Given** the built-in `corporate_navy` theme
  - **When** inspected
  - **Then** `theme.typography.section_header_underline` is `true`, `theme.palette.section_header_fill` is `"none"`, and `format_cache.has_section_header_fill()` is `false`.

- [x] **Scenario 5: Emission verification in workbook**
  - **Given** a table with a section header row and `corporate_navy` theme
  - **When** the workbook is emitted to OpenXML bytes
  - **Then** workbook generation completes successfully, the section header title is formatted with single underline, and trailing cells across measure columns are unwritten (preserving transparent gridlines).

---

## Tasks

- [x] 1. Update `crates/f8-core/src/model/theme.rs`: Add `section_header_underline: bool` with `#[serde(default = "default_section_header_underline")]` defaulting to `true`, and set `section_header_fill: "none"` in `Theme::corporate_navy`.
- [x] 2. Update `crates/f8-core/src/compiler/emission/format.rs`: Synthesize `FormatUnderline::Single` on `section_header_format` when `section_header_underline` is `true`.
- [x] 3. Update documentation in `docs/dsl/formatting.md`.
- [x] 4. Update existing tests in `crates/f8-core/tests/theme_test.rs` and `crates/f8-core/tests/format_cache_test.rs` to reflect `corporate_navy`'s `"none"` fill and underline.
- [x] 5. Add unit and emission tests in `crates/f8-core/tests/theme_test.rs`, `crates/f8-core/tests/format_cache_test.rs`, and `crates/f8-core/tests/workbook_emission_test.rs`.
- [x] 6. Run verification suite (`cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `uv run pytest`).
