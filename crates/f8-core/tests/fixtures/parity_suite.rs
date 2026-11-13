//! Canonical Excel calculation parity test fixture covering all 45 registered standard functions,
//! domain error semantics (`#DIV/0!`, `#VALUE!`, `#NUM!`, `#N/A`, `#REF!`), and calculation edge cases.
//!
//! Organizes tests into 7 vertical tables on a single worksheet:
//! 1. `SeedData`: Independent inputs (numbers, categories, dates, cash flows, sparse cells) for ranges and lookups.
//! 2. `MathParity`: 17 standard math and aggregation functions + edge and error cases.
//! 3. `LogicalParity`: 8 logical, branching, and error-handling functions + edge and error cases.
//! 4. `FinancialParity`: 7 discounting, return, and loan amortization functions + edge and error cases.
//! 5. `LookupParity`: 3 modern lookup and index functions + edge and error cases.
//! 6. `DateParity`: 8 serial date construction, decomposition, and difference functions + edge and error cases.
//! 7. `InformationParity`: 2 cell-blank introspection and `#N/A` generation functions + edge and error cases.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test fixture")]

use f8_core::compiler::eval::EvaluatedModel;
use f8_core::model::PlacementSpec;
use f8_core::model::id::{ColId, RowId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, TableSpec};
use f8_core::value::{CellValue, ExcelError};

use super::{RowBuilder, build_descriptor_col, build_measure_col_label};

/// Relative floating-point tolerance (`1e-9`) shared between Rust assertions and OpenXML parity formulas.
const RELATIVE_TOLERANCE: f64 = 1e-9;

/// Pinned Excel-verified ground-truth expectation for a calculation parity test case.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ParityExpected {
    /// Exact numeric equality (`to_bits()` match, distinguishing `+0.0` and `-0.0`).
    ExactNum(f64),
    /// Floating-point numeric equality within `1e-9 * |expected|` relative tolerance.
    ApproxNum(f64),
    /// Exact boolean equality.
    Bool(bool),
    /// Exact string equality.
    Text(&'static str),
    /// Domain error parity (`COUNTIFS(formula_val, expected_val) = 1`) verifying the exact `ExcelError` variant.
    Error(ExcelError),
}

impl ParityExpected {
    fn to_cell_input(self) -> CellInput {
        match self {
            Self::ExactNum(n) | Self::ApproxNum(n) => CellInput::number(n),
            Self::Bool(b) => CellInput::boolean(b),
            Self::Text(s) => CellInput::string(s),
            Self::Error(err) => CellInput::string(err.as_str()),
        }
    }

    fn parity_check_formula(self, table_id: &str) -> String {
        match self {
            Self::ExactNum(_) | Self::Bool(_) | Self::Text(_) => format!(
                "IF(IFERROR({table_id}[@row, \"formula_val\"] = {table_id}[@row, \"expected_val\"], FALSE), \"PASS\", \"FAIL\")"
            ),
            Self::ApproxNum(_) => format!(
                "IF(IFERROR(ABS({table_id}[@row, \"formula_val\"] - {table_id}[@row, \"expected_val\"]) <= 0.000000001 * ABS({table_id}[@row, \"expected_val\"]), FALSE), \"PASS\", \"FAIL\")"
            ),
            Self::Error(_) => format!(
                "IF(COUNTIFS({table_id}[@row, \"formula_val\"], {table_id}[@row, \"expected_val\"]) = 1, \"PASS\", \"FAIL\")"
            ),
        }
    }
}

/// Definition of a single parity test case with pinned Excel ground truth.
struct ParityCase {
    id: &'static str,
    label: &'static str,
    formula: &'static str,
    expected: ParityExpected,
}

/// Category table grouping of parity test cases.
struct ParityGroup {
    table_id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    cases: &'static [ParityCase],
}

const MATH_CASES: &[ParityCase] = &[
    // Core 17 functions
    ParityCase {
        id: "abs",
        label: "ABS(-42.5)",
        formula: "ABS(-42.5)",
        expected: ParityExpected::ExactNum(42.5),
    },
    ParityCase {
        id: "sum",
        label: "SUM(num_a)",
        formula: "SUM(SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::ExactNum(100.0),
    },
    ParityCase {
        id: "average",
        label: "AVERAGE(num_a)",
        formula: "AVERAGE(SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::ApproxNum(25.0),
    },
    ParityCase {
        id: "count",
        label: "COUNT(num_a)",
        formula: "COUNT(SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::ExactNum(4.0),
    },
    ParityCase {
        id: "counta",
        label: "COUNTA(category)",
        formula: "COUNTA(SeedData[\"r1\"..\"r4\", \"category\"])",
        expected: ParityExpected::ExactNum(4.0),
    },
    ParityCase {
        id: "max",
        label: "MAX(num_a)",
        formula: "MAX(SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::ExactNum(40.0),
    },
    ParityCase {
        id: "min",
        label: "MIN(num_a)",
        formula: "MIN(SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::ExactNum(10.0),
    },
    ParityCase {
        id: "median",
        label: "MEDIAN(num_a) [even count]",
        formula: "MEDIAN(SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::ApproxNum(25.0),
    },
    ParityCase {
        id: "round",
        label: "ROUND(123.456, 2)",
        formula: "ROUND(123.456, 2)",
        expected: ParityExpected::ExactNum(123.46),
    },
    ParityCase {
        id: "roundup",
        label: "ROUNDUP(123.451, 2)",
        formula: "ROUNDUP(123.451, 2)",
        expected: ParityExpected::ExactNum(123.46),
    },
    ParityCase {
        id: "rounddown",
        label: "ROUNDDOWN(123.459, 2)",
        formula: "ROUNDDOWN(123.459, 2)",
        expected: ParityExpected::ExactNum(123.45),
    },
    ParityCase {
        id: "sumproduct",
        label: "SUMPRODUCT(num_a, num_b)",
        formula: "SUMPRODUCT(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"num_b\"])",
        expected: ParityExpected::ExactNum(400.0),
    },
    ParityCase {
        id: "sumifs",
        label: "SUMIFS(num_a, cat, \"Alpha\")",
        formula: "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
        expected: ParityExpected::ExactNum(40.0),
    },
    ParityCase {
        id: "countifs",
        label: "COUNTIFS(cat, \"Alpha\")",
        formula: "COUNTIFS(SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "averageifs",
        label: "AVERAGEIFS(num_a, cat, \"Alpha\")",
        formula: "AVERAGEIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
        expected: ParityExpected::ApproxNum(20.0),
    },
    ParityCase {
        id: "minifs",
        label: "MINIFS(num_a, cat, \"Alpha\")",
        formula: "MINIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
        expected: ParityExpected::ExactNum(10.0),
    },
    ParityCase {
        id: "maxifs",
        label: "MAXIFS(num_a, cat, \"Alpha\")",
        formula: "MAXIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
        expected: ParityExpected::ExactNum(30.0),
    },
    // Edge cases
    ParityCase {
        id: "median_odd_edge",
        label: "MEDIAN(r1..r3) [odd count]",
        formula: "MEDIAN(SeedData[\"r1\"..\"r3\", \"num_a\"])",
        expected: ParityExpected::ExactNum(20.0),
    },
    ParityCase {
        id: "sum_all_null",
        label: "SUM(all_null) [sparse all-null slice -> 0]",
        formula: "SUM(SeedData[\"r1\"..\"r4\", \"all_null\"])",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "min_all_null",
        label: "MIN(all_null) [sparse all-null slice -> 0]",
        formula: "MIN(SeedData[\"r1\"..\"r4\", \"all_null\"])",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "max_all_null",
        label: "MAX(all_null) [sparse all-null slice -> 0]",
        formula: "MAX(SeedData[\"r1\"..\"r4\", \"all_null\"])",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "count_all_null",
        label: "COUNT(all_null) [sparse all-null slice -> 0]",
        formula: "COUNT(SeedData[\"r1\"..\"r4\", \"all_null\"])",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "sumproduct_sparse",
        label: "SUMPRODUCT(sparse, num_b) [sparse null contributes 0 -> 80]",
        formula: "SUMPRODUCT(SeedData[\"r1\"..\"r4\", \"sparse\"], SeedData[\"r1\"..\"r4\", \"num_b\"])",
        expected: ParityExpected::ExactNum(80.0),
    },
    ParityCase {
        id: "round_neg_digits",
        label: "ROUND(1250, -2) [negative digits]",
        formula: "ROUND(1250, -2)",
        expected: ParityExpected::ExactNum(1300.0),
    },
    ParityCase {
        id: "round_half_away",
        label: "ROUND(-2.5, 0) [halfway away from zero]",
        formula: "ROUND(-2.5, 0)",
        expected: ParityExpected::ExactNum(-3.0),
    },
    ParityCase {
        id: "round_taken_int_branch",
        label: "ROUND(10, IF(TRUE, 2, 2.5)) [taken integer branch]",
        formula: "ROUND(10, IF(TRUE, 2, 2.5))",
        expected: ParityExpected::ExactNum(10.0),
    },
    ParityCase {
        id: "roundup_negative",
        label: "ROUNDUP(-123.451, 2) [negative away from zero]",
        formula: "ROUNDUP(-123.451, 2)",
        expected: ParityExpected::ExactNum(-123.46),
    },
    ParityCase {
        id: "rounddown_negative",
        label: "ROUNDDOWN(-123.459, 2) [negative toward zero]",
        formula: "ROUNDDOWN(-123.459, 2)",
        expected: ParityExpected::ExactNum(-123.45),
    },
    ParityCase {
        id: "sumifs_wildcard_rel",
        label: "SUMIFS(num_a, cat, \"A*\", num_a, \">15\") [wildcard + relational]",
        formula: "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"A*\", SeedData[\"r1\"..\"r4\", \"num_a\"], \">15\")",
        expected: ParityExpected::ExactNum(30.0),
    },
    ParityCase {
        id: "minifs_empty_match",
        label: "MINIFS(num_a, cat, \"Missing\") [empty match -> 0]",
        formula: "MINIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Missing\")",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "maxifs_empty_match",
        label: "MAXIFS(num_a, cat, \"Missing\") [empty match -> 0]",
        formula: "MAXIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Missing\")",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "count_sparse",
        label: "COUNT(sparse) [null ignored, formula-blank 0 counted]",
        formula: "COUNT(SeedData[\"r1\"..\"r4\", \"sparse\"])",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "counta_sparse",
        label: "COUNTA(sparse) [null ignored, formula-blank 0 counted]",
        formula: "COUNTA(SeedData[\"r1\"..\"r4\", \"sparse\"])",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "average_sparse",
        label: "AVERAGE(sparse) [(0 + 20) / 2 = 10]",
        formula: "AVERAGE(SeedData[\"r1\"..\"r4\", \"sparse\"])",
        expected: ParityExpected::ExactNum(10.0),
    },
    ParityCase {
        id: "countifs_sparse_blank",
        label: "COUNTIFS(sparse, \"\") [matches null literals only]",
        formula: "COUNTIFS(SeedData[\"r1\"..\"r4\", \"sparse\"], \"\")",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "countifs_nan_on_numbers",
        label: "COUNTIFS(num_a, \"NAN\") [ticker NAN ignores numbers]",
        formula: "COUNTIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], \"NAN\")",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "countifs_nan_on_text",
        label: "COUNTIFS(special_text, \"nan\") [case-insensitive text match]",
        formula: "COUNTIFS(SeedData[\"r1\"..\"r4\", \"special_text\"], \"nan\")",
        expected: ParityExpected::ExactNum(1.0),
    },
    ParityCase {
        id: "countifs_num_skips_nan_inf",
        label: "COUNTIFS(special_text, 42) [matches \"42\", skips NAN/Infinity/inf]",
        formula: "COUNTIFS(SeedData[\"r1\"..\"r4\", \"special_text\"], 42)",
        expected: ParityExpected::ExactNum(1.0),
    },
    ParityCase {
        id: "countifs_gt_skips_inf_text",
        label: "COUNTIFS(special_text, \">0\") [relational skips Infinity/inf text]",
        formula: "COUNTIFS(SeedData[\"r1\"..\"r4\", \"special_text\"], \">0\")",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "sumifs_op_trailing_space",
        label: "SUMIFS(num_a, num_a, \"> 15 \") [operator trailing space trimmed]",
        formula: "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"num_a\"], \"> 15 \")",
        expected: ParityExpected::ExactNum(90.0),
    },
    ParityCase {
        id: "neg_zero_norm",
        label: "0 * -1 [normalized +0.0]",
        formula: "0 * -1",
        expected: ParityExpected::ExactNum(0.0),
    },
    // Error cases
    ParityCase {
        id: "div0_arith_err",
        label: "10 / 0 [#DIV/0!]",
        formula: "10 / 0",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "average_empty_err",
        label: "AVERAGE(all_null) [no numeric cells -> #DIV/0!]",
        formula: "AVERAGE(SeedData[\"r1\"..\"r4\", \"all_null\"])",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "averageifs_empty_err",
        label: "AVERAGEIFS(num_a, cat, \"Missing\") [no matches -> #DIV/0!]",
        formula: "AVERAGEIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Missing\")",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "pow_domain_err",
        label: "(-4) ^ 0.5 [negative fractional power -> #NUM!]",
        formula: "(-4) ^ 0.5",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "pow_overflow_err",
        label: "10 ^ 309 [float overflow -> #NUM!]",
        formula: "10 ^ 309",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "median_empty_err",
        label: "MEDIAN(all_null) [no numeric cells -> #NUM!]",
        formula: "MEDIAN(SeedData[\"r1\"..\"r4\", \"all_null\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
];

const LOGICAL_CASES: &[ParityCase] = &[
    // Core 8 functions
    ParityCase {
        id: "if_fn",
        label: "IF(10 > 5, 100, 200)",
        formula: "IF(10 > 5, 100, 200)",
        expected: ParityExpected::ExactNum(100.0),
    },
    ParityCase {
        id: "ifs_fn",
        label: "IFS(10 < 5, 1, 10 > 5, 2)",
        formula: "IFS(10 < 5, 1, 10 > 5, 2)",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "iferror",
        label: "IFERROR(10 / 0, 999)",
        formula: "IFERROR(10 / 0, 999)",
        expected: ParityExpected::ExactNum(999.0),
    },
    ParityCase {
        id: "ifna",
        label: "IFNA(XLOOKUP(\"missing\", ...), 888)",
        formula: "IFNA(XLOOKUP(\"missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"]), 888)",
        expected: ParityExpected::ExactNum(888.0),
    },
    ParityCase {
        id: "and_fn",
        label: "AND(10 > 5, 20 > 10)",
        formula: "AND(10 > 5, 20 > 10)",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "or_fn",
        label: "OR(10 < 5, 20 > 10)",
        formula: "OR(10 < 5, 20 > 10)",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "not_fn",
        label: "NOT(10 < 5)",
        formula: "NOT(10 < 5)",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "choose_fn",
        label: "CHOOSE(2, 100, 200, 300)",
        formula: "CHOOSE(2, 100, 200, 300)",
        expected: ParityExpected::ExactNum(200.0),
    },
    // Edge cases
    ParityCase {
        id: "if_lazy_shortcircuit",
        label: "IF(TRUE, 42, 10 / 0) [lazy branch short-circuit]",
        formula: "IF(TRUE, 42, 10 / 0)",
        expected: ParityExpected::ExactNum(42.0),
    },
    ParityCase {
        id: "if_monomorphic_text",
        label: "IF(10 > 5, \"PASS\", \"FAIL\") [monomorphic text branches]",
        formula: "IF(10 > 5, \"PASS\", \"FAIL\")",
        expected: ParityExpected::Text("PASS"),
    },
    ParityCase {
        id: "if_na_bottom",
        label: "IF(10 > 5, 100 / 2, NA()) [NA() bottom type unifies with Number]",
        formula: "IF(10 > 5, 100 / 2, NA())",
        expected: ParityExpected::ExactNum(50.0),
    },
    ParityCase {
        id: "paren_eq_chain_left",
        label: "(1 = 1) = TRUE [parenthesized relational]",
        formula: "(1 = 1) = TRUE",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "paren_eq_chain_right",
        label: "TRUE = (1 = 1) [parenthesized relational]",
        formula: "TRUE = (1 = 1)",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "case_insensitive_eq",
        label: "\"Alpha\" = \"alpha\" [case-insensitive equality]",
        formula: "\"Alpha\" = \"alpha\"",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "if_blank_guard",
        label: "IF(ISBLANK(sparse[r1]), \"\", sparse[r1]) [explicit blank guard]",
        formula: "IF(ISBLANK(SeedData[\"r1\", \"sparse\"]), \"\", SeedData[\"r1\", \"sparse\"])",
        expected: ParityExpected::Text(""),
    },
    ParityCase {
        id: "if_blank_guard_arith",
        label: "IF(ISBLANK(sparse[r1]), 0, sparse[r1] + 5) [lazy guarded arithmetic]",
        formula: "IF(ISBLANK(SeedData[\"r1\", \"sparse\"]), 0, SeedData[\"r1\", \"sparse\"] + 5)",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "if_helper_blank_guard",
        label: "IF(sparse_is_null[r1], 0, sparse[r1] + 5) [helper-cell ISBLANK guard]",
        formula: "IF(SeedData[\"r1\", \"sparse_is_null\"], 0, SeedData[\"r1\", \"sparse\"] + 5)",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "and_sparse_slice_true",
        label: "AND(sparse_bool[r1..r2]) [skips null in syntactic slice -> TRUE]",
        formula: "AND(SeedData[\"r1\"..\"r2\", \"sparse_bool\"])",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "if_blank_mid_isblank",
        label: "ISBLANK(IF(TRUE, sparse[r1], 1)) [mid-expression IF blank]",
        formula: "ISBLANK(IF(TRUE, SeedData[\"r1\", \"sparse\"], 1))",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "ifs_blank_mid_isblank",
        label: "ISBLANK(IFS(TRUE, sparse[r1])) [mid-expression IFS blank]",
        formula: "ISBLANK(IFS(TRUE, SeedData[\"r1\", \"sparse\"]))",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "choose_blank_mid_isblank",
        label: "ISBLANK(CHOOSE(1, sparse[r1])) [mid-expression CHOOSE blank]",
        formula: "ISBLANK(CHOOSE(1, SeedData[\"r1\", \"sparse\"]))",
        expected: ParityExpected::Bool(true),
    },
    // Error cases
    ParityCase {
        id: "ifna_passthrough_div0",
        label: "IFNA(10 / 0, 888) [non-#N/A error bubbles -> #DIV/0!]",
        formula: "IFNA(10 / 0, 888)",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "ifs_no_match_err",
        label: "IFS(10 < 5, 1, 20 < 10, 2) [no true branch -> #N/A]",
        formula: "IFS(10 < 5, 1, 20 < 10, 2)",
        expected: ParityExpected::Error(ExcelError::Na),
    },
    ParityCase {
        id: "choose_dyn_oob_err",
        label: "CHOOSE(num_b[r4], 100, 200, 300) [dynamic index 5 out of bounds -> #VALUE!]",
        formula: "CHOOSE(SeedData[\"r4\", \"num_b\"], 100, 200, 300)",
        expected: ParityExpected::Error(ExcelError::Value),
    },
    ParityCase {
        id: "and_all_empty_slice_err",
        label: "AND(sparse_bool[r3..r4]) [all-empty slice -> #VALUE!]",
        formula: "AND(SeedData[\"r3\"..\"r4\", \"sparse_bool\"])",
        expected: ParityExpected::Error(ExcelError::Value),
    },
];

const FINANCIAL_CASES: &[ParityCase] = &[
    // Core 7 functions
    ParityCase {
        id: "npv",
        label: "NPV(0.1, cash_flows)",
        formula: "NPV(0.1, SeedData[\"r2\"..\"r4\", \"cash_flow\"])",
        expected: ParityExpected::ApproxNum(1_054.094_665_664_913_5),
    },
    ParityCase {
        id: "xnpv",
        label: "XNPV(0.1, cash_flows, dates)",
        formula: "XNPV(0.1, SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
        expected: ParityExpected::ApproxNum(145.355_849_538_689_94),
    },
    ParityCase {
        id: "irr",
        label: "IRR(cash_flows)",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"])",
        expected: ParityExpected::ApproxNum(0.127_147_484_418_566_38),
    },
    ParityCase {
        id: "xirr",
        label: "XIRR(cash_flows, dates)",
        formula: "XIRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
        expected: ParityExpected::ApproxNum(0.222_183_448_100_928_05),
    },
    ParityCase {
        id: "pmt",
        label: "PMT(0.06/12, 36, -10000)",
        formula: "PMT(0.06 / 12, 36, -10000)",
        expected: ParityExpected::ApproxNum(304.219_374_515_557_15),
    },
    ParityCase {
        id: "ipmt",
        label: "IPMT(0.06/12, 1, 36, -10000)",
        formula: "IPMT(0.06 / 12, 1, 36, -10000)",
        expected: ParityExpected::ApproxNum(50.0),
    },
    ParityCase {
        id: "ppmt",
        label: "PPMT(0.06/12, 1, 36, -10000)",
        formula: "PPMT(0.06 / 12, 1, 36, -10000)",
        expected: ParityExpected::ApproxNum(254.219_374_515_557_15),
    },
    // Edge cases
    ParityCase {
        id: "irr_custom_guess",
        label: "IRR(cash_flows, 0.25) [custom initial guess]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], 0.25)",
        expected: ParityExpected::ApproxNum(0.127_147_484_418_566_38),
    },
    ParityCase {
        id: "pmt_zero_rate",
        label: "PMT(0, 10, -1000) [zero interest rate]",
        formula: "PMT(0, 10, -1000)",
        expected: ParityExpected::ExactNum(100.0),
    },
    ParityCase {
        id: "pmt_annuity_due_fv",
        label: "PMT(0.06/12, 36, -10000, 2000, 1) [fv + type=1]",
        formula: "PMT(0.06 / 12, 36, -10000, 2000, 1)",
        expected: ParityExpected::ApproxNum(252.114_924_987_508_2),
    },
    ParityCase {
        id: "ipmt_mid_period",
        label: "IPMT(0.06/12, 18, 36, -10000) [mid-schedule period]",
        formula: "IPMT(0.06 / 12, 18, 36, -10000)",
        expected: ParityExpected::ApproxNum(27.505_015_529_156_335),
    },
    ParityCase {
        id: "ppmt_mid_period",
        label: "PPMT(0.06/12, 18, 36, -10000) [mid-schedule period]",
        formula: "PPMT(0.06 / 12, 18, 36, -10000)",
        expected: ParityExpected::ApproxNum(276.714_358_986_400_84),
    },
    // Error cases
    ParityCase {
        id: "irr_no_sign_change_err",
        label: "IRR(r2..r4) [all positive cash flows -> #NUM!]",
        formula: "IRR(SeedData[\"r2\"..\"r4\", \"cash_flow\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xirr_no_sign_change_err",
        label: "XIRR(r2..r4, dates) [all positive cash flows -> #NUM!]",
        formula: "XIRR(SeedData[\"r2\"..\"r4\", \"cash_flow\"], SeedData[\"r2\"..\"r4\", \"date\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xnpv_invalid_rate_err",
        label: "XNPV(-1.0, cash_flows, dates) [rate <= -1 -> #NUM!]",
        formula: "XNPV(-1.0, SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "ipmt_per_oob_err",
        label: "IPMT(0.06/12, 40, 36, -10000) [per > nper -> #NUM!]",
        formula: "IPMT(0.06 / 12, 40, 36, -10000)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "ppmt_per_zero_err",
        label: "PPMT(0.06/12, 0, 36, -10000) [per < 1 -> #NUM!]",
        formula: "PPMT(0.06 / 12, 0, 36, -10000)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
];

const LOOKUP_CASES: &[ParityCase] = &[
    // Core 3 functions
    ParityCase {
        id: "xlookup",
        label: "XLOOKUP(\"Beta\", cat, num_a)",
        formula: "XLOOKUP(\"Beta\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::ExactNum(20.0),
    },
    ParityCase {
        id: "xmatch",
        label: "XMATCH(\"Beta\", cat)",
        formula: "XMATCH(\"Beta\", SeedData[\"r1\"..\"r4\", \"category\"])",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "index_fn",
        label: "INDEX(num_a, 3)",
        formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], 3)",
        expected: ParityExpected::ExactNum(30.0),
    },
    // Edge cases
    ParityCase {
        id: "xlookup_fallback_edge",
        label: "XLOOKUP(\"Missing\", cat, num_a, -1) [if_not_found fallback]",
        formula: "XLOOKUP(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], -1)",
        expected: ParityExpected::ExactNum(-1.0),
    },
    ParityCase {
        id: "xlookup_reverse_edge",
        label: "XLOOKUP(\"Alpha\", cat, num_a, _, 0, -1) [reverse search]",
        formula: "XLOOKUP(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], _, 0, -1)",
        expected: ParityExpected::ExactNum(30.0),
    },
    ParityCase {
        id: "xlookup_text_edge",
        label: "XLOOKUP(30, num_a, category) [text return value]",
        formula: "XLOOKUP(30, SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"])",
        expected: ParityExpected::Text("Alpha"),
    },
    ParityCase {
        id: "xmatch_next_smaller",
        label: "XMATCH(25, num_a, -1) [exact or next smaller]",
        formula: "XMATCH(25, SeedData[\"r1\"..\"r4\", \"num_a\"], -1)",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "xmatch_binary_asc",
        label: "XMATCH(30, num_a, 0, 2) [binary ascending]",
        formula: "XMATCH(30, SeedData[\"r1\"..\"r4\", \"num_a\"], 0, 2)",
        expected: ParityExpected::ExactNum(3.0),
    },
    ParityCase {
        id: "xlookup_binary_asc",
        label: "XLOOKUP(25, num_a, category, \"Miss\", -1, 2) [binary ascending next smaller]",
        formula: "XLOOKUP(25, SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Miss\", -1, 2)",
        expected: ParityExpected::Text("Beta"),
    },
    ParityCase {
        id: "xmatch_binary_desc",
        label: "XMATCH(20, num_desc, 0, -2) [binary descending]",
        formula: "XMATCH(20, SeedData[\"r1\"..\"r4\", \"num_desc\"], 0, -2)",
        expected: ParityExpected::ExactNum(3.0),
    },
    ParityCase {
        id: "xlookup_binary_desc",
        label: "XLOOKUP(25, num_desc, category, \"Miss\", 1, -2) [binary descending next larger]",
        formula: "XLOOKUP(25, SeedData[\"r1\"..\"r4\", \"num_desc\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Miss\", 1, -2)",
        expected: ParityExpected::Text("Beta"),
    },
    ParityCase {
        id: "index_sparse_homogeneous",
        label: "INDEX(sparse, 3) [homogeneous sparse numeric array]",
        formula: "INDEX(SeedData[\"r1\"..\"r4\", \"sparse\"], 3)",
        expected: ParityExpected::ExactNum(20.0),
    },
    ParityCase {
        id: "index_2d_edge",
        label: "INDEX(num_a..num_b, 2, 2) [2D matrix lookup]",
        formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"..\"num_b\"], 2, 2)",
        expected: ParityExpected::ExactNum(3.0),
    },
    ParityCase {
        id: "index_horiz_1d",
        label: "INDEX(r1[num_a..num_b], 2) [2-arg horizontal 1D lookup]",
        formula: "INDEX(SeedData[\"r1\", \"num_a\"..\"num_b\"], 2)",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "index_blank_mid_isblank",
        label: "ISBLANK(INDEX(sparse, 1)) [mid-expression INDEX blank preserved]",
        formula: "ISBLANK(INDEX(SeedData[\"r1\"..\"r4\", \"sparse\"], 1))",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "xlookup_blank_mid_isblank",
        label: "ISBLANK(XLOOKUP(\"Alpha\", cat, sparse, 0)) [mid-expression blank preserved]",
        formula: "ISBLANK(XLOOKUP(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"sparse\"], 0))",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "xlookup_miss_isblank_false",
        label: "ISBLANK(XLOOKUP(\"Missing\", cat, sparse, 0)) [fallback 0 is non-blank]",
        formula: "ISBLANK(XLOOKUP(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"sparse\"], 0))",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "xlookup_zero_skips_null_hits_formula",
        label: "XLOOKUP(0, sparse, num_a) [skips null r1, matches guarded 0 r2]",
        formula: "XLOOKUP(0, SeedData[\"r1\"..\"r4\", \"sparse\"], SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::ExactNum(20.0),
    },
    // Error cases
    ParityCase {
        id: "xlookup_missing_err",
        label: "XLOOKUP(\"Missing\", cat, num_a) [unmatched -> #N/A]",
        formula: "XLOOKUP(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"])",
        expected: ParityExpected::Error(ExcelError::Na),
    },
    ParityCase {
        id: "xmatch_missing_err",
        label: "XMATCH(\"Missing\", cat) [unmatched -> #N/A]",
        formula: "XMATCH(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"])",
        expected: ParityExpected::Error(ExcelError::Na),
    },
    ParityCase {
        id: "index_dyn_oob_err",
        label: "INDEX(num_a, num_b[r4]) [dynamic row 5 out of bounds -> #REF!]",
        formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r4\", \"num_b\"])",
        expected: ParityExpected::Error(ExcelError::Ref),
    },
];

const DATE_CASES: &[ParityCase] = &[
    // Core 8 functions
    ParityCase {
        id: "date_fn",
        label: "DATE(2024, 6, 15)",
        formula: "DATE(2024, 6, 15)",
        expected: ParityExpected::ExactNum(45_458.0),
    },
    ParityCase {
        id: "edate",
        label: "EDATE(DATE(2024, 1, 31), 1) [leap year Feb clamp]",
        formula: "EDATE(DATE(2024, 1, 31), 1)",
        expected: ParityExpected::ExactNum(45_351.0),
    },
    ParityCase {
        id: "eomonth",
        label: "EOMONTH(DATE(2024, 1, 15), 1)",
        formula: "EOMONTH(DATE(2024, 1, 15), 1)",
        expected: ParityExpected::ExactNum(45_351.0),
    },
    ParityCase {
        id: "yearfrac",
        label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0) [US 30/360]",
        formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0)",
        expected: ParityExpected::ApproxNum(0.5),
    },
    ParityCase {
        id: "year_fn",
        label: "YEAR(DATE(2024, 6, 15))",
        formula: "YEAR(DATE(2024, 6, 15))",
        expected: ParityExpected::ExactNum(2024.0),
    },
    ParityCase {
        id: "month_fn",
        label: "MONTH(DATE(2024, 6, 15))",
        formula: "MONTH(DATE(2024, 6, 15))",
        expected: ParityExpected::ExactNum(6.0),
    },
    ParityCase {
        id: "day_fn",
        label: "DAY(DATE(2024, 6, 15))",
        formula: "DAY(DATE(2024, 6, 15))",
        expected: ParityExpected::ExactNum(15.0),
    },
    ParityCase {
        id: "days_fn",
        label: "DAYS(DATE(2024, 6, 15), DATE(2024, 6, 1))",
        formula: "DAYS(DATE(2024, 6, 15), DATE(2024, 6, 1))",
        expected: ParityExpected::ExactNum(14.0),
    },
    // Edge cases
    ParityCase {
        id: "lotus_leap_day_bug",
        label: "DATE(1900, 2, 29) [1900 leap day bug serial 60]",
        formula: "DATE(1900, 2, 29)",
        expected: ParityExpected::ExactNum(60.0),
    },
    ParityCase {
        id: "days_across_1900_bug",
        label: "DAYS(DATE(1900, 3, 1), DATE(1900, 2, 28)) [spans serial 60]",
        formula: "DAYS(DATE(1900, 3, 1), DATE(1900, 2, 28))",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "edate_nonleap_clamp",
        label: "EDATE(DATE(2025, 1, 31), 1) [non-leap Feb 28 clamp]",
        formula: "EDATE(DATE(2025, 1, 31), 1)",
        expected: ParityExpected::ExactNum(45_716.0),
    },
    ParityCase {
        id: "eomonth_neg_offset",
        label: "EOMONTH(DATE(2024, 3, 15), -1) [negative month offset]",
        formula: "EOMONTH(DATE(2024, 3, 15), -1)",
        expected: ParityExpected::ExactNum(45_351.0),
    },
    ParityCase {
        id: "yearfrac_act_act",
        label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 1) [Actual/Actual]",
        formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 1)",
        expected: ParityExpected::ApproxNum(0.497_267_759_562_841_5),
    },
    ParityCase {
        id: "yearfrac_act_360",
        label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 2) [Actual/360]",
        formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 2)",
        expected: ParityExpected::ApproxNum(0.505_555_555_555_555_5),
    },
    ParityCase {
        id: "yearfrac_act_365",
        label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 3) [Actual/365]",
        formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 3)",
        expected: ParityExpected::ApproxNum(0.498_630_136_986_301_4),
    },
    // Error cases
    ParityCase {
        id: "edate_underflow_err",
        label: "EDATE(DATE(1900, 1, 15), -2) [steps before 1900 -> #NUM!]",
        formula: "EDATE(DATE(1900, 1, 15), -2)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "eomonth_overflow_err",
        label: "EOMONTH(DATE(9999, 12, 1), 2) [steps past 9999 -> #NUM!]",
        formula: "EOMONTH(DATE(9999, 12, 1), 2)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "year_edate_bubble_err",
        label: "YEAR(EDATE(DATE(1900, 1, 15), -2)) [upstream #NUM! bubbles]",
        formula: "YEAR(EDATE(DATE(1900, 1, 15), -2))",
        expected: ParityExpected::Error(ExcelError::Num),
    },
];

const INFORMATION_CASES: &[ParityCase] = &[
    // Core 2 functions (ISBLANK, NA) + edge and error cases
    ParityCase {
        id: "isblank_null",
        label: "ISBLANK(sparse[r1]) [null literal -> TRUE]",
        formula: "ISBLANK(SeedData[\"r1\", \"sparse\"])",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "isblank_guarded_zero",
        label: "ISBLANK(sparse[r2]) [guarded formula 0 -> FALSE]",
        formula: "ISBLANK(SeedData[\"r2\", \"sparse\"])",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "isblank_empty_str",
        label: "ISBLANK(\"\") [empty string -> FALSE]",
        formula: "ISBLANK(\"\")",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "isblank_zero",
        label: "ISBLANK(0) [numeric zero -> FALSE]",
        formula: "ISBLANK(0)",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "isblank_false",
        label: "ISBLANK(FALSE) [boolean false -> FALSE]",
        formula: "ISBLANK(FALSE)",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "isblank_div0",
        label: "ISBLANK(10 / 0) [domain error does not bubble -> FALSE]",
        formula: "ISBLANK(10 / 0)",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "na_trapped_ifna",
        label: "IFNA(IF(FALSE, 10, NA()), 0) [trapped NA() -> 0]",
        formula: "IFNA(IF(FALSE, 10, NA()), 0)",
        expected: ParityExpected::ExactNum(0.0),
    },
    // Error cases
    ParityCase {
        id: "na_fn",
        label: "NA() [explicit #N/A]",
        formula: "NA()",
        expected: ParityExpected::Error(ExcelError::Na),
    },
    ParityCase {
        id: "na_propagated",
        label: "1 + IF(FALSE, 10, NA()) [propagated #N/A]",
        formula: "1 + IF(FALSE, 10, NA())",
        expected: ParityExpected::Error(ExcelError::Na),
    },
];

const PARITY_GROUPS: &[ParityGroup] = &[
    ParityGroup {
        table_id: "MathParity",
        title: "Math & Aggregation Functions (17 Functions + Edge & Error Cases)",
        subtitle: "ABS, SUM, AVERAGE, COUNT, COUNTA, MAX, MIN, MEDIAN, ROUND, ROUNDUP, ROUNDDOWN, SUMPRODUCT, *IFS",
        cases: MATH_CASES,
    },
    ParityGroup {
        table_id: "LogicalParity",
        title: "Logical & Branching Functions (8 Functions + Edge & Error Cases)",
        subtitle: "IF, IFS, IFERROR, IFNA, AND, OR, NOT, CHOOSE",
        cases: LOGICAL_CASES,
    },
    ParityGroup {
        table_id: "FinancialParity",
        title: "Financial & Return Functions (7 Functions + Edge & Error Cases)",
        subtitle: "NPV, XNPV, IRR, XIRR, PMT, IPMT, PPMT",
        cases: FINANCIAL_CASES,
    },
    ParityGroup {
        table_id: "LookupParity",
        title: "Lookup & Reference Functions (3 Functions + Edge & Error Cases)",
        subtitle: "XLOOKUP, XMATCH, INDEX",
        cases: LOOKUP_CASES,
    },
    ParityGroup {
        table_id: "DateParity",
        title: "Date Construction & Difference Functions (8 Functions + Edge & Error Cases)",
        subtitle: "DATE, EDATE, EOMONTH, YEARFRAC, YEAR, MONTH, DAY, DAYS",
        cases: DATE_CASES,
    },
    ParityGroup {
        table_id: "InformationParity",
        title: "Information Functions (2 Functions + Edge & Error Cases)",
        subtitle: "ISBLANK, NA",
        cases: INFORMATION_CASES,
    },
];

fn build_category_table(group: &ParityGroup) -> TableSpec {
    let cols = vec![
        build_descriptor_col("func_desc", "Function / Test"),
        build_measure_col_label("formula_val", "Excel Formula"),
        build_measure_col_label("expected_val", "Expected (Excel)"),
        build_measure_col_label("parity_status", "Parity Status"),
    ];

    let rows = group
        .cases
        .iter()
        .map(|case| {
            let check_expr = case.expected.parity_check_formula(group.table_id);
            RowBuilder::descriptor(case.id, "func_desc", case.label)
                .cell_formula("formula_val", case.formula)
                .cell("expected_val", case.expected.to_cell_input())
                .cell_formula("parity_status", &check_expr)
                .build()
        })
        .collect();

    TableSpec {
        id: TableId::from(group.table_id),
        title: group.title.to_string(),
        subtitle: Some(group.subtitle.to_string()),
        currency: None,
        collapsible: false,
        placement: PlacementSpec::default(),
        columns: cols,
        rows,
    }
}

/// Constructs the complete 45-function Excel parity verification model with pinned ground-truth constants.
#[must_use]
pub fn build_parity_model() -> (ProjectManifest, Vec<TableSpec>) {
    let mut sheet_tables = Vec::with_capacity(1 + PARITY_GROUPS.len());
    sheet_tables.push(TableId::from("SeedData"));
    for group in PARITY_GROUPS {
        sheet_tables.push(TableId::from(group.table_id));
    }

    let manifest = ProjectManifest {
        model_name: "ExcelParityVerification".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "ParityVerification".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: sheet_tables,
        }],
    };

    let seed_cols = vec![
        build_descriptor_col("row_id", "Row"),
        build_measure_col_label("num_a", "Vector A"),
        build_measure_col_label("num_b", "Vector B"),
        build_measure_col_label("num_desc", "Vector Desc"),
        build_measure_col_label("category", "Category"),
        build_measure_col_label("date", "Date"),
        build_measure_col_label("cash_flow", "Cash Flow"),
        build_measure_col_label("sparse", "Sparse"),
        build_measure_col_label("all_null", "All Null"),
        build_measure_col_label("sparse_is_null", "Sparse Is Null"),
        build_measure_col_label("sparse_bool", "Sparse Bool"),
        build_measure_col_label("special_text", "Special Text"),
    ];

    let seed_rows = vec![
        RowBuilder::descriptor("r1", "row_id", "Row 1")
            .cell_num("num_a", 10.0)
            .cell_num("num_b", 2.0)
            .cell_num("num_desc", 40.0)
            .cell_str("category", "Alpha")
            .cell_num("date", 45292.0) // 2024-01-01
            .cell_num("cash_flow", -1000.0)
            .cell("sparse", CellInput::null())
            .cell("all_null", CellInput::null())
            .cell_formula("sparse_is_null", "ISBLANK(SeedData[@row, \"sparse\"])")
            .cell("sparse_bool", CellInput::null())
            .cell_str("special_text", "NAN")
            .build(),
        RowBuilder::descriptor("r2", "row_id", "Row 2")
            .cell_num("num_a", 20.0)
            .cell_num("num_b", 3.0)
            .cell_num("num_desc", 30.0)
            .cell_str("category", "Beta")
            .cell_num("date", 45474.0) // 2024-07-01
            .cell_num("cash_flow", 300.0)
            .cell_formula(
                "sparse",
                "IF(ISBLANK(SeedData[\"r1\", \"sparse\"]), 0, SeedData[\"r1\", \"sparse\"])",
            )
            .cell("all_null", CellInput::null())
            .cell_formula("sparse_is_null", "ISBLANK(SeedData[@row, \"sparse\"])")
            .cell("sparse_bool", CellInput::boolean(true))
            .cell_str("special_text", "Infinity")
            .build(),
        RowBuilder::descriptor("r3", "row_id", "Row 3")
            .cell_num("num_a", 30.0)
            .cell_num("num_b", 4.0)
            .cell_num("num_desc", 20.0)
            .cell_str("category", "Alpha")
            .cell_num("date", 45658.0) // 2025-01-01
            .cell_num("cash_flow", 400.0)
            .cell_num("sparse", 20.0)
            .cell("all_null", CellInput::null())
            .cell_formula("sparse_is_null", "ISBLANK(SeedData[@row, \"sparse\"])")
            .cell("sparse_bool", CellInput::null())
            .cell_str("special_text", "42")
            .build(),
        RowBuilder::descriptor("r4", "row_id", "Row 4")
            .cell_num("num_a", 40.0)
            .cell_num("num_b", 5.0)
            .cell_num("num_desc", 10.0)
            .cell_str("category", "Gamma")
            .cell_num("date", 46023.0) // 2026-01-01
            .cell_num("cash_flow", 600.0)
            .cell("sparse", CellInput::null())
            .cell("all_null", CellInput::null())
            .cell_formula("sparse_is_null", "ISBLANK(SeedData[@row, \"sparse\"])")
            .cell("sparse_bool", CellInput::null())
            .cell_str("special_text", "inf")
            .build(),
    ];

    let seed_table = TableSpec {
        id: TableId::from("SeedData"),
        title: "Seed Data".to_string(),
        subtitle: Some("Baseline inputs for ranges, lookups, and cash flows".to_string()),
        currency: None,
        collapsible: false,
        placement: PlacementSpec::default(),
        columns: seed_cols,
        rows: seed_rows,
    };

    let mut tables = Vec::with_capacity(1 + PARITY_GROUPS.len());
    tables.push(seed_table);
    for group in PARITY_GROUPS {
        tables.push(build_category_table(group));
    }

    (manifest, tables)
}

/// Asserts that every parity case in `evaluated` matches its pinned Excel ground-truth constant,
/// every `parity_status` formula evaluates to `"PASS"`, and no unexpected domain errors exist.
pub fn assert_evaluated_parity(evaluated: &EvaluatedModel) {
    let formula_col = ColId::from("formula_val");
    let expected_col = ColId::from("expected_val");
    let status_col = ColId::from("parity_status");
    let mut expected_error_count = 0usize;

    for group in PARITY_GROUPS {
        let table_id = TableId::from(group.table_id);
        for case in group.cases {
            let row_id = RowId::from(case.id);
            let actual = evaluated
                .get_value(&table_id, &row_id, &formula_col)
                .unwrap_or_else(|| {
                    panic!(
                        "Missing evaluated formula_val for case '{}' in table '{}'",
                        case.id, group.table_id
                    )
                });

            match case.expected {
                ParityExpected::ExactNum(expected) => match actual {
                    CellValue::Number(n) => {
                        assert!(
                            n.to_bits() == expected.to_bits(),
                            "Exact numeric mismatch for '{}' in '{}': expected {expected:?} (bits={:#x}), got {n:?} (bits={:#x})",
                            case.id,
                            group.table_id,
                            expected.to_bits(),
                            n.to_bits(),
                        );
                    }
                    CellValue::Text(_)
                    | CellValue::Boolean(_)
                    | CellValue::Empty
                    | CellValue::Error(_) => {
                        panic!(
                            "Type mismatch for '{}' in '{}': expected Number({expected:?}), got {actual:?}",
                            case.id, group.table_id
                        );
                    }
                },
                ParityExpected::ApproxNum(expected) => match actual {
                    CellValue::Number(n) => {
                        let diff = (*n - expected).abs();
                        let max_err = RELATIVE_TOLERANCE * expected.abs();
                        assert!(
                            n.is_finite() && diff <= max_err,
                            "Approximate numeric mismatch for '{}' in '{}': expected {expected:.15}, got {n:.15} (abs_diff={diff:.3e}, max_allowed={max_err:.3e})",
                            case.id,
                            group.table_id,
                        );
                    }
                    CellValue::Text(_)
                    | CellValue::Boolean(_)
                    | CellValue::Empty
                    | CellValue::Error(_) => {
                        panic!(
                            "Type mismatch for '{}' in '{}': expected approx Number({expected:?}), got {actual:?}",
                            case.id, group.table_id
                        );
                    }
                },
                ParityExpected::Bool(expected) => match actual {
                    CellValue::Boolean(b) => {
                        assert_eq!(
                            *b, expected,
                            "Boolean mismatch for '{}' in '{}'",
                            case.id, group.table_id
                        );
                    }
                    CellValue::Number(_)
                    | CellValue::Text(_)
                    | CellValue::Empty
                    | CellValue::Error(_) => {
                        panic!(
                            "Type mismatch for '{}' in '{}': expected Boolean({expected}), got {actual:?}",
                            case.id, group.table_id
                        );
                    }
                },
                ParityExpected::Text(expected) => match actual {
                    CellValue::Text(s) => {
                        assert_eq!(
                            s.as_str(),
                            expected,
                            "Text mismatch for '{}' in '{}'",
                            case.id,
                            group.table_id
                        );
                    }
                    CellValue::Number(_)
                    | CellValue::Boolean(_)
                    | CellValue::Empty
                    | CellValue::Error(_) => {
                        panic!(
                            "Type mismatch for '{}' in '{}': expected Text({expected:?}), got {actual:?}",
                            case.id, group.table_id
                        );
                    }
                },
                ParityExpected::Error(expected_err) => {
                    expected_error_count += 1;
                    match actual {
                        CellValue::Error(actual_err) => {
                            assert_eq!(
                                *actual_err, expected_err,
                                "Domain error mismatch for '{}' in '{}'",
                                case.id, group.table_id
                            );
                        }
                        CellValue::Number(_)
                        | CellValue::Text(_)
                        | CellValue::Boolean(_)
                        | CellValue::Empty => {
                            panic!(
                                "Expected error {expected_err} for '{}' in '{}', got {actual:?}",
                                case.id, group.table_id
                            );
                        }
                    }
                    let recorded = evaluated.get_value(&table_id, &row_id, &expected_col);
                    assert_eq!(
                        recorded,
                        Some(&CellValue::Text(expected_err.as_str().to_string())),
                        "Error case '{}' in '{}' must store canonical error text in expected_val",
                        case.id,
                        group.table_id
                    );
                }
            }

            let status = evaluated.get_value(&table_id, &row_id, &status_col);
            assert_eq!(
                status,
                Some(&CellValue::Text("PASS".to_string())),
                "In-sheet parity_status failed for '{}' in '{}'",
                case.id,
                group.table_id
            );
        }
    }

    let all_errors = evaluated.find_errors();
    assert_eq!(
        all_errors.len(),
        expected_error_count,
        "Unexpected domain error cells in parity model: {all_errors:?}"
    );
}
