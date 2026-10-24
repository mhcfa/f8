use f8_core::compiler::InspectionError as CoreInspectionError;
use f8_core::compiler::error::CompilerError;
use f8_core::compiler::eval::EvalError;
use f8_core::compiler::graph::GraphError;
use f8_core::compiler::graph::error::ResolveError;
use f8_core::compiler::ir::error::IrError;
use f8_core::compiler::lowering::LoweringError;
use f8_core::expr::error::ParseError;
use f8_core::model::error::ModelError;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
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

pub fn map_resolve_error(py: Python<'_>, resolve_err: &ResolveError) -> PyErr {
    let message = resolve_err.to_string();
    let (coordinate, target_table, row, col) = match resolve_err {
        ResolveError::UnknownTable {
            origin,
            target_table,
        } => (
            origin.to_string(),
            Some(target_table.to_string()),
            None,
            None,
        ),
        ResolveError::UnknownRow {
            origin,
            target_table,
            row,
        } => (
            origin.to_string(),
            Some(target_table.to_string()),
            Some(row.to_string()),
            None,
        ),
        ResolveError::UnknownColumn {
            origin,
            target_table,
            column,
        } => (
            origin.to_string(),
            Some(target_table.to_string()),
            None,
            Some(column.to_string()),
        ),
        ResolveError::ActiveColumnMismatch {
            origin,
            target_table,
            column,
        } => (
            origin.to_string(),
            Some(target_table.to_string()),
            None,
            Some(column.to_string()),
        ),
        ResolveError::ActiveRowMismatch {
            origin,
            target_table,
            row,
        } => (
            origin.to_string(),
            Some(target_table.to_string()),
            Some(row.to_string()),
            None,
        ),
        ResolveError::TemporalOffsetOutOfBounds { origin, .. } => {
            (origin.to_string(), None, None, None)
        }
        ResolveError::InvertedRange {
            origin,
            target_table,
            ..
        } => (
            origin.to_string(),
            Some(target_table.to_string()),
            None,
            None,
        ),
        ResolveError::UnregisteredTable { origin, table } => {
            (origin.to_string(), Some(table.to_string()), None, None)
        }
    };
    make_reference_resolution_error(py, message, coordinate, target_table, row, col)
}

pub fn map_inspection_error(py: Python<'_>, err: &CoreInspectionError) -> PyErr {
    let message = err.to_string();
    let entity = match err {
        CoreInspectionError::TableNotFound(t) => t.to_string(),
        CoreInspectionError::CellNotFound(c) => c.to_string(),
        CoreInspectionError::SheetNotFound(s) => s.to_string(),
        CoreInspectionError::InvalidCoordinate(s) => s.clone(),
    };
    make_inspection_error(py, message, entity)
}

pub fn map_compiler_error(py: Python<'_>, err: &CompilerError) -> PyErr {
    let message = err.to_string();
    match err {
        CompilerError::Model(model_err) => {
            let table = match model_err {
                ModelError::InvalidPrecisionForPreset { table, .. }
                | ModelError::InvalidCurrencyForPreset { table, .. }
                | ModelError::EmptyTableSubtitle { table }
                | ModelError::EmptyColumnGroup { table, .. }
                | ModelError::PrecisionOutOfBounds { table, .. }
                | ModelError::UnknownCellColumn { table, .. }
                | ModelError::MutuallyExclusiveCellPayload { table, .. }
                | ModelError::EmptyCellPayload { table, .. }
                | ModelError::UnknownColumn { table, .. }
                | ModelError::UnknownRow { table, .. }
                | ModelError::InvalidIndentOnMeasureCell { table, .. }
                | ModelError::DuplicateRowId { table, .. }
                | ModelError::DuplicateColumnId { table, .. }
                | ModelError::DuplicateTableId { table }
                | ModelError::EmptyColumns { table }
                | ModelError::EmptyRows { table } => Some(table.to_string()),
                _ => None,
            };
            make_specification_error(py, message.clone(), table, message)
        }
        CompilerError::Ir(IrError::FormulaParse {
            table,
            row,
            column,
            source,
        }) => {
            let token_offset = match source {
                ParseError::UnexpectedEof { .. } => None,
                ParseError::UnexpectedToken { offset, .. }
                | ParseError::InvalidNumber { offset, .. }
                | ParseError::InvalidFunctionName { offset, .. }
                | ParseError::UnquotedCoordinateKey { offset, .. }
                | ParseError::TrailingInput { offset, .. }
                | ParseError::RetiredBareAt { offset, .. } => Some(*offset),
            };
            make_formula_parse_error(
                py,
                message,
                table.to_string(),
                row.as_ref().map(ToString::to_string),
                column.as_ref().map(ToString::to_string),
                token_offset,
            )
        }
        CompilerError::Ir(other) => {
            let table = match other {
                IrError::DuplicateCell { id } => Some(id.table.to_string()),
                IrError::DuplicateTable { table, .. }
                | IrError::UnknownTable { table, .. }
                | IrError::UnknownColumn { table, .. }
                | IrError::UnknownRow { table, .. }
                | IrError::EmptyTable { table }
                | IrError::AmbiguousExpressionConflict { table, .. }
                | IrError::UnassignedCell { table, .. }
                | IrError::FormatConflict { table, .. }
                | IrError::DecimalsConflict { table, .. }
                | IrError::CurrencyConflict { table, .. }
                | IrError::ShadeConflict { table, .. } => Some(table.to_string()),
                _ => None,
            };
            make_specification_error(py, message.clone(), table, message)
        }
        CompilerError::Graph(GraphError::CircularDependency { cycle, .. }) => {
            let cycle_strs = cycle.iter().map(ToString::to_string).collect();
            make_cycle_detected_error(py, message, cycle_strs)
        }
        CompilerError::Graph(GraphError::Resolve(resolve_err)) => {
            map_resolve_error(py, resolve_err)
        }
        CompilerError::Eval(EvalError::Resolve(resolve_err)) => map_resolve_error(py, resolve_err),
        CompilerError::Eval(_) => make_f8_error(py, message),
        CompilerError::Layout(_) => make_specification_error(py, message.clone(), None, message),
        CompilerError::Lowering(LoweringError::Resolve(resolve_err)) => {
            map_resolve_error(py, resolve_err)
        }
        CompilerError::Lowering(_) => make_f8_error(py, message),
        CompilerError::Emission(_) => make_emission_error(py, message),
        CompilerError::Inspection(insp_err) => map_inspection_error(py, insp_err),
    }
}

pub fn map_pythonize_error(py: Python<'_>, err: pythonize::PythonizeError) -> PyErr {
    let msg = err.to_string();
    make_specification_error(py, msg.clone(), None, msg)
}
