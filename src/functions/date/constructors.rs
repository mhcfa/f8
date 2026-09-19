//! Date constructor and monthly stepping functions (`DATE`, `EDATE`, `EOMONTH`).

use crate::functions::common::extract_scalar_number;
use crate::functions::date::serial::{DateSerial, validate_date_serial};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
};
use crate::value::{CellValue, ExcelError};

static DATE_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "year",
        "The year of the date (0-9999; 0-1899 adds 1900).",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "month",
        "The month of the year (1-12; rolls over if outside 1-12).",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "day",
        "The day of the month (1-31; rolls over if outside month days).",
        ParamKind::Scalar,
    ),
];

/// `DATE(year, month, day)`: Constructs an Excel serial date number from year, month, and day components.
#[derive(Debug, Default, Clone, Copy)]
pub struct DateFunction;

impl ExcelFunction for DateFunction {
    fn name(&self) -> &'static str {
        "DATE"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::DateTime
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        DATE_PARAMS
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _context: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 3 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 3,
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

        let year_val = match extract_scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let month_val = match extract_scalar_number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let day_val = match extract_scalar_number(&args[2]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if !year_val.is_finite() || !month_val.is_finite() || !day_val.is_finite() {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        if year_val < -10_000.0
            || year_val > 20_000.0
            || month_val.abs() > 1_000_000.0
            || day_val.abs() > 10_000_000.0
        {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let y = year_val.trunc() as i32;
        let m = month_val.trunc() as i32;
        let d = day_val.trunc() as i32;

        match DateSerial::from_ymd(y, m, d) {
            Ok(serial) => Ok(CellValue::Number(serial)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

static EDATE_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "start_date",
        "A serial number that represents the start date.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "months",
        "The number of months before or after start_date.",
        ParamKind::Scalar,
    ),
];

/// `EDATE(start_date, months)`: Returns serial date offset by $n$ calendar months with month-end clamping.
#[derive(Debug, Default, Clone, Copy)]
pub struct EdateFunction;

impl ExcelFunction for EdateFunction {
    fn name(&self) -> &'static str {
        "EDATE"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::DateTime
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        EDATE_PARAMS
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
        if args.len() > 2 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }

        let start_val = match extract_scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let months_val = match extract_scalar_number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if !months_val.is_finite() || months_val.abs() > 120_000.0 {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let s = match validate_date_serial(start_val) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let date = match DateSerial::to_ymd(s) {
            Ok(d) => d,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let m_offset = months_val.trunc() as i64;
        let total_months = (date.year as i64) * 12 + (date.month as i64 - 1) + m_offset;
        let target_year_i64 = total_months.div_euclid(12);
        if !(1900..=9999).contains(&target_year_i64) {
            return Ok(CellValue::Error(ExcelError::Num));
        }
        let target_year = target_year_i64 as i32;
        let target_month = (total_months.rem_euclid(12) + 1) as u32;

        let max_day = DateSerial::days_in_month(target_year, target_month);
        let target_day = date.day.min(max_day);

        match DateSerial::from_ymd(target_year, target_month as i32, target_day as i32) {
            Ok(serial) => Ok(CellValue::Number(serial)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

static EOMONTH_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "start_date",
        "A serial number that represents the start date.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "months",
        "The number of months before or after start_date.",
        ParamKind::Scalar,
    ),
];

/// `EOMONTH(start_date, months)`: Returns serial date of the last day of the month offset by $n$ months.
#[derive(Debug, Default, Clone, Copy)]
pub struct EomonthFunction;

impl ExcelFunction for EomonthFunction {
    fn name(&self) -> &'static str {
        "EOMONTH"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::DateTime
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        EOMONTH_PARAMS
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
        if args.len() > 2 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }

        let start_val = match extract_scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let months_val = match extract_scalar_number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if !months_val.is_finite() || months_val.abs() > 120_000.0 {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let s = match validate_date_serial(start_val) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let date = match DateSerial::to_ymd(s) {
            Ok(d) => d,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let m_offset = months_val.trunc() as i64;
        let total_months = (date.year as i64) * 12 + (date.month as i64 - 1) + m_offset;
        let target_year_i64 = total_months.div_euclid(12);
        if !(1900..=9999).contains(&target_year_i64) {
            return Ok(CellValue::Error(ExcelError::Num));
        }
        let target_year = target_year_i64 as i32;
        let target_month = (total_months.rem_euclid(12) + 1) as u32;

        let max_day = DateSerial::days_in_month(target_year, target_month);

        match DateSerial::from_ymd(target_year, target_month as i32, max_day as i32) {
            Ok(serial) => Ok(CellValue::Number(serial)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}
