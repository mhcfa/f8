#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::compiler::emission::{
    CellFormatKey, EmissionError, FormatCache, parse_fill_color, parse_hex_color,
    translate_border_style,
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
        0,
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
        0,
    );
    let formula_key = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Number),
        Some(0),
        None,
        false, // is_input
        false, // is_descriptor
        0,
    );

    let input_format = cache.get_cell_format(&input_key).expect("input ok");
    let formula_format = cache.get_cell_format(&formula_key).expect("formula ok");

    let input_debug = format!("{input_format:?}");
    let formula_debug = format!("{formula_format:?}");

    let expected_input_color = format!(
        "{:?}",
        parse_hex_color("input_text", &theme.palette.input_text).unwrap()
    );
    let expected_formula_color = format!(
        "{:?}",
        parse_hex_color("formula_text", &theme.palette.formula_text).unwrap()
    );

    assert!(
        input_debug.contains(&format!("color: {expected_input_color}")),
        "Input cell should use input_text ({}): {input_debug}",
        theme.palette.input_text
    );
    assert!(
        formula_debug.contains(&format!("color: {expected_formula_color}")),
        "Formula cell should use formula_text ({}): {formula_debug}",
        theme.palette.formula_text
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
        0,
    );

    let format = cache.get_cell_format(&descriptor_key).expect("format ok");
    let debug = format!("{format:?}");

    let expected_data_color = format!(
        "{:?}",
        parse_hex_color("data_text", &theme.palette.data_text).unwrap()
    );
    let expected_input_color = format!(
        "{:?}",
        parse_hex_color("input_text", &theme.palette.input_text).unwrap()
    );

    assert!(
        debug.contains(&format!("color: {expected_data_color}")),
        "Descriptor cell must use data_text: {debug}"
    );
    assert!(
        !debug.contains(&format!("color: {expected_input_color}")),
        "Descriptor cell must never use input_text: {debug}"
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
        0,
    );
    let total_key = CellFormatKey::new(
        total_role,
        Some(FormatPreset::Currency),
        Some(0),
        Some(CurrencyCode::Usd),
        false,
        false,
        0,
    );

    let subtotal_fmt = cache.get_cell_format(&subtotal_key).expect("subtotal ok");
    let total_fmt = cache.get_cell_format(&total_key).expect("total ok");

    let subtotal_debug = format!("{subtotal_fmt:?}");
    let total_debug = format!("{total_fmt:?}");

    let border_color = format!(
        "{:?}",
        parse_hex_color("border_color", &theme.palette.border_color).unwrap()
    );
    let total_border_color = format!(
        "{:?}",
        parse_hex_color("total_border_color", &theme.palette.total_border_color).unwrap()
    );
    let subtotal_border_style = format!(
        "{:?}",
        translate_border_style(theme.borders.subtotal_border_top)
    );
    let total_top_style = format!(
        "{:?}",
        translate_border_style(theme.borders.total_border_top)
    );
    let total_bottom_style = format!(
        "{:?}",
        translate_border_style(theme.borders.total_border_bottom)
    );

    assert!(
        subtotal_debug.contains(&format!("top_style: {subtotal_border_style}")),
        "Subtotal must have configured top border: {subtotal_debug}"
    );
    assert!(
        subtotal_debug.contains(&format!("top_color: {border_color}")),
        "Subtotal top border color must match palette.border_color: {subtotal_debug}"
    );
    assert!(
        subtotal_debug.contains("bold: true"),
        "Subtotal must be bold: {subtotal_debug}"
    );

    assert!(
        total_debug.contains(&format!("top_style: {total_top_style}")),
        "Total must have configured top border: {total_debug}"
    );
    assert!(
        total_debug.contains(&format!("bottom_style: {total_bottom_style}")),
        "Total must have configured bottom border: {total_debug}"
    );
    assert!(
        total_debug.contains(&format!("top_color: {total_border_color}")),
        "Total top border must match palette.total_border_color: {total_debug}"
    );
    assert!(
        total_debug.contains(&format!("bottom_color: {total_border_color}")),
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
        0,
    );

    let format = cache.get_cell_format(&key).expect("format ok");
    let debug = format!("{format:?}");

    let expected_fill = format!(
        "{:?}",
        parse_hex_color("highlight_fill", &theme.palette.highlight_fill).unwrap()
    );
    let expected_text = format!(
        "{:?}",
        parse_hex_color("highlight_text", &theme.palette.highlight_text).unwrap()
    );

    assert!(
        debug.contains(&format!("background_color: {expected_fill}")),
        "Highlight cell must have highlight_fill: {debug}"
    );
    assert!(
        debug.contains(&format!("color: {expected_text}")),
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
    let super_border = format!(
        "{:?}",
        translate_border_style(theme.borders.super_header_border_bottom)
    );
    assert!(
        super_debug.contains(&format!("bottom_style: {super_border}")),
        "Super header must match borders.super_header_border_bottom: {super_debug}"
    );

    let col_desc_fmt = cache.col_header_format(true);
    let col_desc_debug = format!("{col_desc_fmt:?}");
    assert!(col_desc_debug.contains("horizontal: Left"));
    assert!(col_desc_debug.contains("bold: true"));
    let header_border = format!(
        "{:?}",
        translate_border_style(theme.borders.header_border_bottom)
    );
    assert!(
        col_desc_debug.contains(&format!("bottom_style: {header_border}")),
        "Descriptor header must match borders.header_border_bottom: {col_desc_debug}"
    );

    let col_meas_fmt = cache.col_header_format(false);
    let col_meas_debug = format!("{col_meas_fmt:?}");
    assert!(col_meas_debug.contains("horizontal: Right"));
    assert!(col_meas_debug.contains("bold: true"));
    assert!(
        col_meas_debug.contains(&format!("bottom_style: {header_border}")),
        "Measure header must match borders.header_border_bottom: {col_meas_debug}"
    );

    let sec_fmt = cache.section_header_format(None);
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
        0,
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
        0,
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
        0,
    );

    let format = cache.get_cell_format(&key).expect("ok");
    let debug = format!("{format:?}");

    let total_border_color = format!(
        "{:?}",
        parse_hex_color("total_border_color", &theme.palette.total_border_color).unwrap()
    );
    assert!(debug.contains("top_style: Thin"));
    assert!(debug.contains("bottom_style: Double"));
    assert!(debug.contains("left_style: Thin"));
    assert!(debug.contains("right_style: Double"));
    assert!(debug.contains(&format!("top_color: {total_border_color}")));
    assert!(debug.contains(&format!("bottom_color: {total_border_color}")));
    assert!(debug.contains(&format!("left_color: {total_border_color}")));
    assert!(debug.contains(&format!("right_color: {total_border_color}")));
}

#[test]
fn test_parse_fill_color() {
    // Transparent keywords
    assert_eq!(parse_fill_color("test", "none").unwrap(), None);
    assert_eq!(parse_fill_color("test", "None").unwrap(), None);
    assert_eq!(parse_fill_color("test", "NONE").unwrap(), None);
    assert_eq!(parse_fill_color("test", "transparent").unwrap(), None);
    assert_eq!(parse_fill_color("test", "Transparent").unwrap(), None);
    assert_eq!(parse_fill_color("test", "TRANSPARENT").unwrap(), None);

    // Standard hex colors
    assert_eq!(
        parse_fill_color("test", "#1B365D").unwrap(),
        Some(Color::RGB(0x1B365D))
    );
    assert_eq!(
        parse_fill_color("test", "#FFFFFF").unwrap(),
        Some(Color::RGB(0xFFFFFF))
    );

    // Invalid colors
    assert!(parse_fill_color("test", "not_valid").is_err());
    assert!(parse_fill_color("test", "#12").is_err());
}

#[test]
fn test_transparent_cell_format() {
    // 1. Theme with explicit solid fill has background color set
    let mut filled_theme = Theme::corporate_navy();
    filled_theme.palette.data_fill = "#FFFFFF".to_string();
    let mut filled_cache = FormatCache::new(&filled_theme).expect("valid theme");
    let key = CellFormatKey::new(StyleRole::data(), None, None, None, false, false, 0);
    let filled_format = filled_cache.get_cell_format(&key).expect("ok");
    let filled_debug = format!("{filled_format:?}");
    assert!(
        filled_debug.contains("background_color: RGB(16777215)"),
        "Theme with solid data_fill must set background_color: {filled_debug}"
    );

    // 2. Theme with data_fill = "none" leaves background color as Default (transparent)
    let mut transparent_theme = Theme::corporate_navy();
    transparent_theme.palette.data_fill = "none".to_string();
    let mut trans_cache = FormatCache::new(&transparent_theme).expect("valid theme");
    let trans_format = trans_cache.get_cell_format(&key).expect("ok");
    let trans_debug = format!("{trans_format:?}");
    assert!(
        trans_debug.contains("background_color: Default"),
        "Transparent data_fill must leave background_color as Default: {trans_debug}"
    );
}

#[test]
fn test_transparent_summary_and_headers() {
    let mut theme = Theme::corporate_navy();
    theme.palette.table_title_fill = "none".to_string();
    theme.palette.super_header_fill = "transparent".to_string();
    theme.palette.col_header_fill = "none".to_string();
    theme.palette.section_header_fill = "transparent".to_string();
    theme.palette.total_fill = "none".to_string();

    let mut cache = FormatCache::new(&theme).expect("valid theme");

    // Title format has bold and font size 14, but no custom fill
    let title_fmt = cache.title_format();
    let title_debug = format!("{title_fmt:?}");
    assert!(title_debug.contains("background_color: Default"));
    assert!(title_debug.contains("bold: true"));

    // Super-header format has CenterAcross, but no custom fill
    let super_fmt = cache.super_header_format();
    let super_debug = format!("{super_fmt:?}");
    assert!(super_debug.contains("background_color: Default"));
    assert!(super_debug.contains("CenterAcross"));

    // Column header formats have bold, but no custom fill
    let col_desc = cache.col_header_format(true);
    let col_desc_debug = format!("{col_desc:?}");
    assert!(col_desc_debug.contains("background_color: Default"));

    let col_meas = cache.col_header_format(false);
    let col_meas_debug = format!("{col_meas:?}");
    assert!(col_meas_debug.contains("background_color: Default"));

    // Section header format has bold, but no custom fill
    let sec_fmt = cache.section_header_format(None);
    let sec_debug = format!("{sec_fmt:?}");
    assert!(sec_debug.contains("background_color: Default"));

    // Total format retains accounting borders and bold, but no custom fill
    let total_key = CellFormatKey::new(
        StyleRole {
            horizontal: SemanticStyle::Total,
            vertical: SemanticStyle::Data,
            is_highlight: false,
        },
        None,
        None,
        None,
        false,
        false,
        0,
    );
    let total_fmt = cache.get_cell_format(&total_key).expect("ok");
    let total_debug = format!("{total_fmt:?}");
    assert!(total_debug.contains("background_color: Default"));
    assert!(total_debug.contains("bold: true"));
    assert!(total_debug.contains("top_style: Thin"));
    assert!(total_debug.contains("bottom_style: Double"));
}

#[test]
fn test_input_text_color_overrides_everything() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");
    let input_color = format!(
        "{:?}",
        parse_hex_color("input_text", &theme.palette.input_text).unwrap()
    );
    let highlight_fill = format!(
        "{:?}",
        parse_hex_color("highlight_fill", &theme.palette.highlight_fill).unwrap()
    );
    let total_text = format!(
        "{:?}",
        parse_hex_color("total_text", &theme.palette.total_text).unwrap()
    );
    let highlight_text = format!(
        "{:?}",
        parse_hex_color("highlight_text", &theme.palette.highlight_text).unwrap()
    );

    // 1. Input measure in a Total row -> input_text overrides total_text, retains accounting borders and bold
    let total_top_style = format!(
        "{:?}",
        translate_border_style(theme.borders.total_border_top)
    );
    let total_bottom_style = format!(
        "{:?}",
        translate_border_style(theme.borders.total_border_bottom)
    );
    let input_total_key = CellFormatKey::new(
        StyleRole {
            horizontal: SemanticStyle::Total,
            vertical: SemanticStyle::Data,
            is_highlight: false,
        },
        Some(FormatPreset::Currency),
        Some(0),
        Some(CurrencyCode::Usd),
        true,  // is_input
        false, // is_descriptor
        0,
    );
    let fmt = cache.get_cell_format(&input_total_key).expect("ok");
    let debug = format!("{fmt:?}");
    assert!(
        debug.contains(&format!("color: {input_color}")),
        "Input in total row must use input_text: {debug}"
    );
    assert!(
        debug.contains("bold: true"),
        "Input in total row must be bold: {debug}"
    );
    assert!(
        debug.contains(&format!("top_style: {total_top_style}")),
        "Input in total row must have total top border: {debug}"
    );
    assert!(
        debug.contains(&format!("bottom_style: {total_bottom_style}")),
        "Input in total row must have total bottom border: {debug}"
    );

    // 2. Formula in a Total row -> uses total_text
    let formula_total_key = CellFormatKey::new(
        StyleRole {
            horizontal: SemanticStyle::Total,
            vertical: SemanticStyle::Data,
            is_highlight: false,
        },
        Some(FormatPreset::Currency),
        Some(0),
        Some(CurrencyCode::Usd),
        false, // is_input
        false, // is_descriptor
        0,
    );
    let fmt = cache.get_cell_format(&formula_total_key).expect("ok");
    let debug = format!("{fmt:?}");
    assert!(
        debug.contains(&format!("color: {total_text}")),
        "Formula in total row must use total_text: {debug}"
    );

    // 3. Input measure with Highlight -> input_text overrides highlight_text, retains highlight_fill
    let input_highlight_key = CellFormatKey::new(
        StyleRole {
            horizontal: SemanticStyle::Data,
            vertical: SemanticStyle::Data,
            is_highlight: true,
        },
        Some(FormatPreset::Number),
        Some(2),
        None,
        true,  // is_input
        false, // is_descriptor
        0,
    );
    let fmt = cache.get_cell_format(&input_highlight_key).expect("ok");
    let debug = format!("{fmt:?}");
    assert!(
        debug.contains(&format!("color: {input_color}")),
        "Input with highlight must use input_text: {debug}"
    );
    assert!(
        debug.contains(&format!("background_color: {highlight_fill}")),
        "Input with highlight must retain highlight_fill: {debug}"
    );

    // 4. Formula with Highlight -> uses highlight_text
    let formula_highlight_key = CellFormatKey::new(
        StyleRole {
            horizontal: SemanticStyle::Data,
            vertical: SemanticStyle::Data,
            is_highlight: true,
        },
        Some(FormatPreset::Number),
        Some(2),
        None,
        false, // is_input
        false, // is_descriptor
        0,
    );
    let fmt = cache.get_cell_format(&formula_highlight_key).expect("ok");
    let debug = format!("{fmt:?}");
    assert!(
        debug.contains(&format!("color: {highlight_text}")),
        "Formula with highlight must use highlight_text: {debug}"
    );

    // 5. Descriptor with Highlight -> uses highlight_text, NEVER input_text
    let descriptor_highlight_key = CellFormatKey::new(
        StyleRole {
            horizontal: SemanticStyle::Data,
            vertical: SemanticStyle::Data,
            is_highlight: true,
        },
        Some(FormatPreset::Text),
        None,
        None,
        true, // is_input (literal)
        true, // is_descriptor
        0,
    );
    let fmt = cache
        .get_cell_format(&descriptor_highlight_key)
        .expect("ok");
    let debug = format!("{fmt:?}");
    assert!(
        debug.contains(&format!("color: {highlight_text}")),
        "Descriptor with highlight must use highlight_text: {debug}"
    );
    assert!(
        !debug.contains(&format!("color: {input_color}")),
        "Descriptor must never use input_text: {debug}"
    );
}

#[test]
fn test_scenario_13_row_group_and_section_header_indentation() {
    let theme = Theme::corporate_navy();
    let mut cache = FormatCache::new(&theme).expect("valid theme");

    // 1. Descriptor cells with different indentation levels
    let desc_indent_0 = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Text),
        None,
        None,
        false, // is_input
        true,  // is_descriptor
        0,     // indent
    );
    let desc_indent_1 = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Text),
        None,
        None,
        false, // is_input
        true,  // is_descriptor
        1,     // indent
    );
    let desc_indent_2 = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Text),
        None,
        None,
        false, // is_input
        true,  // is_descriptor
        2,     // indent
    );

    let fmt_0 = cache.get_cell_format(&desc_indent_0).expect("ok");
    let fmt_1 = cache.get_cell_format(&desc_indent_1).expect("ok");
    let fmt_2 = cache.get_cell_format(&desc_indent_2).expect("ok");

    let debug_0 = format!("{fmt_0:?}");
    let debug_1 = format!("{fmt_1:?}");
    let debug_2 = format!("{fmt_2:?}");

    assert!(
        debug_0.contains("indent: 0"),
        "indent 0 expected: {debug_0}"
    );
    assert!(
        debug_1.contains("indent: 1"),
        "indent 1 expected: {debug_1}"
    );
    assert!(
        debug_2.contains("indent: 2"),
        "indent 2 expected: {debug_2}"
    );

    // 2. Measure cells must NEVER be indented even if key has indent: 0
    let measure_key = CellFormatKey::new(
        StyleRole::data(),
        Some(FormatPreset::Number),
        Some(2),
        None,
        false, // is_input
        false, // is_descriptor
        0,     // indent
    );
    let measure_fmt = cache.get_cell_format(&measure_key).expect("ok");
    let measure_debug = format!("{measure_fmt:?}");
    assert!(
        measure_debug.contains("indent: 0"),
        "measure indent 0 expected: {measure_debug}"
    );

    // 3. Section headers: None is unindented (indent 0), Some(1) has indent 1, Some(2) has indent 2
    let section_l1 = cache.section_header_format(None);
    let section_l2 = cache.section_header_format(Some(1));
    let section_l3 = cache.section_header_format(Some(2));

    let debug_s1 = format!("{section_l1:?}");
    let debug_s2 = format!("{section_l2:?}");
    let debug_s3 = format!("{section_l3:?}");

    assert!(
        debug_s1.contains("indent: 0"),
        "section level 1 must have indent 0: {debug_s1}"
    );
    assert!(
        debug_s2.contains("indent: 1"),
        "section level 2 must have indent 1: {debug_s2}"
    );
    assert!(
        debug_s3.contains("indent: 2"),
        "section level 3 must have indent 2: {debug_s3}"
    );
}
