//! Calendar component extraction and day difference functions (`YEAR`, `MONTH`, `DAY`, `DAYS`).

use crate::functions::common::extract_scalar_number;
use crate::functions::date::serial::{DateSerial, validate_date_serial};
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
};
use crate::value::{CellValue, ValueType};

static SERIAL_NUMBER_PARAM: &[ParameterDef] = &[ParameterDef::required(
    "serial_number",
    "The date of the year/month/day you want to find.",
    ParamKind::Scalar,
    ParamType::Number,
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let serial_val =
            match extract_scalar_number(&args[0], self.name(), "serial_number", 1, ctx)? {
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let serial_val =
            match extract_scalar_number(&args[0], self.name(), "serial_number", 1, ctx)? {
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let serial_val =
            match extract_scalar_number(&args[0], self.name(), "serial_number", 1, ctx)? {
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
        ParamType::Number,
    ),
    ParameterDef::required(
        "start_date",
        "The start date for the date calculation.",
        ParamKind::Scalar,
        ParamType::Number,
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let end_res = extract_scalar_number(&args[0], self.name(), "end_date", 1, ctx)?;
        let start_res = extract_scalar_number(&args[1], self.name(), "start_date", 2, ctx)?;

        let end_val = match end_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let start_val = match start_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let s_end = match validate_date_serial(end_val) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let s_start = match validate_date_serial(start_val) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        Ok(CellValue::number(s_end - s_start))
    }
}
