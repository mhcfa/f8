---
title: Currency and Number Format Alignment
date: 2026-09-20
status: completed
target-living-spec:
  - specs/living-specs/model/theme.md
  - specs/living-specs/compiler/format-synthesis.md
---

## Proposal

### What

Update the default number format templates for `currency` and `number` presets across all built-in themes to replace Excel Accounting format with attached Currency format and enforce FAST-standard parenthetical alignment via trailing `_)` whitespace.

### Why

The previous default currency template (`_({sym}* #,##0{dec}_);_({sym}* (#,##0{dec});_({sym}* "-"_);_(@_)`) pinned the currency symbol to the left cell margin and pushed digits to the right margin using the Excel asterisk-fill token (`* `). This caused two visual defects in exported workbooks:
1. Normal rows (e.g. Revenue) rendered with an unwanted leading margin equal to an open parenthesis due to the `_(` prefix.
2. Normal rows (regular font weight) and subtotal rows (bold font weight with top borders, e.g. EBITDA) horizontally misaligned their currency symbols due to font glyph advance differences and Excel's border clipping quirks with asterisk-fill.

Additionally, plain `number` presets lacked trailing `_)` on positive values, preventing positive numbers from aligning vertically with parenthesized negative values.

### Scope

- **In-scope**:
  - Update `NumberFormatsConfig::default()` in `src/model/theme.rs` for `currency` and `number`.
  - Update all unit and integration test assertions reflecting the new format masks.
  - Update DSL formatting documentation in `docs/dsl/formatting.md`.
  - Reconcile living specs in `specs/living-specs/model/theme.md` and `specs/living-specs/compiler/format-synthesis.md`.
- **Out-of-scope**:
  - Changes to percentage, multiple, basis points, date, or text presets.
  - Changes to spatial layout column-width estimation.

### What Changes

- `src/model/theme.rs`:
  - `NumberFormatsConfig::default()` sets `currency` to `"{sym}#,##0{dec}_);({sym}#,##0{dec});\"-\"_)"`.
  - `NumberFormatsConfig::default()` sets `number` to `"#,##0{dec}_);(#,##0{dec});\"-\"_)"`.
- `tests/theme_test.rs`: Update expansion tests for 0 decimals, positive decimals, custom currencies, theme defaults, and JSON roundtrips.
- `tests/format_cache_test.rs`: Update synthesized format mask debug assertion.
- `docs/dsl/formatting.md`: Update theme reference and dynamic expansion documentation.

### Capabilities

- **Modified capabilities**:
  - `FormatPreset::Currency` emits attached currency masks with parenthesized negatives and trailing `_)` alignment for positive and zero values.
  - `FormatPreset::Number` emits parenthesized negatives with trailing `_)` alignment on positive and zero values.

## Design

### Goals

- Currency symbol `{sym}` attaches directly to the right-aligned number without an expansive blank gap or asterisk-fill.
- All numbers, subtotals, and totals in a column align their decimal points and digit positions vertically.
- Negative numbers are enclosed in parentheses `( )` in accordance with FAST standard conventions.
- Zero values display cleanly as a dash `"-"` aligned with numeric digits.

### Invariants

- Currency templates must contain `{sym}` and `{dec}` placeholders.
- Numeric templates must contain the `{dec}` placeholder.
- Expanding `{sym}` and `{dec}` must remain purely functional and idempotent.

### Decisions

- **Choice**: Use attached currency format `{sym}#,##0{dec}_);({sym}#,##0{dec});"-"_)` instead of accounting format `_({sym}* #,##0{dec}_);...`.
  - *Rationale*: Solves left-margin space discrepancy and bold/regular horizontal misalignment while keeping unit and magnitude together on the right.
  - *Alternatives considered*: Keeping accounting format and stripping `_(`. Rejected because asterisk-fill still exhibits glyph metric and border clipping inconsistencies across font weights in Excel.
- **Choice**: Add trailing `_)` to positive and zero values in both `currency` and `number`.
  - *Rationale*: Guarantees perfect vertical column alignment between positive numbers, negative numbers `(...)`, and zero dashes `"-"`.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Currency mask expansion with zero decimals**
  - **Given** active theme `corporate_navy`
  - **When** resolving format for `FormatPreset::Currency` with `decimals: Some(0)` and `currency: None`
  - **Then** resolves to `$#,##0_);($#,##0);"-"_)`.

- [x] **Scenario 2: Currency mask expansion with positive decimals and localized currency**
  - **Given** active theme `corporate_navy`
  - **When** resolving format for `FormatPreset::Currency` with `decimals: Some(2)` and `currency: Some(CurrencyCode::Eur)`
  - **Then** resolves to `€#,##0.00_);(€#,##0.00);"-"_)`.

- [x] **Scenario 3: Number mask expansion with positive decimals**
  - **Given** active theme `corporate_navy`
  - **When** resolving format for `FormatPreset::Number` with `decimals: Some(4)`
  - **Then** resolves to `#,##0.0000_);(#,##0.0000);"-"_)`.

- [x] **Scenario 4: Number mask expansion with default decimals**
  - **Given** active theme `corporate_navy`
  - **When** resolving format for `FormatPreset::Number` with `decimals: None`
  - **Then** resolves to `#,##0_);(#,##0);"-"_)`.

## Tasks

- [x] Update `NumberFormatsConfig::default()` in `src/model/theme.rs`.
- [x] Update test assertions in `tests/theme_test.rs` and `tests/format_cache_test.rs`.
- [x] Update documentation in `docs/dsl/formatting.md`.
- [x] Verify full test suite, clippy, and fmt.
- [x] Re-export test workbooks to `target/test-output/`.
- [x] Reconcile living specs and archive draft spec to `specs/history/`.
