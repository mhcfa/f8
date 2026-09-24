//! Compilation engine and sequential pass orchestration.
//!
//! Orchestrates the multi-pass compilation pipeline transforming high-level
//! model specifications into evaluated models and compiled `.xlsx` workbooks.

pub mod compiled;
pub mod emission;
pub mod error;
pub mod eval;
pub mod graph;
pub mod ir;
pub mod layout;
pub mod lowering;

use std::borrow::Cow;

pub use compiled::{
    CellInspection, CellNeighborInspection, ColumnInspection, CompiledModel, DependencyNode,
    DependencyTree, InspectionError, ModelSummary, RowInspection, SheetCellInspection,
    SheetCellKind, SheetInspection, SheetRowInspection, SheetSummary, TableCellInfo,
    TableInspection, TableSummary, parse_coordinate_tokens,
};
pub use emission::{CellFormatKey, EmissionError, EmissionOptions, FormatCache, WorkbookEmitter};
pub use error::CompilerError;
pub use eval::{CellEvalContext, EvalError, EvaluatedModel, Evaluator, eval_expr};
pub use layout::{
    LayoutError, LayoutMap, LayoutPlanner, PhysicalCoordinate, SheetLayout, StyleRole,
    TablePlacement, col_to_excel_col,
};
pub use lowering::{FormulaLowerer, LoweredFormulaMap, LoweringError};

use crate::functions::registry::FunctionRegistry;
use crate::model::manifest::ProjectManifest;
use crate::model::table::TableSpec;
use crate::model::theme::Theme;

/// High-level compiler driver orchestrating compilation passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Compiler;

/// Internal compilation artifacts prepared for workbook binary emission.
struct EmissionContext<'a> {
    grid: ir::cell::CellGrid,
    layout_map: layout::LayoutMap,
    lowered_map: lowering::LoweredFormulaMap,
    effective_evaluated: Cow<'a, EvaluatedModel>,
}

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

    /// Prepares artifacts for workbook emission: validates specifications, evaluates model if required,
    /// expands IR grid, computes spatial layout, and lowers formula expressions.
    fn prepare_emission_context<'a>(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        theme: &Theme,
        evaluated: Option<&'a EvaluatedModel>,
    ) -> Result<EmissionContext<'a>, CompilerError> {
        manifest.validate()?;
        for table in tables {
            table.validate()?;
        }

        let effective_evaluated = match evaluated {
            Some(eval) => Cow::Borrowed(eval),
            None => Cow::Owned(Self::evaluate(manifest, tables)?),
        };

        let (symbol_table, grid) = ir::expand_workbook(manifest, tables)?;
        let layout_map = layout::LayoutPlanner::plan(
            manifest,
            tables,
            &grid,
            theme,
            Some(&effective_evaluated),
        )?;
        let lowered_map = lowering::FormulaLowerer::lower_grid(&grid, &symbol_table, &layout_map)?;

        Ok(EmissionContext {
            grid,
            layout_map,
            lowered_map,
            effective_evaluated,
        })
    }

    /// Compiles a complete model specification and theme through Passes 1–6a into a [`CompiledModel`].
    ///
    /// Uses the default standard Excel function registry.
    ///
    /// # Errors
    /// Returns [`CompilerError`] if specification validation, IR expansion, dependency graph construction,
    /// in-memory topological calculation, 2D layout planning, or formula lowering fails.
    pub fn compile(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        theme: &Theme,
    ) -> Result<CompiledModel, CompilerError> {
        Self::compile_with_registry(manifest, tables, theme, &FunctionRegistry::standard())
    }

    /// Compiles a complete model specification and theme through Passes 1–6a using a custom function registry.
    ///
    /// # Errors
    /// Returns [`CompilerError`] if any compilation pass encounters an error.
    pub fn compile_with_registry(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        theme: &Theme,
        registry: &FunctionRegistry,
    ) -> Result<CompiledModel, CompilerError> {
        manifest.validate()?;
        for table in tables {
            table.validate()?;
        }
        theme.validate()?;

        let (symbol_table, grid) = ir::expand_workbook(manifest, tables)?;
        let graph = graph::DependencyGraph::from_grid(&grid, &symbol_table)?;
        let evaluated = eval::Evaluator::evaluate(&grid, &graph, &symbol_table, registry)?;
        let layout_map =
            layout::LayoutPlanner::plan(manifest, tables, &grid, theme, Some(&evaluated))?;
        let lowered_map = lowering::FormulaLowerer::lower_grid(&grid, &symbol_table, &layout_map)?;

        let tables_map = tables.iter().map(|t| (t.id.clone(), t.clone())).collect();

        Ok(CompiledModel {
            manifest: manifest.clone(),
            tables: tables_map,
            theme: theme.clone(),
            symbol_table,
            grid,
            graph,
            evaluated,
            layout_map,
            lowered_map,
        })
    }

    /// Compiles a model specification and theme into an in-memory `.xlsx` workbook byte buffer.
    ///
    /// If `evaluated` is `None`, this method automatically compiles and evaluates the model to
    /// pre-populate formula calculation result caches `<v>` or freeze concrete values in a single pass.
    ///
    /// # Errors
    /// Returns [`CompilerError`] if specification validation, IR expansion, spatial layout,
    /// formula lowering, or binary emission fails.
    pub fn emit_workbook(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
        options: &EmissionOptions,
    ) -> Result<Vec<u8>, CompilerError> {
        if let Some(eval) = evaluated {
            let ctx = Self::prepare_emission_context(manifest, tables, theme, Some(eval))?;

            let buffer = emission::WorkbookEmitter::emit_to_buffer(
                manifest,
                tables,
                &ctx.grid,
                &ctx.layout_map,
                &ctx.lowered_map,
                theme,
                Some(&ctx.effective_evaluated),
                options,
            )?;

            Ok(buffer)
        } else {
            let compiled = Self::compile(manifest, tables, theme)?;
            compiled.emit_workbook(options)
        }
    }
}
