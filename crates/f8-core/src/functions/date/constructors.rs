//! Date constructor and monthly stepping functions (`DATE`, `EDATE`, `EOMONTH`).

use crate::functions::common::{scalar_integer, scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
};
use crate::value::{CellValue, DateSerial, ExcelError, ValueType, validate_date_serial};

static DATE_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "year",
        "The 4-digit calendar year of the date (1900-9999).",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "month",
        "The calendar month of the year (1-12).",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "day",
        "The calendar day of the month (1..=days_in_month).",
        ParamKind::Scalar,
        ParamType::Number,
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

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let eval = || -> Result<f64, ExcelError> {
            let year_i64 = scalar_integer(&args[0])?;
            let month_i64 = scalar_integer(&args[1])?;
            let day_i64 = scalar_integer(&args[2])?;

            let y = i32::try_from(year_i64).map_err(|_| ExcelError::Num)?;
            let m = u32::try_from(month_i64).map_err(|_| ExcelError::Num)?;
            let d = u32::try_from(day_i64).map_err(|_| ExcelError::Num)?;

            DateSerial::from_ymd(y, m, d)
        };

        match eval() {
            Ok(serial) => Ok(CellValue::number(serial)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonthStepDay {
    ClampOriginal,
    EndOfMonth,
}

fn step_months(args: &[FunctionArg<'_>], day_mode: MonthStepDay) -> Result<f64, ExcelError> {
    let start_val = validate_date_serial(scalar_number(&args[0])?)?;
    let m_offset = scalar_integer(&args[1])?;
    if m_offset.abs() > 120_000 {
        return Err(ExcelError::Num);
    }

    let date = DateSerial::to_ymd(start_val)?;
    let total_months = i64::from(date.year) * 12 + (i64::from(date.month) - 1) + m_offset;
    let target_year_i64 = total_months.div_euclid(12);
    if !(1900..=9999).contains(&target_year_i64) {
        return Err(ExcelError::Num);
    }
    let target_year = target_year_i64 as i32;
    let target_month = (total_months.rem_euclid(12) + 1) as u32;
    let max_day = DateSerial::gregorian_days_in_month(target_year, target_month);
    let target_day = match day_mode {
        MonthStepDay::ClampOriginal => date.day.min(max_day),
        MonthStepDay::EndOfMonth => max_day,
    };

    DateSerial::from_ymd(target_year, target_month, target_day)
}

static EDATE_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "start_date",
        "A serial number that represents the start date.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "months",
        "The integer number of months before or after start_date.",
        ParamKind::Scalar,
        ParamType::Number,
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
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match step_months(args, MonthStepDay::ClampOriginal) {
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
        ParamType::Number,
    ),
    ParameterDef::required(
        "months",
        "The integer number of months before or after start_date.",
        ParamKind::Scalar,
        ParamType::Number,
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
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        match step_months(args, MonthStepDay::EndOfMonth) {
            Ok(serial) => Ok(CellValue::number(serial)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}
