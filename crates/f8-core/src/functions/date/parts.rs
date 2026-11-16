//! Calendar component extraction functions (`YEAR`, `MONTH`, `DAY`).

use crate::functions::common::{require_args, scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
};
use crate::value::{CellValue, DateSerial, ValueType, validate_date_serial};

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
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let [serial_arg] = require_args(self, args)?;
        let serial_val = match scalar_number(serial_arg).and_then(validate_date_serial) {
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
        let [serial_arg] = require_args(self, args)?;
        let serial_val = match scalar_number(serial_arg).and_then(validate_date_serial) {
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
        let [serial_arg] = require_args(self, args)?;
        let serial_val = match scalar_number(serial_arg).and_then(validate_date_serial) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match DateSerial::to_ymd(serial_val) {
            Ok(d) => Ok(CellValue::number(f64::from(d.day))),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}
