//! Theme-to-OpenXML format synthesis, color translation, and style deduplication engine.

use indexmap::IndexMap;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder};

use super::error::EmissionError;
use crate::compiler::layout::StyleRole;
use crate::model::format::{CurrencyCode, FormatPreset};
use crate::model::theme::{BorderStyle, PaletteConfig, Theme};

/// Parses a hexadecimal color string (`#RGB`, `#RRGGBB`, or `#RRGGBBAA`) into a `rust_xlsxwriter::Color`.
pub fn parse_hex_color(field: &str, s: &str) -> Result<Color, EmissionError> {
    let invalid = || EmissionError::InvalidColor {
        field: field.to_string(),
        color: s.to_string(),
    };

    let hex_part = s.strip_prefix('#').ok_or_else(invalid)?;
    if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }

    let rgb_u32 = match hex_part.len() {
        3 => {
            let mut chars = hex_part.chars();
            let r = chars
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(invalid)?;
            let g = chars
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(invalid)?;
            let b = chars
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(invalid)?;
            let r = (r << 4) | r;
            let g = (g << 4) | g;
            let b = (b << 4) | b;
            (r << 16) | (g << 8) | b
        }
        6 => u32::from_str_radix(hex_part, 16).map_err(|_| invalid())?,
        8 => u32::from_str_radix(&hex_part[..6], 16).map_err(|_| invalid())?,
        _ => return Err(invalid()),
    };

    Ok(Color::RGB(rgb_u32))
}

/// Parses a fill color string into `Option<rust_xlsxwriter::Color>`.
///
/// Accepts `"none"` and `"transparent"` (case-insensitive) as transparent (`Ok(None)`),
/// or standard hexadecimal colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`) as `Ok(Some(Color))`.
pub fn parse_fill_color(field: &str, s: &str) -> Result<Option<Color>, EmissionError> {
    if s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        Ok(None)
    } else {
        parse_hex_color(field, s).map(Some)
    }
}

/// Translates Finx model border style into `rust_xlsxwriter::FormatBorder`.
#[must_use]
pub const fn translate_border_style(style: BorderStyle) -> FormatBorder {
    match style {
        BorderStyle::None => FormatBorder::None,
        BorderStyle::Thin => FormatBorder::Thin,
        BorderStyle::Medium => FormatBorder::Medium,
        BorderStyle::Thick => FormatBorder::Thick,
        BorderStyle::Double => FormatBorder::Double,
        BorderStyle::Dashed => FormatBorder::Dashed,
        BorderStyle::Dotted => FormatBorder::Dotted,
        BorderStyle::Hair => FormatBorder::Hair,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedPalette {
    table_title_fill: Option<Color>,
    table_title_text: Color,
    table_subtitle_text: Color,
    super_header_fill: Option<Color>,
    super_header_text: Color,
    col_header_fill: Option<Color>,
    col_header_text: Color,
    section_header_fill: Option<Color>,
    section_header_text: Color,
    data_fill: Option<Color>,
    data_text: Color,
    subtotal_fill: Option<Color>,
    subtotal_text: Color,
    total_fill: Option<Color>,
    total_text: Color,
    input_text: Color,
    formula_text: Color,
    zebra_fill: Option<Color>,
    highlight_fill: Option<Color>,
    highlight_text: Color,
    gridline_color: Color,
    border_color: Color,
    total_border_color: Color,
}

impl ParsedPalette {
    fn from_config(p: &PaletteConfig) -> Result<Self, EmissionError> {
        Ok(Self {
            table_title_fill: parse_fill_color("table_title_fill", &p.table_title_fill)?,
            table_title_text: parse_hex_color("table_title_text", &p.table_title_text)?,
            table_subtitle_text: parse_hex_color("table_subtitle_text", &p.table_subtitle_text)?,
            super_header_fill: parse_fill_color("super_header_fill", &p.super_header_fill)?,
            super_header_text: parse_hex_color("super_header_text", &p.super_header_text)?,
            col_header_fill: parse_fill_color("col_header_fill", &p.col_header_fill)?,
            col_header_text: parse_hex_color("col_header_text", &p.col_header_text)?,
            section_header_fill: parse_fill_color("section_header_fill", &p.section_header_fill)?,
            section_header_text: parse_hex_color("section_header_text", &p.section_header_text)?,
            data_fill: parse_fill_color("data_fill", &p.data_fill)?,
            data_text: parse_hex_color("data_text", &p.data_text)?,
            subtotal_fill: parse_fill_color("subtotal_fill", &p.subtotal_fill)?,
            subtotal_text: parse_hex_color("subtotal_text", &p.subtotal_text)?,
            total_fill: parse_fill_color("total_fill", &p.total_fill)?,
            total_text: parse_hex_color("total_text", &p.total_text)?,
            input_text: parse_hex_color("input_text", &p.input_text)?,
            formula_text: parse_hex_color("formula_text", &p.formula_text)?,
            zebra_fill: parse_fill_color("zebra_fill", &p.zebra_fill)?,
            highlight_fill: parse_fill_color("highlight_fill", &p.highlight_fill)?,
            highlight_text: parse_hex_color("highlight_text", &p.highlight_text)?,
            gridline_color: parse_hex_color("gridline_color", &p.gridline_color)?,
            border_color: parse_hex_color("border_color", &p.border_color)?,
            total_border_color: parse_hex_color("total_border_color", &p.total_border_color)?,
        })
    }
}

/// Semantic attributes defining a cell's presentation and style signature.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CellFormatKey {
    /// Semantic summary and highlight role.
    pub role: StyleRole,
    /// Optional number formatting preset.
    pub preset: Option<FormatPreset>,
    /// Optional decimal precision override.
    pub decimals: Option<u32>,
    /// Optional currency code.
    pub currency: Option<CurrencyCode>,
    /// Whether the cell value originates from a hardcoded input literal.
    pub is_input: bool,
    /// Whether the cell resides in a descriptor column/axis.
    pub is_descriptor: bool,
}

impl CellFormatKey {
    /// Creates a new cell format key.
    #[must_use]
    pub const fn new(
        role: StyleRole,
        preset: Option<FormatPreset>,
        decimals: Option<u32>,
        currency: Option<CurrencyCode>,
        is_input: bool,
        is_descriptor: bool,
    ) -> Self {
        Self {
            role,
            preset,
            decimals,
            currency,
            is_input,
            is_descriptor,
        }
    }
}

/// Format synthesizer and deduplication cache mapping semantic cell keys and structural headers
/// to OpenXML `rust_xlsxwriter::Format` instances.
#[derive(Debug, Clone)]
pub struct FormatCache {
    theme: Theme,
    palette: ParsedPalette,
    title_format: Format,
    subtitle_format: Format,
    super_header_format: Format,
    col_header_descriptor_format: Format,
    col_header_measure_format: Format,
    section_header_format: Format,
    cache: IndexMap<CellFormatKey, Format>,
}

impl FormatCache {
    /// Initializes a new format cache for the given theme, validating and pre-parsing all palette colors.
    pub fn new(theme: &Theme) -> Result<Self, EmissionError> {
        let palette = ParsedPalette::from_config(&theme.palette)?;

        // 1. Table Title
        let mut title_format = Format::new()
            .set_font_name(&theme.typography.font_family)
            .set_font_size(theme.typography.title_size)
            .set_bold()
            .set_font_color(palette.table_title_text)
            .set_align(FormatAlign::Left);
        if let Some(fill) = palette.table_title_fill {
            title_format = title_format.set_background_color(fill);
        }
        if theme.borders.table_title_border_bottom != BorderStyle::None {
            title_format = title_format
                .set_border_bottom(translate_border_style(
                    theme.borders.table_title_border_bottom,
                ))
                .set_border_bottom_color(palette.border_color);
        }

        // 2. Table Subtitle
        let subtitle_format = Format::new()
            .set_font_name(&theme.typography.font_family)
            .set_font_size(theme.typography.subtitle_size)
            .set_font_color(palette.table_subtitle_text)
            .set_align(FormatAlign::Left);

        // 3. Super-Header
        let mut super_header_format = Format::new()
            .set_font_name(&theme.typography.font_family)
            .set_font_size(theme.typography.super_header_size)
            .set_bold()
            .set_font_color(palette.super_header_text)
            .set_align(FormatAlign::CenterAcross);
        if let Some(fill) = palette.super_header_fill {
            super_header_format = super_header_format.set_background_color(fill);
        }
        if theme.borders.super_header_border_bottom != BorderStyle::None {
            super_header_format = super_header_format
                .set_border_bottom(translate_border_style(
                    theme.borders.super_header_border_bottom,
                ))
                .set_border_bottom_color(palette.border_color);
        }

        // 4. Column Header (Descriptor - Left Aligned)
        let mut col_header_descriptor_format = Format::new()
            .set_font_name(&theme.typography.font_family)
            .set_font_size(theme.typography.col_header_size)
            .set_bold()
            .set_font_color(palette.col_header_text)
            .set_align(FormatAlign::Left);
        if let Some(fill) = palette.col_header_fill {
            col_header_descriptor_format = col_header_descriptor_format.set_background_color(fill);
        }
        if theme.borders.header_border_bottom != BorderStyle::None {
            col_header_descriptor_format = col_header_descriptor_format
                .set_border_bottom(translate_border_style(theme.borders.header_border_bottom))
                .set_border_bottom_color(palette.border_color);
        }

        // 5. Column Header (Measure - Right Aligned)
        let mut col_header_measure_format = Format::new()
            .set_font_name(&theme.typography.font_family)
            .set_font_size(theme.typography.col_header_size)
            .set_bold()
            .set_font_color(palette.col_header_text)
            .set_align(FormatAlign::Right);
        if let Some(fill) = palette.col_header_fill {
            col_header_measure_format = col_header_measure_format.set_background_color(fill);
        }
        if theme.borders.header_border_bottom != BorderStyle::None {
            col_header_measure_format = col_header_measure_format
                .set_border_bottom(translate_border_style(theme.borders.header_border_bottom))
                .set_border_bottom_color(palette.border_color);
        }

        // 6. Section Header
        let mut section_header_format = Format::new()
            .set_font_name(&theme.typography.font_family)
            .set_font_size(theme.typography.section_header_size)
            .set_bold()
            .set_font_color(palette.section_header_text)
            .set_align(FormatAlign::Left);
        if let Some(fill) = palette.section_header_fill {
            section_header_format = section_header_format.set_background_color(fill);
        }

        Ok(Self {
            theme: theme.clone(),
            palette,
            title_format,
            subtitle_format,
            super_header_format,
            col_header_descriptor_format,
            col_header_measure_format,
            section_header_format,
            cache: IndexMap::new(),
        })
    }

    /// Returns the cached format for the given cell format key, or synthesizes, caches, and returns it.
    pub fn get_cell_format(&mut self, key: &CellFormatKey) -> Result<Format, EmissionError> {
        if let Some(format) = self.cache.get(key) {
            return Ok(format.clone());
        }

        let mut format = Format::new()
            .set_font_name(&self.theme.typography.font_family)
            .set_font_size(self.theme.typography.data_size);

        // 1. Bolding (Total and Subtotal are bold)
        let is_summary = key.role.horizontal.is_total()
            || key.role.vertical.is_total()
            || key.role.horizontal.is_subtotal()
            || key.role.vertical.is_subtotal();
        if is_summary {
            format = format.set_bold();
        }

        // 2. Alignment (Descriptor Left, Measure Right)
        format = if key.is_descriptor {
            format.set_align(FormatAlign::Left)
        } else {
            format.set_align(FormatAlign::Right)
        };

        // 3. Background Fill Precedence:
        // is_highlight > total > subtotal > data
        let fill = if key.role.is_highlight {
            self.palette.highlight_fill
        } else if key.role.horizontal.is_total() || key.role.vertical.is_total() {
            self.palette.total_fill
        } else if key.role.horizontal.is_subtotal() || key.role.vertical.is_subtotal() {
            self.palette.subtotal_fill
        } else {
            self.palette.data_fill
        };
        if let Some(fill_color) = fill {
            format = format.set_background_color(fill_color);
        }

        // 4. Text Color Precedence:
        // Input measure literal overrides everything (highlight, total, subtotal, formula/data)
        // Then: is_highlight > total > subtotal > (descriptor ? data : formula)
        format = if key.is_input && !key.is_descriptor {
            format.set_font_color(self.palette.input_text)
        } else if key.role.is_highlight {
            format.set_font_color(self.palette.highlight_text)
        } else if key.role.horizontal.is_total() || key.role.vertical.is_total() {
            format.set_font_color(self.palette.total_text)
        } else if key.role.horizontal.is_subtotal() || key.role.vertical.is_subtotal() {
            format.set_font_color(self.palette.subtotal_text)
        } else if key.is_descriptor {
            format.set_font_color(self.palette.data_text)
        } else {
            format.set_font_color(self.palette.formula_text)
        };

        // 5. Accounting Borders
        // Horizontal: Subtotal -> Thin top border; Total -> Thin top + Double bottom
        if key.role.horizontal.is_subtotal() {
            if self.theme.borders.subtotal_border_top != BorderStyle::None {
                format = format
                    .set_border_top(translate_border_style(
                        self.theme.borders.subtotal_border_top,
                    ))
                    .set_border_top_color(self.palette.border_color);
            }
        } else if key.role.horizontal.is_total() {
            if self.theme.borders.total_border_top != BorderStyle::None {
                format = format
                    .set_border_top(translate_border_style(self.theme.borders.total_border_top))
                    .set_border_top_color(self.palette.total_border_color);
            }
            if self.theme.borders.total_border_bottom != BorderStyle::None {
                format = format
                    .set_border_bottom(translate_border_style(
                        self.theme.borders.total_border_bottom,
                    ))
                    .set_border_bottom_color(self.palette.total_border_color);
            }
        }

        // Vertical: Subtotal -> Left border; Total -> Left + Right borders
        if key.role.vertical.is_subtotal() {
            if self.theme.borders.col_subtotal_border_left != BorderStyle::None {
                format = format
                    .set_border_left(translate_border_style(
                        self.theme.borders.col_subtotal_border_left,
                    ))
                    .set_border_left_color(self.palette.border_color);
            }
        } else if key.role.vertical.is_total() {
            if self.theme.borders.col_total_border_left != BorderStyle::None {
                format = format
                    .set_border_left(translate_border_style(
                        self.theme.borders.col_total_border_left,
                    ))
                    .set_border_left_color(self.palette.total_border_color);
            }
            if self.theme.borders.col_total_border_right != BorderStyle::None {
                format = format
                    .set_border_right(translate_border_style(
                        self.theme.borders.col_total_border_right,
                    ))
                    .set_border_right_color(self.palette.total_border_color);
            }
        }

        // Descriptor right border (if configured in theme)
        if key.is_descriptor
            && self.theme.borders.descriptor_border_right != BorderStyle::None
            && !key.role.vertical.is_total()
        {
            format = format
                .set_border_right(translate_border_style(
                    self.theme.borders.descriptor_border_right,
                ))
                .set_border_right_color(self.palette.border_color);
        }

        // 6. Number Format Mask
        if let Some(preset) = key.preset {
            let mask = self
                .theme
                .resolve_number_format(preset, key.decimals, key.currency)?;
            format = format.set_num_format(&mask);
        }

        // 7. Cache insertion
        self.cache.insert(key.clone(), format.clone());
        Ok(format)
    }

    /// Returns the table title format.
    #[must_use]
    pub fn title_format(&self) -> Format {
        self.title_format.clone()
    }

    /// Returns the table subtitle format.
    #[must_use]
    pub fn subtitle_format(&self) -> Format {
        self.subtitle_format.clone()
    }

    /// Returns the super-header format.
    #[must_use]
    pub fn super_header_format(&self) -> Format {
        self.super_header_format.clone()
    }

    /// Returns the column header format (descriptor or measure).
    #[must_use]
    pub fn col_header_format(&self, is_descriptor: bool) -> Format {
        if is_descriptor {
            self.col_header_descriptor_format.clone()
        } else {
            self.col_header_measure_format.clone()
        }
    }

    /// Returns the section header format.
    #[must_use]
    pub fn section_header_format(&self) -> Format {
        self.section_header_format.clone()
    }

    /// Returns the number of cached data cell formats.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns true if the cache contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Returns a reference to the active theme.
    #[must_use]
    pub const fn theme(&self) -> &Theme {
        &self.theme
    }
}
