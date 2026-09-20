#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::compiler::emission::{
    CellFormatKey, EmissionError, FormatCache, parse_hex_color, translate_border_style,
};
use finx::compiler::layout::StyleRole;
use finx::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use finx::model::theme::{BorderStyle, Theme};
use rust_xlsxwriter::{Color, FormatBorder};

#[test]
fn test_hex_color_parser() {
    // 6 hex digits
    assert_eq!(
        parse_hex_color("test", "#1B365D").unwrap(),
        Color::RGB(0x1B365D)
    );
    // 3 hex digits
    assert_eq!(
        parse_hex_color("test", "#123").unwrap(),
        Color::RGB(0x112233)
    );
    // 8 hex digits with alpha channel
    assert_eq!(
        parse_hex_color("test", "#1B365DFF").unwrap(),
        Color::RGB(0x1B365D)
    );

    // Invalid formats
    assert!(parse_hex_color("test", "1B365D").is_err());
    assert!(parse_hex_color("test", "#12").is_err());
    assert!(parse_hex_color("test", "#12345").is_err());
    assert!(parse_hex_color("test", "#1234567").is_err());
    assert!(parse_hex_color("test", "#GGGGGG").is_err());
    assert!(parse_hex_color("test", "#1234567Z").is_err());
}

#[test]
fn test_translate_border_style() {
    assert_eq!(
        translate_border_style(BorderStyle::None),
        FormatBorder::None
    );
    assert_eq!(
        translate_border_style(BorderStyle::Thin),
        FormatBorder::Thin
    );
    assert_eq!(
        translate_border_style(BorderStyle::Medium),
        FormatBorder::Medium
    );
    assert_eq!(
        translate_border_style(BorderStyle::Thick),
        FormatBorder::Thick
    );
    assert_eq!(
        translate_border_style(BorderStyle::Double),
        FormatBorder::Double
    );
    assert_eq!(
        translate_border_style(BorderStyle::Dashed),
        FormatBorder::Dashed
    );
    assert_eq!(
        translate_border_style(BorderStyle::Dotted),
        FormatBorder::Dotted
    );
    assert_eq!(
        translate_border_style(BorderStyle::Hair),
        FormatBorder::Hair
    );
}

#[test]
fn test_scenario_1_dynamic_number_format_mask_synthesis() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    let key = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Currency),
        Some(2),
        Some(CurrencyCode::Usd),
        false,
        false,
    );
    let format = cache.get_cell_format(&key).expect("format ok");
    let debug = format!("{format:?}");

    assert!(
        debug.contains(r#"num_format: "$#,##0.00_);($#,##0.00);\"-\"_)""#),
        "Expected currency number format mask in: {debug}"
    );
}

#[test]
fn test_scenario_2_hardcode_input_vs_formula_text_color() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    let input_key = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Number),
        Some(0),
        None,
        true,  // is_input
        false, // is_descriptor
    );
    let formula_key = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Number),
        Some(0),
        None,
        false, // is_input
        false, // is_descriptor
    );

    let input_format = cache.get_cell_format(&input_key).expect("input ok");
    let formula_format = cache.get_cell_format(&formula_key).expect("formula ok");

    let input_debug = format!("{input_format:?}");
    let formula_debug = format!("{formula_format:?}");

    // theme.palette.input_text is "#002060" -> 0x002060 = 8288
    assert!(
        input_debug.contains("color: RGB(8288)"),
        "Input cell should use input_text (#002060): {input_debug}"
    );
    // theme.palette.formula_text is "#000000" -> 0x000000 = 0
    assert!(
        formula_debug.contains("color: RGB(0)"),
        "Formula cell should use formula_text (#000000): {formula_debug}"
    );
}

#[test]
fn test_scenario_3_descriptor_labels_suppress_input_blue_text() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    let descriptor_key = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Text),
        None,
        None,
        true, // is_input (literal row title)
        true, // is_descriptor
    );

    let format = cache.get_cell_format(&descriptor_key).expect("format ok");
    let debug = format!("{format:?}");

    // theme.palette.data_text is "#1F2937" -> 0x1F2937 = 2042167
    assert!(
        debug.contains("color: RGB(2042167)"),
        "Descriptor cell must use data_text, never input_text: {debug}"
    );
    assert!(
        debug.contains("horizontal: Left"),
        "Descriptor cell must be left-aligned: {debug}"
    );
}

#[test]
fn test_scenario_4_subtotal_and_total_accounting_borders_and_colors() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    let subtotal_role = StyleRole {
        horizontal: SemanticStyle::Subtotal,
        vertical: SemanticStyle::Data,
        is_highlight: false,
    };
    let total_role = StyleRole {
        horizontal: SemanticStyle::Total,
        vertical: SemanticStyle::Data,
        is_highlight: false,
    };

    let subtotal_key = CellFormatKey::new(
        subtotal_role,
        Some(FormatPreset::Currency),
        Some(0),
        Some(CurrencyCode::Usd),
        false,
        false,
    );
    let total_key = CellFormatKey::new(
        total_role,
        Some(FormatPreset::Currency),
        Some(0),
        Some(CurrencyCode::Usd),
        false,
        false,
    );

    let subtotal_fmt = cache.get_cell_format(&subtotal_key).expect("subtotal ok");
    let total_fmt = cache.get_cell_format(&total_key).expect("total ok");

    let subtotal_debug = format!("{subtotal_fmt:?}");
    let total_debug = format!("{total_fmt:?}");

    // Subtotal: top_style Thin, border_color #D1D5DB = 13751771, bold: true
    assert!(
        subtotal_debug.contains("top_style: Thin"),
        "Subtotal must have thin top border: {subtotal_debug}"
    );
    assert!(
        subtotal_debug.contains("top_color: RGB(13751771)"),
        "Subtotal top border color must match palette.border_color: {subtotal_debug}"
    );
    assert!(
        subtotal_debug.contains("bold: true"),
        "Subtotal must be bold: {subtotal_debug}"
    );

    // Total: top_style Thin, bottom_style Double, total_border_color #1B365D = 1783389, bold: true
    assert!(
        total_debug.contains("top_style: Thin"),
        "Total must have thin top border: {total_debug}"
    );
    assert!(
        total_debug.contains("bottom_style: Double"),
        "Total must have double bottom border: {total_debug}"
    );
    assert!(
        total_debug.contains("top_color: RGB(1783389)"),
        "Total top border must match palette.total_border_color: {total_debug}"
    );
    assert!(
        total_debug.contains("bottom_color: RGB(1783389)"),
        "Total bottom border must match palette.total_border_color: {total_debug}"
    );
    assert!(
        total_debug.contains("bold: true"),
        "Total must be bold: {total_debug}"
    );
}

#[test]
fn test_scenario_5_highlight_style_role_emphasis() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    let highlight_role = StyleRole {
        horizontal: SemanticStyle::Data,
        vertical: SemanticStyle::Data,
        is_highlight: true,
    };
    let key = CellFormatKey::new(
        highlight_role,
        Some(FormatPreset::Percentage),
        Some(1),
        None,
        false,
        false,
    );

    let format = cache.get_cell_format(&key).expect("format ok");
    let debug = format!("{format:?}");

    // palette.highlight_fill is "#FEF3C7" -> 0xFEF3C7 = 16708551
    // palette.highlight_text is "#92400E" -> 0x92400E = 9584654
    assert!(
        debug.contains("background_color: RGB(16708551)"),
        "Highlight cell must have highlight_fill: {debug}"
    );
    assert!(
        debug.contains("color: RGB(9584654)"),
        "Highlight cell must have highlight_text: {debug}"
    );
}

#[test]
fn test_scenario_6_structural_header_formats_and_alignments() {
    let theme = Theme::corporate_navy();
    let cache = FormatCache::new(&theme).expect("valid theme");

    let title_fmt = cache.title_format();
    let title_debug = format!("{title_fmt:?}");
    assert!(title_debug.contains(r#"name: "Aptos""#));
    assert!(title_debug.contains(r#"size: "14""#));
    assert!(title_debug.contains("bold: true"));
    assert!(title_debug.contains("horizontal: Left"));

    let subtitle_fmt = cache.subtitle_format();
    let subtitle_debug = format!("{subtitle_fmt:?}");
    assert!(subtitle_debug.contains(r#"size: "9""#));
    assert!(subtitle_debug.contains("horizontal: Left"));

    let super_fmt = cache.super_header_format();
    let super_debug = format!("{super_fmt:?}");
    assert!(super_debug.contains(r#"size: "10""#));
    assert!(super_debug.contains("bold: true"));
    assert!(super_debug.contains("horizontal: Center"));
    assert!(super_debug.contains("bottom_style: Thin"));

    let col_desc_fmt = cache.col_header_format(true);
    let col_desc_debug = format!("{col_desc_fmt:?}");
    assert!(col_desc_debug.contains("horizontal: Left"));
    assert!(col_desc_debug.contains("bold: true"));
    assert!(col_desc_debug.contains("bottom_style: Thin"));

    let col_meas_fmt = cache.col_header_format(false);
    let col_meas_debug = format!("{col_meas_fmt:?}");
    assert!(col_meas_debug.contains("horizontal: Right"));
    assert!(col_meas_debug.contains("bold: true"));
    assert!(col_meas_debug.contains("bottom_style: Thin"));

    let sec_fmt = cache.section_header_format();
    let sec_debug = format!("{sec_fmt:?}");
    assert!(sec_debug.contains("horizontal: Left"));
    assert!(sec_debug.contains("bold: true"));
}

#[test]
fn test_scenario_7_format_deduplication_cache_hits() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    let key = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Number),
        Some(0),
        None,
        false,
        false,
    );

    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());

    let _fmt1 = cache.get_cell_format(&key).expect("ok");
    assert_eq!(cache.len(), 1);

    let _fmt2 = cache.get_cell_format(&key).expect("ok");
    assert_eq!(cache.len(), 1);

    let _fmt3 = cache.get_cell_format(&key).expect("ok");
    assert_eq!(cache.len(), 1);
    assert!(!cache.is_empty());
}

#[test]
fn test_scenario_8_unformatted_general_preset_handling() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    let key = CellFormatKey::new(
        StyleRole::data(),
        None, // preset is None
        None,
        None,
        false,
        false,
    );

    let format = cache.get_cell_format(&key).expect("ok");
    let debug = format!("{format:?}");

    // Number format should be default/empty ("" or not set)
    assert!(
        debug.contains(r#"num_format: """#),
        "Unformatted preset must not set custom num_format: {debug}"
    );
    assert!(debug.contains(r#"name: "Aptos""#));
    assert!(debug.contains(r#"size: "10""#));
}

#[test]
fn test_scenario_9_invalid_hex_color_diagnostics() {
    let mut invalid_theme = Theme::corporate_navy();
    invalid_theme.palette.table_title_fill = "not-a-color".to_string();

    let result = FormatCache::new(&invalid_theme);
    match result {
        Err(EmissionError::InvalidColor { field, color }) => {
            assert_eq!(field, "table_title_fill");
            assert_eq!(color, "not-a-color");
        }
        other => panic!("Expected EmissionError::InvalidColor, got: {other:?}"),
    }
}

#[test]
fn test_orthogonal_2d_total_box_borders() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    // Both horizontal total and vertical total
    let total_box_role = StyleRole {
        horizontal: SemanticStyle::Total,
        vertical: SemanticStyle::Total,
        is_highlight: false,
    };
    let key = CellFormatKey::new(
        total_box_role,
        Some(FormatPreset::Currency),
        Some(0),
        Some(CurrencyCode::Usd),
        false,
        false,
    );

    let format = cache.get_cell_format(&key).expect("ok");
    let debug = format!("{format:?}");

    assert!(debug.contains("top_style: Thin"));
    assert!(debug.contains("bottom_style: Double"));
    assert!(debug.contains("left_style: Thin"));
    assert!(debug.contains("right_style: Double"));
    assert!(debug.contains("top_color: RGB(1783389)"));
    assert!(debug.contains("bottom_color: RGB(1783389)"));
    assert!(debug.contains("left_color: RGB(1783389)"));
    assert!(debug.contains("right_color: RGB(1783389)"));
}
