use crate::functions::common::extract_scalar_number;
use crate::functions::date::serial::{DateSerial, validate_date_serial};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
};
use crate::value::{CellValue, ExcelError};

/// Supported financial day-count conventions for `YEARFRAC`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayCountBasis {
    /// US (NASD) 30/360 (basis: 0).
    Us30_360 = 0,
    /// Actual/Actual (basis: 1).
    ActualActual = 1,
    /// Actual/360 (basis: 2).
    Actual360 = 2,
    /// Actual/365 (basis: 3).
    Actual365 = 3,
    /// European 30/360 (basis: 4).
    European30_360 = 4,
}

impl DayCountBasis {
    /// Parses an integer basis value into a `DayCountBasis`.
    pub fn from_i64(val: i64) -> Result<Self, ExcelError> {
        match val {
            0 => Ok(Self::Us30_360),
            1 => Ok(Self::ActualActual),
            2 => Ok(Self::Actual360),
            3 => Ok(Self::Actual365),
            4 => Ok(Self::European30_360),
            _ => Err(ExcelError::Num),
        }
    }
}

/// Calculates the year fraction between two serial dates according to a day-count convention.
pub fn calculate_yearfrac(
    start_date: f64,
    end_date: f64,
    basis: DayCountBasis,
) -> Result<f64, ExcelError> {
    let s1 = validate_date_serial(start_date)?;
    let s2 = validate_date_serial(end_date)?;

    if s1 == s2 {
        return Ok(0.0);
    }

    // Order independence: compute fraction between smaller and larger date
    let (d1, d2) = if s1 <= s2 { (s1, s2) } else { (s2, s1) };

    let date1 = DateSerial::to_ymd(d1)?;
    let date2 = DateSerial::to_ymd(d2)?;

    match basis {
        DayCountBasis::Us30_360 => {
            let y1 = date1.year;
            let m1 = date1.month as i32;
            let mut d1_day = date1.day as i32;

            let y2 = date2.year;
            let m2 = date2.month as i32;
            let mut d2_day = date2.day as i32;

            let is_d1_last_of_feb = m1 == 2 && d1_day == DateSerial::days_in_month(y1, 2) as i32;
            let is_d2_last_of_feb = m2 == 2 && d2_day == DateSerial::days_in_month(y2, 2) as i32;

            if is_d1_last_of_feb {
                d1_day = 30;
                if is_d2_last_of_feb {
                    d2_day = 30;
                }
            } else if d1_day == 31 {
                d1_day = 30;
            }

            if d2_day == 31 && d1_day >= 30 {
                d2_day = 30;
            }

            let days = 360 * (y2 - y1) + 30 * (m2 - m1) + (d2_day - d1_day);
            Ok(days as f64 / 360.0)
        }

        DayCountBasis::ActualActual => {
            let actual_days = d2 - d1;
            let y1 = date1.year;
            let y2 = date2.year;

            let divisor = if y1 == y2 {
                if DateSerial::is_excel_leap_year(y1) {
                    366.0
                } else {
                    365.0
                }
            } else if y2 - y1 == 1
                && (date1.month > date2.month
                    || (date1.month == date2.month && date1.day >= date2.day))
            {
                // Spans <= 1 calendar year across a year boundary
                let has_leap_day = if DateSerial::is_excel_leap_year(y1) && date1.month <= 2 {
                    true
                } else {
                    DateSerial::is_excel_leap_year(y2)
                        && (date2.month > 2 || (date2.month == 2 && date2.day == 29))
                };
                if has_leap_day { 366.0 } else { 365.0 }
            } else {
                // Spans > 1 calendar year: average days per year across all included calendar years
                let total_years = (y2 - y1 + 1) as f64;
                let mut total_days = 0.0;
                for y in y1..=y2 {
                    total_days += DateSerial::days_in_year(y) as f64;
                }
                total_days / total_years
            };

            Ok(actual_days / divisor)
        }

        DayCountBasis::Actual360 => {
            let actual_days = d2 - d1;
            Ok(actual_days / 360.0)
        }

        DayCountBasis::Actual365 => {
            let actual_days = d2 - d1;
            Ok(actual_days / 365.0)
        }

        DayCountBasis::European30_360 => {
            let y1 = date1.year;
            let m1 = date1.month as i32;
            let mut d1_day = date1.day as i32;

            let y2 = date2.year;
            let m2 = date2.month as i32;
            let mut d2_day = date2.day as i32;

            if d1_day == 31 {
                d1_day = 30;
            }
            if d2_day == 31 {
                d2_day = 30;
            }

            let days = 360 * (y2 - y1) + 30 * (m2 - m1) + (d2_day - d1_day);
            Ok(days as f64 / 360.0)
        }
    }
}

static YEARFRAC_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "start_date",
        "A serial number that represents the start date.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "end_date",
        "A serial number that represents the end date.",
        ParamKind::Scalar,
    ),
    ParameterDef {
        name: "basis",
        description: "The type of day count basis to use (0: US 30/360, 1: Actual/Actual, 2: Actual/360, 3: Actual/365, 4: European 30/360).",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(0.0)),
    },
];

/// `YEARFRAC(start_date, end_date, [basis])`: Calculates the fraction of the year represented by the number of whole days between two dates.
#[derive(Debug, Default, Clone, Copy)]
pub struct YearfracFunction;

impl ExcelFunction for YearfracFunction {
    fn name(&self) -> &'static str {
        "YEARFRAC"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::DateTime
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        YEARFRAC_PARAMS
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _context: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 2 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }
        if args.len() > 3 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        }

        let start_val = match extract_scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let end_val = match extract_scalar_number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let basis_val: f64 = if args.len() > 2 && !matches!(args[2], FunctionArg::Omitted) {
            match extract_scalar_number(&args[2]) {
                Ok(v) => v,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.0
        };

        if !basis_val.is_finite() || basis_val < 0.0 || basis_val >= 5.0 {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let basis_i64 = basis_val.trunc() as i64;
        let basis = match DayCountBasis::from_i64(basis_i64) {
            Ok(b) => b,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match calculate_yearfrac(start_val, end_val, basis) {
            Ok(frac) => Ok(CellValue::Number(frac)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}
