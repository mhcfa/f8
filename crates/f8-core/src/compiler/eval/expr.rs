use std::cmp::Ordering;

use indexmap::IndexMap;

use super::context::CellEvalContext;
use super::error::EvalError;
use crate::compiler::graph::resolve_table_ref;
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::expr::ast::{BinaryOp, Expr, Literal, UnaryOp};
use crate::functions::error::FunctionError;
use crate::functions::registry::{FunctionRegistry, StageAError};
use crate::functions::traits::{ComparisonSide, EvalContext, ExpectationOrigin, TypeExpectation};
use crate::value::{CellValue, ExcelError, ExpectedType, TypeError, ValueType};

/// Evaluates a formula AST expression within the context of an origin cell coordinate.
///
/// - Statically validates all function calls, types, and slice shapes across the AST before evaluating.
/// - Preserves uncoerced `CellValue::Empty` when references or reference-returning branch/lookup functions
///   (`INDEX`, `XLOOKUP`, `IF`, `IFS`, `CHOOSE`) resolve to a blank cell;
///   root `EmptyFormulaResult` rejection applies at the formula-cell boundary in `Evaluator::evaluate`.
///
/// # Errors
/// Returns `EvalError` on internal compiler, function signature, resolution, slice, or typing failures.
/// Excel domain errors (`#DIV/0!`, `#VALUE!`, `#NUM!`, `#N/A`, `#REF!`) return `Ok(CellValue::Error(_))`.
pub fn eval_expr(
    expr: &Expr,
    origin: &CellId,
    memoized_values: &IndexMap<CellId, CellValue>,
    symbol_table: &SymbolTable,
    registry: &FunctionRegistry,
) -> Result<CellValue, EvalError> {
    registry
        .validate_expr_with_shapes(expr, |table_ref| {
            resolve_table_ref(origin, table_ref, symbol_table)
                .map(|slice| Some((slice.rows, slice.cols, slice.axis_role)))
        })
        .map_err(|err| match err {
            StageAError::Function(source) => EvalError::function(origin.clone(), source),
            StageAError::Resolve(source) => EvalError::Resolve(source),
        })?;
    let empty_error_types = IndexMap::new();
    eval_expr_validated(
        expr,
        origin,
        memoized_values,
        &empty_error_types,
        symbol_table,
        registry,
    )
}

/// Evaluates an already-validated formula AST expression at `origin`, preserving mid-expression `CellValue::Empty`.
pub(crate) fn eval_expr_validated(
    expr: &Expr,
    origin: &CellId,
    memoized_values: &IndexMap<CellId, CellValue>,
    memoized_error_types: &IndexMap<CellId, ValueType>,
    symbol_table: &SymbolTable,
    registry: &FunctionRegistry,
) -> Result<CellValue, EvalError> {
    match expr {
        Expr::Literal(lit) => Ok(eval_literal(lit)),

        Expr::Reference(table_ref) => {
            let slice = resolve_table_ref(origin, table_ref, symbol_table)?;
            if table_ref.is_slice() {
                return Err(EvalError::SliceInScalarContext {
                    cell: Box::new(origin.clone()),
                    rows: slice.rows,
                    cols: slice.cols,
                });
            }
            match slice.cells.as_slice() {
                [] => Ok(CellValue::Error(ExcelError::Ref)),
                [target] => {
                    let val = memoized_values
                        .get(target)
                        .cloned()
                        .ok_or_else(|| EvalError::unevaluated_dependency(target.clone()))?;
                    Ok(val)
                }
                [_, _, ..] => Err(EvalError::SliceInScalarContext {
                    cell: Box::new(origin.clone()),
                    rows: slice.rows,
                    cols: slice.cols,
                }),
            }
        }

        Expr::Unary {
            op,
            expr: inner_expr,
        } => {
            let inner = eval_expr_validated(
                inner_expr,
                origin,
                memoized_values,
                memoized_error_types,
                symbol_table,
                registry,
            )?;
            let num = match inner.strict_number() {
                Ok(Ok(n)) => n,
                Ok(Err(excel_err)) => {
                    let ctx = CellEvalContext::new(
                        origin,
                        memoized_values,
                        memoized_error_types,
                        symbol_table,
                        registry,
                    );
                    let exp = TypeExpectation {
                        expected: ExpectedType::Number,
                        origin: ExpectationOrigin::UnaryOp(op.as_str()),
                    };
                    ctx.validate_dormant_expr(inner_expr, Some(exp))
                        .map_err(|source| EvalError::function(origin.clone(), source))?;
                    return Ok(CellValue::Error(excel_err));
                }
                Err(found) => {
                    return Err(EvalError::Type {
                        cell: Box::new(origin.clone()),
                        source: TypeError::InvalidOperandType {
                            operator: op.as_str(),
                            expected: ExpectedType::Number,
                            found,
                        },
                    });
                }
            };

            let res = match op {
                UnaryOp::Neg => -num,
            };

            Ok(CellValue::number(res))
        }

        Expr::Binary { op, left, right } => {
            let left_val = eval_expr_validated(
                left,
                origin,
                memoized_values,
                memoized_error_types,
                symbol_table,
                registry,
            )?;
            let right_val = eval_expr_validated(
                right,
                origin,
                memoized_values,
                memoized_error_types,
                symbol_table,
                registry,
            )?;
            let ctx = CellEvalContext::new(
                origin,
                memoized_values,
                memoized_error_types,
                symbol_table,
                registry,
            );

            eval_binary_op(*op, &ctx, left, &left_val, right, &right_val)
        }

        Expr::FunctionCall { name, args } => {
            let func = registry.lookup(name).ok_or_else(|| {
                EvalError::function(origin.clone(), FunctionError::UnknownFunction(name.clone()))
            })?;

            let ctx = CellEvalContext::new(
                origin,
                memoized_values,
                memoized_error_types,
                symbol_table,
                registry,
            );
            let bound_args = registry
                .bind_arguments(func, args, &ctx)
                .map_err(|source| EvalError::function(origin.clone(), source))?;

            let result = func
                .evaluate(&bound_args, &ctx)
                .map_err(|source| EvalError::function(origin.clone(), source))?;

            let normalized = match result {
                CellValue::Number(n) => CellValue::number(n),
                other @ (CellValue::Text(_)
                | CellValue::Boolean(_)
                | CellValue::Empty
                | CellValue::Error(_)) => other,
            };
            Ok(normalized)
        }
    }
}

/// Evaluates a constant literal AST node into a `CellValue`.
fn eval_literal(lit: &Literal) -> CellValue {
    match lit {
        Literal::Number(n) => CellValue::number(*n),
        Literal::String(s) => CellValue::Text(s.clone()),
        Literal::Boolean(b) => CellValue::Boolean(*b),
    }
}

/// Evaluates binary arithmetic and relational comparison operators.
fn eval_binary_op(
    op: BinaryOp,
    ctx: &CellEvalContext<'_>,
    left_expr: &Expr,
    left: &CellValue,
    right_expr: &Expr,
    right: &CellValue,
) -> Result<CellValue, EvalError> {
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
            eval_arithmetic_op(op, ctx, left_expr, left, right_expr, right)
        }
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Gte => eval_comparison_op(op, ctx, left_expr, left, right_expr, right),
    }
}

/// Evaluates binary arithmetic operations (`+`, `-`, `*`, `/`, `^`) with strict numeric typing.
fn eval_arithmetic_op(
    op: BinaryOp,
    ctx: &CellEvalContext<'_>,
    left_expr: &Expr,
    left: &CellValue,
    right_expr: &Expr,
    right: &CellValue,
) -> Result<CellValue, EvalError> {
    let lhs_res = left.strict_number().map_err(|found| EvalError::Type {
        cell: Box::new(ctx.origin.clone()),
        source: TypeError::InvalidOperandType {
            operator: op.as_str(),
            expected: ExpectedType::Number,
            found,
        },
    })?;
    let rhs_res = right.strict_number().map_err(|found| EvalError::Type {
        cell: Box::new(ctx.origin.clone()),
        source: TypeError::InvalidOperandType {
            operator: op.as_str(),
            expected: ExpectedType::Number,
            found,
        },
    })?;

    let exp = TypeExpectation {
        expected: ExpectedType::Number,
        origin: ExpectationOrigin::BinaryOp(op.as_str()),
    };
    if lhs_res.is_err() {
        ctx.validate_dormant_expr(left_expr, Some(exp))
            .map_err(|source| EvalError::function(ctx.origin.clone(), source))?;
    }
    if rhs_res.is_err() {
        ctx.validate_dormant_expr(right_expr, Some(exp))
            .map_err(|source| EvalError::function(ctx.origin.clone(), source))?;
    }

    let lhs = match lhs_res {
        Ok(n) => n,
        Err(excel_err) => return Ok(CellValue::Error(excel_err)),
    };
    let rhs = match rhs_res {
        Ok(n) => n,
        Err(excel_err) => return Ok(CellValue::Error(excel_err)),
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
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Gte => unreachable!("non-arithmetic operator passed to eval_arithmetic_op"),
    };

    Ok(CellValue::number(result))
}

/// Evaluates binary comparison operations (`=`, `<>`, `<`, `<=`, `>`, `>=`) with strict homogeneous non-`Empty` typing.
fn eval_comparison_op(
    op: BinaryOp,
    ctx: &CellEvalContext<'_>,
    left_expr: &Expr,
    left: &CellValue,
    right_expr: &Expr,
    right: &CellValue,
) -> Result<CellValue, EvalError> {
    let ord = match left.compare_relational(right) {
        Ok(Ok(ord)) => ord,
        Ok(Err(excel_err)) => {
            let non_empty_exp = TypeExpectation {
                expected: ExpectedType::NonEmpty,
                origin: ExpectationOrigin::BinaryOp(op.as_str()),
            };
            let left_vt = if left.is_error() {
                ctx.validate_dormant_expr(left_expr, Some(non_empty_exp))
                    .map_err(|source| EvalError::function(ctx.origin.clone(), source))?
            } else {
                left.value_type()
            };
            let right_vt = if right.is_error() {
                let right_exp = if let Some(l_vt) = left_vt
                    && let Some(exp_ty) = ExpectedType::from_value_type(l_vt)
                {
                    TypeExpectation {
                        expected: exp_ty,
                        origin: ExpectationOrigin::ComparisonOp {
                            operator: op.as_str(),
                            other_side: l_vt,
                            side: ComparisonSide::Right,
                        },
                    }
                } else {
                    non_empty_exp
                };
                ctx.validate_dormant_expr(right_expr, Some(right_exp))
                    .map_err(|source| EvalError::function(ctx.origin.clone(), source))?
            } else {
                right.value_type()
            };
            if left.is_error()
                && left_vt.is_none()
                && let Some(r_vt) = right_vt
                && let Some(exp_ty) = ExpectedType::from_value_type(r_vt)
            {
                let left_exp = TypeExpectation {
                    expected: exp_ty,
                    origin: ExpectationOrigin::ComparisonOp {
                        operator: op.as_str(),
                        other_side: r_vt,
                        side: ComparisonSide::Left,
                    },
                };
                ctx.validate_dormant_expr(left_expr, Some(left_exp))
                    .map_err(|source| EvalError::function(ctx.origin.clone(), source))?;
            }
            if let (Some(lt), Some(rt)) = (left_vt, right_vt)
                && lt != rt
            {
                return Err(EvalError::Type {
                    cell: Box::new(ctx.origin.clone()),
                    source: TypeError::IncompatibleComparison {
                        operator: op.as_str(),
                        left: lt,
                        right: rt,
                    },
                });
            }
            return Ok(CellValue::Error(excel_err));
        }
        Err((lt, rt)) => {
            let source = if lt == ValueType::Empty || rt == ValueType::Empty {
                TypeError::InvalidOperandType {
                    operator: op.as_str(),
                    expected: ExpectedType::NonEmpty,
                    found: ValueType::Empty,
                }
            } else {
                TypeError::IncompatibleComparison {
                    operator: op.as_str(),
                    left: lt,
                    right: rt,
                }
            };
            return Err(EvalError::Type {
                cell: Box::new(ctx.origin.clone()),
                source,
            });
        }
    };

    let outcome = match op {
        BinaryOp::Eq => ord == Ordering::Equal,
        BinaryOp::NotEq => ord != Ordering::Equal,
        BinaryOp::Lt => ord == Ordering::Less,
        BinaryOp::Lte => ord != Ordering::Greater,
        BinaryOp::Gt => ord == Ordering::Greater,
        BinaryOp::Gte => ord != Ordering::Less,
        BinaryOp::Pow | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Add | BinaryOp::Sub => {
            unreachable!("non-comparison operator passed to eval_comparison_op")
        }
    };

    Ok(CellValue::Boolean(outcome))
}
