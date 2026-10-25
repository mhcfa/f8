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
use crate::expr::ast::{BinaryOp, Expr, Literal, TableRef, UnaryOp};

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

/// Computes `(col_pinned, row_pinned)` for a single range endpoint.
///
/// Static `Key` endpoints are pinned (`$`) on their respective axis, except for 1D horizontal
/// static-column ranges on a dynamic row (e.g. `Table[@row, "c1".."c2"]` -> `B18:C18`),
/// where `pin_static_col_on_dynamic_row` is `false`.
fn range_endpoint_pinning(
    col_dynamic: bool,
    row_dynamic: bool,
    pin_static_col_on_dynamic_row: bool,
) -> (bool, bool) {
    let col_pinned = !col_dynamic && (!row_dynamic || pin_static_col_on_dynamic_row);
    let row_pinned = !row_dynamic;
    (col_pinned, row_pinned)
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

    let is_row_range = target_ref.row.is_range();
    let is_col_range = target_ref.col.is_range();
    let is_range = is_row_range || is_col_range;

    let (start_col_dyn, end_col_dyn) = target_ref.col.dynamic_bounds();
    let (start_row_dyn, end_row_dyn) = target_ref.row.dynamic_bounds();

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
        let pin_static_col_on_dynamic_row = is_row_range || start_col_dyn || end_col_dyn;
        let start_pinned =
            range_endpoint_pinning(start_col_dyn, start_row_dyn, pin_static_col_on_dynamic_row);
        let end_pinned =
            range_endpoint_pinning(end_col_dyn, end_row_dyn, pin_static_col_on_dynamic_row);

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
            start_pinned,
            end_pinned,
            origin_sheet,
        ))
    } else {
        let pinned = !start_col_dyn && !start_row_dyn;
        Ok(address::format_single_cell(
            &top_left_coord.sheet,
            top_left_coord.col,
            top_left_coord.row,
            pinned,
            pinned,
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
