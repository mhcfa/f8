use std::cmp::Ordering;

use indexmap::IndexMap;

use super::context::CellEvalContext;
use super::error::EvalError;
use crate::compiler::graph::resolve_table_ref;
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::expr::ast::{BinaryOp, Expr, Literal, UnaryOp};
use crate::functions::error::FunctionError;
use crate::functions::registry::FunctionRegistry;
use crate::value::{CellValue, ExcelError};

/// Evaluates a formula AST expression within the context of an origin cell coordinate.
///
/// Returns `Ok(CellValue)` on successful evaluation or Excel domain error (`#DIV/0!`, `#VALUE!`, `#NUM!`),
/// or `Err(EvalError)` on internal compiler, resolution, or typing failures.
pub fn eval_expr(
    expr: &Expr,
    origin: &CellId,
    memoized_values: &IndexMap<CellId, CellValue>,
    symbol_table: &SymbolTable,
    registry: &FunctionRegistry,
) -> Result<CellValue, EvalError> {
    match expr {
        Expr::Literal(lit) => Ok(eval_literal(lit)),

        Expr::Omitted => Err(EvalError::unexpected_omitted(origin.clone())),

        Expr::Reference(table_ref) => {
            let slice = resolve_table_ref(origin, table_ref, symbol_table)?;
            match slice.cells.as_slice() {
                [] => Ok(CellValue::Error(ExcelError::Ref)),
                [target] => {
                    let val = memoized_values
                        .get(target)
                        .cloned()
                        .ok_or_else(|| EvalError::unevaluated_dependency(target.clone()))?;
                    Ok(val)
                }
                _ => Ok(CellValue::Error(ExcelError::Value)),
            }
        }

        Expr::Unary { op, expr } => {
            let inner = eval_expr(expr, origin, memoized_values, symbol_table, registry)?;
            if let CellValue::Error(e) = inner {
                return Ok(CellValue::Error(e));
            }

            let num = match inner.coerce_to_number() {
                Ok(n) => n,
                Err(err) => return Ok(CellValue::Error(err)),
            };

            let res = match op {
                UnaryOp::Pos => num,
                UnaryOp::Neg => -num,
            };

            Ok(CellValue::Number(normalize_zero(res)))
        }

        Expr::Binary { op, left, right } => {
            let left_val = eval_expr(left, origin, memoized_values, symbol_table, registry)?;
            let right_val = eval_expr(right, origin, memoized_values, symbol_table, registry)?;

            eval_binary_op(*op, &left_val, &right_val)
        }

        Expr::FunctionCall { name, args } => {
            let func = registry.lookup(name).ok_or_else(|| {
                EvalError::function(origin.clone(), FunctionError::UnknownFunction(name.clone()))
            })?;

            let ctx = CellEvalContext::new(origin, memoized_values, symbol_table, registry);
            let bound_args = registry
                .bind_arguments(func, args, &ctx)
                .map_err(|source| EvalError::function(origin.clone(), source))?;

            let result = func
                .evaluate(&bound_args, &ctx)
                .map_err(|source| EvalError::function(origin.clone(), source))?;

            let normalized = match result {
                CellValue::Number(n) => CellValue::Number(normalize_zero(n)),
                other => other,
            };
            Ok(normalized)
        }
    }
}

/// Evaluates a constant literal AST node into a `CellValue`.
fn eval_literal(lit: &Literal) -> CellValue {
    match lit {
        Literal::Number(n) => CellValue::Number(normalize_zero(*n)),
        Literal::String(s) => CellValue::Text(s.clone()),
        Literal::Boolean(b) => CellValue::Boolean(*b),
    }
}

/// Evaluates binary arithmetic and relational comparison operators.
fn eval_binary_op(
    op: BinaryOp,
    left: &CellValue,
    right: &CellValue,
) -> Result<CellValue, EvalError> {
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
            eval_arithmetic_op(op, left, right)
        }
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Gte => eval_comparison_op(op, left, right),
    }
}

/// Evaluates binary arithmetic operations (`+`, `-`, `*`, `/`, `^`) with Excel parity.
fn eval_arithmetic_op(
    op: BinaryOp,
    left: &CellValue,
    right: &CellValue,
) -> Result<CellValue, EvalError> {
    // Error propagation takes precedence
    if let CellValue::Error(e) = left {
        return Ok(CellValue::Error(*e));
    }
    if let CellValue::Error(e) = right {
        return Ok(CellValue::Error(*e));
    }

    let lhs = match left.coerce_to_number() {
        Ok(n) => n,
        Err(err) => return Ok(CellValue::Error(err)),
    };
    let rhs = match right.coerce_to_number() {
        Ok(n) => n,
        Err(err) => return Ok(CellValue::Error(err)),
    };

    let result = match op {
        BinaryOp::Add => lhs + rhs,
        BinaryOp::Sub => lhs - rhs,
        BinaryOp::Mul => lhs * rhs,
        BinaryOp::Div => {
            if rhs == 0.0 {
                return Ok(CellValue::Error(ExcelError::Div0));
            }
            lhs / rhs
        }
        BinaryOp::Pow => {
            if lhs == 0.0 && rhs < 0.0 {
                return Ok(CellValue::Error(ExcelError::Div0));
            }
            if lhs < 0.0 && rhs.fract() != 0.0 {
                return Ok(CellValue::Error(ExcelError::Num));
            }
            lhs.powf(rhs)
        }
        _ => unreachable!("non-arithmetic operator passed to eval_arithmetic_op"),
    };

    if result.is_nan() || result.is_infinite() {
        return Ok(CellValue::Error(ExcelError::Num));
    }

    Ok(CellValue::Number(normalize_zero(result)))
}

/// Evaluates binary comparison operations (`=`, `<>`, `<`, `<=`, `>`, `>=`) with Excel parity.
fn eval_comparison_op(
    op: BinaryOp,
    left: &CellValue,
    right: &CellValue,
) -> Result<CellValue, EvalError> {
    // Error propagation takes precedence
    if let CellValue::Error(e) = left {
        return Ok(CellValue::Error(*e));
    }
    if let CellValue::Error(e) = right {
        return Ok(CellValue::Error(*e));
    }

    let ord = left.excel_cmp(right);
    let outcome = match op {
        BinaryOp::Eq => ord == Ordering::Equal,
        BinaryOp::NotEq => ord != Ordering::Equal,
        BinaryOp::Lt => ord == Ordering::Less,
        BinaryOp::Lte => ord != Ordering::Greater,
        BinaryOp::Gt => ord == Ordering::Greater,
        BinaryOp::Gte => ord != Ordering::Less,
        _ => unreachable!("non-comparison operator passed to eval_comparison_op"),
    };

    Ok(CellValue::Boolean(outcome))
}

/// Normalizes floating point negative zero `-0.0` to `0.0`.
fn normalize_zero(val: f64) -> f64 {
    if val == 0.0 { 0.0 } else { val }
}
