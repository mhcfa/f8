//! Excel 1900 calendar date serial bounds and conversion routines.

use crate::value::ExcelError;

/// Serial day number for Unix epoch (`1970-01-01`).
pub const UNIX_EPOCH_SERIAL: f64 = 25569.0;

/// Minimum valid input calendar date serial (`1900-01-01`).
pub const MIN_INPUT_DATE_SERIAL: f64 = 1.0;

/// Maximum valid Excel serial day number (`9999-12-31`).
pub const MAX_EXCEL_SERIAL: f64 = 2958465.0;

/// Fictitious Excel leap day serial number (`1900-02-29`).
pub const EXCEL_1900_LEAP_DAY_SERIAL: f64 = 60.0;

/// Returns `true` when `serial` is a finite input calendar date serial in `1.0..=2_958_465.0`
/// (permitting intra-day fractional serials up to `2958465.999...`).
#[inline]
#[must_use]
pub fn is_valid_input_date_serial(serial: f64) -> bool {
    serial.is_finite() && serial >= MIN_INPUT_DATE_SERIAL && serial.floor() <= MAX_EXCEL_SERIAL
}

/// Validates that an input Excel serial date number lies within `1.0..=2_958_465.0`
/// and floors valid serials to integer day counts.
///
/// Rejects `NaN`, infinities, negative serials, sub-`1.0` serials (`1900-01-00`),
/// and serials beyond `9999-12-31` (`>= 2_958_466.0`).
#[inline]
pub fn validate_date_serial(serial: f64) -> Result<f64, ExcelError> {
    if !is_valid_input_date_serial(serial) {
        Err(ExcelError::Num)
    } else {
        Ok(serial.floor())
    }
}

/// Represents a decomposed calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTuple {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

/// Core calendar and Excel 1900 serial date conversion routines.
pub struct DateSerial;

impl DateSerial {
    /// Checks whether a year is a leap year under the standard Gregorian calendar rules.
    #[inline]
    #[must_use]
    pub fn is_gregorian_leap_year(year: i32) -> bool {
        year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    }

    /// Checks whether a year is treated as a leap year under Excel's 1900 date system.
    ///
    /// Replicates the Lotus 1-2-3 bug where 1900 is treated as a leap year.
    #[inline]
    #[must_use]
    pub fn is_excel_leap_year(year: i32) -> bool {
        if year == 1900 {
            true
        } else {
            Self::is_gregorian_leap_year(year)
        }
    }

    /// Returns the number of days in a given year under Excel's date system.
    #[inline]
    #[must_use]
    pub fn days_in_year(year: i32) -> u32 {
        if Self::is_excel_leap_year(year) {
            366
        } else {
            365
        }
    }

    /// Returns the number of days in a given calendar month of a given year (1-based month).
    #[must_use]
    pub fn days_in_month(year: i32, month: u32) -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Self::is_excel_leap_year(year) {
                    29
                } else {
                    28
                }
            }
            _ => 0,
        }
    }

    /// Returns the number of days in a given calendar month under standard Gregorian leap-year rules (`1900-02` -> `28`).
    #[must_use]
    pub fn gregorian_days_in_month(year: i32, month: u32) -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Self::is_gregorian_leap_year(year) {
                    29
                } else {
                    28
                }
            }
            _ => 0,
        }
    }

    /// Counts Gregorian leap years strictly between year 1901 and `end_year` inclusive.
    fn count_gregorian_leaps_since_1900(end_year: i32) -> i64 {
        if end_year < 1901 {
            return 0;
        }
        let y = end_year as i64;
        (y / 4 - y / 100 + y / 400) - (1900 / 4 - 1900 / 100 + 1900 / 400)
    }

    /// Returns the cumulative Gregorian days from `1900-01-01` to `year-01-01`.
    fn gregorian_days_to_year_start(year: i32) -> i64 {
        if year <= 1900 {
            0
        } else {
            let elapsed_years = (year - 1900) as i64;
            let leap_days = Self::count_gregorian_leaps_since_1900(year - 1);
            elapsed_years * 365 + leap_days
        }
    }

    /// Converts a strict 4-digit calendar year (`1900..=9999`), month (`1..=12`), and day (`1..=days_in_month`)
    /// into an Excel serial day number without two-digit year expansion or month/day rollover.
    ///
    /// Preserves the Lotus 1-2-3 / Excel 1900 leap year bug (`1900-02-29` -> `60.0`).
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Result<f64, ExcelError> {
        if !(1900..=9999).contains(&year) || !(1..=12).contains(&month) {
            return Err(ExcelError::Num);
        }

        let max_day = Self::days_in_month(year, month);
        if day < 1 || day > max_day {
            return Err(ExcelError::Num);
        }

        let base_serial = Self::first_of_month_serial(year, month)?;
        let final_serial = base_serial + f64::from(day - 1);

        if !(MIN_INPUT_DATE_SERIAL..=MAX_EXCEL_SERIAL).contains(&final_serial) {
            return Err(ExcelError::Num);
        }

        Ok(final_serial)
    }

    /// Computes the Excel serial number for the first day of a normalized month (`year-month-01`).
    fn first_of_month_serial(year: i32, month: u32) -> Result<f64, ExcelError> {
        if year == 1900 {
            let serial = match month {
                1 => 1.0,  // 1900-01-01
                2 => 32.0, // 1900-02-01
                _ => {
                    // Month >= 3 in 1900: days before month in 1900 (treating Feb as 29) + 1
                    let mut days = 0;
                    for m in 1..month {
                        days += Self::days_in_month(1900, m);
                    }
                    days as f64 + 1.0
                }
            };
            return Ok(serial);
        }

        // For year > 1900, all dates fall after 1900-02-28 and carry the +2 offset relative to true Gregorian 1900-01-01
        let greg_year_days = Self::gregorian_days_to_year_start(year);
        let mut days_in_year_before_month = 0;
        for m in 1..month {
            days_in_year_before_month += Self::gregorian_days_in_month(year, m);
        }

        let total_greg_days = greg_year_days + days_in_year_before_month as i64;
        // Excel serial = gregorian_days + 2
        Ok((total_greg_days + 2) as f64)
    }

    /// Converts an input Excel serial date (`1.0..=2_958_465.0`) into a decomposed calendar date.
    ///
    /// Truncates fractional day times to integer day counts and rejects `< 1.0` (including `0.0`).
    pub fn to_ymd(serial: f64) -> Result<DateTuple, ExcelError> {
        let s_f64 = validate_date_serial(serial)?;
        let s = s_f64 as i64;

        // Serial 60 is the fictitious Lotus 1-2-3 / Excel leap day: 1900-02-29
        if s == EXCEL_1900_LEAP_DAY_SERIAL as i64 {
            return Ok(DateTuple {
                year: 1900,
                month: 2,
                day: 29,
            });
        }

        // Dates in Jan and Feb 1900 (serials 1..=59)
        if s < 60 {
            return if s <= 31 {
                Ok(DateTuple {
                    year: 1900,
                    month: 1,
                    day: s as u32,
                })
            } else {
                Ok(DateTuple {
                    year: 1900,
                    month: 2,
                    day: (s - 31) as u32,
                })
            };
        }

        // For serials > 60: true astronomical Gregorian days from 1900-01-01 is (s - 2)
        let gregorian_days = s - 2;

        // Underestimate year and bracket forward
        let mut year = 1900 + (gregorian_days / 366) as i32;
        while Self::gregorian_days_to_year_start(year + 1) <= gregorian_days {
            year += 1;
        }

        let mut rem_days = gregorian_days - Self::gregorian_days_to_year_start(year);

        let mut month = 1;
        for m in 1..=12 {
            let dim = Self::gregorian_days_in_month(year, m);
            if rem_days < dim as i64 {
                month = m;
                break;
            }
            rem_days -= dim as i64;
        }

        let day = (rem_days + 1) as u32;

        Ok(DateTuple { year, month, day })
    }

    /// Converts a display serial (`0.0..=2_958_465.0`) into a decomposed calendar date solely for
    /// Tier-2 `FormatPreset::Date` formatting, preserving Excel's `0.0 -> 1900-01-00` display parity.
    pub(crate) fn to_display_ymd(serial: f64) -> Result<DateTuple, ExcelError> {
        if !serial.is_finite() || serial < 0.0 || serial.floor() > MAX_EXCEL_SERIAL {
            return Err(ExcelError::Num);
        }
        if serial < MIN_INPUT_DATE_SERIAL {
            return Ok(DateTuple {
                year: 1900,
                month: 1,
                day: 0,
            });
        }
        Self::to_ymd(serial)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test code")]
mod tests {
    use super::*;

    #[test]
    fn test_excel_epoch_and_1900_leap_day() {
        assert_eq!(DateSerial::from_ymd(1900, 1, 1).unwrap(), 1.0);
        assert_eq!(DateSerial::from_ymd(1900, 2, 28).unwrap(), 59.0);
        assert_eq!(DateSerial::from_ymd(1900, 2, 29).unwrap(), 60.0);
        assert_eq!(DateSerial::from_ymd(1900, 3, 1).unwrap(), 61.0);

        assert_eq!(
            DateSerial::to_ymd(1.0).unwrap(),
            DateTuple {
                year: 1900,
                month: 1,
                day: 1
            }
        );
        assert_eq!(
            DateSerial::to_ymd(59.0).unwrap(),
            DateTuple {
                year: 1900,
                month: 2,
                day: 28
            }
        );
        assert_eq!(
            DateSerial::to_ymd(60.0).unwrap(),
            DateTuple {
                year: 1900,
                month: 2,
                day: 29
            }
        );
        assert_eq!(
            DateSerial::to_ymd(61.0).unwrap(),
            DateTuple {
                year: 1900,
                month: 3,
                day: 1
            }
        );
    }

    #[test]
    fn test_unix_epoch_and_modern_dates() {
        assert_eq!(DateSerial::from_ymd(1970, 1, 1).unwrap(), 25569.0);
        assert_eq!(
            DateSerial::to_ymd(25569.0).unwrap(),
            DateTuple {
                year: 1970,
                month: 1,
                day: 1
            }
        );

        // 2024-02-29 (leap day in modern calendar)
        let leap_serial = DateSerial::from_ymd(2024, 2, 29).unwrap();
        assert_eq!(
            DateSerial::to_ymd(leap_serial).unwrap(),
            DateTuple {
                year: 2024,
                month: 2,
                day: 29
            }
        );

        // Maximum date: 9999-12-31
        assert_eq!(DateSerial::from_ymd(9999, 12, 31).unwrap(), 2958465.0);
        assert_eq!(
            DateSerial::to_ymd(2958465.0).unwrap(),
            DateTuple {
                year: 9999,
                month: 12,
                day: 31
            }
        );
    }

    #[test]
    fn test_strict_from_ymd_rejects_rollover_and_short_years() {
        assert_eq!(DateSerial::from_ymd(0, 1, 1), Err(ExcelError::Num));
        assert_eq!(DateSerial::from_ymd(24, 12, 31), Err(ExcelError::Num));
        assert_eq!(DateSerial::from_ymd(100, 1, 1), Err(ExcelError::Num));
        assert_eq!(DateSerial::from_ymd(1899, 12, 31), Err(ExcelError::Num));
        assert_eq!(DateSerial::from_ymd(10000, 1, 1), Err(ExcelError::Num));

        assert_eq!(DateSerial::from_ymd(2024, 0, 1), Err(ExcelError::Num));
        assert_eq!(DateSerial::from_ymd(2024, 13, 1), Err(ExcelError::Num));

        assert_eq!(DateSerial::from_ymd(2024, 1, 0), Err(ExcelError::Num));
        assert_eq!(DateSerial::from_ymd(2024, 1, 32), Err(ExcelError::Num));
        assert_eq!(DateSerial::from_ymd(2024, 6, 31), Err(ExcelError::Num));
        assert_eq!(DateSerial::from_ymd(2025, 2, 29), Err(ExcelError::Num));
    }

    #[test]
    fn test_input_date_serial_bounds() {
        assert!(!is_valid_input_date_serial(0.0));
        assert!(!is_valid_input_date_serial(0.5));
        assert!(!is_valid_input_date_serial(-1.0));
        assert!(!is_valid_input_date_serial(f64::NAN));
        assert!(!is_valid_input_date_serial(f64::INFINITY));
        assert!(!is_valid_input_date_serial(2958466.0));
        assert!(is_valid_input_date_serial(1.0));
        assert!(is_valid_input_date_serial(60.0));
        assert!(is_valid_input_date_serial(2958465.0));
        assert!(is_valid_input_date_serial(2958465.9));
    }

    #[test]
    fn test_validate_date_serial_vs_display_ymd() {
        // Negative, sub-1.0, and non-finite serials are rejected by validate_date_serial & to_ymd
        for bad in [-1.0, -0.5, -0.0001, 0.0, 0.5, f64::NAN, f64::INFINITY] {
            assert_eq!(validate_date_serial(bad), Err(ExcelError::Num));
            assert_eq!(DateSerial::to_ymd(bad), Err(ExcelError::Num));
        }

        // Display helper preserves 0.0..1.0 -> 1900-01-00 while rejecting < 0.0 and > MAX
        assert_eq!(
            DateSerial::to_display_ymd(0.0).unwrap(),
            DateTuple {
                year: 1900,
                month: 1,
                day: 0
            }
        );
        assert_eq!(
            DateSerial::to_display_ymd(0.75).unwrap(),
            DateTuple {
                year: 1900,
                month: 1,
                day: 0
            }
        );
        assert_eq!(DateSerial::to_display_ymd(-0.001), Err(ExcelError::Num));
        assert_eq!(DateSerial::to_display_ymd(2958466.0), Err(ExcelError::Num));

        // Fractional times on maximum date (9999-12-31 at 6:00 PM) are accepted
        assert_eq!(validate_date_serial(2958465.75).unwrap(), 2958465.0);
        assert_eq!(
            DateSerial::to_ymd(2958465.75).unwrap(),
            DateTuple {
                year: 9999,
                month: 12,
                day: 31
            }
        );

        // Numbers >= 2958466.0 (year >= 10000) are rejected
        assert_eq!(validate_date_serial(2958466.0), Err(ExcelError::Num));
        assert_eq!(DateSerial::to_ymd(2958466.0), Err(ExcelError::Num));
    }

    #[test]
    fn test_days_in_month_vs_gregorian_days_in_month() {
        // 1900 split: Excel calendar preserves 29 days in Feb 1900, Gregorian uses 28
        assert_eq!(DateSerial::days_in_month(1900, 2), 29);
        assert_eq!(DateSerial::gregorian_days_in_month(1900, 2), 28);

        // Standard leap and non-leap years agree across both helpers
        assert_eq!(DateSerial::days_in_month(2000, 2), 29);
        assert_eq!(DateSerial::gregorian_days_in_month(2000, 2), 29);
        assert_eq!(DateSerial::days_in_month(2024, 2), 29);
        assert_eq!(DateSerial::gregorian_days_in_month(2024, 2), 29);
        assert_eq!(DateSerial::days_in_month(2023, 2), 28);
        assert_eq!(DateSerial::gregorian_days_in_month(2023, 2), 28);
        assert_eq!(DateSerial::days_in_month(2100, 2), 28);
        assert_eq!(DateSerial::gregorian_days_in_month(2100, 2), 28);
    }
}
