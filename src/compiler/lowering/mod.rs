//! Pass 6a: Formula lowering and reference coordinate resolution.
//!
//! Transforms high-level semantic AST expressions into concrete Excel formula strings
//! with appropriate coordinate referencing (`$`), single-quoted sheet qualifiers,
//! uppercase function names, and operator precedence parenthesization.

pub mod address;
pub mod error;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

pub use error::LoweringError;

use crate::compiler::graph::resolve::{resolve_col_selector, resolve_row_selector};
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::compiler::layout::LayoutMap;
use crate::expr::ast::{
    BinaryOp, ColSelector, Expr, Literal, RelativeOffset, RowSelector, TableRef, UnaryOp,
};

/// Deterministic map of lowered Excel formula strings indexed by [`CellId`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoweredFormulaMap {
    /// Lowered Excel formula strings prefixed with `=` (e.g. `"=C10-C11"`).
    pub formulas: IndexMap<CellId, String>,
}

impl LoweredFormulaMap {
    /// Creates an empty formula map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            formulas: IndexMap::new(),
        }
    }

    /// Returns the lowered formula string for a cell, if present.
    #[must_use]
    pub fn get(&self, cell_id: &CellId) -> Option<&str> {
        self.formulas.get(cell_id).map(String::as_str)
    }

    /// Returns the number of lowered formula cells in the map.
    #[must_use]
    pub fn len(&self) -> usize {
        self.formulas.len()
    }

    /// Returns `true` if the map contains no lowered formulas.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.formulas.is_empty()
    }
}

/// Stateless lowering engine translating semantic AST formulas to native Excel formula strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FormulaLowerer;

impl FormulaLowerer {
    /// Lowers a single formula expression for a given cell into an Excel formula string prefixed with `=`.
    ///
    /// # Errors
    /// Returns [`LoweringError`] if reference resolution fails, coordinates are missing from the layout map,
    /// or a syntax error occurs.
    pub fn lower_cell(
        origin: &CellId,
        expr: &Expr,
        symbol_table: &SymbolTable,
        layout_map: &LayoutMap,
    ) -> Result<String, LoweringError> {
        let inner = lower_expr_inner(expr, origin, symbol_table, layout_map)?;
        Ok(format!("={inner}"))
    }

    /// Lowers all formula cells in the workbook grid into a deterministic [`LoweredFormulaMap`].
    ///
    /// # Errors
    /// Returns [`LoweringError`] if any formula fails to lower.
    pub fn lower_grid(
        grid: &CellGrid,
        symbol_table: &SymbolTable,
        layout_map: &LayoutMap,
    ) -> Result<LoweredFormulaMap, LoweringError> {
        let mut formulas = IndexMap::new();
        for (cell_id, cell_node) in grid.iter() {
            if let CellPayload::Formula(expr) = &cell_node.payload {
                let lowered = Self::lower_cell(cell_id, expr, symbol_table, layout_map)?;
                formulas.insert(cell_id.clone(), lowered);
            }
        }
        Ok(LoweredFormulaMap { formulas })
    }
}

/// Returns `true` if the column selector is dynamic (tracks active evaluation column or relative offset).
fn is_col_dynamic(col_sel: &ColSelector) -> bool {
    matches!(
        col_sel,
        ColSelector::Active
            | ColSelector::Offset(RelativeOffset::Prev(_))
            | ColSelector::Offset(RelativeOffset::Next(_))
    )
}

/// Returns `true` if the row selector is dynamic (tracks active evaluation row or relative offset).
fn is_row_dynamic(row_sel: &RowSelector) -> bool {
    matches!(
        row_sel,
        RowSelector::Active
            | RowSelector::Offset(RelativeOffset::Prev(_))
            | RowSelector::Offset(RelativeOffset::Next(_))
    )
}

/// Lowers a semantic table reference into a concrete Excel coordinate or range string.
pub fn lower_table_ref(
    origin: &CellId,
    target_ref: &TableRef,
    symbol_table: &SymbolTable,
    layout_map: &LayoutMap,
) -> Result<String, LoweringError> {
    let origin_table = symbol_table.get_table(&origin.table).ok_or_else(|| {
        crate::compiler::graph::error::ResolveError::unknown_table(origin, origin.table.clone())
    })?;

    let target_table = symbol_table.get_table(&target_ref.table).ok_or_else(|| {
        crate::compiler::graph::error::ResolveError::unknown_table(origin, target_ref.table.clone())
    })?;

    let origin_coord =
        layout_map
            .cells
            .get(origin)
            .ok_or_else(|| LoweringError::MissingCellCoordinate {
                cell: origin.clone(),
            })?;
    let origin_sheet = &origin_coord.sheet;

    let rows = resolve_row_selector(origin, target_table, origin_table, &target_ref.row)?;
    let cols = resolve_col_selector(origin, target_table, origin_table, &target_ref.col)?;

    let first_row = rows.first().ok_or_else(|| LoweringError::EmptyRange {
        origin: origin.clone(),
        target_table: target_ref.table.clone(),
    })?;
    let last_row = rows.last().ok_or_else(|| LoweringError::EmptyRange {
        origin: origin.clone(),
        target_table: target_ref.table.clone(),
    })?;
    let first_col = cols.first().ok_or_else(|| LoweringError::EmptyRange {
        origin: origin.clone(),
        target_table: target_ref.table.clone(),
    })?;
    let last_col = cols.last().ok_or_else(|| LoweringError::EmptyRange {
        origin: origin.clone(),
        target_table: target_ref.table.clone(),
    })?;

    let col_dynamic = is_col_dynamic(&target_ref.col);
    let row_dynamic = is_row_dynamic(&target_ref.row);
    let is_range = rows.len() > 1 || cols.len() > 1;

    let (col_pinned, row_pinned) = match (col_dynamic, row_dynamic) {
        (true, true) => (false, false),
        (true, false) => (false, rows.len() > 1),
        (false, true) => (false, false),
        (false, false) => (true, true),
    };

    let top_left_id = CellId::new(
        target_ref.table.clone(),
        first_row.clone(),
        first_col.clone(),
    );
    let top_left_coord = layout_map
        .cells
        .get(&top_left_id)
        .ok_or(LoweringError::MissingCellCoordinate { cell: top_left_id })?;

    if is_range {
        let bottom_right_id =
            CellId::new(target_ref.table.clone(), last_row.clone(), last_col.clone());
        let bottom_right_coord =
            layout_map
                .cells
                .get(&bottom_right_id)
                .ok_or(LoweringError::MissingCellCoordinate {
                    cell: bottom_right_id,
                })?;

        Ok(address::format_range(
            &top_left_coord.sheet,
            (top_left_coord.col, top_left_coord.row),
            (bottom_right_coord.col, bottom_right_coord.row),
            col_pinned,
            row_pinned,
            origin_sheet,
        ))
    } else {
        Ok(address::format_single_cell(
            &top_left_coord.sheet,
            top_left_coord.col,
            top_left_coord.row,
            col_pinned,
            row_pinned,
            origin_sheet,
        ))
    }
}

/// Recursively lowers an AST expression into a formula fragment without leading `=`.
fn lower_expr_inner(
    expr: &Expr,
    origin: &CellId,
    symbol_table: &SymbolTable,
    layout_map: &LayoutMap,
) -> Result<String, LoweringError> {
    match expr {
        Expr::Literal(lit) => match lit {
            Literal::Number(n) => {
                if n.is_nan() || n.is_infinite() {
                    return Err(LoweringError::SyntaxError {
                        message: format!("invalid numeric literal '{n}' in formula"),
                    });
                }
                Ok(n.to_string())
            }
            Literal::Boolean(true) => Ok("TRUE".to_string()),
            Literal::Boolean(false) => Ok("FALSE".to_string()),
            Literal::String(s) => {
                let escaped = s.replace('"', "\"\"");
                Ok(format!("\"{escaped}\""))
            }
        },
        Expr::Omitted => Ok(String::new()),
        Expr::Reference(table_ref) => lower_table_ref(origin, table_ref, symbol_table, layout_map),
        Expr::Unary { op, expr } => {
            let inner_str = lower_expr_inner(expr, origin, symbol_table, layout_map)?;
            let formatted = format_unary_operand(expr, inner_str, *op);
            Ok(format!("{}{formatted}", op.as_str()))
        }
        Expr::Binary { op, left, right } => {
            let left_str = lower_expr_inner(left, origin, symbol_table, layout_map)?;
            let right_str = lower_expr_inner(right, origin, symbol_table, layout_map)?;

            let formatted_left = format_binary_operand(left, left_str, *op, false);
            let formatted_right = format_binary_operand(right, right_str, *op, true);

            Ok(format!("{formatted_left}{}{formatted_right}", op.as_str()))
        }
        Expr::FunctionCall { name, args } => {
            let mut arg_strings = Vec::with_capacity(args.len());
            for arg in args {
                arg_strings.push(lower_expr_inner(arg, origin, symbol_table, layout_map)?);
            }
            Ok(format!(
                "{}({})",
                name.to_uppercase(),
                arg_strings.join(",")
            ))
        }
    }
}

/// Formats an operand of a binary operation, wrapping in parentheses if precedence requires it.
fn format_binary_operand(
    child_expr: &Expr,
    child_str: String,
    parent_op: BinaryOp,
    is_right: bool,
) -> String {
    let wrap = match child_expr {
        Expr::Binary { op: child_op, .. } => {
            let child_prec = child_op.precedence();
            let parent_prec = parent_op.precedence();
            if child_prec < parent_prec {
                true
            } else if child_prec == parent_prec {
                if is_right {
                    matches!(parent_op, BinaryOp::Sub | BinaryOp::Div | BinaryOp::Pow)
                } else {
                    parent_op.is_right_associative()
                }
            } else {
                false
            }
        }
        Expr::Unary { op: child_op, .. } => {
            is_right || child_op.precedence() < parent_op.precedence()
        }
        _ => false,
    };

    if wrap {
        format!("({child_str})")
    } else {
        child_str
    }
}

/// Formats an operand of a unary operation, wrapping in parentheses if precedence requires it.
fn format_unary_operand(child_expr: &Expr, child_str: String, _parent_op: UnaryOp) -> String {
    let wrap = matches!(child_expr, Expr::Binary { .. });

    if wrap {
        format!("({child_str})")
    } else {
        child_str
    }
}
