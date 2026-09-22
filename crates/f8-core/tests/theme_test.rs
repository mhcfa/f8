#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

use f8_core::model::{BorderStyle, CurrencyCode, FormatPreset, ModelError, ProjectManifest, Theme};

#[test]
fn test_scenario_1_builtin_theme_catalog_resolution() {
    let theme = Theme::resolve("corporate_navy", None).expect("failed to resolve built-in theme");
    assert_eq!(theme.name, "corporate_navy");

    // Validate theme passes all internal validation
    theme.validate().expect("built-in theme should be valid");

    // Verify typography and subtitle_size
    assert_eq!(theme.typography.subtitle_size, 9.0);
    assert!(theme.typography.title_size > theme.typography.data_size);

    // Verify metrics
    assert!(theme.metrics.gutter_row_height > 0.0);
    assert!(theme.metrics.gutter_col_width > 0.0);
    assert!(theme.metrics.title_gutter_row_height > 0.0);
    assert!(theme.metrics.col_padding > 0.0);
    assert!(theme.metrics.min_col_width > 0.0);

    // Verify borders
    assert_eq!(theme.borders.total_border_bottom, BorderStyle::Double);
    assert_eq!(theme.borders.col_total_border_right, BorderStyle::Double);

    // Verify Theme::built_in only recognizes corporate_navy
    assert!(Theme::built_in("corporate_navy").is_some());
    assert!(Theme::built_in("classic_green").is_none());
    assert!(Theme::built_in("modern_slate").is_none());
    assert!(Theme::built_in("minimal_monochrome").is_none());

    // Verify removed preset themes are rejected when resolved without external file
    let removed_themes = ["classic_green", "modern_slate", "minimal_monochrome"];
    for name in removed_themes {
        let err = Theme::resolve(name, None).unwrap_err();
        assert!(matches!(
            err,
            ModelError::ThemeNotFound { name: ref n, ref path } if n == name && path == Path::new(name)
        ));
    }
}

#[test]
fn test_scenario_2_dynamic_dec_and_sym_expansion_zero_decimals() {
    let theme = Theme::corporate_navy();

    // Currency with zero decimals, currency None (defaults to USD / $)
    let curr_mask = theme
        .resolve_number_format(FormatPreset::Currency, Some(0), None)
        .expect("currency format resolution failed");
    assert_eq!(curr_mask, "$#,##0_);($#,##0);\"-\"_)");

    // Number with zero decimals
    let num_mask = theme
        .resolve_number_format(FormatPreset::Number, Some(0), None)
        .expect("number format resolution failed");
    assert_eq!(num_mask, "#,##0_);(#,##0);\"-\"_)");

    // Percentage with zero decimals
    let pct_mask = theme
        .resolve_number_format(FormatPreset::Percentage, Some(0), None)
        .expect("percentage format resolution failed");
    assert_eq!(pct_mask, "0%");

    // Multiple with zero decimals
    let mult_mask = theme
        .resolve_number_format(FormatPreset::Multiple, Some(0), None)
        .expect("multiple format resolution failed");
    assert_eq!(mult_mask, "0\"x\"");

    // BasisPoints with zero decimals
    let bps_mask = theme
        .resolve_number_format(FormatPreset::BasisPoints, Some(0), None)
        .expect("basis points format resolution failed");
    assert_eq!(bps_mask, "#,##0\" bps\"");
}

#[test]
fn test_scenario_3_dynamic_dec_and_sym_expansion_positive_decimals_and_currency() {
    let theme = Theme::corporate_navy();

    // Currency: 2 decimals, EUR
    let eur_mask = theme
        .resolve_number_format(FormatPreset::Currency, Some(2), Some(CurrencyCode::Eur))
        .expect("eur currency resolution failed");
    assert_eq!(eur_mask, "€#,##0.00_);(€#,##0.00);\"-\"_)");

    // Currency: 2 decimals, CHF (quoted symbol)
    let chf_mask = theme
        .resolve_number_format(FormatPreset::Currency, Some(2), Some(CurrencyCode::Chf))
        .expect("chf currency resolution failed");
    assert_eq!(chf_mask, "\"CHF\"#,##0.00_);(\"CHF\"#,##0.00);\"-\"_)");

    // Percentage: 1 decimal
    let pct_mask = theme
        .resolve_number_format(FormatPreset::Percentage, Some(1), None)
        .expect("percentage resolution failed");
    assert_eq!(pct_mask, "0.0%");

    // Number: 4 decimals
    let num_mask = theme
        .resolve_number_format(FormatPreset::Number, Some(4), None)
        .expect("number resolution failed");
    assert_eq!(num_mask, "#,##0.0000_);(#,##0.0000);\"-\"_)");
}

#[test]
fn test_scenario_4_fallback_to_theme_defaults() {
    let theme = Theme::corporate_navy();

    // Currency default: 0 decimals, USD
    let curr_mask = theme
        .resolve_number_format(FormatPreset::Currency, None, None)
        .expect("default currency resolution failed");
    assert_eq!(curr_mask, "$#,##0_);($#,##0);\"-\"_)");

    // Percentage default: 1 decimal
    let pct_mask = theme
        .resolve_number_format(FormatPreset::Percentage, None, None)
        .expect("default percentage resolution failed");
    assert_eq!(pct_mask, "0.0%");

    // Multiple default: 1 decimal
    let mult_mask = theme
        .resolve_number_format(FormatPreset::Multiple, None, None)
        .expect("default multiple resolution failed");
    assert_eq!(mult_mask, "0.0\"x\"");

    // Number default: 0 decimals
    let num_mask = theme
        .resolve_number_format(FormatPreset::Number, None, None)
        .expect("default number resolution failed");
    assert_eq!(num_mask, "#,##0_);(#,##0);\"-\"_)");

    // BasisPoints default: 0 decimals
    let bps_mask = theme
        .resolve_number_format(FormatPreset::BasisPoints, None, None)
        .expect("default basis points resolution failed");
    assert_eq!(bps_mask, "#,##0\" bps\"");
}

#[test]
fn test_scenario_5_non_numeric_presets_ignore_decimals_and_currency() {
    let theme = Theme::corporate_navy();

    let date_mask = theme
        .resolve_number_format(FormatPreset::Date, Some(2), Some(CurrencyCode::Eur))
        .expect("date format resolution failed");
    assert_eq!(date_mask, "yyyy-mm-dd");

    let text_mask = theme
        .resolve_number_format(FormatPreset::Text, Some(2), Some(CurrencyCode::Eur))
        .expect("text format resolution failed");
    assert_eq!(text_mask, "@");
}

#[test]
fn test_scenario_6_external_custom_theme_loading() {
    let temp_dir = std::env::temp_dir().join(format!("f8_theme_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    let custom_theme_json = r##"{
        "$schema": "https://f8.dev/schemas/theme.json",
        "name": "firm_custom",
        "typography": {
            "font_family": "Calibri",
            "title_size": 16.0,
            "subtitle_size": 10.0,
            "super_header_size": 11.0,
            "col_header_size": 11.0,
            "section_header_size": 11.0,
            "data_size": 11.0
        },
        "metrics": {
            "title_row_height": 26.0,
            "header_row_height": 20.0,
            "section_row_height": 20.0,
            "data_row_height": 16.0,
            "gutter_row_height": 10.0,
            "gutter_col_width": 4.0,
            "title_gutter_row_height": 9.0,
            "char_width_multiplier": 1.2,
            "col_padding": 2.5,
            "min_col_width": 14.0
        },
        "palette": {
            "table_title_fill": "#2B4C7E",
            "table_title_text": "#FFFFFF",
            "table_subtitle_text": "#555555",
            "super_header_fill": "#3B5C8E",
            "super_header_text": "#FFFFFF",
            "col_header_fill": "#EBF1F5",
            "col_header_text": "#2B4C7E",
            "section_header_fill": "#F5F8FA",
            "section_header_text": "#2B4C7E",
            "data_fill": "#FFFFFF",
            "data_text": "#222222",
            "subtotal_fill": "#F5F8FA",
            "subtotal_text": "#2B4C7E",
            "total_fill": "#EBF1F5",
            "total_text": "#2B4C7E",
            "input_text": "#002060",
            "formula_text": "#000000",
            "zebra_fill": "#FAFCFD",
            "highlight_fill": "#FEF3C7",
            "highlight_text": "#92400E",
            "gridline_color": "#E5E7EB",
            "border_color": "#D1D5DB",
            "total_border_color": "#2B4C7E"
        },
        "borders": {
            "table_title_border_bottom": "none",
            "super_header_border_bottom": "thin",
            "header_border_bottom": "thin",
            "descriptor_border_right": "none",
            "subtotal_border_top": "thin",
            "total_border_top": "thin",
            "total_border_bottom": "double",
            "col_subtotal_border_left": "thin",
            "col_total_border_left": "thin",
            "col_total_border_right": "double"
        },
        "number_formats": {
            "currency": "{sym}#,##0{dec}_);({sym}#,##0{dec});\"-\"_)",
            "number": "#,##0{dec}_);(#,##0{dec});\"-\"_)",
            "percentage": "0{dec}%",
            "multiple": "0{dec}\"x\"",
            "basis_points": "#,##0{dec}\" bps\"",
            "date": "yyyy-mm-dd",
            "text": "@",
            "defaults": {
                "currency_decimals": 2,
                "number_decimals": 2,
                "percentage_decimals": 2,
                "multiple_decimals": 2,
                "basis_points_decimals": 0
            }
        }
    }"##;

    let theme_file = temp_dir.join("custom_theme.json");
    fs::write(&theme_file, custom_theme_json).expect("failed to write custom theme file");

    let loaded =
        Theme::resolve("custom_theme.json", Some(&temp_dir)).expect("failed to load custom theme");
    assert_eq!(loaded.name, "firm_custom");
    assert_eq!(loaded.typography.font_family, "Calibri");
    assert_eq!(loaded.number_formats.defaults.currency_decimals, 2);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_scenario_7_rejection_of_invalid_theme_configurations() {
    // 1. Invalid color code (not starting with # or bad hex)
    let mut theme = Theme::corporate_navy();
    theme.palette.table_title_fill = "blue".to_string();
    assert!(matches!(
        theme.validate().unwrap_err(),
        ModelError::InvalidThemeColor { field, color } if field == "table_title_fill" && color == "blue"
    ));

    // 2. Non-positive metric
    let mut theme = Theme::corporate_navy();
    theme.metrics.data_row_height = 0.0;
    assert!(matches!(
        theme.validate().unwrap_err(),
        ModelError::InvalidThemeMetric { metric, value } if metric == "data_row_height" && value == 0.0
    ));

    let mut theme = Theme::corporate_navy();
    theme.metrics.col_padding = -1.0;
    assert!(matches!(
        theme.validate().unwrap_err(),
        ModelError::InvalidThemeMetric { metric, .. } if metric == "col_padding"
    ));

    // 3. Missing {dec} placeholder in number format template
    let mut theme = Theme::corporate_navy();
    theme.number_formats.number = "#,##0;(#,##0);\"-\"".to_string();
    assert!(matches!(
        theme.validate().unwrap_err(),
        ModelError::MissingDecimalPlaceholder { preset } if preset == FormatPreset::Number
    ));

    // 4. Missing {sym} placeholder in currency format template
    let mut theme = Theme::corporate_navy();
    theme.number_formats.currency = "$#,##0{dec}_);($#,##0{dec});\"-\"_)".to_string();
    assert!(matches!(
        theme.validate().unwrap_err(),
        ModelError::MissingCurrencySymbolPlaceholder
    ));

    // 5. Default precision exceeding maximum bounds (> 14)
    let mut theme = Theme::corporate_navy();
    theme.number_formats.defaults.currency_decimals = 15;
    assert!(matches!(
        theme.validate().unwrap_err(),
        ModelError::ThemePrecisionOutOfBounds { preset, decimals, max }
            if preset == FormatPreset::Currency && decimals == 15 && max == 14
    ));

    // 6. Format resolution precision exceeding bounds (> 14)
    let theme = Theme::corporate_navy();
    assert!(matches!(
        theme.resolve_number_format(FormatPreset::Percentage, Some(15), None).unwrap_err(),
        ModelError::ThemePrecisionOutOfBounds { preset, decimals, max }
            if preset == FormatPreset::Percentage && decimals == 15 && max == 14
    ));
}

#[test]
fn test_scenario_8_rejection_of_nonexistent_theme() {
    let err = Theme::resolve("neon_cyberpunk", None).unwrap_err();
    assert!(matches!(
        err,
        ModelError::ThemeNotFound { name, path } if name == "neon_cyberpunk" && path == Path::new("neon_cyberpunk")
    ));
}

#[test]
fn test_scenario_9_manifest_theme_default_fallback() {
    // 1. Manifest with omitted theme deserializes to "corporate_navy"
    let json = r#"{
        "model_name": "Test Model",
        "sheets": []
    }"#;
    let manifest: ProjectManifest =
        serde_json::from_str(json).expect("failed to deserialize manifest without theme");
    assert_eq!(manifest.theme, "corporate_navy");

    let resolved = manifest
        .resolve_theme(None)
        .expect("failed to resolve theme from manifest");
    assert_eq!(resolved.name, "corporate_navy");

    // 2. Manifest with empty theme string also falls back to "corporate_navy"
    let json_empty = r#"{
        "model_name": "Test Model",
        "theme": "",
        "sheets": []
    }"#;
    let manifest_empty: ProjectManifest =
        serde_json::from_str(json_empty).expect("failed to deserialize manifest with empty theme");
    let resolved_empty = manifest_empty
        .resolve_theme(None)
        .expect("failed to resolve empty theme from manifest");
    assert_eq!(resolved_empty.name, "corporate_navy");
}

#[test]
fn test_scenario_10_transparent_fill_validation() {
    let mut theme = Theme::corporate_navy();

    // 1. Fill fields accept "none" and "transparent" in various casings
    theme.palette.data_fill = "none".to_string();
    theme.palette.subtotal_fill = "transparent".to_string();
    theme.palette.total_fill = "None".to_string();
    theme.palette.col_header_fill = "TRANSPARENT".to_string();
    theme.palette.section_header_fill = "none".to_string();
    theme.palette.super_header_fill = "none".to_string();
    theme.palette.table_title_fill = "none".to_string();
    theme.palette.zebra_fill = "transparent".to_string();
    theme.palette.highlight_fill = "none".to_string();
    theme.validate().expect("transparent fills should be valid");

    // 2. Text and border fields reject "none" and "transparent"
    let mut invalid_text_theme = Theme::corporate_navy();
    invalid_text_theme.palette.data_text = "none".to_string();
    assert!(matches!(
        invalid_text_theme.validate().unwrap_err(),
        ModelError::InvalidThemeColor { field, color }
            if field == "data_text" && color == "none"
    ));

    let mut invalid_border_theme = Theme::corporate_navy();
    invalid_border_theme.palette.border_color = "transparent".to_string();
    assert!(matches!(
        invalid_border_theme.validate().unwrap_err(),
        ModelError::InvalidThemeColor { field, color }
            if field == "border_color" && color == "transparent"
    ));

    // 3. Fill fields reject invalid non-hex strings
    let mut invalid_fill_theme = Theme::corporate_navy();
    invalid_fill_theme.palette.data_fill = "blurry_blue".to_string();
    assert!(matches!(
        invalid_fill_theme.validate().unwrap_err(),
        ModelError::InvalidThemeColor { field, color }
            if field == "data_fill" && color == "blurry_blue"
    ));
}
