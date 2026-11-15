//! Calendar component extraction and day difference functions (`YEAR`, `MONTH`, `DAY`, `DAYS`).

use crate::functions::common::scalar_number;
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
};
use crate::value::{CellValue, DateSerial, ValueType};

static SERIAL_NUMBER_PARAM: &[ParameterDef] = &[ParameterDef::required(
    "serial_number",
    "The date of the year/month/day you want to find.",
    ParamKind::Scalar,
    ParamType::DateSerial,
)];

/// `YEAR(serial_number)`: Returns the year corresponding to a serial date (1900-9999).
#[derive(Debug, Default, Clone, Copy)]
pub struct YearFunction;

impl ExcelFunction for YearFunction {
    fn name(&self) -> &'static str {
        "YEAR"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::DateTime
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        SERIAL_NUMBER_PARAM
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let serial_val = match scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match DateSerial::to_ymd(serial_val) {
            Ok(d) => Ok(CellValue::number(f64::from(d.year))),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

/// `MONTH(serial_number)`: Returns the month of a date represented by a serial number (1-12).
#[derive(Debug, Default, Clone, Copy)]
pub struct MonthFunction;

impl ExcelFunction for MonthFunction {
    fn name(&self) -> &'static str {
        "MONTH"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::DateTime
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        SERIAL_NUMBER_PARAM
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let serial_val = match scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match DateSerial::to_ymd(serial_val) {
            Ok(d) => Ok(CellValue::number(f64::from(d.month))),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

/// `DAY(serial_number)`: Returns the day of the month represented by a serial number (1-31).
#[derive(Debug, Default, Clone, Copy)]
pub struct DayFunction;

impl ExcelFunction for DayFunction {
    fn name(&self) -> &'static str {
        "DAY"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::DateTime
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        SERIAL_NUMBER_PARAM
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let serial_val = match scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match DateSerial::to_ymd(serial_val) {
            Ok(d) => Ok(CellValue::number(f64::from(d.day))),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

static DAYS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "end_date",
        "The end date for the date calculation.",
        ParamKind::Scalar,
        ParamType::DateSerial,
    ),
    ParameterDef::required(
        "start_date",
        "The start date for the date calculation.",
        ParamKind::Scalar,
        ParamType::DateSerial,
    ),
];

/// `DAYS(end_date, start_date)`: Returns the number of days between two dates.
#[derive(Debug, Default, Clone, Copy)]
pub struct DaysFunction;

impl ExcelFunction for DaysFunction {
    fn name(&self) -> &'static str {
        "DAYS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::DateTime
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        DAYS_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let end_val = match scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let start_val = match scalar_number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        Ok(CellValue::number(end_val.floor() - start_val.floor()))
    }
}
