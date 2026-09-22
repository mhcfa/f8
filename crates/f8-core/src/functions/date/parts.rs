//! Calendar component extraction and day difference functions (`YEAR`, `MONTH`, `DAY`, `DAYS`).

use crate::functions::common::extract_scalar_number;
use crate::functions::date::serial::{DateSerial, validate_date_serial};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
};
use crate::value::CellValue;

static SERIAL_NUMBER_PARAM: &[ParameterDef] = &[ParameterDef::required(
    "serial_number",
    "The date of the year/month/day you want to find.",
    ParamKind::Scalar,
)];

/// `YEAR(serial_number)`: Returns the year corresponding to a serial date (1900–9999).
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

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _context: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }
        if args.len() > 1 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 1,
                found: args.len(),
            });
        }

        let serial_val = match extract_scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match DateSerial::to_ymd(serial_val) {
            Ok(d) => Ok(CellValue::Number(d.year as f64)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

/// `MONTH(serial_number)`: Returns the month of a date represented by a serial number (1–12).
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

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _context: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }
        if args.len() > 1 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 1,
                found: args.len(),
            });
        }

        let serial_val = match extract_scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match DateSerial::to_ymd(serial_val) {
            Ok(d) => Ok(CellValue::Number(d.month as f64)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

/// `DAY(serial_number)`: Returns the day of the month represented by a serial number (1–31).
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

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _context: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }
        if args.len() > 1 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 1,
                found: args.len(),
            });
        }

        let serial_val = match extract_scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match DateSerial::to_ymd(serial_val) {
            Ok(d) => Ok(CellValue::Number(d.day as f64)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

static DAYS_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "end_date",
        "The end date for the date calculation.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "start_date",
        "The start date for the date calculation.",
        ParamKind::Scalar,
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

        let end_val = match extract_scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let start_val = match extract_scalar_number(&args[1]) {
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

        let diff = s_end - s_start;
        Ok(CellValue::Number(diff))
    }
}
