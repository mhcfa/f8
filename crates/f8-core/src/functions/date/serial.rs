pub use crate::functions::common::extract_scalar_number;
use crate::value::ExcelError;

/// Serial day number for Unix epoch (`1970-01-01`).
pub const UNIX_EPOCH_SERIAL: f64 = 25569.0;

/// Minimum valid Excel serial day number (`1900-01-00`).
pub const MIN_EXCEL_SERIAL: f64 = 0.0;

/// Maximum valid Excel serial day number (`9999-12-31`).
pub const MAX_EXCEL_SERIAL: f64 = 2958465.0;

/// Fictitious Excel leap day serial number (`1900-02-29`).
pub const EXCEL_1900_LEAP_DAY_SERIAL: f64 = 60.0;

/// Validates that an Excel serial date number is within the valid date bounds (`0.0..=2958465.0`).
///
/// Disallows NaN and negative numbers (e.g. `-0.5`).
/// Floors valid serials to integer day counts so that timestamps on the maximum date
/// (`9999-12-31`, e.g. `2958465.9`) are accepted.
#[inline]
pub fn validate_date_serial(serial: f64) -> Result<f64, ExcelError> {
    if serial.is_nan() || serial < MIN_EXCEL_SERIAL || serial.floor() > MAX_EXCEL_SERIAL {
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
    pub fn is_gregorian_leap_year(year: i32) -> bool {
        year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    }

    /// Checks whether a year is treated as a leap year under Excel's 1900 date system.
    ///
    /// Replicates the Lotus 1-2-3 bug where 1900 is treated as a leap year.
    #[inline]
    pub fn is_excel_leap_year(year: i32) -> bool {
        if year == 1900 {
            true
        } else {
            Self::is_gregorian_leap_year(year)
        }
    }

    /// Returns the number of days in a given year under Excel's date system.
    #[inline]
    pub fn days_in_year(year: i32) -> u32 {
        if Self::is_excel_leap_year(year) {
            366
        } else {
            365
        }
    }

    /// Returns the number of days in a given calendar month of a given year (1-based month).
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

    /// Converts a year, month, and day into an Excel serial day number.
    ///
    /// Supports:
    /// - 2-digit / offset years: $0 \le \text{year} \le 1899 \rightarrow \text{year} + 1900$.
    /// - Out-of-range months and days: propagates integer rollovers across month and year boundaries.
    /// - Lotus 1-2-3 / Excel 1900 leap year bug: serial `60.0 = 1900-02-29`.
    pub fn from_ymd(raw_year: i32, raw_month: i32, raw_day: i32) -> Result<f64, ExcelError> {
        // Expand 2-digit / offset years: 0..=1899 -> +1900
        let year = if (0..=1899).contains(&raw_year) {
            raw_year + 1900
        } else {
            raw_year
        };

        if !(1900..=9999).contains(&year) {
            return Err(ExcelError::Num);
        }

        // Normalize month rollover (1-based to 0-based euclidean division)
        let month_offset = (raw_month as i64) - 1;
        let year_adj = month_offset.div_euclid(12);
        let normalized_month = (month_offset.rem_euclid(12) + 1) as u32;
        let normalized_year = (year as i64) + year_adj;

        if !(1900..=9999).contains(&normalized_year) {
            return Err(ExcelError::Num);
        }
        let norm_year_i32 = normalized_year as i32;

        // Base serial for normalized_year-normalized_month-01
        let base_serial = Self::first_of_month_serial(norm_year_i32, normalized_month)?;

        // Apply day offset: day 1 is already included in base_serial
        let final_serial = base_serial + (raw_day as f64 - 1.0);

        if !(MIN_EXCEL_SERIAL..=MAX_EXCEL_SERIAL).contains(&final_serial) {
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
        let is_leap = Self::is_gregorian_leap_year(year);
        for m in 1..month {
            let dim = match m {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 => {
                    if is_leap {
                        29
                    } else {
                        28
                    }
                }
                _ => 0,
            };
            days_in_year_before_month += dim;
        }

        let total_greg_days = greg_year_days + days_in_year_before_month as i64;
        // Excel serial = gregorian_days + 2
        Ok((total_greg_days + 2) as f64)
    }

    /// Converts an Excel serial date number into a decomposed calendar date.
    ///
    /// Truncates fractional day times to integer day counts.
    pub fn to_ymd(serial: f64) -> Result<DateTuple, ExcelError> {
        let s_f64 = validate_date_serial(serial)?;
        let s = s_f64 as i64;

        // Excel day 0 is 1900-01-00
        if s == 0 {
            return Ok(DateTuple {
                year: 1900,
                month: 1,
                day: 0,
            });
        }

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
        let is_leap = Self::is_gregorian_leap_year(year);

        let mut month = 1;
        for m in 1..=12 {
            let dim = match m {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 => {
                    if is_leap {
                        29
                    } else {
                        28
                    }
                }
                _ => 30,
            };
            if rem_days < dim as i64 {
                month = m;
                break;
            }
            rem_days -= dim as i64;
        }

        let day = (rem_days + 1) as u32;

        Ok(DateTuple { year, month, day })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
    fn test_month_and_day_rollovers() {
        // 2024-13-01 -> 2025-01-01
        assert_eq!(
            DateSerial::from_ymd(2024, 13, 1).unwrap(),
            DateSerial::from_ymd(2025, 1, 1).unwrap()
        );

        // 2024-01-32 -> 2024-02-01
        assert_eq!(
            DateSerial::from_ymd(2024, 1, 32).unwrap(),
            DateSerial::from_ymd(2024, 2, 1).unwrap()
        );

        // 2024-01-00 -> 2023-12-31
        assert_eq!(
            DateSerial::from_ymd(2024, 1, 0).unwrap(),
            DateSerial::from_ymd(2023, 12, 31).unwrap()
        );

        // 2024-00-01 -> 2023-12-01
        assert_eq!(
            DateSerial::from_ymd(2024, 0, 1).unwrap(),
            DateSerial::from_ymd(2023, 12, 1).unwrap()
        );
    }

    #[test]
    fn test_two_digit_years() {
        // 100 -> 2000
        assert_eq!(
            DateSerial::from_ymd(100, 1, 1).unwrap(),
            DateSerial::from_ymd(2000, 1, 1).unwrap()
        );
        // 0 -> 1900
        assert_eq!(DateSerial::from_ymd(0, 1, 1).unwrap(), 1.0);
    }

    #[test]
    fn test_validate_date_serial_and_boundary_fractions() {
        // Negative serial numbers must be rejected
        assert_eq!(validate_date_serial(-0.5), Err(ExcelError::Num));
        assert_eq!(validate_date_serial(-0.0001), Err(ExcelError::Num));
        assert_eq!(validate_date_serial(-1.0), Err(ExcelError::Num));
        assert_eq!(DateSerial::to_ymd(-0.5), Err(ExcelError::Num));
        assert_eq!(DateSerial::to_ymd(-0.001), Err(ExcelError::Num));

        // NaN must be rejected
        assert_eq!(validate_date_serial(f64::NAN), Err(ExcelError::Num));

        // Boundary: 0.0 is valid
        assert_eq!(validate_date_serial(0.0).unwrap(), 0.0);

        // Fractional times on maximum date (9999-12-31 at 6:00 PM) must be accepted
        assert_eq!(validate_date_serial(2958465.75).unwrap(), 2958465.0);
        assert_eq!(
            DateSerial::to_ymd(2958465.75).unwrap(),
            DateTuple {
                year: 9999,
                month: 12,
                day: 31
            }
        );

        // Numbers >= 2958466.0 (year >= 10000) must be rejected
        assert_eq!(validate_date_serial(2958466.0), Err(ExcelError::Num));
        assert_eq!(DateSerial::to_ymd(2958466.0), Err(ExcelError::Num));
    }
}
