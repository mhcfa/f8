use f8_core::model::format::{CurrencyCode, FormatPreset};
use f8_core::value::error::ExcelError;
use f8_core::value::{CellValue, format_accounting_number, format_cell_value};

#[test]
fn test_format_cell_value_scalar_variants() {
    assert_eq!(format_cell_value(&CellValue::Empty, None, None, None), "");
    assert_eq!(
        format_cell_value(&CellValue::Text("Growth Rate".into()), None, None, None),
        "Growth Rate"
    );
    assert_eq!(
        format_cell_value(&CellValue::Boolean(true), None, None, None),
        "TRUE"
    );
    assert_eq!(
        format_cell_value(&CellValue::Boolean(false), None, None, None),
        "FALSE"
    );
    assert_eq!(
        format_cell_value(&CellValue::Error(ExcelError::Div0), None, None, None),
        "#DIV/0!"
    );
    assert_eq!(
        format_cell_value(&CellValue::Error(ExcelError::Value), None, None, None),
        "#VALUE!"
    );
    assert_eq!(
        format_cell_value(&CellValue::Error(ExcelError::Ref), None, None, None),
        "#REF!"
    );
}

#[test]
fn test_format_currency_presets() {
    // Default USD formatting with 1 decimal
    let val = CellValue::Number(115.0);
    assert_eq!(
        format_cell_value(
            &val,
            Some(FormatPreset::Currency),
            Some(1),
            Some(CurrencyCode::Usd)
        ),
        "$ 115.0"
    );

    // Negative currency with parentheses
    let neg = CellValue::Number(-42.75);
    assert_eq!(
        format_cell_value(
            &neg,
            Some(FormatPreset::Currency),
            Some(2),
            Some(CurrencyCode::Usd)
        ),
        "($ 42.75)"
    );

    // EUR and GBP symbols
    assert_eq!(
        format_cell_value(
            &val,
            Some(FormatPreset::Currency),
            Some(0),
            Some(CurrencyCode::Eur)
        ),
        "€ 115"
    );
    assert_eq!(
        format_cell_value(
            &val,
            Some(FormatPreset::Currency),
            Some(0),
            Some(CurrencyCode::Gbp)
        ),
        "£ 115"
    );
}

#[test]
fn test_format_percentage_presets() {
    let p = CellValue::Number(0.15);
    assert_eq!(
        format_cell_value(&p, Some(FormatPreset::Percentage), Some(1), None),
        "15.0%"
    );

    let p_neg = CellValue::Number(-0.025);
    assert_eq!(
        format_cell_value(&p_neg, Some(FormatPreset::Percentage), Some(2), None),
        "(2.50%)"
    );
}

#[test]
fn test_format_multiple_and_bps_presets() {
    let m = CellValue::Number(8.5);
    assert_eq!(
        format_cell_value(&m, Some(FormatPreset::Multiple), Some(1), None),
        "8.5x"
    );
    let m_neg = CellValue::Number(-1.2);
    assert_eq!(
        format_cell_value(&m_neg, Some(FormatPreset::Multiple), Some(1), None),
        "(1.2x)"
    );

    let bps = CellValue::Number(0.0075);
    assert_eq!(
        format_cell_value(&bps, Some(FormatPreset::BasisPoints), Some(0), None),
        "75 bps"
    );
    let bps_neg = CellValue::Number(-0.0025);
    assert_eq!(
        format_cell_value(&bps_neg, Some(FormatPreset::BasisPoints), Some(0), None),
        "(25 bps)"
    );
}

#[test]
fn test_format_number_comma_separation() {
    assert_eq!(
        format_accounting_number(123456789.0, Some(FormatPreset::Number), Some(0), None),
        "123,456,789"
    );
    assert_eq!(
        format_accounting_number(-123456789.5, Some(FormatPreset::Number), Some(1), None),
        "(123,456,789.5)"
    );
}
