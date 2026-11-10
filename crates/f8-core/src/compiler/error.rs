//! Unified compiler error hierarchy.
//!
//! Encapsulates diagnostic and execution errors across all compilation passes.

use serde::Serialize;
use thiserror::Error;

use crate::compiler::compiled::error::InspectionError;
use crate::compiler::emission::error::EmissionError;
use crate::compiler::eval::error::EvalError;
use crate::compiler::graph::error::{GraphError, ResolveError};
use crate::compiler::ir::error::IrError;
use crate::compiler::layout::error::LayoutError;
use crate::compiler::lowering::error::LoweringError;
use crate::expr::error::ParseError;
use crate::model::error::ModelError;

/// Unified compiler error encompassing specification, intermediate representation,
/// dependency graph, and runtime evaluation failures.
#[derive(Debug, Error)]
pub enum CompilerError {
    /// Specification validation error occurring during Pass 1.
    #[error("Specification validation error: {0}")]
    Model(#[from] ModelError),

    /// Grid expansion or cell intermediate representation error occurring during Pass 2.
    #[error("Intermediate representation error: {0}")]
    Ir(#[from] IrError),

    /// Reference resolution, cyclic dependency, or graph construction error occurring during Pass 3.
    #[error("Dependency graph error: {0}")]
    Graph(#[from] GraphError),

    /// In-memory formula calculation error occurring during Pass 4.
    #[error("Evaluation error: {0}")]
    Eval(#[from] EvalError),

    /// Spatial layout planning error occurring during Pass 5.
    #[error("Spatial layout error: {0}")]
    Layout(#[from] LayoutError),

    /// Formula lowering error occurring during Pass 6a.
    #[error("Formula lowering error: {0}")]
    Lowering(#[from] LoweringError),

    /// OpenXML format synthesis and workbook emission error occurring during Pass 6b.
    #[error("Emission error: {0}")]
    Emission(#[from] EmissionError),

    /// Model inspection or coordinate lookup error.
    #[error("Inspection error: {0}")]
    Inspection(#[from] InspectionError),
}

/// Canonical structured diagnostic payload extracted from compiler and inspection errors
/// for outer language bindings (`f8-python`, `f8-wasm`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum ErrorDiagnostic {
    F8Error {
        message: String,
    },
    CycleDetectedError {
        message: String,
        cycle: Vec<String>,
    },
    ReferenceResolutionError {
        message: String,
        coordinate: String,
        target_table: Option<String>,
        row: Option<String>,
        col: Option<String>,
    },
    SpecificationError {
        message: String,
        table: Option<String>,
        reason: String,
    },
    FormulaParseError {
        message: String,
        table: String,
        row: Option<String>,
        col: Option<String>,
        token_offset: Option<usize>,
    },
    InspectionError {
        message: String,
        entity: String,
    },
    EmissionError {
        message: String,
    },
    TypeError {
        message: String,
    },
}

impl ResolveError {
    /// Extracts a structured [`ErrorDiagnostic`] from a coordinate resolution error.
    #[must_use]
    pub fn diagnostic(&self) -> ErrorDiagnostic {
        let message = self.to_string();
        let (coordinate, target_table, row, col) = match self {
            Self::UnknownTable {
                origin,
                target_table,
            } => (
                origin.to_string(),
                Some(target_table.to_string()),
                None,
                None,
            ),
            Self::UnknownRow {
                origin,
                target_table,
                row,
            } => (
                origin.to_string(),
                Some(target_table.to_string()),
                Some(row.to_string()),
                None,
            ),
            Self::UnknownColumn {
                origin,
                target_table,
                column,
            } => (
                origin.to_string(),
                Some(target_table.to_string()),
                None,
                Some(column.to_string()),
            ),
            Self::ActiveColumnMismatch {
                origin,
                target_table,
                column,
            } => (
                origin.to_string(),
                Some(target_table.to_string()),
                None,
                Some(column.to_string()),
            ),
            Self::ActiveRowMismatch {
                origin,
                target_table,
                row,
            } => (
                origin.to_string(),
                Some(target_table.to_string()),
                Some(row.to_string()),
                None,
            ),
            Self::TemporalOffsetOutOfBounds { origin, offset: _ } => {
                (origin.to_string(), None, None, None)
            }
            Self::InvertedRange {
                origin,
                target_table,
                start: _,
                end: _,
            } => (
                origin.to_string(),
                Some(target_table.to_string()),
                None,
                None,
            ),
            Self::UnregisteredTable { origin, table } => {
                (origin.to_string(), Some(table.to_string()), None, None)
            }
        };
        ErrorDiagnostic::ReferenceResolutionError {
            message,
            coordinate,
            target_table,
            row,
            col,
        }
    }
}

impl InspectionError {
    /// Extracts a structured [`ErrorDiagnostic`] from an inspection error.
    #[must_use]
    pub fn diagnostic(&self) -> ErrorDiagnostic {
        let message = self.to_string();
        let entity = match self {
            Self::TableNotFound(t) => t.to_string(),
            Self::CellNotFound(c) => c.to_string(),
            Self::SheetNotFound(s) => s.to_string(),
            Self::InvalidCoordinate(s) => s.clone(),
        };
        ErrorDiagnostic::InspectionError { message, entity }
    }
}

impl CompilerError {
    /// Extracts a structured [`ErrorDiagnostic`] from a compiler error.
    #[must_use]
    pub fn diagnostic(&self) -> ErrorDiagnostic {
        let message = self.to_string();
        match self {
            Self::Model(model_err) => {
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
                    ModelError::DuplicateSheetName { .. }
                    | ModelError::ThemeNotFound { .. }
                    | ModelError::InvalidThemeColor { .. }
                    | ModelError::InvalidThemeMetric { .. }
                    | ModelError::InvalidColWidthBounds { .. }
                    | ModelError::MissingDecimalPlaceholder { .. }
                    | ModelError::MissingCurrencySymbolPlaceholder
                    | ModelError::ThemePrecisionOutOfBounds { .. } => None,
                };
                ErrorDiagnostic::SpecificationError {
                    message: message.clone(),
                    table,
                    reason: message,
                }
            }
            Self::Ir(IrError::FormulaParse {
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
                ErrorDiagnostic::FormulaParseError {
                    message,
                    table: table.to_string(),
                    row: row.as_ref().map(ToString::to_string),
                    col: column.as_ref().map(ToString::to_string),
                    token_offset,
                }
            }
            Self::Ir(other) => {
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
                    IrError::UnknownSheet { .. }
                    | IrError::FormulaParse { .. }
                    | IrError::Model(_) => None,
                };
                ErrorDiagnostic::SpecificationError {
                    message: message.clone(),
                    table,
                    reason: message,
                }
            }
            Self::Graph(GraphError::CircularDependency { cycle, .. }) => {
                let cycle_strs = cycle.iter().map(ToString::to_string).collect();
                ErrorDiagnostic::CycleDetectedError {
                    message,
                    cycle: cycle_strs,
                }
            }
            Self::Graph(GraphError::Resolve(resolve_err)) => resolve_err.diagnostic(),
            Self::Eval(EvalError::Resolve(resolve_err)) => resolve_err.diagnostic(),
            Self::Eval(
                EvalError::CellNotFound(_)
                | EvalError::UnevaluatedDependency(_)
                | EvalError::Function { .. }
                | EvalError::UnexpectedOmitted(_)
                | EvalError::TableNotFound(_)
                | EvalError::RowNotFound { .. },
            ) => ErrorDiagnostic::F8Error { message },
            Self::Layout(layout_err) => {
                let table = match layout_err {
                    LayoutError::UnassignedTable { table }
                    | LayoutError::UnknownTable { table, .. }
                    | LayoutError::InvalidPlacementTarget { table, .. } => Some(table.to_string()),
                    LayoutError::TableOverlap { second_table, .. } => {
                        Some(second_table.to_string())
                    }
                    LayoutError::GridLimitExceeded { .. } => None,
                };
                ErrorDiagnostic::SpecificationError {
                    message: message.clone(),
                    table,
                    reason: message,
                }
            }
            Self::Lowering(LoweringError::Resolve(resolve_err)) => resolve_err.diagnostic(),
            Self::Lowering(
                LoweringError::MissingCellCoordinate { .. }
                | LoweringError::EmptyRange { .. }
                | LoweringError::SyntaxError { .. },
            ) => ErrorDiagnostic::F8Error { message },
            Self::Emission(
                EmissionError::Xlsx(_)
                | EmissionError::InvalidColor { .. }
                | EmissionError::Model(_)
                | EmissionError::MissingSheet { .. }
                | EmissionError::MissingTable { .. }
                | EmissionError::MissingCell { .. },
            ) => ErrorDiagnostic::EmissionError { message },
            Self::Inspection(insp_err) => insp_err.diagnostic(),
        }
    }
}
