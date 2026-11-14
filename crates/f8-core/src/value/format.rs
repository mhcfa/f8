//! Accounting typography and cell value formatting.

use std::iter::repeat_n;

use crate::model::format::{CurrencyCode, FormatPreset};
use crate::value::{CellValue, Decimal15, DecimalRoundingMode, ExcelError};

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
    if matches!(format, Some(FormatPreset::Date)) {
        return match crate::value::date::DateSerial::to_display_ymd(val) {
            Ok(dt) => format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day),
            Err(e) => e.as_str().to_string(),
        };
    }

    if !val.is_finite() {
        return ExcelError::Num.as_str().to_string();
    }

    let (dec_u32, auto_trim_zero_tenths) = match format {
        Some(FormatPreset::Percentage) | Some(FormatPreset::Multiple) => {
            (decimals.unwrap_or(1), false)
        }
        Some(FormatPreset::BasisPoints)
        | Some(FormatPreset::Currency)
        | Some(FormatPreset::Number) => (decimals.unwrap_or(0), false),
        Some(FormatPreset::Integer) => (0, false),
        Some(FormatPreset::Text) | Some(FormatPreset::Date) | None => match decimals {
            Some(d) => (d, false),
            None => (1, true),
        },
    };

    let (scale_exp10, suffix) = match format {
        Some(FormatPreset::Percentage) => (2, "%"),
        Some(FormatPreset::Multiple) => (0, "x"),
        Some(FormatPreset::BasisPoints) => (4, " bps"),
        Some(FormatPreset::Currency)
        | Some(FormatPreset::Number)
        | Some(FormatPreset::Integer)
        | Some(FormatPreset::Text)
        | Some(FormatPreset::Date)
        | None => (0, ""),
    };

    let (int_digits, mut frac_digits) = round_scaled_decimal(val, scale_exp10, dec_u32);
    if auto_trim_zero_tenths && frac_digits == "0" {
        frac_digits.clear();
    }

    let is_non_zero =
        int_digits.bytes().any(|b| b != b'0') || frac_digits.bytes().any(|b| b != b'0');
    let is_neg = val < 0.0 && is_non_zero;

    let grouped_int = if matches!(format, Some(FormatPreset::Integer)) {
        int_digits
    } else {
        format_int_with_commas(int_digits)
    };
    let formatted_num = if frac_digits.is_empty() {
        grouped_int
    } else {
        format!("{grouped_int}.{frac_digits}")
    };

    match format {
        Some(FormatPreset::Currency) => {
            let sym = currency.unwrap_or_default().symbol();
            if is_neg {
                format!("({sym}{formatted_num})")
            } else {
                format!("{sym}{formatted_num}")
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
        Some(FormatPreset::Number)
        | Some(FormatPreset::Integer)
        | Some(FormatPreset::Text)
        | Some(FormatPreset::Date)
        | None => {
            if is_neg {
                format!("({formatted_num})")
            } else {
                formatted_num
            }
        }
    }
}

fn format_int_with_commas(int_digits: String) -> String {
    if int_digits.len() <= 3 {
        return int_digits;
    }
    let bytes = int_digits.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + (len - 1) / 3);
    for (idx, &b) in bytes.iter().enumerate() {
        if idx > 0 && (len - idx).is_multiple_of(3) {
            out.push(',');
        }
        out.push(char::from(b));
    }
    out
}

fn zero_frac(decimals: usize) -> String {
    repeat_n('0', decimals).collect()
}

fn round_scaled_decimal(val: f64, scale_exp10: i32, decimals: u32) -> (String, String) {
    let dec_usize = decimals as usize;
    let Some(dec) = Decimal15::from_f64(val) else {
        return ("0".to_string(), zero_frac(dec_usize));
    };
    let Some(rounded) = dec.round_scaled(
        scale_exp10,
        i64::from(decimals),
        DecimalRoundingMode::HalfAway,
    ) else {
        return ("0".to_string(), zero_frac(dec_usize));
    };

    let mantissa = rounded.coeff.to_string();
    let m = rounded.num_digits as usize;
    if rounded.top_exp10 >= 0 {
        let int_len = (rounded.top_exp10 + 1) as usize;
        if let (Some(int_part), Some(frac_prefix)) =
            (mantissa.get(..int_len), mantissa.get(int_len..))
            && !frac_prefix.is_empty()
        {
            let mut frac_digits = String::with_capacity(dec_usize);
            frac_digits.push_str(frac_prefix);
            frac_digits.extend(repeat_n('0', dec_usize.saturating_sub(frac_prefix.len())));
            (int_part.to_string(), frac_digits)
        } else {
            let mut int_digits = mantissa;
            int_digits.extend(repeat_n('0', int_len.saturating_sub(m)));
            (int_digits, zero_frac(dec_usize))
        }
    } else {
        let leading_zeros = (-rounded.top_exp10 - 1) as usize;
        let trailing_zeros = dec_usize.saturating_sub(leading_zeros + m);
        let mut frac_digits = String::with_capacity(dec_usize);
        frac_digits.extend(repeat_n('0', leading_zeros));
        frac_digits.push_str(&mantissa);
        frac_digits.extend(repeat_n('0', trailing_zeros));
        ("0".to_string(), frac_digits)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]
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
            "$1,234.56"
        );
        assert_eq!(
            format_accounting_number(
                -1234.56,
                Some(FormatPreset::Currency),
                Some(2),
                Some(CurrencyCode::Usd)
            ),
            "($1,234.56)"
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

        // Integer preset (no commas even for large numbers or negatives)
        assert_eq!(
            format_accounting_number(2029.0, Some(FormatPreset::Integer), None, None),
            "2029"
        );
        assert_eq!(
            format_accounting_number(1000000.0, Some(FormatPreset::Integer), None, None),
            "1000000"
        );
        assert_eq!(
            format_accounting_number(-42.0, Some(FormatPreset::Integer), None, None),
            "(42)"
        );

        // Currency default decimals (0)
        assert_eq!(
            format_accounting_number(1250.0, Some(FormatPreset::Currency), None, None),
            "$1,250"
        );

        // Date preset (ISO YYYY-MM-DD, 1900-01-00 display parity, and out-of-bounds error)
        assert_eq!(
            format_accounting_number(0.0, Some(FormatPreset::Date), None, None),
            "1900-01-00"
        );
        assert_eq!(
            format_accounting_number(45657.0, Some(FormatPreset::Date), None, None),
            "2024-12-31"
        );
        assert_eq!(
            format_accounting_number(-1.0, Some(FormatPreset::Date), None, None),
            "#NUM!"
        );

        // Number preset always uses commas; Integer preset omits commas
        assert_eq!(
            format_accounting_number(2029.0, Some(FormatPreset::Number), Some(0), None),
            "2,029"
        );
        assert_eq!(format_accounting_number(1999.0, None, None, None), "1,999");
        assert_eq!(
            format_accounting_number(2024.5, Some(FormatPreset::Number), Some(1), None),
            "2,024.5"
        );

        // Near-zero negative values rounding to zero at target decimal precision
        assert_eq!(
            format_accounting_number(-0.05, Some(FormatPreset::Currency), Some(0), None),
            "$0"
        );
        assert_eq!(
            format_accounting_number(-0.001, Some(FormatPreset::Number), Some(2), None),
            "0.00"
        );
    }
}
