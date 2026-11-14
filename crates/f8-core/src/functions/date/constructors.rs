//! Date constructor and monthly stepping functions (`DATE`, `EDATE`, `EOMONTH`).

use crate::functions::common::{extract_scalar_integer, extract_scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ValidationPolicy,
};
use crate::value::{CellValue, DateSerial, ExcelError, ValueType, validate_date_serial};

static DATE_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "year",
        "The 4-digit calendar year of the date (1900-9999).",
        ParamKind::Scalar,
        ParamType::Integer,
    ),
    ParameterDef::required(
        "month",
        "The calendar month of the year (1-12).",
        ParamKind::Scalar,
        ParamType::Integer,
    ),
    ParameterDef::required(
        "day",
        "The calendar day of the month (1..=days_in_month).",
        ParamKind::Scalar,
        ParamType::Integer,
    ),
];

/// `DATE(year, month, day)`: Constructs an Excel serial date number from strict calendar year, month, and day components.
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

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::CalendarDate
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let year_res = extract_scalar_integer(&args[0], self.name(), "year", 1, ctx)?;
        let month_res = extract_scalar_integer(&args[1], self.name(), "month", 2, ctx)?;
        let day_res = extract_scalar_integer(&args[2], self.name(), "day", 3, ctx)?;

        let year_i64 = match year_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let month_i64 = match month_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let day_i64 = match day_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let Ok(y) = i32::try_from(year_i64) else {
            return Ok(CellValue::Error(ExcelError::Num));
        };
        let Ok(m) = u32::try_from(month_i64) else {
            return Ok(CellValue::Error(ExcelError::Num));
        };
        let Ok(d) = u32::try_from(day_i64) else {
            return Ok(CellValue::Error(ExcelError::Num));
        };

        match DateSerial::from_ymd(y, m, d) {
            Ok(serial) => Ok(CellValue::number(serial)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

static EDATE_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "start_date",
        "A serial number that represents the start date.",
        ParamKind::Scalar,
        ParamType::DateSerial,
    ),
    ParameterDef::required(
        "months",
        "The integer number of months before or after start_date.",
        ParamKind::Scalar,
        ParamType::Integer,
    ),
];

/// `EDATE(start_date, months)`: Returns serial date offset by `months` calendar months with month-end clamping.
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

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let start_res = extract_scalar_number(&args[0], self.name(), "start_date", 1, ctx)?;
        let months_res = extract_scalar_integer(&args[1], self.name(), "months", 2, ctx)?;

        let start_val = match start_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let m_offset = match months_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if m_offset.abs() > 120_000 {
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

        let total_months = i64::from(date.year) * 12 + (i64::from(date.month) - 1) + m_offset;
        let target_year_i64 = total_months.div_euclid(12);
        if !(1900..=9999).contains(&target_year_i64) {
            return Ok(CellValue::Error(ExcelError::Num));
        }
        let target_year = target_year_i64 as i32;
        let target_month = (total_months.rem_euclid(12) + 1) as u32;

        let max_day = DateSerial::gregorian_days_in_month(target_year, target_month);
        let target_day = date.day.min(max_day);

        match DateSerial::from_ymd(target_year, target_month, target_day) {
            Ok(serial) => Ok(CellValue::number(serial)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

static EOMONTH_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "start_date",
        "A serial number that represents the start date.",
        ParamKind::Scalar,
        ParamType::DateSerial,
    ),
    ParameterDef::required(
        "months",
        "The integer number of months before or after start_date.",
        ParamKind::Scalar,
        ParamType::Integer,
    ),
];

/// `EOMONTH(start_date, months)`: Returns serial date of the last day of the month offset by `months` calendar months.
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

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let start_res = extract_scalar_number(&args[0], self.name(), "start_date", 1, ctx)?;
        let months_res = extract_scalar_integer(&args[1], self.name(), "months", 2, ctx)?;

        let start_val = match start_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let m_offset = match months_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if m_offset.abs() > 120_000 {
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

        let total_months = i64::from(date.year) * 12 + (i64::from(date.month) - 1) + m_offset;
        let target_year_i64 = total_months.div_euclid(12);
        if !(1900..=9999).contains(&target_year_i64) {
            return Ok(CellValue::Error(ExcelError::Num));
        }
        let target_year = target_year_i64 as i32;
        let target_month = (total_months.rem_euclid(12) + 1) as u32;

        let max_day = DateSerial::gregorian_days_in_month(target_year, target_month);

        match DateSerial::from_ymd(target_year, target_month, max_day) {
            Ok(serial) => Ok(CellValue::number(serial)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}
