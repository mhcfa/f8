//! Compilation engine and sequential pass orchestration.
//!
//! Orchestrates the multi-pass compilation pipeline transforming high-level
//! model specifications into evaluated models and compiled `.xlsx` workbooks.

pub mod emission;
pub mod error;
pub mod eval;
pub mod graph;
pub mod ir;
pub mod layout;
pub mod lowering;

pub use emission::{CellFormatKey, EmissionError, FormatCache};
pub use error::CompilerError;
pub use eval::{CellEvalContext, EvalError, EvaluatedModel, Evaluator, eval_expr};
pub use layout::{
    LayoutError, LayoutMap, LayoutPlanner, PhysicalCoordinate, SectionHeaderRow, SheetLayout,
    StyleRole, SuperHeaderPlacement, TablePlacement,
};
pub use lowering::{FormulaLowerer, LoweredFormulaMap, LoweringError};

use crate::functions::registry::FunctionRegistry;
use crate::model::manifest::ProjectManifest;
use crate::model::table::TableSpec;
use crate::model::theme::Theme;

/// High-level compiler driver orchestrating compilation passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Compiler;

impl Compiler {
    /// Compiles and topologically evaluates a model specification into an in-memory evaluated model.
    ///
    /// Uses the default standard Excel function registry.
    ///
    /// # Errors
    /// Returns [`CompilerError`] if specification validation fails (Pass 1), grid expansion fails (Pass 2),
    /// circular dependencies or unresolved references are detected (Pass 3), or runtime evaluation encounters
    /// an internal compiler failure (Pass 4).
    pub fn evaluate(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
    ) -> Result<EvaluatedModel, CompilerError> {
        Self::evaluate_with_registry(manifest, tables, &FunctionRegistry::standard())
    }

    /// Compiles and topologically evaluates a model specification using a custom function registry.
    ///
    /// # Errors
    /// Returns [`CompilerError`] if any compilation pass encounters an error.
    pub fn evaluate_with_registry(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        registry: &FunctionRegistry,
    ) -> Result<EvaluatedModel, CompilerError> {
        manifest.validate()?;
        for table in tables {
            table.validate()?;
        }

        let (symbol_table, grid) = ir::expand_workbook(manifest, tables)?;
        let graph = graph::DependencyGraph::from_grid(&grid, &symbol_table)?;
        let evaluated_model = eval::Evaluator::evaluate(&grid, &graph, &symbol_table, registry)?;

        Ok(evaluated_model)
    }

    /// Plans the 2D spatial layout for a workbook model and theme (Pass 5).
    ///
    /// # Errors
    /// Returns [`CompilerError`] if specification validation, IR expansion, or spatial layout planning encounters an error.
    pub fn plan_layout(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
    ) -> Result<LayoutMap, CompilerError> {
        manifest.validate()?;
        for table in tables {
            table.validate()?;
        }

        let (_symbol_table, grid) = ir::expand_workbook(manifest, tables)?;
        let layout_map = layout::LayoutPlanner::plan(manifest, tables, &grid, theme, evaluated)?;
        Ok(layout_map)
    }

    /// Lowers all formula expressions across the workbook into concrete Excel formula strings (Pass 6a).
    ///
    /// # Errors
    /// Returns [`CompilerError`] if specification validation, IR expansion, or formula lowering encounters an error.
    pub fn lower_formulas(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        layout_map: &LayoutMap,
    ) -> Result<LoweredFormulaMap, CompilerError> {
        manifest.validate()?;
        for table in tables {
            table.validate()?;
        }

        let (symbol_table, grid) = ir::expand_workbook(manifest, tables)?;
        let lowered_map = lowering::FormulaLowerer::lower_grid(&grid, &symbol_table, layout_map)?;
        Ok(lowered_map)
    }
}
