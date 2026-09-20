use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::error::ModelError;
use super::format::{CurrencyCode, FormatPreset};

/// Complete visual styling, typography, dimensions, palette, borders, and number format specification.
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

/// Font family and point sizes for workbook hierarchy.
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
        }
    }
}

/// Physical row heights, column character widths, gutters, and margins.
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

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            title_row_height: 24.0,
            header_row_height: 18.0,
            section_row_height: 18.0,
            data_row_height: 15.0,
            gutter_row_height: 9.0,
            gutter_col_width: 3.0,
            title_gutter_row_height: 8.0,
            char_width_multiplier: 1.15,
            col_padding: 2.0,
            min_col_width: 12.0,
        }
    }
}

/// Color assignments across all workbook components.
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

/// Line style for cell borders.
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

/// Border styling rules for structural headers and summary totals.
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

impl Default for BordersConfig {
    fn default() -> Self {
        Self {
            table_title_border_bottom: BorderStyle::None,
            super_header_border_bottom: BorderStyle::Thin,
            header_border_bottom: BorderStyle::Thin,
            descriptor_border_right: BorderStyle::None,
            subtotal_border_top: BorderStyle::Thin,
            total_border_top: BorderStyle::Thin,
            total_border_bottom: BorderStyle::Double,
            col_subtotal_border_left: BorderStyle::Thin,
            col_total_border_left: BorderStyle::Thin,
            col_total_border_right: BorderStyle::Double,
        }
    }
}

/// Number format templates and default precisions across all semantic presets.
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

impl Default for NumberFormatsConfig {
    fn default() -> Self {
        Self {
            currency: "{sym}#,##0{dec}_);({sym}#,##0{dec});\"-\"_)".to_string(),
            number: "#,##0{dec}_);(#,##0{dec});\"-\"_)".to_string(),
            percentage: "0{dec}%".to_string(),
            multiple: "0{dec}\"x\"".to_string(),
            basis_points: "#,##0{dec}\" bps\"".to_string(),
            date: "yyyy-mm-dd".to_string(),
            text: "@".to_string(),
            defaults: FormatDefaults::default(),
        }
    }
}

/// Default fractional decimal precision per numeric preset family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatDefaults {
    pub currency_decimals: u32,
    pub number_decimals: u32,
    pub percentage_decimals: u32,
    pub multiple_decimals: u32,
    pub basis_points_decimals: u32,
}

impl Default for FormatDefaults {
    fn default() -> Self {
        Self {
            currency_decimals: 0,
            number_decimals: 0,
            percentage_decimals: 1,
            multiple_decimals: 1,
            basis_points_decimals: 0,
        }
    }
}

fn is_valid_hex_color(s: &str) -> bool {
    if !s.starts_with('#') {
        return false;
    }
    let hex = &s[1..];
    (hex.len() == 3 || hex.len() == 6 || hex.len() == 8)
        && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn validate_metric(metric: &str, value: f64) -> Result<(), ModelError> {
    if !value.is_finite() || value <= 0.0 {
        Err(ModelError::InvalidThemeMetric {
            metric: metric.to_string(),
            value,
        })
    } else {
        Ok(())
    }
}

impl TypographyConfig {
    pub fn validate(&self) -> Result<(), ModelError> {
        validate_metric("title_size", self.title_size)?;
        validate_metric("subtitle_size", self.subtitle_size)?;
        validate_metric("super_header_size", self.super_header_size)?;
        validate_metric("col_header_size", self.col_header_size)?;
        validate_metric("section_header_size", self.section_header_size)?;
        validate_metric("data_size", self.data_size)?;
        Ok(())
    }
}

impl MetricsConfig {
    pub fn validate(&self) -> Result<(), ModelError> {
        validate_metric("title_row_height", self.title_row_height)?;
        validate_metric("header_row_height", self.header_row_height)?;
        validate_metric("section_row_height", self.section_row_height)?;
        validate_metric("data_row_height", self.data_row_height)?;
        validate_metric("gutter_row_height", self.gutter_row_height)?;
        validate_metric("gutter_col_width", self.gutter_col_width)?;
        validate_metric("title_gutter_row_height", self.title_gutter_row_height)?;
        validate_metric("char_width_multiplier", self.char_width_multiplier)?;
        validate_metric("col_padding", self.col_padding)?;
        validate_metric("min_col_width", self.min_col_width)?;
        Ok(())
    }
}

impl PaletteConfig {
    pub fn validate(&self) -> Result<(), ModelError> {
        let fields = [
            ("table_title_fill", &self.table_title_fill),
            ("table_title_text", &self.table_title_text),
            ("table_subtitle_text", &self.table_subtitle_text),
            ("super_header_fill", &self.super_header_fill),
            ("super_header_text", &self.super_header_text),
            ("col_header_fill", &self.col_header_fill),
            ("col_header_text", &self.col_header_text),
            ("section_header_fill", &self.section_header_fill),
            ("section_header_text", &self.section_header_text),
            ("data_fill", &self.data_fill),
            ("data_text", &self.data_text),
            ("subtotal_fill", &self.subtotal_fill),
            ("subtotal_text", &self.subtotal_text),
            ("total_fill", &self.total_fill),
            ("total_text", &self.total_text),
            ("input_text", &self.input_text),
            ("formula_text", &self.formula_text),
            ("zebra_fill", &self.zebra_fill),
            ("highlight_fill", &self.highlight_fill),
            ("highlight_text", &self.highlight_text),
            ("gridline_color", &self.gridline_color),
            ("border_color", &self.border_color),
            ("total_border_color", &self.total_border_color),
        ];

        for (field, color) in fields {
            if !is_valid_hex_color(color) {
                return Err(ModelError::InvalidThemeColor {
                    field: field.to_string(),
                    color: color.clone(),
                });
            }
        }
        Ok(())
    }
}

impl NumberFormatsConfig {
    pub fn validate(&self) -> Result<(), ModelError> {
        let numeric_templates = [
            (FormatPreset::Currency, &self.currency),
            (FormatPreset::Number, &self.number),
            (FormatPreset::Percentage, &self.percentage),
            (FormatPreset::Multiple, &self.multiple),
            (FormatPreset::BasisPoints, &self.basis_points),
        ];

        for (preset, template) in numeric_templates {
            if !template.contains("{dec}") {
                return Err(ModelError::MissingDecimalPlaceholder { preset });
            }
        }

        if !self.currency.contains("{sym}") {
            return Err(ModelError::MissingCurrencySymbolPlaceholder);
        }

        let default_precisions = [
            (FormatPreset::Currency, self.defaults.currency_decimals),
            (FormatPreset::Number, self.defaults.number_decimals),
            (FormatPreset::Percentage, self.defaults.percentage_decimals),
            (FormatPreset::Multiple, self.defaults.multiple_decimals),
            (
                FormatPreset::BasisPoints,
                self.defaults.basis_points_decimals,
            ),
        ];

        for (preset, decimals) in default_precisions {
            if decimals > 14 {
                return Err(ModelError::ThemePrecisionOutOfBounds {
                    preset,
                    decimals,
                    max: 14,
                });
            }
        }

        Ok(())
    }
}

impl Theme {
    /// Default institutional theme name.
    pub const DEFAULT_NAME: &'static str = "corporate_navy";

    /// Validates all palette colors, positive dimension metrics, template placeholders, and precision bounds.
    pub fn validate(&self) -> Result<(), ModelError> {
        self.typography.validate()?;
        self.metrics.validate()?;
        self.palette.validate()?;
        self.number_formats.validate()?;
        Ok(())
    }

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
    ) -> Result<String, ModelError> {
        let (template, default_dec) = match preset {
            FormatPreset::Date => return Ok(self.number_formats.date.clone()),
            FormatPreset::Text => return Ok(self.number_formats.text.clone()),
            FormatPreset::Currency => (
                &self.number_formats.currency,
                self.number_formats.defaults.currency_decimals,
            ),
            FormatPreset::Number => (
                &self.number_formats.number,
                self.number_formats.defaults.number_decimals,
            ),
            FormatPreset::Percentage => (
                &self.number_formats.percentage,
                self.number_formats.defaults.percentage_decimals,
            ),
            FormatPreset::Multiple => (
                &self.number_formats.multiple,
                self.number_formats.defaults.multiple_decimals,
            ),
            FormatPreset::BasisPoints => (
                &self.number_formats.basis_points,
                self.number_formats.defaults.basis_points_decimals,
            ),
        };

        let dec = decimals.unwrap_or(default_dec);
        if dec > 14 {
            return Err(ModelError::ThemePrecisionOutOfBounds {
                preset,
                decimals: dec,
                max: 14,
            });
        }

        let dec_str = if dec == 0 {
            String::new()
        } else {
            format!(".{}", "0".repeat(dec as usize))
        };

        let mut expanded = template.replace("{dec}", &dec_str);
        if preset == FormatPreset::Currency {
            let sym = currency.unwrap_or_default().excel_symbol();
            expanded = expanded.replace("{sym}", sym);
        }

        Ok(expanded)
    }

    /// Resolves a theme by built-in identifier (e.g. "corporate_navy") or relative file path to a theme JSON.
    pub fn resolve(name_or_path: &str, base_dir: Option<&Path>) -> Result<Self, ModelError> {
        if let Some(built_in) = Self::built_in(name_or_path) {
            return Ok(built_in);
        }

        let path = match base_dir {
            Some(base) => base.join(name_or_path),
            None => PathBuf::from(name_or_path),
        };

        if !path.exists() {
            return Err(ModelError::ThemeNotFound {
                name: name_or_path.to_string(),
                path,
            });
        }

        let content =
            std::fs::read_to_string(&path).map_err(|source| ModelError::ThemeIoError {
                path: path.clone(),
                source,
            })?;

        let theme: Theme =
            serde_json::from_str(&content).map_err(|source| ModelError::ThemeParseError {
                path: path.clone(),
                source,
            })?;

        theme.validate()?;
        Ok(theme)
    }

    /// Looks up a statically embedded built-in theme by name.
    pub fn built_in(name: &str) -> Option<Self> {
        match name {
            Self::DEFAULT_NAME => Some(Self::corporate_navy()),
            "classic_green" => Some(Self::classic_green()),
            "modern_slate" => Some(Self::modern_slate()),
            "minimal_monochrome" => Some(Self::minimal_monochrome()),
            _ => None,
        }
    }

    /// Institutional private equity and investment banking theme (default).
    pub fn corporate_navy() -> Self {
        Self {
            schema: None,
            name: Self::DEFAULT_NAME.to_string(),
            typography: TypographyConfig::default(),
            metrics: MetricsConfig::default(),
            palette: PaletteConfig {
                table_title_fill: "#1B365D".to_string(),
                table_title_text: "#FFFFFF".to_string(),
                table_subtitle_text: "#4B5563".to_string(),
                super_header_fill: "#2C4D75".to_string(),
                super_header_text: "#FFFFFF".to_string(),
                col_header_fill: "#F0F4F8".to_string(),
                col_header_text: "#1B365D".to_string(),
                section_header_fill: "#F7F9FC".to_string(),
                section_header_text: "#1B365D".to_string(),
                data_fill: "#FFFFFF".to_string(),
                data_text: "#1F2937".to_string(),
                subtotal_fill: "#F7F9FC".to_string(),
                subtotal_text: "#1B365D".to_string(),
                total_fill: "#F0F4F8".to_string(),
                total_text: "#1B365D".to_string(),
                input_text: "#002060".to_string(),
                formula_text: "#000000".to_string(),
                zebra_fill: "#FAFBFC".to_string(),
                highlight_fill: "#FEF3C7".to_string(),
                highlight_text: "#92400E".to_string(),
                gridline_color: "#E5E7EB".to_string(),
                border_color: "#D1D5DB".to_string(),
                total_border_color: "#1B365D".to_string(),
            },
            borders: BordersConfig::default(),
            number_formats: NumberFormatsConfig::default(),
        }
    }

    /// Traditional financial modeling and corporate finance theme.
    pub fn classic_green() -> Self {
        Self {
            schema: None,
            name: "classic_green".to_string(),
            typography: TypographyConfig::default(),
            metrics: MetricsConfig::default(),
            palette: PaletteConfig {
                table_title_fill: "#1B4D3E".to_string(),
                table_title_text: "#FFFFFF".to_string(),
                table_subtitle_text: "#4B5563".to_string(),
                super_header_fill: "#2E6B56".to_string(),
                super_header_text: "#FFFFFF".to_string(),
                col_header_fill: "#F0F7F4".to_string(),
                col_header_text: "#1B4D3E".to_string(),
                section_header_fill: "#F5FAF7".to_string(),
                section_header_text: "#1B4D3E".to_string(),
                data_fill: "#FFFFFF".to_string(),
                data_text: "#1F2937".to_string(),
                subtotal_fill: "#F5FAF7".to_string(),
                subtotal_text: "#1B4D3E".to_string(),
                total_fill: "#F0F7F4".to_string(),
                total_text: "#1B4D3E".to_string(),
                input_text: "#002060".to_string(),
                formula_text: "#000000".to_string(),
                zebra_fill: "#F9FCFA".to_string(),
                highlight_fill: "#FEF3C7".to_string(),
                highlight_text: "#92400E".to_string(),
                gridline_color: "#E5E7EB".to_string(),
                border_color: "#D1D5DB".to_string(),
                total_border_color: "#1B4D3E".to_string(),
            },
            borders: BordersConfig::default(),
            number_formats: NumberFormatsConfig::default(),
        }
    }

    /// Modern tech, startup, and venture capital slate theme.
    pub fn modern_slate() -> Self {
        Self {
            schema: None,
            name: "modern_slate".to_string(),
            typography: TypographyConfig::default(),
            metrics: MetricsConfig::default(),
            palette: PaletteConfig {
                table_title_fill: "#334155".to_string(),
                table_title_text: "#FFFFFF".to_string(),
                table_subtitle_text: "#64748B".to_string(),
                super_header_fill: "#475569".to_string(),
                super_header_text: "#FFFFFF".to_string(),
                col_header_fill: "#F1F5F9".to_string(),
                col_header_text: "#0F172A".to_string(),
                section_header_fill: "#F8FAFC".to_string(),
                section_header_text: "#0F172A".to_string(),
                data_fill: "#FFFFFF".to_string(),
                data_text: "#0F172A".to_string(),
                subtotal_fill: "#F8FAFC".to_string(),
                subtotal_text: "#0F172A".to_string(),
                total_fill: "#F1F5F9".to_string(),
                total_text: "#0F172A".to_string(),
                input_text: "#002060".to_string(),
                formula_text: "#000000".to_string(),
                zebra_fill: "#F8FAFC".to_string(),
                highlight_fill: "#FEF3C7".to_string(),
                highlight_text: "#92400E".to_string(),
                gridline_color: "#E2E8F0".to_string(),
                border_color: "#CBD5E1".to_string(),
                total_border_color: "#334155".to_string(),
            },
            borders: BordersConfig::default(),
            number_formats: NumberFormatsConfig::default(),
        }
    }

    /// High-contrast black & white audit and print-ready theme.
    pub fn minimal_monochrome() -> Self {
        Self {
            schema: None,
            name: "minimal_monochrome".to_string(),
            typography: TypographyConfig::default(),
            metrics: MetricsConfig::default(),
            palette: PaletteConfig {
                table_title_fill: "#000000".to_string(),
                table_title_text: "#FFFFFF".to_string(),
                table_subtitle_text: "#666666".to_string(),
                super_header_fill: "#333333".to_string(),
                super_header_text: "#FFFFFF".to_string(),
                col_header_fill: "#FFFFFF".to_string(),
                col_header_text: "#000000".to_string(),
                section_header_fill: "#FFFFFF".to_string(),
                section_header_text: "#000000".to_string(),
                data_fill: "#FFFFFF".to_string(),
                data_text: "#000000".to_string(),
                subtotal_fill: "#FFFFFF".to_string(),
                subtotal_text: "#000000".to_string(),
                total_fill: "#FFFFFF".to_string(),
                total_text: "#000000".to_string(),
                input_text: "#000000".to_string(),
                formula_text: "#000000".to_string(),
                zebra_fill: "#FFFFFF".to_string(),
                highlight_fill: "#EEEEEE".to_string(),
                highlight_text: "#000000".to_string(),
                gridline_color: "#D0D0D0".to_string(),
                border_color: "#000000".to_string(),
                total_border_color: "#000000".to_string(),
            },
            borders: BordersConfig::default(),
            number_formats: NumberFormatsConfig::default(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::corporate_navy()
    }
}
