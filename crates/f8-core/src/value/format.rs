//! Accounting typography and cell value formatting.

use crate::model::format::{CurrencyCode, FormatPreset};
use crate::value::CellValue;

/// Formats a cell value according to accounting rules and format presets.
#[must_use]
pub fn format_cell_value(
    value: &CellValue,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
) -> String {
    match value {
        CellValue::Number(n) => format_accounting_number(*n, format, decimals, currency),
        CellValue::Text(s) => s.clone(),
        CellValue::Boolean(b) => {
            if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        CellValue::Empty => String::new(),
        CellValue::Error(e) => e.as_str().to_string(),
    }
}

/// Formats an accounting number with parentheses for negative values, commas, and currency/percent symbols.
#[must_use]
pub fn format_accounting_number(
    val: f64,
    format: Option<FormatPreset>,
    decimals: Option<u32>,
    currency: Option<CurrencyCode>,
) -> String {
    let is_neg = val < 0.0 && val.abs() > 1e-9;
    let abs_val = val.abs();

    let dec = match format {
        Some(FormatPreset::Percentage) => decimals.unwrap_or(1) as usize,
        Some(FormatPreset::Multiple) => decimals.unwrap_or(1) as usize,
        Some(FormatPreset::BasisPoints) => decimals.unwrap_or(0) as usize,
        Some(FormatPreset::Currency) => decimals.unwrap_or(1) as usize,
        Some(FormatPreset::Number) => decimals.unwrap_or(0) as usize,
        _ => decimals
            .map(|d| d as usize)
            .unwrap_or(if abs_val.fract().abs() > 1e-6 { 1 } else { 0 }),
    };

    let (scaled_val, suffix) = match format {
        Some(FormatPreset::Percentage) => (abs_val * 100.0, "%"),
        Some(FormatPreset::Multiple) => (abs_val, "x"),
        Some(FormatPreset::BasisPoints) => (abs_val * 10000.0, " bps"),
        _ => (abs_val, ""),
    };

    let formatted_num = format_with_commas(scaled_val, dec);

    match format {
        Some(FormatPreset::Currency) => {
            let sym = currency.unwrap_or_default().symbol();
            if is_neg {
                format!("({sym} {formatted_num})")
            } else {
                format!("{sym} {formatted_num}")
            }
        }
        Some(FormatPreset::Percentage)
        | Some(FormatPreset::Multiple)
        | Some(FormatPreset::BasisPoints) => {
            if is_neg {
                format!("({formatted_num}{suffix})")
            } else {
                format!("{formatted_num}{suffix}")
            }
        }
        _ => {
            if is_neg {
                format!("({formatted_num})")
            } else {
                formatted_num
            }
        }
    }
}

fn format_with_commas(val: f64, decimals: usize) -> String {
    let raw = format!("{val:.decimals$}");
    let parts: Vec<&str> = raw.split('.').collect();
    let int_part = parts[0];
    let frac_part = parts.get(1);

    let mut result = String::new();
    let chars: Vec<char> = int_part.chars().collect();
    let len = chars.len();
    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(ch);
    }
    if let Some(frac) = frac_part {
        result.push('.');
        result.push_str(frac);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::error::ExcelError;

    #[test]
    fn test_format_cell_value_types() {
        assert_eq!(format_cell_value(&CellValue::Empty, None, None, None), "");
        assert_eq!(
            format_cell_value(&CellValue::Text("Revenue".into()), None, None, None),
            "Revenue"
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
    }

    #[test]
    fn test_format_accounting_number_presets() {
        // Currency positive and negative
        assert_eq!(
            format_accounting_number(
                1234.56,
                Some(FormatPreset::Currency),
                Some(2),
                Some(CurrencyCode::Usd)
            ),
            "$ 1,234.56"
        );
        assert_eq!(
            format_accounting_number(
                -1234.56,
                Some(FormatPreset::Currency),
                Some(2),
                Some(CurrencyCode::Usd)
            ),
            "($ 1,234.56)"
        );

        // Percentage
        assert_eq!(
            format_accounting_number(0.155, Some(FormatPreset::Percentage), Some(1), None),
            "15.5%"
        );
        assert_eq!(
            format_accounting_number(-0.05, Some(FormatPreset::Percentage), Some(2), None),
            "(5.00%)"
        );

        // Multiple
        assert_eq!(
            format_accounting_number(12.5, Some(FormatPreset::Multiple), Some(1), None),
            "12.5x"
        );

        // Basis Points
        assert_eq!(
            format_accounting_number(0.0075, Some(FormatPreset::BasisPoints), Some(0), None),
            "75 bps"
        );

        // Default number
        assert_eq!(
            format_accounting_number(1000000.0, Some(FormatPreset::Number), Some(0), None),
            "1,000,000"
        );
        assert_eq!(format_accounting_number(-42.0, None, None, None), "(42)");
    }
}
