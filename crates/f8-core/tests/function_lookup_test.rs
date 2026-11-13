//! Integration tests for Excel modern lookup and reference functions suite.
//!
//! Covers `XLOOKUP`, `XMATCH`, and `INDEX` conforming to Excel calculation parity.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use std::cell::Cell;

use indexmap::IndexMap;

use f8_core::compiler::eval::{EvalError, eval_expr};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::{Expr, Literal};
use f8_core::functions::traits::{EvalContext, FunctionArg, ReferenceKind};
use f8_core::functions::{FunctionCategory, FunctionError, FunctionRegistry};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError, ExpectedType, TypeError, ValueType};

/// Mock evaluation context for direct function execution and lazy branch counting.
struct MockContext {
    eval_count: Cell<usize>,
}

impl MockContext {
    fn new() -> Self {
        Self {
            eval_count: Cell::new(0),
        }
    }

    fn count(&self) -> usize {
        self.eval_count.get()
    }
}

impl EvalContext for MockContext {
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError> {
        self.eval_count.set(self.eval_count.get() + 1);
        match expr {
            Expr::Literal(Literal::Number(n)) => Ok(CellValue::Number(*n)),
            Expr::Literal(Literal::String(s)) => Ok(CellValue::Text(s.clone())),
            Expr::Literal(Literal::Boolean(b)) => Ok(CellValue::Boolean(*b)),
            Expr::Reference(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::FunctionCall { .. } => Ok(CellValue::Text("FallbackEvaluated".into())),
        }
    }

    fn eval_range(&self, _expr: &Expr) -> Result<(usize, usize, Vec<CellValue>), FunctionError> {
        Ok((0, 0, Vec::new()))
    }
}

fn make_array(rows: usize, cols: usize, values: Vec<CellValue>) -> FunctionArg<'static> {
    FunctionArg::array(rows, cols, values, ReferenceKind::Slice)
}

fn eval_str(
    formula: &str,
    symbols: &SymbolTable,
    values: &IndexMap<CellId, CellValue>,
    registry: &FunctionRegistry,
    origin: &CellId,
) -> CellValue {
    let expr = Expr::parse(formula).expect("valid formula");
    eval_expr(&expr, origin, values, symbols, registry).expect("eval successfully")
}

// ============================================================================
// Registration and Metadata
// ============================================================================

#[test]
fn test_registration_and_metadata() {
    let registry = FunctionRegistry::standard();

    let lookup_funcs = ["XLOOKUP", "XMATCH", "INDEX"];
    for name in lookup_funcs {
        let func = registry
            .lookup(name)
            .unwrap_or_else(|| panic!("Function {name} should be registered"));
        assert_eq!(func.name(), name);
        assert_eq!(func.category(), FunctionCategory::Lookup);
    }

    let xlookup = registry.lookup("XLOOKUP").unwrap();
    assert_eq!(xlookup.min_args(), 3);
    assert_eq!(xlookup.max_args(), Some(6));

    let xmatch = registry.lookup("XMATCH").unwrap();
    assert_eq!(xmatch.min_args(), 2);
    assert_eq!(xmatch.max_args(), Some(4));

    let index = registry.lookup("INDEX").unwrap();
    assert_eq!(index.min_args(), 2);
    assert_eq!(index.max_args(), Some(3));
}

// ============================================================================
// Scenario 1: Exact match with XLOOKUP (vertical and horizontal)
// ============================================================================

#[test]
fn test_scenario_1_exact_match_xlookup() {
    let registry = FunctionRegistry::standard();
    let xlookup = registry.lookup("XLOOKUP").unwrap();
    let ctx = MockContext::new();

    // Vertical column ranges
    let keys = make_array(
        3,
        1,
        vec![
            CellValue::Text("A".into()),
            CellValue::Text("B".into()),
            CellValue::Text("C".into()),
        ],
    );
    let values = make_array(
        3,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
    );

    // XLOOKUP("B", keys, values) -> 20.0
    let res = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("B".into())),
                keys.clone(),
                values.clone(),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(20.0));

    // Case-insensitive ASCII matching: "b" matches "B"
    let res_ci = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("b".into())),
                keys,
                values,
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_ci, CellValue::Number(20.0));

    // Horizontal row ranges
    let h_keys = make_array(
        1,
        3,
        vec![
            CellValue::Text("X".into()),
            CellValue::Text("Y".into()),
            CellValue::Text("Z".into()),
        ],
    );
    let h_values = make_array(
        1,
        3,
        vec![
            CellValue::Number(100.0),
            CellValue::Number(200.0),
            CellValue::Number(300.0),
        ],
    );

    let res_h = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Y".into())),
                h_keys,
                h_values,
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_h, CellValue::Number(200.0));
}

// ============================================================================
// Scenario 2: Fallback handling with XLOOKUP
// ============================================================================

#[test]
fn test_scenario_2_fallback_handling_xlookup() {
    let registry = FunctionRegistry::standard();
    let xlookup = registry.lookup("XLOOKUP").unwrap();
    let ctx = MockContext::new();

    let keys = make_array(
        2,
        1,
        vec![CellValue::Text("A".into()), CellValue::Text("B".into())],
    );
    let values = make_array(2, 1, vec![CellValue::Number(10.0), CellValue::Number(20.0)]);

    // Missing without if_not_found -> #N/A
    let res_na = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("C".into())),
                keys.clone(),
                values.clone(),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_na, CellValue::Error(ExcelError::Na));

    // Missing with homogeneous numeric fallback -> -1.0
    let res_fb = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("C".into())),
                keys.clone(),
                values.clone(),
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_fb, CellValue::Number(-1.0));

    // Heterogeneous fallback ("MissingValue" vs numeric return_array) fails with IncompatibleBranchTypes
    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("C".into())),
                keys,
                values,
                FunctionArg::Scalar(CellValue::Text("MissingValue".into())),
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::IncompatibleBranchTypes {
            function: "XLOOKUP",
            position: 4,
            expected: ValueType::Number,
            found: ValueType::Text,
        }))
    );
}

// ============================================================================
// Scenario 3: Short-circuiting lazy fallback in XLOOKUP
// ============================================================================

#[test]
fn test_scenario_3_short_circuiting_lazy_fallback() {
    let registry = FunctionRegistry::standard();
    let xlookup = registry.lookup("XLOOKUP").unwrap();
    let ctx = MockContext::new();

    let keys = make_array(
        2,
        1,
        vec![
            CellValue::Text("Found".into()),
            CellValue::Text("Other".into()),
        ],
    );
    let values = make_array(2, 1, vec![CellValue::Number(42.0), CellValue::Number(99.0)]);

    let dummy_expr = Expr::Literal(Literal::Number(1.0));

    // When key IS found, lazy fallback should NEVER be evaluated (eval count remains 0)
    let res_found = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Found".into())),
                keys.clone(),
                values.clone(),
                FunctionArg::Lazy(&dummy_expr),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_found, CellValue::Number(42.0));
    assert_eq!(ctx.count(), 0);

    // When key is NOT found, lazy fallback IS evaluated (eval count becomes 1)
    let res_not_found = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Missing".into())),
                keys,
                values,
                FunctionArg::Lazy(&dummy_expr),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_not_found, CellValue::Number(1.0));
    assert_eq!(ctx.count(), 1);
}

// ============================================================================
// Scenario 4: Reverse search in XLOOKUP and XMATCH
// ============================================================================

#[test]
fn test_scenario_4_reverse_search_xlookup_and_xmatch() {
    let registry = FunctionRegistry::standard();
    let xlookup = registry.lookup("XLOOKUP").unwrap();
    let xmatch = registry.lookup("XMATCH").unwrap();
    let ctx = MockContext::new();

    // Duplicate keys: "A" at index 0 and index 2
    let keys = make_array(
        3,
        1,
        vec![
            CellValue::Text("A".into()),
            CellValue::Text("B".into()),
            CellValue::Text("A".into()),
        ],
    );
    let values = make_array(
        3,
        1,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
        ],
    );

    // Forward search (default): returns first match (1.0)
    let fwd_val = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("A".into())),
                keys.clone(),
                values.clone(),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(fwd_val, CellValue::Number(1.0));

    let fwd_pos = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("A".into())),
                keys.clone(),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(fwd_pos, CellValue::Number(1.0));

    // Reverse search (search_mode = -1): returns last match (3.0, pos 3)
    let rev_val = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("A".into())),
                keys.clone(),
                values,
                FunctionArg::Omitted,
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(rev_val, CellValue::Number(3.0));

    let rev_pos = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("A".into())),
                keys,
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(rev_pos, CellValue::Number(3.0));
}

// ============================================================================
// Scenario 5: Approximate match (next smaller and next larger)
// ============================================================================

#[test]
fn test_scenario_5_approximate_match() {
    let registry = FunctionRegistry::standard();
    let xmatch = registry.lookup("XMATCH").unwrap();
    let xlookup = registry.lookup("XLOOKUP").unwrap();
    let ctx = MockContext::new();

    let brackets = make_array(
        3,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
    );
    let rates = make_array(
        3,
        1,
        vec![
            CellValue::Number(0.05),
            CellValue::Number(0.10),
            CellValue::Number(0.15),
        ],
    );

    // match_mode = -1 (exact or next smaller)
    // 25 -> smaller is 20.0 (index 2 in 1-based, pos 2.0)
    let smaller_pos = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25.0)),
                brackets.clone(),
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(smaller_pos, CellValue::Number(2.0));

    let smaller_val = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25.0)),
                brackets.clone(),
                rates.clone(),
                FunctionArg::Omitted,
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(smaller_val, CellValue::Number(0.10));

    // match_mode = 1 (exact or next larger)
    // 25 -> larger is 30.0 (index 3 in 1-based, pos 3.0)
    let larger_pos = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25.0)),
                brackets.clone(),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(larger_pos, CellValue::Number(3.0));

    let larger_val = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25.0)),
                brackets.clone(),
                rates,
                FunctionArg::Omitted,
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(larger_val, CellValue::Number(0.15));

    // Boundary: value smaller than all items with match_mode = -1 -> #N/A
    let none_smaller = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(5.0)),
                brackets,
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(none_smaller, CellValue::Error(ExcelError::Na));
}

// ============================================================================
// Scenario 6: Strict lookup mode validation and wildcard retirement
// ============================================================================

#[test]
fn test_scenario_6_wildcard_matching() {
    let registry = FunctionRegistry::standard();
    let xmatch = registry.lookup("XMATCH").unwrap();
    let xlookup = registry.lookup("XLOOKUP").unwrap();
    let ctx = MockContext::new();

    let keys = make_array(
        3,
        1,
        vec![
            CellValue::Text("Apple".into()),
            CellValue::Text("Banana".into()),
            CellValue::Text("Apricot".into()),
        ],
    );
    let values = make_array(
        3,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
    );

    // Wildcard match_mode = 2 is retired on both XMATCH and XLOOKUP
    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Ap*".into())),
                keys.clone(),
                FunctionArg::Scalar(CellValue::Number(2.0)),
            ],
            &ctx,
        ),
        Err(FunctionError::InvalidLookupMode {
            function: "XMATCH",
            param: "match_mode",
            position: 3,
            mode: 2,
        })
    );

    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("?anana".into())),
                keys.clone(),
                values.clone(),
                FunctionArg::Omitted,
                FunctionArg::Scalar(CellValue::Number(2.0)),
            ],
            &ctx,
        ),
        Err(FunctionError::InvalidLookupMode {
            function: "XLOOKUP",
            param: "match_mode",
            position: 5,
            mode: 2,
        })
    );

    // Validation order: ParamType::Integer ("abc" -> InvalidArgumentType, 1.5 -> NonIntegerArgument)
    // -> Error scalar -> NonLiteralLookupMode -> invalid integer -> InvalidLookupMode
    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Apple".into())),
                keys.clone(),
                FunctionArg::Scalar(CellValue::Text("abc".into())),
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "XMATCH",
            param: "match_mode",
            position: 3,
            expected: ExpectedType::Integer,
            found: ValueType::Text,
        }))
    );

    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Apple".into())),
                keys.clone(),
                FunctionArg::Scalar(CellValue::Number(1.5)),
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::NonIntegerArgument {
            function: "XMATCH",
            param: "match_mode",
            position: 3,
            value: "1.5".into(),
        }))
    );

    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Apple".into())),
                keys.clone(),
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
            ],
            &ctx,
        ),
        Err(FunctionError::NonLiteralLookupMode {
            function: "XMATCH",
            param: "match_mode",
            position: 3,
        })
    );

    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Apple".into())),
                keys,
                values,
                FunctionArg::Omitted,
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(99.0)),
            ],
            &ctx,
        ),
        Err(FunctionError::InvalidLookupMode {
            function: "XLOOKUP",
            param: "search_mode",
            position: 6,
            mode: 99,
        })
    );
}

// ============================================================================
// Scenario 7: Binary search ascending and descending (clean monotonic keys)
// ============================================================================

#[test]
fn test_scenario_7_binary_search() {
    let registry = FunctionRegistry::standard();
    let xmatch = registry.lookup("XMATCH").unwrap();
    let ctx = MockContext::new();

    // Ascending sorted array: [10, 20, 30, 40, 50]
    let asc = make_array(
        5,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
            CellValue::Number(40.0),
            CellValue::Number(50.0),
        ],
    );

    // search_mode = 2 (binary ascending)
    let asc_res = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(40.0)),
                asc,
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(asc_res, CellValue::Number(4.0));

    // Descending sorted array: [50, 40, 30, 20, 10]
    let desc = make_array(
        5,
        1,
        vec![
            CellValue::Number(50.0),
            CellValue::Number(40.0),
            CellValue::Number(30.0),
            CellValue::Number(20.0),
            CellValue::Number(10.0),
        ],
    );

    // search_mode = -2 (binary descending)
    let desc_res = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(40.0)),
                desc,
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(-2.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(desc_res, CellValue::Number(2.0));

    // Binary search rejects Empty elements in lookup_array with ExpectedType::NonEmpty
    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(20.0)),
                make_array(
                    3,
                    1,
                    vec![
                        CellValue::Number(10.0),
                        CellValue::Empty,
                        CellValue::Number(30.0),
                    ],
                ),
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "XMATCH",
            param: "lookup_array",
            position: 2,
            expected: ExpectedType::NonEmpty,
            found: ValueType::Empty,
        }))
    );

    // Binary search rejects Error elements in lookup_array with ErrorInBinaryLookupArray
    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(20.0)),
                make_array(
                    3,
                    1,
                    vec![
                        CellValue::Number(10.0),
                        CellValue::Error(ExcelError::Div0),
                        CellValue::Number(30.0),
                    ],
                ),
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
            ],
            &ctx,
        ),
        Err(FunctionError::ErrorInBinaryLookupArray {
            function: "XMATCH",
            search_mode: 2,
        })
    );

    // Binary search rejects unsorted ascending and unsorted descending lookup_array keys
    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(20.0)),
                make_array(
                    3,
                    1,
                    vec![
                        CellValue::Number(10.0),
                        CellValue::Number(30.0),
                        CellValue::Number(20.0),
                    ],
                ),
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
            ],
            &ctx,
        ),
        Err(FunctionError::UnsortedBinaryLookupArray {
            function: "XMATCH",
            search_mode: 2,
        })
    );

    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(20.0)),
                make_array(
                    3,
                    1,
                    vec![
                        CellValue::Number(10.0),
                        CellValue::Number(20.0),
                        CellValue::Number(30.0),
                    ],
                ),
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(-2.0)),
            ],
            &ctx,
        ),
        Err(FunctionError::UnsortedBinaryLookupArray {
            function: "XMATCH",
            search_mode: -2,
        })
    );
}

// ============================================================================
// Scenario 8: Dimension mismatch and 2D rejection in XLOOKUP and XMATCH
// ============================================================================

#[test]
fn test_scenario_8_dimension_validation() {
    let registry = FunctionRegistry::standard();
    let xmatch = registry.lookup("XMATCH").unwrap();
    let xlookup = registry.lookup("XLOOKUP").unwrap();
    let ctx = MockContext::new();

    // 2D lookup array (2x2) is rejected strictly with ExpectedOneDimensionalSlice
    let matrix_2d = make_array(
        2,
        2,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
            CellValue::Number(4.0),
        ],
    );

    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(1.0)),
                matrix_2d.clone(),
            ],
            &ctx,
        ),
        Err(FunctionError::ExpectedOneDimensionalSlice {
            function: "XMATCH",
            param: "lookup_array",
            rows: 2,
            cols: 2,
        })
    );

    // Mismatched dimensions between lookup_array (2x1) and return_array (3x1) in XLOOKUP fail strictly
    // (even when lookup_value is an ExcelError)
    let lookup_arr = make_array(2, 1, vec![CellValue::Number(1.0), CellValue::Number(2.0)]);
    let return_arr = make_array(
        3,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
    );

    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                lookup_arr,
                return_arr,
            ],
            &ctx,
        ),
        Err(FunctionError::DimensionMismatch {
            function: "XLOOKUP",
            expected_rows: 2,
            expected_cols: 1,
            found_rows: 3,
            found_cols: 1,
        })
    );

    // Error in lookup_value bubbles immediately
    let err_lookup_val = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                make_array(1, 1, vec![CellValue::Number(1.0)]),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(err_lookup_val, CellValue::Error(ExcelError::Div0));
}

// ============================================================================
// Scenario 9: INDEX on 1D vector and 2D matrix
// ============================================================================

#[test]
fn test_scenario_9_index_dimensions_and_bounds() {
    let registry = FunctionRegistry::standard();
    let index = registry.lookup("INDEX").unwrap();
    let ctx = MockContext::new();

    // 1D row vector (1x3): INDEX(row, 2) treats 2 as column index
    let row_vec = make_array(
        1,
        3,
        vec![
            CellValue::Number(100.0),
            CellValue::Number(200.0),
            CellValue::Number(300.0),
        ],
    );
    let r_val = index
        .evaluate(
            &[row_vec.clone(), FunctionArg::Scalar(CellValue::Number(2.0))],
            &ctx,
        )
        .unwrap();
    assert_eq!(r_val, CellValue::Number(200.0));

    // 1D column vector (3x1): INDEX(col, 3)
    let col_vec = make_array(
        3,
        1,
        vec![
            CellValue::Text("First".into()),
            CellValue::Text("Second".into()),
            CellValue::Text("Third".into()),
        ],
    );
    let c_val = index
        .evaluate(
            &[col_vec, FunctionArg::Scalar(CellValue::Number(3.0))],
            &ctx,
        )
        .unwrap();
    assert_eq!(c_val, CellValue::Text("Third".into()));

    // 2D matrix (2x3):
    // [1, 2, 3]
    // [4, 5, 6]
    let matrix = make_array(
        2,
        3,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
            CellValue::Number(4.0),
            CellValue::Number(5.0),
            CellValue::Number(6.0),
        ],
    );

    // INDEX(matrix, 2, 3) -> 6.0
    let m_val = index
        .evaluate(
            &[
                matrix.clone(),
                FunctionArg::Scalar(CellValue::Number(2.0)),
                FunctionArg::Scalar(CellValue::Number(3.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(m_val, CellValue::Number(6.0));

    // 2D matrix omitting column fails strictly with MissingColumnIndexFor2DSlice
    assert_eq!(
        index.evaluate(
            &[matrix.clone(), FunctionArg::Scalar(CellValue::Number(2.0))],
            &ctx,
        ),
        Err(FunctionError::MissingColumnIndexFor2DSlice {
            function: "INDEX",
            rows: 2,
            cols: 3,
        })
    );

    // Out-of-bounds row (3 > 2) -> #REF!
    let err_oob_row = index
        .evaluate(
            &[
                matrix.clone(),
                FunctionArg::Scalar(CellValue::Number(3.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(err_oob_row, CellValue::Error(ExcelError::Ref));

    // Out-of-bounds col (4 > 3) -> #REF!
    let err_oob_col = index
        .evaluate(
            &[
                matrix,
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(4.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(err_oob_col, CellValue::Error(ExcelError::Ref));
}

// ============================================================================
// Scenario 10: 2D matrix intersection with INDEX + XMATCH
// ============================================================================

#[test]
fn test_scenario_10_index_xmatch_matrix_intersection() {
    let registry = FunctionRegistry::standard();
    let xmatch = registry.lookup("XMATCH").unwrap();
    let index = registry.lookup("INDEX").unwrap();
    let ctx = MockContext::new();

    // Row headers: ["North", "South"]
    let row_headers = make_array(
        2,
        1,
        vec![
            CellValue::Text("North".into()),
            CellValue::Text("South".into()),
        ],
    );

    // Column headers: ["Q1", "Q2", "Q3"]
    let col_headers = make_array(
        1,
        3,
        vec![
            CellValue::Text("Q1".into()),
            CellValue::Text("Q2".into()),
            CellValue::Text("Q3".into()),
        ],
    );

    // Matrix (2x3):
    // North: [100, 110, 120]
    // South: [200, 210, 220]
    let matrix = make_array(
        2,
        3,
        vec![
            CellValue::Number(100.0),
            CellValue::Number(110.0),
            CellValue::Number(120.0),
            CellValue::Number(200.0),
            CellValue::Number(210.0),
            CellValue::Number(220.0),
        ],
    );

    // XMATCH("South", row_headers) -> 2.0
    let row_idx = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("South".into())),
                row_headers,
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(row_idx, CellValue::Number(2.0));

    // XMATCH("Q3", col_headers) -> 3.0
    let col_idx = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("Q3".into())),
                col_headers,
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(col_idx, CellValue::Number(3.0));

    // INDEX(matrix, 2, 3) -> 220.0
    let intersection = index
        .evaluate(
            &[
                matrix,
                FunctionArg::Scalar(row_idx),
                FunctionArg::Scalar(col_idx),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(intersection, CellValue::Number(220.0));
}

// ============================================================================
// End-to-end formula evaluation with model references
// ============================================================================

#[test]
fn test_end_to_end_formula_evaluation() {
    let mut symbols = SymbolTable::new();
    let sheet_id = SheetId::from("sheet1");

    let assumptions = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Assumptions".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("c1"),
            ColumnSpec::new("c2"),
            ColumnSpec::new("c3"),
        ],
        rows: vec![RowSpec::new("headers"), RowSpec::new("values")],
    };
    symbols
        .register_table(sheet_id, &assumptions)
        .expect("registered");

    let registry = FunctionRegistry::standard();
    let mut values = IndexMap::new();

    // Populate table cells:
    // headers: ["A", "B", "C"]
    // values: [100, 200, 300]
    let c_headers_1 = CellId::new("Assumptions", "headers", "c1");
    let c_headers_2 = CellId::new("Assumptions", "headers", "c2");
    let c_headers_3 = CellId::new("Assumptions", "headers", "c3");
    let c_values_1 = CellId::new("Assumptions", "values", "c1");
    let c_values_2 = CellId::new("Assumptions", "values", "c2");
    let c_values_3 = CellId::new("Assumptions", "values", "c3");

    values.insert(c_headers_1, CellValue::Text("A".into()));
    values.insert(c_headers_2, CellValue::Text("B".into()));
    values.insert(c_headers_3, CellValue::Text("C".into()));
    values.insert(c_values_1, CellValue::Number(100.0));
    values.insert(c_values_2, CellValue::Number(200.0));
    values.insert(c_values_3, CellValue::Number(300.0));

    let origin = CellId::new("Assumptions", "values", "c1");

    // XLOOKUP formula: XLOOKUP("B", Assumptions["headers", "c1".."c3"], Assumptions["values", "c1".."c3"])
    let xlookup_res = eval_str(
        r#"XLOOKUP("B", Assumptions["headers", "c1".."c3"], Assumptions["values", "c1".."c3"])"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(xlookup_res, CellValue::Number(200.0));

    // XMATCH formula: XMATCH("C", Assumptions["headers", "c1".."c3"]) -> 3.0
    let xmatch_res = eval_str(
        r#"XMATCH("C", Assumptions["headers", "c1".."c3"])"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(xmatch_res, CellValue::Number(3.0));

    // INDEX formula on 1D horizontal row: INDEX(Assumptions["values", "c1".."c3"], 2) -> 200.0
    let index_res = eval_str(
        r#"INDEX(Assumptions["values", "c1".."c3"], 2)"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(index_res, CellValue::Number(200.0));
}

// ============================================================================
// Excel parity edge cases: type safety, empty vs zero, wildcard numeric, error bubbling
// ============================================================================

#[test]
fn test_excel_parity_edge_cases() {
    let registry = FunctionRegistry::standard();
    let xlookup = registry.lookup("XLOOKUP").unwrap();
    let xmatch = registry.lookup("XMATCH").unwrap();
    let index = registry.lookup("INDEX").unwrap();
    let ctx = MockContext::new();

    // 1. Empty cell separation from 0.0 and ""
    let empty_and_zero_keys = make_array(2, 1, vec![CellValue::Empty, CellValue::Number(0.0)]);
    let values = make_array(2, 1, vec![CellValue::Number(10.0), CellValue::Number(20.0)]);

    let res_zero = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(0.0)),
                empty_and_zero_keys.clone(),
                values.clone(),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_zero, CellValue::Number(20.0));

    let match_zero = xmatch
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(0.0)),
                empty_and_zero_keys.clone(),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(match_zero, CellValue::Number(2.0));

    // Empty lookup_value is strictly rejected with ExpectedType::NonEmpty
    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Empty),
                empty_and_zero_keys.clone(),
                values.clone(),
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "XLOOKUP",
            param: "lookup_value",
            position: 1,
            expected: ExpectedType::NonEmpty,
            found: ValueType::Empty,
        }))
    );

    // Cross-type lookup_value ("") against numeric lookup_array fails with InvalidArgumentType
    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text(String::new())),
                empty_and_zero_keys,
                values,
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "XLOOKUP",
            param: "lookup_array",
            position: 2,
            expected: ExpectedType::Text,
            found: ValueType::Number,
        }))
    );

    // 2. Linear approximate matching skips Empty and Error cells when keys are homogeneous
    let sparse_error_keys = make_array(
        4,
        1,
        vec![
            CellValue::Error(ExcelError::Div0),
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Number(30.0),
        ],
    );
    let sparse_vals = make_array(
        4,
        1,
        vec![
            CellValue::Number(2.0),
            CellValue::Number(3.0),
            CellValue::Number(4.0),
            CellValue::Number(5.0),
        ],
    );

    let approx_smaller = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25.0)),
                sparse_error_keys.clone(),
                sparse_vals.clone(),
                FunctionArg::Omitted,
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(approx_smaller, CellValue::Number(3.0)); // matches 10.0, skipping Div0 and Empty

    let approx_larger = xlookup
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25.0)),
                sparse_error_keys,
                sparse_vals.clone(),
                FunctionArg::Omitted,
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(approx_larger, CellValue::Number(5.0)); // matches 30.0, skipping Div0 and Empty

    // 3. Heterogeneous lookup_array, return_array, and INDEX array are rejected prior to Error bubbling
    let mixed_keys = make_array(
        4,
        1,
        vec![
            CellValue::Text("Zebra".into()),
            CellValue::Error(ExcelError::Div0),
            CellValue::Number(10.0),
            CellValue::Empty,
        ],
    );
    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                mixed_keys,
                sparse_vals,
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "XLOOKUP",
            param: "lookup_array",
            position: 2,
            expected: ExpectedType::Text,
            found: ValueType::Number,
        }))
    );

    let mixed_return = make_array(
        2,
        1,
        vec![CellValue::Number(10.0), CellValue::Text("two".into())],
    );
    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                make_array(
                    2,
                    1,
                    vec![CellValue::Text("a".into()), CellValue::Text("b".into())]
                ),
                mixed_return.clone(),
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "XLOOKUP",
            param: "return_array",
            position: 3,
            expected: ExpectedType::Number,
            found: ValueType::Text,
        }))
    );

    assert_eq!(
        index.evaluate(
            &[
                mixed_return,
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "INDEX",
            param: "array",
            position: 1,
            expected: ExpectedType::Number,
            found: ValueType::Text,
        }))
    );

    // 4. Scalar arguments in ParamKind::Range slots fail with InvalidArgumentKind,
    // while 1x1 SingleCell range references propagate cell errors cleanly
    assert_eq!(
        xlookup.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("A".into())),
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                make_array(1, 1, vec![CellValue::Number(1.0)]),
            ],
            &ctx,
        ),
        Err(FunctionError::InvalidArgumentKind {
            function: "XLOOKUP",
            expected: "range",
            found: "scalar",
        })
    );

    let single_cell_div0 = FunctionArg::array(
        1,
        1,
        vec![CellValue::Error(ExcelError::Div0)],
        ReferenceKind::SingleCell,
    );
    assert_eq!(
        xlookup
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Text("A".into())),
                    make_array(1, 1, vec![CellValue::Text("A".into())]),
                    single_cell_div0.clone(),
                ],
                &ctx,
            )
            .unwrap(),
        CellValue::Error(ExcelError::Div0)
    );

    assert_eq!(
        xmatch.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Text("A".into())),
                FunctionArg::Scalar(CellValue::Error(ExcelError::Ref)),
            ],
            &ctx,
        ),
        Err(FunctionError::InvalidArgumentKind {
            function: "XMATCH",
            expected: "range",
            found: "scalar",
        })
    );

    assert_eq!(
        index.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        ),
        Err(FunctionError::InvalidArgumentKind {
            function: "INDEX",
            expected: "range",
            found: "scalar",
        })
    );

    assert_eq!(
        index
            .evaluate(
                &[
                    single_cell_div0,
                    FunctionArg::Scalar(CellValue::Number(1.0))
                ],
                &ctx,
            )
            .unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
}

#[test]
fn test_xlookup_dormant_if_not_found_static_validation() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");
    let registry = FunctionRegistry::standard();

    let table = TableSpec {
        id: TableId::from("Lookup"),
        title: "Lookup".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("k"), ColumnSpec::new("v")],
        rows: vec![RowSpec::new("r1"), RowSpec::new("r2")],
    };
    symbols.register_table(sheet, &table).unwrap();

    let origin = CellId::new("Lookup", "r1", "v");
    let mut values = IndexMap::new();
    values.insert(CellId::new("Lookup", "r1", "k"), CellValue::Number(1.0));
    values.insert(CellId::new("Lookup", "r2", "k"), CellValue::Number(2.0));
    values.insert(CellId::new("Lookup", "r1", "v"), CellValue::Number(10.0));
    values.insert(CellId::new("Lookup", "r2", "v"), CellValue::Number(20.0));

    // Valid omitted optional `if_not_found` slot succeeds
    assert_eq!(
        eval_str(
            "XLOOKUP(1, Lookup[\"r1\"..\"r2\", \"k\"], Lookup[\"r1\"..\"r2\", \"v\"], _, 0, 1)",
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Number(10.0)
    );

    // Even when key 1 matches and `if_not_found` is dormant, invalid calls inside `if_not_found` fail statically
    let unknown_expr = Expr::parse(
        "XLOOKUP(1, Lookup[\"r1\"..\"r2\", \"k\"], Lookup[\"r1\"..\"r2\", \"v\"], UNKNOWN_FN())",
    )
    .unwrap();
    let err_unknown = eval_expr(&unknown_expr, &origin, &values, &symbols, &registry).unwrap_err();
    let EvalError::Function { cell, source } = err_unknown else {
        panic!("expected EvalError::Function, got {err_unknown:?}");
    };
    assert_eq!(*cell, origin);
    assert_eq!(*source, FunctionError::UnknownFunction("UNKNOWN_FN".into()));

    let arity_expr = Expr::parse(
        "XLOOKUP(1, Lookup[\"r1\"..\"r2\", \"k\"], Lookup[\"r1\"..\"r2\", \"v\"], ABS(1, 2))",
    )
    .unwrap();
    let err_arity = eval_expr(&arity_expr, &origin, &values, &symbols, &registry).unwrap_err();
    let EvalError::Function { cell, source } = err_arity else {
        panic!("expected EvalError::Function, got {err_arity:?}");
    };
    assert_eq!(*cell, origin);
    assert_eq!(
        *source,
        FunctionError::TooManyArguments {
            function: "ABS",
            expected: 1,
            found: 2,
        }
    );
}

#[test]
fn test_lookup_positional_omission_contract() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");
    let registry = FunctionRegistry::standard();

    let table = TableSpec {
        id: TableId::from("Lookup"),
        title: "Lookup".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("k"),
            ColumnSpec::new("v"),
            ColumnSpec::new("c3"),
        ],
        rows: vec![
            RowSpec::new("r1"),
            RowSpec::new("r2"),
            RowSpec::new("r3"),
            RowSpec::new("r_vec"),
        ],
    };
    symbols.register_table(sheet, &table).unwrap();

    let origin = CellId::new("Lookup", "r1", "v");
    let mut values = IndexMap::new();
    // Duplicate "A" keys at r1 (10) and r3 (30) to distinguish forward vs reverse search (-1)
    values.insert(
        CellId::new("Lookup", "r1", "k"),
        CellValue::Text("A".into()),
    );
    values.insert(
        CellId::new("Lookup", "r2", "k"),
        CellValue::Text("B".into()),
    );
    values.insert(
        CellId::new("Lookup", "r3", "k"),
        CellValue::Text("A".into()),
    );
    values.insert(CellId::new("Lookup", "r1", "v"), CellValue::Number(10.0));
    values.insert(CellId::new("Lookup", "r2", "v"), CellValue::Number(20.0));
    values.insert(CellId::new("Lookup", "r3", "v"), CellValue::Number(30.0));
    // Row vector across columns k, v, c3 on r_vec: [100, 200, 300]
    values.insert(
        CellId::new("Lookup", "r_vec", "k"),
        CellValue::Number(100.0),
    );
    values.insert(
        CellId::new("Lookup", "r_vec", "v"),
        CellValue::Number(200.0),
    );
    values.insert(
        CellId::new("Lookup", "r_vec", "c3"),
        CellValue::Number(300.0),
    );

    // Scenario 1: XMATCH and XLOOKUP with intermediate `_` skips
    assert_eq!(
        eval_str(
            "XMATCH(\"A\", Lookup[\"r1\"..\"r3\", \"k\"], _, -1)",
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Number(3.0)
    );
    assert_eq!(
        eval_str(
            "XLOOKUP(\"A\", Lookup[\"r1\"..\"r3\", \"k\"], Lookup[\"r1\"..\"r3\", \"v\"], _, _, -1)",
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Number(30.0)
    );
    assert_eq!(
        eval_str(
            "XLOOKUP(\"MISSING\", Lookup[\"r1\"..\"r3\", \"k\"], Lookup[\"r1\"..\"r3\", \"v\"], _, _, -1)",
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Error(ExcelError::Na)
    );

    // Scenario 2: 2-argument INDEX on 1x3 row vector selects element 2 without trailing padding
    assert_eq!(
        eval_str(
            "INDEX(Lookup[\"r_vec\", \"k\"..\"c3\"], 2)",
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Number(200.0)
    );

    // Scenario 3 & 4: Trailing `_` and required `_` rejected on lookup functions
    let error_cases = [
        (
            "INDEX(Lookup[\"r1\"..\"r3\", \"v\"], 2, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "INDEX",
                param: "col_num",
                position: 3,
            },
        ),
        (
            "XMATCH(10, Lookup[\"r1\"..\"r3\", \"v\"], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XMATCH",
                param: "match_mode",
                position: 3,
            },
        ),
        (
            "XLOOKUP(10, Lookup[\"r1\"..\"r3\", \"k\"], Lookup[\"r1\"..\"r3\", \"v\"], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XLOOKUP",
                param: "if_not_found",
                position: 4,
            },
        ),
        (
            "XLOOKUP(10, Lookup[\"r1\"..\"r3\", \"k\"], Lookup[\"r1\"..\"r3\", \"v\"], _, 0, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XLOOKUP",
                param: "search_mode",
                position: 6,
            },
        ),
        (
            "INDEX(Lookup[\"r1\"..\"r3\", \"v\"], _, 1)",
            FunctionError::CannotOmitRequiredArgument {
                function: "INDEX",
                param: "row_num",
                position: 2,
            },
        ),
    ];

    for (formula, expected_err) in error_cases {
        let expr = Expr::parse(formula).unwrap();
        let err = eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap_err();
        let EvalError::Function { cell, source } = err else {
            panic!("expected EvalError::Function for {formula}, got {err:?}");
        };
        assert_eq!(*cell, origin);
        assert_eq!(*source, expected_err, "mismatch on {formula}");
    }
}
