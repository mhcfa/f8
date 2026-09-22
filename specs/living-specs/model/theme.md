---
title: Pluggable Themes & Dynamic Format Masks
last_updated: 2026-09-21
domain: model
status: active
---

## Purpose

The `theme` subsystem decouples workbook presentation styling, visual palette, typography, spatial metrics, and accounting number formats from calculation logic and table definitions. It provides strongly typed configuration schemas, dynamic `{dec}` and `{sym}` Excel format mask synthesis, the institutional built-in theme (`corporate_navy`), external theme resolution, and strict semantic validation.

## Architecture & Boundaries

- **Modules & components**:
  - `f8_core::model::theme`: Core structures (`Theme`, `TypographyConfig`, `MetricsConfig`, `PaletteConfig`, `BordersConfig`, `BorderStyle`, `NumberFormatsConfig`, `FormatDefaults`), validation rules, built-in theme lookup, and format mask generator.
  - `f8_core::model::manifest`: Root manifest theme resolution (`ProjectManifest::resolve_theme`) with default fallback to `"corporate_navy"`.
  - `f8_core::model::error`: Diagnostic errors (`ThemeNotFound`, `InvalidThemeColor`, `InvalidThemeMetric`, `MissingDecimalPlaceholder`, `MissingCurrencySymbolPlaceholder`, `ThemeParseError`, `ThemePrecisionOutOfBounds`).
- **Data flow & interfaces**:
  1. Manifest / Config $\rightarrow$ `Theme::resolve_builtin(name)` or `Theme::from_json_str(json)` $\rightarrow$ Built-in lookup or JSON string deserialization + validation $\rightarrow$ `Theme`.
  2. Grid expansion / Emitter $\rightarrow$ `theme.resolve_number_format(preset, decimals, currency)` $\rightarrow$ Synthesized Excel number format mask string.
  3. Pass 5 (Spatial Layout) $\rightarrow$ Consumes `theme.typography` and `theme.metrics` for row heights, column widths, gutters, and margins.
  4. Pass 6 (Excel Emission) $\rightarrow$ Consumes `theme.palette`, `theme.borders`, and synthesized format masks for OpenXML styling.
- **Dependencies**:
  - `serde`, `serde_json`: Serialization and deserialization of theme configuration schemas.
  - `thiserror`: Structured diagnostics for theme loading and validation.

## Requirements & Invariants

### 1. Theme Configuration Schema

A `Theme` contains complete specifications for `typography`, `metrics`, `palette`, `borders`, and `number_formats`.

#### Scenarios

- **Valid built-in theme**:
  - **When** `Theme::resolve_builtin` is invoked for `"corporate_navy"`
  - **Then** the built-in theme is loaded and passes validation.
- **Removed preset themes**:
  - **When** `Theme::resolve_builtin` is invoked for removed presets (e.g. `"classic_green"`, `"modern_slate"`, or `"minimal_monochrome"`)
  - **Then** `ModelError::ThemeNotFound` is returned.
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

Themes enforce strict metric positivity, fill transparency keywords, and hexadecimal color formatting:
- **Palette fills**: Must be `"none"` or `"transparent"` (case-insensitive) for transparent backgrounds, or start with `#` and contain 3, 6, or 8 hexadecimal digits (`#RGB`, `#RRGGBB`, `#RRGGBBAA`). Invalid fill colors return `ModelError::InvalidThemeColor`.
- **Palette text and border colors**: Must start with `#` and contain 3, 6, or 8 hexadecimal digits (`#RGB`, `#RRGGBB`, `#RRGGBBAA`). Keywords `"none"` and `"transparent"` are rejected for text and border colors (`ModelError::InvalidThemeColor`).
- **Metrics & typography**: All font sizes, row heights, column metrics, gutters, and character padding must be finite and strictly positive ($> 0.0$). Non-positive or non-finite values return `ModelError::InvalidThemeMetric`.
- **Template placeholders**: Numeric templates must contain `{dec}`; currency templates must contain both `{dec}` and `{sym}`. Missing placeholders return `ModelError::MissingDecimalPlaceholder` or `ModelError::MissingCurrencySymbolPlaceholder`.
