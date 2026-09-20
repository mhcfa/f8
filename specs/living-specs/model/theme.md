---
title: Pluggable Themes & Dynamic Format Masks
last_updated: 2026-09-20
domain: model
status: active
---

## Purpose

The `theme` subsystem decouples workbook presentation styling, visual palette, typography, spatial metrics, and accounting number formats from calculation logic and table definitions. It provides strongly typed configuration schemas, dynamic `{dec}` and `{sym}` Excel format mask synthesis, institutional built-in theme presets, external theme resolution, and strict semantic validation.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::model::theme`: Core structures (`Theme`, `TypographyConfig`, `MetricsConfig`, `PaletteConfig`, `BordersConfig`, `BorderStyle`, `NumberFormatsConfig`, `FormatDefaults`), validation rules, built-in themes catalog, and format mask generator.
  - `finx::model::manifest`: Root manifest theme resolution (`ProjectManifest::resolve_theme`) with default fallback to `"corporate_navy"`.
  - `finx::model::error`: Diagnostic errors (`ThemeNotFound`, `InvalidThemeColor`, `InvalidThemeMetric`, `MissingDecimalPlaceholder`, `MissingCurrencySymbolPlaceholder`, `ThemeIoError`, `ThemeParseError`, `ThemePrecisionOutOfBounds`).
- **Data flow & interfaces**:
  1. Manifest / CLI / Config $\rightarrow$ `Theme::resolve(name_or_path, base_dir)` $\rightarrow$ Built-in lookup or JSON file deserialization + validation $\rightarrow$ `Theme`.
  2. Grid expansion / Emitter $\rightarrow$ `theme.resolve_number_format(preset, decimals, currency)` $\rightarrow$ Synthesized Excel number format mask string.
  3. Pass 5 (Spatial Layout) $\rightarrow$ Consumes `theme.typography` and `theme.metrics` for row heights, column widths, gutters, and margins.
  4. Pass 6 (Excel Emission) $\rightarrow$ Consumes `theme.palette`, `theme.borders`, and synthesized format masks for OpenXML styling.
- **Dependencies**:
  - `serde`, `serde_json`: Serialization and deserialization of theme configuration schemas.
  - `std::path::{Path, PathBuf}`: Resolution of relative and absolute theme file paths.
  - `thiserror`: Structured diagnostics for theme loading and validation.

## Requirements & Invariants

### 1. Theme Configuration Schema

A `Theme` contains complete specifications for `typography`, `metrics`, `palette`, `borders`, and `number_formats`.

#### Scenarios

- **Valid built-in themes**:
  - **When** `Theme::resolve` is invoked for `"corporate_navy"`, `"classic_green"`, `"modern_slate"`, or `"minimal_monochrome"`
  - **Then** the corresponding built-in theme is loaded and passes validation.
- **Manifest fallback**:
  - **When** a `ProjectManifest` omits `theme` or specifies an empty string
  - **Then** `manifest.resolve_theme` cleanly resolves to `"corporate_navy"`.

### 2. Dynamic `{dec}` and `{sym}` Format Mask Generation

Format masks dynamically expand decimal precision and currency symbols from single pattern templates:
- `{dec}`: Expands to `""` for 0 decimals, and `.{zeros}` for $1 \le N \le 14$ decimals.
- `{sym}`: Expands to `currency.unwrap_or_default().excel_symbol()`, using quoted symbols for multi-character or non-ASCII codes (`"CHF"`, `"kr"`, `"₹"`) and unquoted for single-character glyphs (`$`, `€`, `£`, `¥`).
- Non-numeric presets (`date`, `text`): Return static template strings without modification.

#### Scenarios

- **Zero decimals expansion**:
  - **When** `theme.resolve_number_format` is invoked with `decimals: Some(0)`
  - **Then** `{dec}` is stripped cleanly (e.g. `$#,##0_);($#,##0);"-"_)`, `0%`, `0"x"`, `#,##0" bps"`).
- **Positive precision and international currencies**:
  - **When** invoked with `decimals: Some(2)` and `currency: Some(CurrencyCode::Chf)`
  - **Then** `{sym}` expands to `"\"CHF\""` and `{dec}` to `".00"`.
- **Default precision fallback**:
  - **When** `decimals` is `None`
  - **Then** the preset's configured default precision in `number_formats.defaults` is used.
- **Precision out of bounds**:
  - **When** `decimals` exceeds 14
  - **Then** `ModelError::ThemePrecisionOutOfBounds` is returned.

### 3. Metric and Color Validation

Themes enforce strict metric positivity and hexadecimal color formatting:
- **Palette colors**: Must start with `#` and contain 3, 6, or 8 hexadecimal digits (`#RGB`, `#RRGGBB`, `#RRGGBBAA`). Invalid colors return `ModelError::InvalidThemeColor`.
- **Metrics & typography**: All font sizes, row heights, column metrics, gutters, and character padding must be finite and strictly positive ($> 0.0$). Non-positive or non-finite values return `ModelError::InvalidThemeMetric`.
- **Template placeholders**: Numeric templates must contain `{dec}`; currency templates must contain both `{dec}` and `{sym}`. Missing placeholders return `ModelError::MissingDecimalPlaceholder` or `ModelError::MissingCurrencySymbolPlaceholder`.
