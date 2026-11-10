use f8_core::compiler::InspectionError as CoreInspectionError;
use f8_core::compiler::error::{CompilerError, ErrorDiagnostic};
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyTypeError};
use pyo3::prelude::*;

create_exception!(
    _core,
    F8Error,
    PyException,
    "Base exception for all f8 compiler, evaluation, inspection, and emission errors."
);
create_exception!(
    _core,
    CycleDetectedError,
    F8Error,
    "Raised when the dependency graph contains a circular cell reference."
);
create_exception!(
    _core,
    ReferenceResolutionError,
    F8Error,
    "Raised when a formula coordinate or range reference cannot be resolved."
);
create_exception!(
    _core,
    SpecificationError,
    F8Error,
    "Raised when a model manifest or table specification violates schema or IR invariants."
);
create_exception!(
    _core,
    FormulaParseError,
    F8Error,
    "Raised when an f8 DSL expression fails lexical or syntactic parsing."
);
create_exception!(
    _core,
    InspectionError,
    F8Error,
    "Raised when an inspected sheet, table, or cell coordinate does not exist."
);
create_exception!(
    _core,
    EmissionError,
    F8Error,
    "Raised when OpenXML (.xlsx) workbook generation or file I/O fails."
);

pub fn register_exceptions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("F8Error", m.py().get_type::<F8Error>())?;
    m.add(
        "CycleDetectedError",
        m.py().get_type::<CycleDetectedError>(),
    )?;
    m.add(
        "ReferenceResolutionError",
        m.py().get_type::<ReferenceResolutionError>(),
    )?;
    m.add(
        "SpecificationError",
        m.py().get_type::<SpecificationError>(),
    )?;
    m.add("FormulaParseError", m.py().get_type::<FormulaParseError>())?;
    m.add("InspectionError", m.py().get_type::<InspectionError>())?;
    m.add("EmissionError", m.py().get_type::<EmissionError>())?;
    Ok(())
}

fn set_attr_quietly<'py, V>(py: Python<'py>, err: &PyErr, name: &str, value: V)
where
    V: IntoPyObject<'py>,
{
    let bound_err = err.value(py);
    let _ = bound_err.setattr(name, value);
}

pub fn make_f8_error(py: Python<'_>, message: String) -> PyErr {
    let err = F8Error::new_err((message.clone(),));
    set_attr_quietly(py, &err, "message", message);
    err
}

pub fn make_cycle_detected_error(py: Python<'_>, message: String, cycle: Vec<String>) -> PyErr {
    let err = CycleDetectedError::new_err((message.clone(),));
    set_attr_quietly(py, &err, "message", message);
    set_attr_quietly(py, &err, "cycle", cycle);
    err
}

pub fn make_reference_resolution_error(
    py: Python<'_>,
    message: String,
    coordinate: String,
    target_table: Option<String>,
    row: Option<String>,
    col: Option<String>,
) -> PyErr {
    let err = ReferenceResolutionError::new_err((message.clone(),));
    set_attr_quietly(py, &err, "message", message);
    set_attr_quietly(py, &err, "coordinate", coordinate);
    set_attr_quietly(py, &err, "target_table", target_table);
    set_attr_quietly(py, &err, "row", row);
    set_attr_quietly(py, &err, "col", col);
    err
}

pub fn make_specification_error(
    py: Python<'_>,
    message: String,
    table: Option<String>,
    reason: String,
) -> PyErr {
    let err = SpecificationError::new_err((message.clone(),));
    set_attr_quietly(py, &err, "message", message);
    set_attr_quietly(py, &err, "table", table);
    set_attr_quietly(py, &err, "reason", reason);
    err
}

pub fn make_formula_parse_error(
    py: Python<'_>,
    message: String,
    table: String,
    row: Option<String>,
    col: Option<String>,
    token_offset: Option<usize>,
) -> PyErr {
    let err = FormulaParseError::new_err((message.clone(),));
    set_attr_quietly(py, &err, "message", message);
    set_attr_quietly(py, &err, "table", table);
    set_attr_quietly(py, &err, "row", row);
    set_attr_quietly(py, &err, "col", col);
    set_attr_quietly(py, &err, "token_offset", token_offset);
    err
}

pub fn make_inspection_error(py: Python<'_>, message: String, entity: String) -> PyErr {
    let err = InspectionError::new_err((message.clone(),));
    set_attr_quietly(py, &err, "message", message);
    set_attr_quietly(py, &err, "entity", entity);
    err
}

pub fn make_emission_error(py: Python<'_>, message: String) -> PyErr {
    let err = EmissionError::new_err((message.clone(),));
    set_attr_quietly(py, &err, "message", message);
    err
}

pub fn diagnostic_to_py_err(py: Python<'_>, diag: ErrorDiagnostic) -> PyErr {
    match diag {
        ErrorDiagnostic::F8Error { message } => make_f8_error(py, message),
        ErrorDiagnostic::CycleDetectedError { message, cycle } => {
            make_cycle_detected_error(py, message, cycle)
        }
        ErrorDiagnostic::ReferenceResolutionError {
            message,
            coordinate,
            target_table,
            row,
            col,
        } => make_reference_resolution_error(py, message, coordinate, target_table, row, col),
        ErrorDiagnostic::SpecificationError {
            message,
            table,
            reason,
        } => make_specification_error(py, message, table, reason),
        ErrorDiagnostic::FormulaParseError {
            message,
            table,
            row,
            col,
            token_offset,
        } => make_formula_parse_error(py, message, table, row, col, token_offset),
        ErrorDiagnostic::InspectionError { message, entity } => {
            make_inspection_error(py, message, entity)
        }
        ErrorDiagnostic::EmissionError { message } => make_emission_error(py, message),
        ErrorDiagnostic::TypeError { message } => PyTypeError::new_err(message),
    }
}

pub fn map_inspection_error(py: Python<'_>, err: &CoreInspectionError) -> PyErr {
    diagnostic_to_py_err(py, err.diagnostic())
}

pub fn map_compiler_error(py: Python<'_>, err: &CompilerError) -> PyErr {
    diagnostic_to_py_err(py, err.diagnostic())
}

pub fn map_pythonize_error(py: Python<'_>, err: pythonize::PythonizeError) -> PyErr {
    let msg = err.to_string();
    make_specification_error(py, msg.clone(), None, msg)
}
