use serde::{Deserialize, Serialize};

use super::error::ModelError;
use super::format::{CurrencyCode, FormatPreset};

/// Theme specification inside a project manifest: either a built-in theme name or an inline custom theme.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ThemeSpec {
    Name(String),
    Custom(Box<Theme>),
}

impl Default for ThemeSpec {
    fn default() -> Self {
        Self::Name(Theme::DEFAULT_NAME.to_string())
    }
}

impl<'de> Deserialize<'de> for ThemeSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ThemeSpecVisitor;

        impl<'de> serde::de::Visitor<'de> for ThemeSpecVisitor {
            type Value = ThemeSpec;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a built-in theme name string or a custom theme map")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(ThemeSpec::Name(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(ThemeSpec::Name(v))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let theme = Theme::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(ThemeSpec::Custom(Box::new(theme)))
            }
        }

        deserializer.deserialize_any(ThemeSpecVisitor)
    }
}

/// Complete visual styling, typography, dimensions, palette, borders, and number format specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

fn default_section_header_underline() -> bool {
    true
}

/// Font family and point sizes for workbook hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
            title_size: 12.0,
            subtitle_size: 9.0,
            super_header_size: 10.0,
            col_header_size: 10.0,
            section_header_size: 10.0,
            data_size: 10.0,
            section_header_underline: true,
        }
    }
}

fn default_max_col_width() -> f64 {
    75.0
}

/// Column character widths, gutters, and margins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    pub gutter_col_width: f64,
    pub char_width_multiplier: f64,
    pub col_padding: f64,
    pub min_col_width: f64,
    #[serde(default = "default_max_col_width")]
    pub max_col_width: f64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            gutter_col_width: 2.0,
            char_width_multiplier: 1.0,
            col_padding: 0.0,
            min_col_width: 12.0,
            max_col_width: default_max_col_width(),
        }
    }
}

/// Color assignments across all workbook components.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    pub shade_muted_fill: String,
    pub shade_alt_fill: String,
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
#[serde(deny_unknown_fields)]
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
            super_header_border_bottom: BorderStyle::None,
            header_border_bottom: BorderStyle::Thin,
            descriptor_border_right: BorderStyle::None,
            subtotal_border_top: BorderStyle::Thin,
            total_border_top: BorderStyle::Thin,
            total_border_bottom: BorderStyle::Double,
            col_subtotal_border_left: BorderStyle::Dashed,
            col_total_border_left: BorderStyle::Thin,
            col_total_border_right: BorderStyle::None,
        }
    }
}

fn default_integer_format() -> String {
    "0_);(-0);\"-\"_);@_)".to_string()
}

fn default_general_format() -> String {
    "General_);(General);0_);@_)".to_string()
}

/// Number format templates and default precisions across all semantic presets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumberFormatsConfig {
    pub currency: String,
    pub number: String,
    pub percentage: String,
    pub multiple: String,
    pub basis_points: String,
    pub date: String,
    pub text: String,
    #[serde(default = "default_integer_format")]
    pub integer: String,
    #[serde(default = "default_general_format")]
    pub general: String,
    pub defaults: FormatDefaults,
}

impl Default for NumberFormatsConfig {
    fn default() -> Self {
        Self {
            currency: "{sym}#,##0{dec}_);({sym}#,##0{dec});\"-\"_);@_)".to_string(),
            number: "#,##0{dec}_);(#,##0{dec});\"-\"_);@_)".to_string(),
            percentage: "0{dec}%_);(0{dec}%);\"-\"_);@_)".to_string(),
            multiple: "0{dec}\"x\"_);(0{dec}\"x\");\"-\"_);@_)".to_string(),
            basis_points: "#,##0{dec}\" bps\"_);(#,##0{dec}\" bps\");\"-\"_);@_)".to_string(),
            date: "yyyy-mm-dd_)".to_string(),
            text: "@_)".to_string(),
            integer: default_integer_format(),
            general: default_general_format(),
            defaults: FormatDefaults::default(),
        }
    }
}

/// Default fractional decimal precision per numeric preset family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

pub(crate) fn parse_hex_rgb(s: &str) -> Option<u32> {
    let hex_part = s.strip_prefix('#')?;
    if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    match hex_part.len() {
        3 => {
            let mut chars = hex_part.chars();
            let r = chars.next()?.to_digit(16)?;
            let g = chars.next()?.to_digit(16)?;
            let b = chars.next()?.to_digit(16)?;
            let r = (r << 4) | r;
            let g = (g << 4) | g;
            let b = (b << 4) | b;
            Some((r << 16) | (g << 8) | b)
        }
        6 => u32::from_str_radix(hex_part, 16).ok(),
        8 => u32::from_str_radix(&hex_part[..6], 16).ok(),
        _ => None,
    }
}

pub(crate) fn is_valid_hex_color(s: &str) -> bool {
    parse_hex_rgb(s).is_some()
}

fn is_valid_fill_color(s: &str) -> bool {
    s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") || is_valid_hex_color(s)
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

fn validate_non_negative_metric(metric: &str, value: f64) -> Result<(), ModelError> {
    if !value.is_finite() || value < 0.0 {
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
        validate_metric("gutter_col_width", self.gutter_col_width)?;
        validate_metric("char_width_multiplier", self.char_width_multiplier)?;
        validate_non_negative_metric("col_padding", self.col_padding)?;
        validate_metric("min_col_width", self.min_col_width)?;
        validate_metric("max_col_width", self.max_col_width)?;
        if self.max_col_width < self.min_col_width {
            return Err(ModelError::InvalidColWidthBounds {
                min_col_width: self.min_col_width,
                max_col_width: self.max_col_width,
            });
        }
        Ok(())
    }
}

impl PaletteConfig {
    pub fn validate(&self) -> Result<(), ModelError> {
        let fill_fields = [
            ("table_title_fill", &self.table_title_fill),
            ("super_header_fill", &self.super_header_fill),
            ("col_header_fill", &self.col_header_fill),
            ("section_header_fill", &self.section_header_fill),
            ("data_fill", &self.data_fill),
            ("subtotal_fill", &self.subtotal_fill),
            ("total_fill", &self.total_fill),
            ("shade_muted_fill", &self.shade_muted_fill),
            ("shade_alt_fill", &self.shade_alt_fill),
            ("highlight_fill", &self.highlight_fill),
        ];

        for (field, color) in fill_fields {
            if !is_valid_fill_color(color) {
                return Err(ModelError::InvalidThemeColor {
                    field: field.to_string(),
                    color: color.clone(),
                });
            }
        }

        let color_fields = [
            ("table_title_text", &self.table_title_text),
            ("table_subtitle_text", &self.table_subtitle_text),
            ("super_header_text", &self.super_header_text),
            ("col_header_text", &self.col_header_text),
            ("section_header_text", &self.section_header_text),
            ("data_text", &self.data_text),
            ("subtotal_text", &self.subtotal_text),
            ("total_text", &self.total_text),
            ("input_text", &self.input_text),
            ("formula_text", &self.formula_text),
            ("highlight_text", &self.highlight_text),
            ("gridline_color", &self.gridline_color),
            ("border_color", &self.border_color),
            ("total_border_color", &self.total_border_color),
        ];

        for (field, color) in color_fields {
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
    /// Returns the configured default fractional precision for a numeric format preset.
    #[must_use]
    pub const fn default_decimals(&self, preset: FormatPreset) -> Option<u32> {
        match preset {
            FormatPreset::Currency => Some(self.defaults.currency_decimals),
            FormatPreset::Number => Some(self.defaults.number_decimals),
            FormatPreset::Percentage => Some(self.defaults.percentage_decimals),
            FormatPreset::Multiple => Some(self.defaults.multiple_decimals),
            FormatPreset::BasisPoints => Some(self.defaults.basis_points_decimals),
            FormatPreset::Date | FormatPreset::Text | FormatPreset::Integer => None,
        }
    }

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

        for (preset, _) in numeric_templates {
            if let Some(decimals) = self.default_decimals(preset)
                && decimals > 14
            {
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
    /// - For presets without dynamic precision (`date`, `text`, `integer`), the static template is returned.
    pub fn resolve_number_format(
        &self,
        preset: FormatPreset,
        decimals: Option<u32>,
        currency: Option<CurrencyCode>,
    ) -> Result<String, ModelError> {
        let template = match preset {
            FormatPreset::Date => return Ok(self.number_formats.date.clone()),
            FormatPreset::Text => return Ok(self.number_formats.text.clone()),
            FormatPreset::Integer => return Ok(self.number_formats.integer.clone()),
            FormatPreset::Currency => &self.number_formats.currency,
            FormatPreset::Number => &self.number_formats.number,
            FormatPreset::Percentage => &self.number_formats.percentage,
            FormatPreset::Multiple => &self.number_formats.multiple,
            FormatPreset::BasisPoints => &self.number_formats.basis_points,
        };

        let dec = decimals
            .or_else(|| self.number_formats.default_decimals(preset))
            .unwrap_or(0);
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

    /// Resolves a built-in theme by name ("corporate_navy").
    ///
    /// # Errors
    /// Returns [`ModelError::ThemeNotFound`] if the theme name does not match any built-in theme.
    pub fn resolve_builtin(name: &str) -> Result<Self, ModelError> {
        match name {
            Self::DEFAULT_NAME => Ok(Self::corporate_navy()),
            _ => Err(ModelError::ThemeNotFound {
                name: name.to_string(),
            }),
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
                table_title_fill: "#00356B".to_string(),
                table_title_text: "#FFFFFF".to_string(),
                table_subtitle_text: "#525252".to_string(),
                super_header_fill: "#E5E5E5".to_string(),
                super_header_text: "#000000".to_string(),
                col_header_fill: "none".to_string(),
                col_header_text: "#000000".to_string(),
                section_header_fill: "none".to_string(),
                section_header_text: "#000000".to_string(),
                data_fill: "none".to_string(),
                data_text: "#000000".to_string(),
                subtotal_fill: "none".to_string(),
                subtotal_text: "#000000".to_string(),
                total_fill: "none".to_string(),
                total_text: "#000000".to_string(),
                input_text: "#0000FF".to_string(),
                formula_text: "#000000".to_string(),
                shade_muted_fill: "#FAFAFA".to_string(),
                shade_alt_fill: "#F5F5F5".to_string(),
                highlight_fill: "#FEF3C7".to_string(),
                highlight_text: "#000000".to_string(),
                gridline_color: "#E5E5E5".to_string(),
                border_color: "#000000".to_string(),
                total_border_color: "#00356B".to_string(),
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
