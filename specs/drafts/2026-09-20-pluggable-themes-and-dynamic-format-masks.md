---
title: Pluggable Themes & Dynamic Format Masks
date: 2026-09-20
status: draft
target-living-spec: specs/living-specs/model/theme.md
---

## Proposal

### What

Implement Finx's pluggable workbook theme system and dynamic format mask synthesis engine:
1. Define the comprehensive `Theme` schema (`typography`, `metrics`, `palette`, `borders`, `number_formats`) in `finx::model::theme`, including body/summary styling, financial hardcode color coding, free-form subtitle typography, and spatial gutters (inter-table and title buffer gutters).
2. Implement dynamic `{dec}` and `{sym}` expansion in `theme.resolve_number_format(preset, decimals, currency)` to synthesize concrete Excel number format masks for any `(FormatPreset, Option<u32>, Option<CurrencyCode>)` tuple, replacing `{sym}` with the currency's Excel-safe symbol (defaulting to `CurrencyCode::Usd`) and `{dec}` with the requested or default precision zeros.
3. Package 4 institutional built-in themes (`corporate_navy`, `classic_green`, `modern_slate`, `minimal_monochrome`).
4. Implement theme resolution in `finx::model::theme` and `ProjectManifest`, supporting both built-in preset identifiers and external custom theme JSON files with default fallback to `"corporate_navy"`.
5. Provide strict semantic validation for theme palette colors (`#RGB` / `#RRGGBB` / `#RRGGBBAA`), positive dimension metrics, template placeholders, and precision bounds.

### Why

Presentation styling must remain completely decoupled from calculation logic and table definitions:
- **Prerequisite for Pass 5 (Spatial Layout)**: Computing physical row heights, typography-scaled column widths, margins (Column A gutter), inter-table buffer row heights, and title buffer gutters requires theme metrics (`typography` and `metrics`).
- **Prerequisite for Pass 6 (Excel Emission)**: Serializing `.xlsx` binaries requires theme `palette` fills (including total/subtotal tints and financial input/formula text colors), `borders`, and concrete Excel number format masks.
- **Elimination of mask explosion & currency localization**: Rather than storing dozens of fixed format strings, the `{dec}` and `{sym}` template engine dynamically synthesizes institutional accounting masks for arbitrary fractional precision (`0`, `1`, `2`, `4`, etc.) and localized currency symbols (`$`, `€`, `£`, `¥`, `CHF`) from a single template per preset family.
- **Consistent institutional branding**: Financial models can change their entire aesthetic, color palette, and font metrics by altering a single `theme` field in `finx.json` (or omitting it to use institutional corporate navy).

### Scope

- **In-Scope**:
  - Theme data structures (`Theme`, `TypographyConfig`, `MetricsConfig`, `PaletteConfig`, `BordersConfig`, `BorderStyle`, `NumberFormatsConfig`, `FormatDefaults`) in `src/model/theme.rs`.
  - Built-in theme catalog (`corporate_navy`, `classic_green`, `modern_slate`, `minimal_monochrome`).
  - Dynamic decimal and currency mask generator (`theme.resolve_number_format(preset, decimals, currency)`).
  - Manifest theme loader and default fallback (`Theme::resolve(name_or_path, base_dir)` and `ProjectManifest::resolve_theme`).
  - Manifest serde default (`#[serde(default = "default_theme")]` returning `"corporate_navy"`).
  - Validation rules (hex colors `#RGB` / `#RRGGBB` / `#RRGGBBAA`, positive metrics, precision limits $\le 14$, template sanity).
  - Diagnostic error variants in `finx::model::error::ModelError`.
  - Comprehensive unit tests for theme deserialization, `{dec}` and `{sym}` expansion across all presets, validation error cases, and built-in theme completeness.

- **Out-of-Scope**:
  - Pass 5 spatial layout planning, margins, gutters, column width arbitration, and row heights (Phase 3).
  - Pass 6 formula lowering and OpenXML binary emission via `rust_xlsxwriter` (Phase 4).
  - CLI commands for theme listing/inspection.

### What Changes

- **Affected modules**:
  - `src/model/theme.rs`: New module defining the theme schema, built-ins, and `{dec}` / `{sym}` expansion.
  - `src/model/mod.rs`: Export `theme` module and key theme types.
  - `src/model/error.rs`: Add theme-related error variants to `ModelError`.
  - `src/model/manifest.rs`: Add `resolve_theme` helper and `#[serde(default = "default_theme")]` fallback to `ProjectManifest`.
  - `tests/theme_test.rs`: New integration test suite covering themes and format mask synthesis.

### Capabilities

- **New capabilities**:
  - Model specifications can reference built-in themes (`"corporate_navy"`, `"classic_green"`, `"modern_slate"`, `"minimal_monochrome"`), specify a relative path to a custom theme JSON file (e.g. `"./themes/custom.json"`), or omit `theme` to default to `"corporate_navy"`.
  - Callers can resolve any `(FormatPreset, Option<u32>, Option<CurrencyCode>)` into a standard Excel format string with dynamic precision and currency symbol expansion.
  - Omitted `decimals` automatically fall back to the active theme's configured defaults for that preset family; omitted `currency` defaults to `CurrencyCode::Usd`.
  - Downstream layout and emission passes have access to a strongly typed, pre-validated `Theme` object with full body, summary, subtitle, input/formula, and spatial margin/gutter styling.

- **Modified capabilities**:
  - `ProjectManifest::validate` can optionally verify theme resolution if a base directory is supplied.

---

## Design

### Goals

- **Single source of truth**: Visual identity (colors, fonts, heights, borders, number masks) lives exclusively in themes.
- **Dynamic precision & currency localization**: `{dec}` and `{sym}` template expansion generates accurate accounting format masks for any precision and localized currency without maintaining combinatorial string variants.
- **Zero runtime filesystem dependency for built-in themes**: Built-in themes are statically compiled into the binary.
- **Deterministic and infallible resolution for valid models**: Once loaded and validated, theme lookups and mask generations are pure, deterministic, and panic-free.

### Invariants

1. **Preset completeness**: Every theme must define format templates and default precisions for all numeric `FormatPreset` variants (`currency`, `number`, `percentage`, `multiple`, `basis_points`) plus static templates for non-numeric presets (`date`, `text`).
2. **Template validity**: Numeric format templates must contain the `{dec}` placeholder. The `currency` template must additionally contain `{sym}`. Non-numeric templates (`date`, `text`) must not require `{dec}` or `{sym}`.
3. **Deterministic expansion**:
   - Replaces **all** occurrences of `{dec}` and `{sym}` in the template string.
   - For `currency`, `{sym}` expands to `currency.unwrap_or_default().excel_symbol()`. Single-character symbols (`$`, `€`, `£`, `¥`) substitute directly; multi-character codes and non-ASCII glyphs (`CHF`, `kr`, `C$`, `A$`, `₹`) substitute quoted (`\"CHF\"`, `\"kr\"`, `\"₹\"`, etc.) to prevent Excel format token collisions.
   - For `decimals: Some(0)`, `{dec}` expands to empty string `""`.
   - For `decimals: Some(n)` where $0 < n \le 14$, `{dec}` expands to `.` followed by $n$ zeros (e.g. `2` $\rightarrow$ `".00"`).
   - For `decimals: Some(n)` where $n > 14$, resolution returns `ModelError::ThemePrecisionOutOfBounds`.
   - For `decimals: None`, resolution uses `theme.number_formats.defaults` for that preset and expands accordingly. If a theme's defaults specify precision $> 14$, validation returns `ModelError::ThemePrecisionOutOfBounds`.
4. **Metric positivity**: All font sizes, row heights, column metrics, and character padding must be finite, strictly positive floating-point numbers ($> 0.0$).
5. **Color validity**: All palette entries must be valid hexadecimal color strings starting with `#` and containing 3, 6, or 8 hex digits (`#RGB`, `#RRGGBB`, or `#RRGGBBAA`).
6. **Manifest default & schema tolerance**: The root manifest defaults `theme` to `"corporate_navy"` if omitted. The `Theme` schema tolerates and deserializes optional `"$schema"` fields cleanly.

### Decisions

- **Decision: Dynamic `{dec}` and `{sym}` expansion method on `Theme`**
  - *Rationale*: Excel number format strings use repeating zeros after a decimal point (e.g. `#,##0.00`) and accounting currency indicators (`$*`, `€*`, `£*`). Storing separate templates for every conceivable decimal place and international currency code leads to schema bloat. A single template with global `{dec}` and `{sym}` replacement solves this cleanly.
  - *Alternatives considered*: Hardcoding `$` into currency strings and requiring users to author separate theme files for EUR/GBP models. Rejected because it forces redundant theme duplication when only the currency code changes.

- **Decision: Granular palette roles for body, summary, and financial inputs/formulas**
  - *Rationale*: High-quality financial models require distinct text and fill styling for body data, intermediate subtotals, final totals, free-form subtitles, and institutional hardcode color coding (blue inputs, black formulas). Adding `data_fill`, `data_text`, `subtotal_fill`, `subtotal_text`, `total_fill`, `total_text`, `table_subtitle_text`, `input_text`, `formula_text`, and `total_border_color` enables complete skinning (including dark mode and Wall Street standards) without schema hacks.

- **Decision: First-class spatial metrics in `MetricsConfig` (Gutters and Margins)**
  - *Rationale*: Layout Pass 5 requires concrete metrics for sheet margins (Column A gutter), inter-table buffer row heights, title/subtitle buffer rows, and column character padding. Exposing `gutter_col_width`, `gutter_row_height`, `title_gutter_row_height`, and `col_padding` in the theme prevents hardcoding magic layout numbers in the compiler.

- **Decision: Statically embedded built-in themes**
  - *Rationale*: Built-in themes must work in all environments (CLI, WebAssembly, serverless, headless test harnesses) without requiring external file assets or directory path assumptions.

- **Decision: `BorderStyle` enum instead of raw strings**
  - *Rationale*: Excel supports specific border styles (`none`, `thin`, `medium`, `thick`, `double`, `dashed`, `dotted`, `hair`). Using a strongly typed enum prevents invalid border specifications at deserialize time.

### Data Contracts

#### 1. Theme Configuration Structures

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub name: String,
    pub typography: TypographyConfig,
    pub metrics: MetricsConfig,
    pub palette: PaletteConfig,
    pub borders: BordersConfig,
    pub number_formats: NumberFormatsConfig,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub title_row_height: f64,
    pub header_row_height: f64,
    pub section_row_height: f64,
    pub data_row_height: f64,
    pub gutter_row_height: f64,
    pub gutter_col_width: f64,
    pub title_gutter_row_height: f64,
    pub char_width_multiplier: f64,
    pub col_padding: f64,
    pub min_col_width: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteConfig {
    pub table_title_fill: String,
    pub table_title_text: String,
    pub table_subtitle_text: String,
    pub super_header_fill: String,
    pub super_header_text: String,
    pub col_header_fill: String,
    pub col_header_text: String,
    pub section_header_fill: String,
    pub section_header_text: String,
    pub data_fill: String,
    pub data_text: String,
    pub subtotal_fill: String,
    pub subtotal_text: String,
    pub total_fill: String,
    pub total_text: String,
    pub input_text: String,
    pub formula_text: String,
    pub zebra_fill: String,
    pub highlight_fill: String,
    pub highlight_text: String,
    pub gridline_color: String,
    pub border_color: String,
    pub total_border_color: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    #[default]
    None,
    Thin,
    Medium,
    Thick,
    Double,
    Dashed,
    Dotted,
    Hair,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BordersConfig {
    pub table_title_border_bottom: BorderStyle,
    pub super_header_border_bottom: BorderStyle,
    pub header_border_bottom: BorderStyle,
    pub descriptor_border_right: BorderStyle,
    pub subtotal_border_top: BorderStyle,
    pub total_border_top: BorderStyle,
    pub total_border_bottom: BorderStyle,
    pub col_subtotal_border_left: BorderStyle,
    pub col_total_border_left: BorderStyle,
    pub col_total_border_right: BorderStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberFormatsConfig {
    pub currency: String,
    pub number: String,
    pub percentage: String,
    pub multiple: String,
    pub basis_points: String,
    pub date: String,
    pub text: String,
    pub defaults: FormatDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatDefaults {
    pub currency_decimals: u32,
    pub number_decimals: u32,
    pub percentage_decimals: u32,
    pub multiple_decimals: u32,
    pub basis_points_decimals: u32,
}
```

#### 2. Number Format Resolution Interface

```rust
impl Theme {
    /// Resolves a format preset, optional precision, and optional currency code into a concrete Excel format mask.
    ///
    /// - If `decimals` is `None`, the preset's default precision from `number_formats.defaults` is used.
    /// - For numeric presets, `{dec}` is expanded dynamically.
    /// - For `currency`, `{sym}` expands to `currency.unwrap_or_default().excel_symbol()`.
    /// - For non-numeric presets (`date`, `text`), the static template is returned.
    pub fn resolve_number_format(
        &self,
        preset: FormatPreset,
        decimals: Option<u32>,
        currency: Option<CurrencyCode>,
    ) -> Result<String, ModelError>;
}
```

#### 3. Diagnostic Errors (`ModelError`)

```rust
pub enum ModelError {
    // ... existing variants ...

    #[error("Theme '{name}' not found in built-in catalog and file does not exist: {path}")]
    ThemeNotFound { name: String, path: PathBuf },

    #[error("Invalid hex color code '{color}' in theme palette for field '{field}'")]
    InvalidThemeColor { field: String, color: String },

    #[error("Invalid theme metric '{metric}': expected positive finite value, found {value}")]
    InvalidThemeMetric { metric: String, value: f64 },

    #[error("Theme format template for '{preset}' must contain '{{dec}}' placeholder")]
    MissingDecimalPlaceholder { preset: String },

    #[error("Theme format template for 'currency' must contain '{{sym}}' placeholder")]
    MissingCurrencySymbolPlaceholder,

    #[error("Failed to read theme file '{path}': {source}")]
    ThemeIoError { path: PathBuf, source: std::io::Error },

    #[error("Failed to parse theme file '{path}': {source}")]
    ThemeParseError { path: PathBuf, source: serde_json::Error },

    #[error("Theme format for '{preset}' specifies precision ({decimals}) exceeding maximum supported precision ({max})")]
    ThemePrecisionOutOfBounds {
        preset: FormatPreset,
        decimals: u32,
        max: u32,
    },
}
```

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Built-in theme catalog resolution**
  - **Given** theme identifiers `"corporate_navy"`, `"classic_green"`, `"modern_slate"`, and `"minimal_monochrome"`
  - **When** `Theme::resolve` is invoked with each name
  - **Then** the corresponding built-in theme is returned with valid typography (including `subtitle_size`), palette (including body, subtitle, summary, input/formula, and total border colors), metrics (including margins, gutters, title buffer rows, and padding), and borders.

- [ ] **Scenario 2: Dynamic `{dec}` and `{sym}` expansion for zero decimals**
  - **Given** active theme `corporate_navy` and `decimals: Some(0)`
  - **When** `theme.resolve_number_format` is invoked for `FormatPreset::Currency` with `currency: None`
  - **Then** `{sym}` expands to `"$"`, and `{dec}` expands to `""` yielding `_($* #,##0_);_($* (#,##0);_($* "-"_);_(@_)`.
  - **When** invoked for `Number`, `Percentage`, `Multiple`, and `BasisPoints`
  - **Then** all `{dec}` placeholders expand to `""` (e.g. `#,##0;(#,##0);"-"`, `0%`, `0"x"`, `#,##0" bps"`).

- [ ] **Scenario 3: Dynamic `{dec}` and `{sym}` expansion for positive decimals and custom currency**
  - **Given** active theme `corporate_navy`
  - **When** `theme.resolve_number_format` is invoked for `FormatPreset::Currency` with `decimals: Some(2)` and `currency: Some(CurrencyCode::Eur)`
  - **Then** all `{dec}` placeholders expand to `".00"` and `{sym}` to `"€"` yielding `_(€* #,##0.00_);_(€* (#,##0.00);_(€* "-"_);_(@_)`.
  - **When** invoked for `Currency` with `Some(CurrencyCode::Chf)`
  - **Then** `{sym}` expands to `"\"CHF\""` yielding `_("CHF"* #,##0.00_);_("CHF"* (#,##0.00);_("CHF"* "-"_);_(@_)`.
  - **When** `decimals: Some(1)` is invoked for `FormatPreset::Percentage`
  - **Then** `{dec}` expands to `".0"` yielding `0.0%`.
  - **When** `decimals: Some(4)` is invoked for `FormatPreset::Number`
  - **Then** all `{dec}` placeholders expand to `".0000"` yielding `#,##0.0000;(#,##0.0000);"-"`.

- [ ] **Scenario 4: Fallback to theme defaults when precision or currency is omitted**
  - **Given** active theme `corporate_navy` with defaults (currency=0, number=0, percentage=1, multiple=1, basis_points=0)
  - **When** `theme.resolve_number_format(preset, None, None)` is called
  - **Then** `currency` renders with `"$"` and 0 decimals, `percentage` with 1 decimal (`0.0%`), and `multiple` with 1 decimal (`0.0"x"`).

- [ ] **Scenario 5: Non-numeric presets ignore decimals and currency**
  - **Given** active theme `corporate_navy`
  - **When** `theme.resolve_number_format` is invoked for `Date` or `Text` with `Some(2)` and `Some(CurrencyCode::Eur)`
  - **Then** the static masks `yyyy-mm-dd` and `@` are returned unaltered.

- [ ] **Scenario 6: External custom theme loading**
  - **Given** a valid custom theme JSON file on disk containing optional `"$schema"` and custom palette/metrics
  - **When** `Theme::resolve("./themes/custom.json", Some(base_dir))` is called
  - **Then** the custom theme is loaded, validated, and returned.

- [ ] **Scenario 7: Rejection of invalid theme configurations**
  - **Given** a custom theme JSON with an invalid color (e.g. `"blue"` instead of `#1B365D`), non-positive row height or padding ($\le 0.0$), missing `{dec}` in templates, missing `{sym}` in currency template, or precision exceeding maximum bounds ($> 14$)
  - **When** theme validation or format resolution is executed
  - **Then** the appropriate `ModelError` (`InvalidThemeColor`, `InvalidThemeMetric`, `MissingDecimalPlaceholder`, `MissingCurrencySymbolPlaceholder`, or `ThemePrecisionOutOfBounds`) is returned.

- [ ] **Scenario 8: Rejection of non-existent theme**
  - **Given** an unknown theme name `"neon_cyberpunk"` that does not exist in built-ins and has no file on disk
  - **When** `Theme::resolve("neon_cyberpunk", None)` is called
  - **Then** `ModelError::ThemeNotFound` is returned.

- [ ] **Scenario 9: Manifest theme default fallback**
  - **Given** a `ProjectManifest` where `theme` is omitted or empty
  - **When** `manifest.resolve_theme(base_dir)` is invoked
  - **Then** it cleanly resolves to the built-in `"corporate_navy"` theme.

---

## Tasks

### Phase 1: Theme Data Structures & Serde Model
- [ ] Create `src/model/theme.rs` with `Theme`, `TypographyConfig`, `MetricsConfig`, `PaletteConfig`, `BordersConfig`, `BorderStyle`, `NumberFormatsConfig`, and `FormatDefaults`.
- [ ] Add theme error variants (`ThemeNotFound`, `InvalidThemeColor`, `InvalidThemeMetric`, `MissingDecimalPlaceholder`, `MissingCurrencySymbolPlaceholder`, `ThemeIoError`, `ThemeParseError`, `ThemePrecisionOutOfBounds`) to `src/model/error.rs`.
- [ ] Implement `Theme::validate(&self)` checking hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`), metric positivity, and `{dec}` / `{sym}` placeholders.
- [ ] Export `theme` module in `src/model/mod.rs`.

### Phase 2: Dynamic Format Mask Generation
- [ ] Implement `resolve_number_format(&self, preset: FormatPreset, decimals: Option<u32>, currency: Option<CurrencyCode>) -> Result<String, ModelError>`.
- [ ] Implement `{dec}` and `{sym}` global expansion helper function handling 0 decimals (`""`), $1 \le N \le 14$ decimals (`.00...0`), currency Excel mask symbol (`code.excel_symbol()`), and rejecting $N > 14$ with `ThemePrecisionOutOfBounds`.
- [ ] Implement non-numeric preset static passthrough.

### Phase 3: Built-in Institutional Themes
- [ ] Implement `corporate_navy` (default institutional private equity / investment banking theme).
- [ ] Implement `classic_green` (traditional corporate finance / hunter green theme).
- [ ] Implement `modern_slate` (tech / venture capital slate theme).
- [ ] Implement `minimal_monochrome` (high-contrast black & white print/audit theme).
- [ ] Add `Theme::built_in(name: &str) -> Option<Self>` lookup.

### Phase 4: Theme Resolution & Manifest Integration
- [ ] Implement `Theme::resolve(name_or_path: &str, base_dir: Option<&Path>) -> Result<Theme, ModelError>`.
- [ ] Implement `ProjectManifest::resolve_theme(&self, base_dir: Option<&Path>) -> Result<Theme, ModelError>` with `"corporate_navy"` default.
- [ ] Add `#[serde(default = "default_theme")]` to `ProjectManifest.theme`.

### Phase 5: Verification & Testing
- [ ] Create `tests/theme_test.rs` covering all 9 acceptance scenarios.
- [ ] Verify complete project compilation: `cargo check`.
- [ ] Run full test suite: `cargo test`.
- [ ] Run linter: `cargo clippy --all-targets -- -D warnings`.
- [ ] Run formatter check: `cargo fmt --check`.
