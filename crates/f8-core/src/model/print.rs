use std::fmt;

use serde::{Deserialize, Serialize};

use super::error::ModelError;
use super::theme::{Theme, parse_hex_rgb};

/// Fixed institutional left page margin in inches.
pub const PRINT_MARGIN_LEFT: f64 = 0.4;

/// Fixed institutional right page margin in inches.
pub const PRINT_MARGIN_RIGHT: f64 = 0.4;

/// Fixed institutional top page margin in inches.
pub const PRINT_MARGIN_TOP: f64 = 0.6;

/// Fixed institutional bottom page margin in inches.
pub const PRINT_MARGIN_BOTTOM: f64 = 0.6;

/// Fixed institutional header margin in inches.
pub const PRINT_MARGIN_HEADER: f64 = 0.25;

/// Fixed institutional footer margin in inches.
pub const PRINT_MARGIN_FOOTER: f64 = 0.25;

/// Font size in points applied to compiled print headers and footers.
pub const PRINT_HEADER_FOOTER_FONT_SIZE: u8 = 8;

/// Maximum UTF-16 code unit length allowed by Excel for a compiled header or footer string.
pub const MAX_EXCEL_HEADER_FOOTER_UTF16: usize = 255;

/// Maximum number of manual horizontal page breaks allowed by Excel on a single worksheet.
pub const MAX_EXCEL_PAGE_BREAKS: usize = 1023;

/// Raw `rust_xlsxwriter` header/footer macro tokens that are rewritten inside `set_header` / `set_footer`.
const FORBIDDEN_XLSXWRITER_MACROS: [&str; 8] = [
    "&[Tab]",
    "&[Date]",
    "&[File]",
    "&[Page]",
    "&[Path]",
    "&[Time]",
    "&[Pages]",
    "&[Picture]",
];

const MM_PER_INCH: f64 = 25.4;
const A4_SHORT_MM: f64 = 210.0;
const A4_LONG_MM: f64 = 297.0;
const LETTER_SHORT_IN: f64 = 8.5;
const LETTER_LONG_IN: f64 = 11.0;

const fn default_true() -> bool {
    true
}

/// Physical paper size for worksheet printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperSize {
    A4,
    Letter,
}

impl PaperSize {
    /// Returns the OpenXML `pageSetup/@paperSize` numeric code.
    #[must_use]
    pub const fn excel_code(self) -> u8 {
        match self {
            Self::A4 => 9,
            Self::Letter => 1,
        }
    }

    /// Returns the canonical snake_case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A4 => "a4",
            Self::Letter => "letter",
        }
    }

    /// Returns `(printable_width_in, printable_height_in)` after subtracting fixed institutional margins.
    #[must_use]
    pub fn printable_size_inches(self, orientation: PageOrientation) -> (f64, f64) {
        let (short_in, long_in) = match self {
            Self::A4 => (A4_SHORT_MM / MM_PER_INCH, A4_LONG_MM / MM_PER_INCH),
            Self::Letter => (LETTER_SHORT_IN, LETTER_LONG_IN),
        };
        let (page_w, page_h) = match orientation {
            PageOrientation::Portrait => (short_in, long_in),
            PageOrientation::Landscape => (long_in, short_in),
        };
        (
            page_w - (PRINT_MARGIN_LEFT + PRINT_MARGIN_RIGHT),
            page_h - (PRINT_MARGIN_TOP + PRINT_MARGIN_BOTTOM),
        )
    }
}

impl fmt::Display for PaperSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Printed page orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageOrientation {
    #[default]
    Landscape,
    Portrait,
}

impl PageOrientation {
    /// Returns the canonical snake_case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Landscape => "landscape",
            Self::Portrait => "portrait",
        }
    }
}

impl fmt::Display for PageOrientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Fit-to-page scaling policy for worksheet printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrintFitMode {
    #[default]
    FitWidth,
    FitPage,
}

impl PrintFitMode {
    /// Returns the canonical snake_case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FitWidth => "fit_width",
            Self::FitPage => "fit_page",
        }
    }
}

impl fmt::Display for PrintFitMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Print header or footer section discriminator for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeaderFooterSection {
    Header,
    Footer,
}

impl HeaderFooterSection {
    /// Returns the canonical lowercase identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Footer => "footer",
        }
    }
}

impl fmt::Display for HeaderFooterSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Horizontal zone within a print header or footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeaderFooterZone {
    Left,
    Center,
    Right,
}

impl HeaderFooterZone {
    /// Returns the canonical lowercase identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }

    const fn excel_code(self) -> &'static str {
        match self {
            Self::Left => "&L",
            Self::Center => "&C",
            Self::Right => "&R",
        }
    }
}

impl fmt::Display for HeaderFooterZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Structured 3-zone worksheet header or footer specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderFooterSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,
}

impl HeaderFooterSpec {
    /// Returns true if all three zones are unset.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.left.is_none() && self.center.is_none() && self.right.is_none()
    }
}

/// Canonical opt-in worksheet print configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetPrintSpec {
    pub paper_size: PaperSize,
    #[serde(default)]
    pub orientation: PageOrientation,
    #[serde(default)]
    pub fit: PrintFitMode,
    #[serde(default = "default_true")]
    pub center_horizontally: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<HeaderFooterSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<HeaderFooterSpec>,
}

impl SheetPrintSpec {
    /// Creates a worksheet print configuration with canonical defaults for the given paper size.
    #[must_use]
    pub const fn new(paper_size: PaperSize) -> Self {
        Self {
            paper_size,
            orientation: PageOrientation::Landscape,
            fit: PrintFitMode::FitWidth,
            center_horizontally: true,
            header: None,
            footer: None,
        }
    }
}

fn expand_header_footer_zone(
    sheet: &str,
    section: HeaderFooterSection,
    zone: HeaderFooterZone,
    template: &str,
    model_name: &str,
) -> Result<String, ModelError> {
    let invalid = |reason: String| ModelError::InvalidPrintHeaderFooterZone {
        sheet: sheet.to_string(),
        section,
        zone,
        reason,
    };

    let mut expanded = String::with_capacity(template.len());
    let mut has_dynamic_token = false;
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    expanded.push('{');
                } else {
                    let mut token = String::new();
                    let mut closed = false;
                    for next_ch in chars.by_ref() {
                        if next_ch == '}' {
                            closed = true;
                            break;
                        }
                        if next_ch == '{' {
                            return Err(invalid("unmatched '{' in template".to_string()));
                        }
                        token.push(next_ch);
                    }
                    if !closed {
                        return Err(invalid("unmatched '{' in template".to_string()));
                    }
                    match token.as_str() {
                        "page" => {
                            expanded.push_str("&P");
                            has_dynamic_token = true;
                        }
                        "pages" => {
                            expanded.push_str("&N");
                            has_dynamic_token = true;
                        }
                        "sheet" => {
                            expanded.push_str("&A");
                            has_dynamic_token = true;
                        }
                        "date" => {
                            expanded.push_str("&D");
                            has_dynamic_token = true;
                        }
                        "model" => {
                            for mc in model_name.chars() {
                                if mc == '&' {
                                    expanded.push_str("&&");
                                } else {
                                    expanded.push(mc);
                                }
                            }
                        }
                        other => {
                            return Err(invalid(format!("unknown placeholder '{{{other}}}'")));
                        }
                    }
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    expanded.push('}');
                } else {
                    return Err(invalid("unmatched '}' in template".to_string()));
                }
            }
            '&' => {
                expanded.push_str("&&");
            }
            other => {
                expanded.push(other);
            }
        }
    }

    if expanded.chars().any(char::is_control) {
        return Err(invalid(
            "expanded zone text must not contain control characters".to_string(),
        ));
    }

    if !has_dynamic_token && expanded.trim().is_empty() {
        return Err(invalid(
            "expanded zone text must not be empty or whitespace".to_string(),
        ));
    }

    for raw_macro in FORBIDDEN_XLSXWRITER_MACROS {
        if expanded.contains(raw_macro) {
            return Err(invalid(format!(
                "expanded zone text must not contain raw Excel macro '{raw_macro}'"
            )));
        }
    }

    Ok(expanded)
}

fn validate_print_theme_font(sheet: &str, font_family: &str) -> Result<(), ModelError> {
    if font_family.trim().is_empty() {
        return Err(ModelError::InvalidPrintThemeFont {
            sheet: sheet.to_string(),
            font_family: font_family.to_string(),
            reason: "font_family must not be empty or whitespace",
        });
    }
    if font_family.chars().any(char::is_control) {
        return Err(ModelError::InvalidPrintThemeFont {
            sheet: sheet.to_string(),
            font_family: font_family.to_string(),
            reason: "font_family must not contain control characters",
        });
    }
    if font_family.contains(['"', ',', '&']) {
        return Err(ModelError::InvalidPrintThemeFont {
            sheet: sheet.to_string(),
            font_family: font_family.to_string(),
            reason: "font_family must not contain '\"', ',', or '&'",
        });
    }
    Ok(())
}

/// Validates a structured header or footer specification independent of theme typography.
///
/// # Errors
/// Returns [`ModelError::EmptyPrintHeaderFooter`] if all zones are `None`, or
/// [`ModelError::InvalidPrintHeaderFooterZone`] if any populated zone violates template syntax or
/// post-expansion safety invariants.
pub fn validate_header_footer_spec(
    sheet: &str,
    section: HeaderFooterSection,
    spec: &HeaderFooterSpec,
    model_name: &str,
) -> Result<(), ModelError> {
    if spec.is_empty() {
        return Err(ModelError::EmptyPrintHeaderFooter {
            sheet: sheet.to_string(),
            section,
        });
    }
    for (zone, template_opt) in [
        (HeaderFooterZone::Left, &spec.left),
        (HeaderFooterZone::Center, &spec.center),
        (HeaderFooterZone::Right, &spec.right),
    ] {
        if let Some(template) = template_opt {
            expand_header_footer_zone(sheet, section, zone, template, model_name)?;
        }
    }
    Ok(())
}

/// Compiles a structured header or footer specification into an Excel control-coded string
/// styled with the resolved workbook theme and validated against Excel's 255 UTF-16 limit.
///
/// # Errors
/// Returns [`ModelError::EmptyPrintHeaderFooter`], [`ModelError::InvalidPrintHeaderFooterZone`],
/// [`ModelError::InvalidPrintThemeFont`], [`ModelError::InvalidThemeColor`], or
/// [`ModelError::PrintHeaderFooterTooLong`].
pub fn compile_header_footer(
    sheet: &str,
    section: HeaderFooterSection,
    spec: &HeaderFooterSpec,
    model_name: &str,
    theme: &Theme,
) -> Result<String, ModelError> {
    if spec.is_empty() {
        return Err(ModelError::EmptyPrintHeaderFooter {
            sheet: sheet.to_string(),
            section,
        });
    }

    let font_family = theme.typography.font_family.as_str();
    validate_print_theme_font(sheet, font_family)?;

    let rgb = parse_hex_rgb(&theme.palette.table_subtitle_text).ok_or_else(|| {
        ModelError::InvalidThemeColor {
            field: "table_subtitle_text".to_string(),
            color: theme.palette.table_subtitle_text.clone(),
        }
    })?;

    let style_prefix =
        format!("&{PRINT_HEADER_FOOTER_FONT_SIZE}&K{rgb:06X}&\"{font_family},Regular\"");

    let mut compiled = String::new();
    for (zone, template_opt) in [
        (HeaderFooterZone::Left, &spec.left),
        (HeaderFooterZone::Center, &spec.center),
        (HeaderFooterZone::Right, &spec.right),
    ] {
        if let Some(template) = template_opt {
            let expanded = expand_header_footer_zone(sheet, section, zone, template, model_name)?;
            compiled.push_str(zone.excel_code());
            compiled.push_str(&style_prefix);
            compiled.push_str(&expanded);
        }
    }

    let utf16_len = compiled.encode_utf16().count();
    if utf16_len > MAX_EXCEL_HEADER_FOOTER_UTF16 {
        return Err(ModelError::PrintHeaderFooterTooLong {
            sheet: sheet.to_string(),
            section,
            utf16_len,
            max: MAX_EXCEL_HEADER_FOOTER_UTF16,
        });
    }

    Ok(compiled)
}
