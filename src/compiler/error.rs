//! Unified compiler error hierarchy.
//!
//! Encapsulates diagnostic and execution errors across all compilation passes.

use thiserror::Error;

use crate::compiler::eval::error::EvalError;
use crate::compiler::graph::error::GraphError;
use crate::compiler::ir::error::IrError;
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
    Layout(#[from] crate::compiler::layout::error::LayoutError),
}
