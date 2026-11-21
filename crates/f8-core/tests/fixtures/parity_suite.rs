//! Canonical Excel calculation parity test fixture covering all 37 registered standard functions,
//! domain error semantics (`#DIV/0!`, `#VALUE!`, `#NUM!`, `#N/A`, `#REF!`), and calculation edge cases.
//!
//! Organizes tests across two worksheets:
//! - Sheet 1 (`ParityVerification`): 7 vertical tables where live Excel matches mathematical ground truth:
//!   1. `SeedData`: Independent inputs (numbers, categories, dates, cash flows, sparse cells) for ranges and lookups.
//!   2. `MathParity`: 14 standard math and aggregation functions + edge and error cases.
//!   3. `LogicalParity`: 6 logical, branching, and error-handling functions + edge and error cases.
//!   4. `FinancialParity`: 5 discounting, return, and annuity functions + edge and error cases.
//!   5. `LookupParity`: 3 modern lookup and index functions + edge and error cases.
//!   6. `DateParity`: 7 serial date construction and decomposition functions + edge and error cases.
//!   7. `InformationParity`: 2 error introspection and `#N/A` generation functions + edge and error cases.
//! - Sheet 2 (`ExcelDivergences`): `RuntimeDivergences` documenting live Pass 4b formulas where `f8`
//!   enforces strict calendar dates, date serials `>= 1`, exact discrete integers, binary-search
//!   monotonicity, and mathematical `IRR`/`XIRR` ground truth over legacy Excel quirks.

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
    /// Domain error parity (`ISERROR(formula_val)` in live Excel sheet + exact `ExcelError` variant assertion in Rust).
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
            Self::Error(_) => {
                format!("IF(ISERROR({table_id}[@row, \"formula_val\"]), \"PASS\", \"FAIL\")")
            }
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

/// Intentional runtime divergence case where `f8` Pass 4b differs from a legacy Excel calculation/error quirk.
struct DivergenceCase {
    id: &'static str,
    label: &'static str,
    formula: &'static str,
    expected: ParityExpected,
    excel_quirk: &'static str,
    rationale: &'static str,
}

const MATH_CASES: &[ParityCase] = &[
    // Core 14 functions
    ParityCase {
        id: "abs",
        label: "ABS(-42.5)",
        formula: "ABS(-42.5)",
        expected: ParityExpected::ExactNum(42.5),
    },
    ParityCase {
        id: "ln",
        label: "LN(10)",
        formula: "LN(10)",
        expected: ParityExpected::ApproxNum(std::f64::consts::LN_10),
    },
    ParityCase {
        id: "exp",
        label: "EXP(1)",
        formula: "EXP(1)",
        expected: ParityExpected::ApproxNum(std::f64::consts::E),
    },
    ParityCase {
        id: "mod_fn",
        label: "MOD(7, 3)",
        formula: "MOD(7, 3)",
        expected: ParityExpected::ExactNum(1.0),
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
        id: "round_dec_1005",
        label: "ROUND(1.005, 2) [base-10 tie -> 1.01]",
        formula: "ROUND(1.005, 2)",
        expected: ParityExpected::ExactNum(1.01),
    },
    ParityCase {
        id: "round_dec_2675",
        label: "ROUND(2.675, 2) [base-10 tie -> 2.68]",
        formula: "ROUND(2.675, 2)",
        expected: ParityExpected::ExactNum(2.68),
    },
    ParityCase {
        id: "roundup_exact_007",
        label: "ROUNDUP(0.07, 2) [no binary drift -> 0.07]",
        formula: "ROUNDUP(0.07, 2)",
        expected: ParityExpected::ExactNum(0.07),
    },
    ParityCase {
        id: "rounddown_exact_029",
        label: "ROUNDDOWN(0.29, 2) [no binary drift -> 0.29]",
        formula: "ROUNDDOWN(0.29, 2)",
        expected: ParityExpected::ExactNum(0.29),
    },
    ParityCase {
        id: "round_large_digits_noop",
        label: "ROUND(100, 308) [> 15 sig digits no-op -> 100]",
        formula: "ROUND(100, 308)",
        expected: ParityExpected::ExactNum(100.0),
    },
    ParityCase {
        id: "round_small_exp_22",
        label: "ROUND(1.2345 * (10 ^ -20), 22) [tiny exponent decimal round]",
        formula: "ROUND(1.2345 * (10 ^ -20), 22)",
        expected: ParityExpected::ExactNum(1.23e-20),
    },
    ParityCase {
        id: "count_sparse",
        label: "COUNT(sparse) [null ignored, formula 0 counted]",
        formula: "COUNT(SeedData[\"r1\"..\"r4\", \"sparse\"])",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "average_sparse",
        label: "AVERAGE(sparse) [(0 + 20) / 2 = 10]",
        formula: "AVERAGE(SeedData[\"r1\"..\"r4\", \"sparse\"])",
        expected: ParityExpected::ExactNum(10.0),
    },
    ParityCase {
        id: "neg_zero_norm",
        label: "0 * -1 [normalized +0.0]",
        formula: "0 * -1",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "pow_unary_neg_prec",
        label: "-2 ^ 2 [unary negation outside power -> -4]",
        formula: "-2 ^ 2",
        expected: ParityExpected::ExactNum(-4.0),
    },
    ParityCase {
        id: "pow_paren_neg_base",
        label: "(-2) ^ 2 [parenthesized negative base -> 4]",
        formula: "(-2) ^ 2",
        expected: ParityExpected::ExactNum(4.0),
    },
    ParityCase {
        id: "pow_right_assoc",
        label: "2 ^ 3 ^ 2 [right-associative power -> 512]",
        formula: "2 ^ 3 ^ 2",
        expected: ParityExpected::ExactNum(512.0),
    },
    ParityCase {
        id: "pow_left_grouped",
        label: "(2 ^ 3) ^ 2 [left-grouped power -> 64]",
        formula: "(2 ^ 3) ^ 2",
        expected: ParityExpected::ExactNum(64.0),
    },
    ParityCase {
        id: "pow_unary_neg_exp_assoc",
        label: "2 ^ -2 ^ 3 [2 ^ -(2 ^ 3) -> 0.00390625]",
        formula: "2 ^ -2 ^ 3",
        expected: ParityExpected::ExactNum(0.003_906_25),
    },
    ParityCase {
        id: "sci_pos_exp",
        label: "1.5E+6 + 0.25E+4 [scientific positive exponents -> 1502500]",
        formula: "1.5E+6 + 0.25E+4",
        expected: ParityExpected::ExactNum(1_502_500.0),
    },
    ParityCase {
        id: "sci_neg_exp_sub",
        label: "3e-9 - 1e-9 [scientific negative exponents -> 2e-9]",
        formula: "3e-9 - 1e-9",
        expected: ParityExpected::ApproxNum(2e-9),
    },
    ParityCase {
        id: "sci_scale_cancel",
        label: "1.5e-9 * 1e9 [scientific inverse scaling -> 1.5]",
        formula: "1.5e-9 * 1e9",
        expected: ParityExpected::ApproxNum(1.5),
    },
    ParityCase {
        id: "sci_large_threshold",
        label: "1e15 / 1E9 [scientific 1e15 boundary -> 1000000]",
        formula: "1e15 / 1E9",
        expected: ParityExpected::ExactNum(1_000_000.0),
    },
    ParityCase {
        id: "sci_zero_exp_norm",
        label: "0e-5 [scientific zero normalized to +0.0]",
        formula: "0e-5",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "ln_one",
        label: "LN(1) [exact +0.0]",
        formula: "LN(1)",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "ln_exp_identity",
        label: "LN(EXP(1)) [inverse identity -> 1]",
        formula: "LN(EXP(1))",
        expected: ParityExpected::ApproxNum(1.0),
    },
    ParityCase {
        id: "ln_half",
        label: "LN(0.5) [-ln(2)]",
        formula: "LN(0.5)",
        expected: ParityExpected::ApproxNum(-std::f64::consts::LN_2),
    },
    ParityCase {
        id: "exp_zero",
        label: "EXP(0) [exact 1]",
        formula: "EXP(0)",
        expected: ParityExpected::ExactNum(1.0),
    },
    ParityCase {
        id: "exp_neg_one",
        label: "EXP(-1) [1 / e]",
        formula: "EXP(-1)",
        expected: ParityExpected::ApproxNum(1.0 / std::f64::consts::E),
    },
    ParityCase {
        id: "exp_underflow",
        label: "EXP(-1000) [clean underflow -> +0.0]",
        formula: "EXP(-1000)",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "mod_neg_dividend",
        label: "MOD(-7, 3) [floored divisor sign -> 2]",
        formula: "MOD(-7, 3)",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "mod_neg_divisor",
        label: "MOD(7, -3) [floored divisor sign -> -2]",
        formula: "MOD(7, -3)",
        expected: ParityExpected::ExactNum(-2.0),
    },
    ParityCase {
        id: "mod_both_neg",
        label: "MOD(-7, -3) [both negative -> -1]",
        formula: "MOD(-7, -3)",
        expected: ParityExpected::ExactNum(-1.0),
    },
    ParityCase {
        id: "mod_exact_div",
        label: "MOD(6, 3) [exact multiple -> +0.0]",
        formula: "MOD(6, 3)",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "mod_neg_exact_div",
        label: "MOD(-6, 3) [negative exact multiple -> +0.0]",
        formula: "MOD(-6, 3)",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "mod_zero_neg_div",
        label: "MOD(0, -3) [zero dividend -> +0.0]",
        formula: "MOD(0, -3)",
        expected: ParityExpected::ExactNum(0.0),
    },
    ParityCase {
        id: "mod_frac_pos",
        label: "MOD(5.5, 2) [dyadic fraction -> 1.5]",
        formula: "MOD(5.5, 2)",
        expected: ParityExpected::ExactNum(1.5),
    },
    ParityCase {
        id: "mod_frac_neg_num",
        label: "MOD(-5.5, 2) [negative dyadic fraction -> 0.5]",
        formula: "MOD(-5.5, 2)",
        expected: ParityExpected::ExactNum(0.5),
    },
    ParityCase {
        id: "mod_frac_neg_div",
        label: "MOD(5.5, -2) [negative divisor dyadic fraction -> -0.5]",
        formula: "MOD(5.5, -2)",
        expected: ParityExpected::ExactNum(-0.5),
    },
    ParityCase {
        id: "mod_frac_unit",
        label: "MOD(3.25, 1) [unit fractional part -> 0.25]",
        formula: "MOD(3.25, 1)",
        expected: ParityExpected::ExactNum(0.25),
    },
    // Error cases
    ParityCase {
        id: "div0_arith_err",
        label: "10 / 0 [#DIV/0!]",
        formula: "10 / 0",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "pow_zero_neg_exp_err",
        label: "0 ^ -2 [zero base negative exponent -> #DIV/0!]",
        formula: "0 ^ -2",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "average_empty_err",
        label: "AVERAGE(all_null) [no numeric cells -> #DIV/0!]",
        formula: "AVERAGE(SeedData[\"r1\"..\"r4\", \"all_null\"])",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "pow_zero_zero_err",
        label: "0 ^ 0 [zero to zero power -> #NUM!]",
        formula: "0 ^ 0",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "pow_neg_zero_zero_err",
        label: "(-0) ^ 0 [negative zero to zero power -> #NUM!]",
        formula: "(-0) ^ 0",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "pow_zero_neg_zero_err",
        label: "0 ^ (-0) [zero to negative zero power -> #NUM!]",
        formula: "0 ^ (-0)",
        expected: ParityExpected::Error(ExcelError::Num),
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
    ParityCase {
        id: "roundup_overflow_neg309_err",
        label: "ROUNDUP(1, -309) [10^309 overflow -> #NUM!]",
        formula: "ROUNDUP(1, -309)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "roundup_overflow_max_err",
        label: "ROUNDUP(1.5 * (10 ^ 308), -308) [2e308 overflow -> #NUM!]",
        formula: "ROUNDUP(1.5 * (10 ^ 308), -308)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "ln_zero_err",
        label: "LN(0) [non-positive domain -> #NUM!]",
        formula: "LN(0)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "ln_neg_err",
        label: "LN(-5) [negative domain -> #NUM!]",
        formula: "LN(-5)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "exp_overflow_err",
        label: "EXP(710) [float overflow -> #NUM!]",
        formula: "EXP(710)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "mod_div0_err",
        label: "MOD(5, 0) [zero divisor -> #DIV/0!]",
        formula: "MOD(5, 0)",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "mod_zero_div0_err",
        label: "MOD(0, 0) [zero divisor -> #DIV/0!]",
        formula: "MOD(0, 0)",
        expected: ParityExpected::Error(ExcelError::Div0),
    },
    ParityCase {
        id: "mod_huge_quotient_err",
        label: "MOD(1e20, 3) [huge quotient -> #NUM!]",
        formula: "MOD(1e20, 3)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
];

const LOGICAL_CASES: &[ParityCase] = &[
    // Core 6 functions
    ParityCase {
        id: "if_fn",
        label: "IF(10 > 5, 100, 200)",
        formula: "IF(10 > 5, 100, 200)",
        expected: ParityExpected::ExactNum(100.0),
    },
    ParityCase {
        id: "iferror",
        label: "IFERROR(10 / 0, 999)",
        formula: "IFERROR(10 / 0, 999)",
        expected: ParityExpected::ExactNum(999.0),
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
        id: "and_sparse_slice_true",
        label: "AND(sparse_bool[r1..r2]) [skips null in syntactic slice -> TRUE]",
        formula: "AND(SeedData[\"r1\"..\"r2\", \"sparse_bool\"])",
        expected: ParityExpected::Bool(true),
    },
    // Error cases
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
    // Core 5 functions
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
    // Edge cases
    ParityCase {
        id: "irr_custom_guess",
        label: "IRR(cash_flows, 0.25) [custom initial guess]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], 0.25)",
        expected: ParityExpected::ApproxNum(0.127_147_484_418_566_38),
    },
    ParityCase {
        id: "irr_leading_zero",
        label: "IRR([0, 0, -100, 110]) [leading zero cash flows]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cf_lead_zero\"])",
        expected: ParityExpected::ApproxNum(0.1),
    },
    ParityCase {
        id: "irr_multi_root_low_guess",
        label: "IRR([-1600, 10000, -10000, 0]) [default guess -> 0.25 root]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cf_multi_root\"])",
        expected: ParityExpected::ApproxNum(0.25),
    },
    ParityCase {
        id: "irr_multi_root_high_guess",
        label: "IRR([-1600, 10000, -10000, 0], 3) [high guess -> 4.0 root]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cf_multi_root\"], 3)",
        expected: ParityExpected::ApproxNum(4.0),
    },
    ParityCase {
        id: "irr_salvage_wipeout",
        label: "IRR([-1000000, 1, 0, 0]) [severe-loss root near -1.0]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cf_salvage\"])",
        expected: ParityExpected::ApproxNum(-0.999_999),
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
    // Error cases
    ParityCase {
        id: "irr_no_sign_change_err",
        label: "IRR(r2..r4) [all positive cash flows -> #NUM!]",
        formula: "IRR(SeedData[\"r2\"..\"r4\", \"cash_flow\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "irr_rootless_err",
        label: "IRR([100, -10, 100, 0]) [no real root -> #NUM!]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cf_rootless\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xirr_no_sign_change_err",
        label: "XIRR(r2..r4, dates) [all positive cash flows -> #NUM!]",
        formula: "XIRR(SeedData[\"r2\"..\"r4\", \"cash_flow\"], SeedData[\"r2\"..\"r4\", \"date\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xirr_guess_neg_one_err",
        label: "XIRR(cash_flows, dates, -1.0) [guess <= -1 -> #NUM!]",
        formula: "XIRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"], -1.0)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xirr_guess_lt_neg_one_err",
        label: "XIRR(cash_flows, dates, -1.5) [guess <= -1 -> #NUM!]",
        formula: "XIRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"], -1.5)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xirr_rootless_err",
        label: "XIRR([100, -10, 100, 0], dates) [no real root -> #NUM!]",
        formula: "XIRR(SeedData[\"r1\"..\"r4\", \"cf_rootless\"], SeedData[\"r1\"..\"r4\", \"date\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xnpv_zero_rate_err",
        label: "XNPV(0, cash_flows, dates) [rate <= 0 -> #NUM!]",
        formula: "XNPV(0, SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xnpv_neg_rate_err",
        label: "XNPV(-0.05, cash_flows, dates) [rate <= 0 -> #NUM!]",
        formula: "XNPV(-0.05, SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xnpv_invalid_rate_err",
        label: "XNPV(-1.0, cash_flows, dates) [rate <= 0 -> #NUM!]",
        formula: "XNPV(-1.0, SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "pmt_zero_nper_err",
        label: "PMT(0.05, 0, -1000) [nper = 0 -> #NUM!]",
        formula: "PMT(0.05, 0, -1000)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "pmt_rate_neg_one_err",
        label: "PMT(-1.0, 5, -1000) [rate <= -1 -> #NUM!]",
        formula: "PMT(-1.0, 5, -1000)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "xnpv_neg_date_serial_err",
        label: "XNPV(0.1, dates, cash_flows) [negative date serial -1000 -> #NUM!]",
        formula: "XNPV(0.1, SeedData[\"r1\"..\"r4\", \"date\"], SeedData[\"r1\"..\"r4\", \"cash_flow\"])",
        expected: ParityExpected::Error(ExcelError::Num),
    },
];

const DIVERGENCE_CASES: &[DivergenceCase] = &[
    // IRR & XIRR solver and domain symmetry
    DivergenceCase {
        id: "xirr_leading_zero",
        label: "XIRR([0, 0, -100, 110], dates) [leading zero cash flows]",
        formula: "XIRR(SeedData[\"r1\"..\"r4\", \"cf_lead_zero\"], SeedData[\"r1\"..\"r4\", \"date\"])",
        expected: ParityExpected::ApproxNum(0.1),
        excel_quirk: "2.98023E-09",
        rationale: "Leading zero cash flows on early dates do not alter NPV; true mathematical IRR is 0.10 (Excel stalls near 0)",
    },
    DivergenceCase {
        id: "xirr_salvage_wipeout",
        label: "XIRR([-1000000, 1], r1..r2 dates) [severe-loss root near -1.0]",
        formula: "XIRR(SeedData[\"r1\"..\"r2\", \"cf_salvage\"], SeedData[\"r1\"..\"r2\", \"date\"])",
        expected: ParityExpected::ApproxNum(-0.999_999_999_999_073_1),
        excel_quirk: "#NUM!",
        rationale: "Exact 182-day root (1e-6)^(365/182) - 1 = -0.9999999999990731 exists in (-1, +inf) (Excel unbracketed Newton overshoots -1)",
    },
    DivergenceCase {
        id: "irr_guess_neg_one_err",
        label: "IRR(cash_flows, -1.0) [guess <= -1 -> #NUM!]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], -1.0)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "#VALUE!",
        rationale: "Discount factor 1 + rate <= 0 is a numeric domain pole (#NUM!), matching XIRR, XNPV, and PMT (Excel IRR returns #VALUE!)",
    },
    DivergenceCase {
        id: "irr_guess_lt_neg_one_err",
        label: "IRR(cash_flows, -1.5) [guess <= -1 -> #NUM!]",
        formula: "IRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], -1.5)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "#VALUE!",
        rationale: "Discount factor 1 + rate <= 0 is a numeric domain pole (#NUM!), matching XIRR, XNPV, and PMT (Excel IRR returns #VALUE!)",
    },
    // Strict calendar DATE(year, month, day): no 2-digit year expansion or month/day rollover
    DivergenceCase {
        id: "date_two_digit_year_err",
        label: "DATE(24, 12, 31) [2-digit year < 1900 -> #NUM!]",
        formula: "DATE(24, 12, 31)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "9132 (1924-12-31)",
        rationale: "Requires explicit 4-digit year in 1900..=9999; rejects Lotus/Excel implicit 1900 + year expansion on 0..=1899",
    },
    DivergenceCase {
        id: "date_month_overflow_rollover_err",
        label: "DATE(2024, 14, 1) [month > 12 rollover -> #NUM!]",
        formula: "DATE(2024, 14, 1)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "45689 (2025-02-01)",
        rationale: "Requires calendar month in 1..=12; rejects silent forward rollover into subsequent year (use EDATE for month offsets)",
    },
    DivergenceCase {
        id: "date_month_zero_rollover_err",
        label: "DATE(2024, 0, 1) [month < 1 rollover -> #NUM!]",
        formula: "DATE(2024, 0, 1)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "45261 (2023-12-01)",
        rationale: "Requires calendar month in 1..=12; rejects silent backward rollover into prior year",
    },
    DivergenceCase {
        id: "date_day_overflow_30day_month_err",
        label: "DATE(2024, 6, 31) [invalid June 31 rollover -> #NUM!]",
        formula: "DATE(2024, 6, 31)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "45474 (2024-07-01)",
        rationale: "Requires calendar day in 1..=days_in_month; rejects moving 1 day forward to July 1 on nonexistent June 31",
    },
    DivergenceCase {
        id: "date_day_overflow_nonleap_feb29_err",
        label: "DATE(2025, 2, 29) [non-leap Feb 29 rollover -> #NUM!]",
        formula: "DATE(2025, 2, 29)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "45717 (2025-03-01)",
        rationale: "2025 is not a leap year; rejects moving 1 day forward to March 1 on nonexistent 2025-02-29",
    },
    DivergenceCase {
        id: "date_day_zero_rollover_err",
        label: "DATE(2024, 1, 0) [day < 1 backward rollover -> #NUM!]",
        formula: "DATE(2024, 1, 0)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "45291 (2023-12-31)",
        rationale: "Requires calendar day in 1..=days_in_month; rejects moving 1 day backward to prior month-end on day 0",
    },
    // Input date serial 0 (1900-01-00) rejection across date & scheduled cash-flow functions
    DivergenceCase {
        id: "year_serial_zero_err",
        label: "YEAR(0) [serial 0 (1900-01-00) -> #NUM!]",
        formula: "YEAR(0)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "1900",
        rationale: "Input date serials must be >= 1 (1900-01-01); rejects fictitious Lotus serial 0 (1900-01-00) from zero/uninitialized cells",
    },
    DivergenceCase {
        id: "month_serial_zero_err",
        label: "MONTH(0) [serial 0 (1900-01-00) -> #NUM!]",
        formula: "MONTH(0)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "1",
        rationale: "Input date serials must be >= 1 (1900-01-01); rejects fictitious Lotus serial 0 (1900-01-00)",
    },
    DivergenceCase {
        id: "day_serial_zero_err",
        label: "DAY(0) [serial 0 (1900-01-00) -> #NUM!]",
        formula: "DAY(0)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "0",
        rationale: "Calendar day 0 does not exist; rejects fictitious Lotus serial 0 (1900-01-00) where Excel returns day 0",
    },
    DivergenceCase {
        id: "edate_serial_zero_err",
        label: "EDATE(0, 1) [serial 0 start_date -> #NUM!]",
        formula: "EDATE(0, 1)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "31 (1900-01-31)",
        rationale: "start_date must be a valid calendar serial in 1..=2958465; rejects stepping from fictitious 1900-01-00",
    },
    DivergenceCase {
        id: "eomonth_serial_zero_err",
        label: "EOMONTH(0, 0) [serial 0 start_date -> #NUM!]",
        formula: "EOMONTH(0, 0)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "31 (1900-01-31)",
        rationale: "start_date must be a valid calendar serial in 1..=2958465; rejects computing month-end from fictitious 1900-01-00",
    },
    DivergenceCase {
        id: "yearfrac_serial_zero_err",
        label: "YEARFRAC(0, DATE(1900, 7, 1), 0) [serial 0 start_date -> #NUM!]",
        formula: "YEARFRAC(0, DATE(1900, 7, 1), 0)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "0.502777778",
        rationale: "start_date and end_date must be valid calendar serials in 1..=2958465; rejects day-count fractions anchored at 1900-01-00",
    },
    DivergenceCase {
        id: "xnpv_serial_zero_date_err",
        label: "XNPV(0.1, r1..r2 cf, [0, 20] dates) [serial 0 in dates -> #NUM!]",
        formula: "XNPV(0.1, SeedData[\"r1\"..\"r2\", \"cash_flow\"], SeedData[\"r2\"..\"r3\", \"sparse\"])",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "-701.559724",
        rationale: "Scheduled cash-flow dates in XNPV and XIRR must be valid date serials in 1..=2958465; rejects serial 0",
    },
    DivergenceCase {
        id: "xirr_serial_zero_date_err",
        label: "XIRR(r1..r2 cf, [0, 20] dates) [serial 0 in dates -> #NUM!]",
        formula: "XIRR(SeedData[\"r1\"..\"r2\", \"cash_flow\"], SeedData[\"r2\"..\"r3\", \"sparse\"])",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "#NUM! / -0.999999999",
        rationale: "Scheduled cash-flow dates in XNPV and XIRR must be valid date serials in 1..=2958465; rejects serial 0",
    },
    // Strict discrete integers (strict_integer) and binary annuity timing flag
    DivergenceCase {
        id: "round_non_int_digits_err",
        label: "ROUND(123.456, 2.5) [non-integer num_digits -> #NUM!]",
        formula: "ROUND(123.456, 2.5)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "123.46",
        rationale: "Discrete integer parameter num_digits requires exact integer (fract() == 0.0); rejects silent truncation of 2.5 to 2",
    },
    DivergenceCase {
        id: "roundup_non_int_digits_err",
        label: "ROUNDUP(123.456, 2.5) [non-integer num_digits -> #NUM!]",
        formula: "ROUNDUP(123.456, 2.5)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "123.46",
        rationale: "Discrete integer parameter num_digits requires exact integer; rejects silent truncation of 2.5 to 2",
    },
    DivergenceCase {
        id: "rounddown_non_int_digits_err",
        label: "ROUNDDOWN(123.456, 2.5) [non-integer num_digits -> #NUM!]",
        formula: "ROUNDDOWN(123.456, 2.5)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "123.45",
        rationale: "Discrete integer parameter num_digits requires exact integer; rejects silent truncation of 2.5 to 2",
    },
    DivergenceCase {
        id: "date_non_int_component_err",
        label: "DATE(2024, 6, 15.7) [non-integer day component -> #NUM!]",
        formula: "DATE(2024, 6, 15.7)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "45458 (2024-06-15)",
        rationale: "Calendar year, month, and day components require exact integers; rejects silent fractional truncation",
    },
    DivergenceCase {
        id: "edate_non_int_months_err",
        label: "EDATE(DATE(2024, 1, 15), 1.9) [non-integer months -> #NUM!]",
        formula: "EDATE(DATE(2024, 1, 15), 1.9)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "45337 (2024-02-15)",
        rationale: "Month offset requires exact integer; rejects silent truncation of 1.9 months to 1 month",
    },
    DivergenceCase {
        id: "yearfrac_non_int_basis_err",
        label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0.7) [non-integer basis -> #NUM!]",
        formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0.7)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "0.5",
        rationale: "Day-count convention basis code requires exact integer in 0..=4; rejects silent truncation of 0.7 to basis 0",
    },
    DivergenceCase {
        id: "choose_non_int_index_err",
        label: "CHOOSE(1.9, 100, 200, 300) [non-integer index_num -> #NUM!]",
        formula: "CHOOSE(1.9, 100, 200, 300)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "100",
        rationale: "1-based branch selector index_num requires exact integer; rejects silent truncation of 1.9 to branch 1",
    },
    DivergenceCase {
        id: "index_non_int_row_err",
        label: "INDEX(num_a, 2.8) [non-integer row_num -> #NUM!]",
        formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], 2.8)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "20",
        rationale: "1-based coordinate index row_num requires exact integer; rejects silent truncation of 2.8 to row 2",
    },
    DivergenceCase {
        id: "pmt_invalid_type_err",
        label: "PMT(0.05, 10, -1000, 0, 2) [annuity type outside {0, 1} -> #NUM!]",
        formula: "PMT(0.05, 10, -1000, 0, 2)",
        expected: ParityExpected::Error(ExcelError::Num),
        excel_quirk: "123.33769",
        rationale: "Annuity payment timing flag type must be 0 (end) or 1 (beginning); rejects Excel quirk treating any non-zero number as 1",
    },
    // Lookup wildcard exclusion, binary-search monotonicity/blank checks, and 1-based INDEX
    DivergenceCase {
        id: "xlookup_wildcard_mode_err",
        label: "XLOOKUP(\"G*\", category, num_a, _, 2) [wildcard match_mode=2 -> #VALUE!]",
        formula: "XLOOKUP(\"G*\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], _, 2)",
        expected: ParityExpected::Error(ExcelError::Value),
        excel_quirk: "40",
        rationale: "Wildcard pattern matching (match_mode = 2) is excluded so string lookup keys always compare literally",
    },
    DivergenceCase {
        id: "xmatch_wildcard_mode_err",
        label: "XMATCH(\"G*\", category, 2) [wildcard match_mode=2 -> #VALUE!]",
        formula: "XMATCH(\"G*\", SeedData[\"r1\"..\"r4\", \"category\"], 2)",
        expected: ParityExpected::Error(ExcelError::Value),
        excel_quirk: "4",
        rationale: "Wildcard pattern matching (match_mode = 2) is excluded so string lookup keys always compare literally",
    },
    DivergenceCase {
        id: "xmatch_binary_asc_unsorted_err",
        label: "XMATCH(\"Alpha\", category, 0, 2) [unsorted binary ascending -> #VALUE!]",
        formula: "XMATCH(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], 0, 2)",
        expected: ParityExpected::Error(ExcelError::Value),
        excel_quirk: "1",
        rationale: "Binary search (search_mode = 2) requires monotonically non-decreasing keys; rejects unsorted array instead of bisecting silently",
    },
    DivergenceCase {
        id: "xmatch_binary_desc_unsorted_err",
        label: "XMATCH(20, num_a, 0, -2) [ascending array in binary descending -> #VALUE!]",
        formula: "XMATCH(20, SeedData[\"r1\"..\"r4\", \"num_a\"], 0, -2)",
        expected: ParityExpected::Error(ExcelError::Value),
        excel_quirk: "#N/A",
        rationale: "Binary search (search_mode = -2) requires monotonically non-increasing keys; returns #VALUE! instead of a false #N/A miss",
    },
    DivergenceCase {
        id: "xmatch_binary_sparse_empty_err",
        label: "XMATCH(20, sparse, 0, 2) [empty cell in binary lookup_array -> #VALUE!]",
        formula: "XMATCH(20, SeedData[\"r1\"..\"r4\", \"sparse\"], 0, 2)",
        expected: ParityExpected::Error(ExcelError::Value),
        excel_quirk: "3",
        rationale: "Binary search requires every lookup_array element to be non-empty so bisection ordering is total and well-defined",
    },
    DivergenceCase {
        id: "index_zero_row_err",
        label: "INDEX(num_a, 0) [row_num = 0 whole-vector slice -> #REF!]",
        formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], 0)",
        expected: ParityExpected::Error(ExcelError::Ref),
        excel_quirk: "Spill / #VALUE!",
        rationale: "Enforces strict 1-based scalar indexing (row_num >= 1); dynamic array slicing via index 0 is excluded",
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
        id: "xmatch_binary_asc_dup_exact",
        label: "XMATCH(20, num_dup_asc, 0, 2) [binary asc exact duplicate -> first]",
        formula: "XMATCH(20, SeedData[\"r1\"..\"r4\", \"num_dup_asc\"], 0, 2)",
        expected: ParityExpected::ExactNum(1.0),
    },
    ParityCase {
        id: "xlookup_binary_asc_dup_smaller_hit",
        label: "XLOOKUP(20, num_dup_asc, num_a, -1, -1, 2) [binary asc -1 duplicate hit -> first]",
        formula: "XLOOKUP(20, SeedData[\"r1\"..\"r4\", \"num_dup_asc\"], SeedData[\"r1\"..\"r4\", \"num_a\"], -1, -1, 2)",
        expected: ParityExpected::ExactNum(10.0),
    },
    ParityCase {
        id: "xlookup_binary_asc_dup_larger_hit",
        label: "XLOOKUP(20, num_dup_asc, num_a, -1, 1, 2) [binary asc 1 duplicate hit -> first]",
        formula: "XLOOKUP(20, SeedData[\"r1\"..\"r4\", \"num_dup_asc\"], SeedData[\"r1\"..\"r4\", \"num_a\"], -1, 1, 2)",
        expected: ParityExpected::ExactNum(10.0),
    },
    ParityCase {
        id: "xmatch_binary_asc_dup_smaller_miss",
        label: "XMATCH(30, num_dup_asc, -1, 2) [binary asc -1 miss -> last smaller]",
        formula: "XMATCH(30, SeedData[\"r1\"..\"r4\", \"num_dup_asc\"], -1, 2)",
        expected: ParityExpected::ExactNum(2.0),
    },
    ParityCase {
        id: "xmatch_binary_asc_dup_larger_miss",
        label: "XMATCH(30, num_dup_asc, 1, 2) [binary asc 1 miss -> first larger]",
        formula: "XMATCH(30, SeedData[\"r1\"..\"r4\", \"num_dup_asc\"], 1, 2)",
        expected: ParityExpected::ExactNum(3.0),
    },
    ParityCase {
        id: "xmatch_binary_desc_dup_exact",
        label: "XMATCH(20, num_dup_desc, 0, -2) [binary desc exact duplicate -> last]",
        formula: "XMATCH(20, SeedData[\"r1\"..\"r4\", \"num_dup_desc\"], 0, -2)",
        expected: ParityExpected::ExactNum(4.0),
    },
    ParityCase {
        id: "xlookup_binary_desc_dup_smaller_hit",
        label: "XLOOKUP(20, num_dup_desc, num_a, -1, -1, -2) [binary desc -1 duplicate hit -> last]",
        formula: "XLOOKUP(20, SeedData[\"r1\"..\"r4\", \"num_dup_desc\"], SeedData[\"r1\"..\"r4\", \"num_a\"], -1, -1, -2)",
        expected: ParityExpected::ExactNum(40.0),
    },
    ParityCase {
        id: "xlookup_binary_desc_dup_larger_hit",
        label: "XLOOKUP(20, num_dup_desc, num_a, -1, 1, -2) [binary desc 1 duplicate hit -> last]",
        formula: "XLOOKUP(20, SeedData[\"r1\"..\"r4\", \"num_dup_desc\"], SeedData[\"r1\"..\"r4\", \"num_a\"], -1, 1, -2)",
        expected: ParityExpected::ExactNum(40.0),
    },
    ParityCase {
        id: "xmatch_binary_desc_dup_smaller_miss",
        label: "XMATCH(30, num_dup_desc, -1, -2) [binary desc -1 miss -> first smaller]",
        formula: "XMATCH(30, SeedData[\"r1\"..\"r4\", \"num_dup_desc\"], -1, -2)",
        expected: ParityExpected::ExactNum(3.0),
    },
    ParityCase {
        id: "xmatch_binary_desc_dup_larger_miss",
        label: "XMATCH(30, num_dup_desc, 1, -2) [binary desc 1 miss -> last larger]",
        formula: "XMATCH(30, SeedData[\"r1\"..\"r4\", \"num_dup_desc\"], 1, -2)",
        expected: ParityExpected::ExactNum(2.0),
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
        id: "xlookup_zero_skips_null_hits_formula",
        label: "XLOOKUP(0, sparse, num_a) [skips null r1, matches 0 r2]",
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
    ParityCase {
        id: "xlookup_invalid_match_mode_err",
        label: "XLOOKUP(\"Alpha\", cat, num_a, _, 99) [invalid match_mode 99 -> #VALUE!]",
        formula: "XLOOKUP(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], _, 99)",
        expected: ParityExpected::Error(ExcelError::Value),
    },
    ParityCase {
        id: "xmatch_invalid_search_mode_err",
        label: "XMATCH(\"Alpha\", cat, 0, 99) [invalid search_mode 99 -> #VALUE!]",
        formula: "XMATCH(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], 0, 99)",
        expected: ParityExpected::Error(ExcelError::Value),
    },
];

const DATE_CASES: &[ParityCase] = &[
    // Core 7 functions
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
    // Edge cases
    ParityCase {
        id: "lotus_leap_day_bug",
        label: "DATE(1900, 2, 29) [1900 leap day bug serial 60]",
        formula: "DATE(1900, 2, 29)",
        expected: ParityExpected::ExactNum(60.0),
    },
    ParityCase {
        id: "serial_diff_across_1900_bug",
        label: "DATE(1900, 3, 1) - DATE(1900, 2, 28) [spans serial 60]",
        formula: "DATE(1900, 3, 1) - DATE(1900, 2, 28)",
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
        id: "edate_1900_feb_clamp",
        label: "EDATE(DATE(1900, 1, 31), 1) [Gregorian Feb 1900 -> 59]",
        formula: "EDATE(DATE(1900, 1, 31), 1)",
        expected: ParityExpected::ExactNum(59.0),
    },
    ParityCase {
        id: "eomonth_1900_leap_bug_clamp",
        label: "EOMONTH(DATE(1900, 2, 29), 0) [Gregorian Feb 1900 -> 59]",
        formula: "EOMONTH(DATE(1900, 2, 29), 0)",
        expected: ParityExpected::ExactNum(59.0),
    },
    ParityCase {
        id: "yearfrac_us30_360_feb_to_mar31",
        label: "YEARFRAC(DATE(2023, 2, 28), DATE(2023, 3, 31), 0) [31/360]",
        formula: "YEARFRAC(DATE(2023, 2, 28), DATE(2023, 3, 31), 0)",
        expected: ParityExpected::ApproxNum(31.0 / 360.0),
    },
    ParityCase {
        id: "yearfrac_us30_360_feb_to_feb",
        label: "YEARFRAC(DATE(2023, 2, 28), DATE(2024, 2, 29), 0) [both end-of-Feb -> 1.0]",
        formula: "YEARFRAC(DATE(2023, 2, 28), DATE(2024, 2, 29), 0)",
        expected: ParityExpected::ExactNum(1.0),
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
    ParityCase {
        id: "date_year_neg_err",
        label: "DATE(-1, 1, 1) [negative year -> #NUM!]",
        formula: "DATE(-1, 1, 1)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "date_year_overflow_err",
        label: "DATE(10000, 1, 1) [year > 9999 -> #NUM!]",
        formula: "DATE(10000, 1, 1)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "year_neg_serial_err",
        label: "YEAR(-1) [negative date serial -> #NUM!]",
        formula: "YEAR(-1)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "year_overflow_serial_err",
        label: "YEAR(3000000) [serial > 2958465 -> #NUM!]",
        formula: "YEAR(3000000)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "yearfrac_invalid_basis_err",
        label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 5) [basis > 4 -> #NUM!]",
        formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 5)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
    ParityCase {
        id: "yearfrac_neg_basis_err",
        label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), -1) [basis < 0 -> #NUM!]",
        formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), -1)",
        expected: ParityExpected::Error(ExcelError::Num),
    },
];

const INFORMATION_CASES: &[ParityCase] = &[
    // Core 2 functions (ISERROR, NA) + edge and error cases
    ParityCase {
        id: "iserror_div0",
        label: "ISERROR(1 / 0) [#DIV/0! -> TRUE]",
        formula: "ISERROR(1 / 0)",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "iserror_value",
        label: "ISERROR(AND(sparse_bool[r3..r4])) [#VALUE! -> TRUE]",
        formula: "ISERROR(AND(SeedData[\"r3\"..\"r4\", \"sparse_bool\"]))",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "iserror_ref",
        label: "ISERROR(INDEX(num_a, num_b[r4])) [#REF! -> TRUE]",
        formula: "ISERROR(INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r4\", \"num_b\"]))",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "iserror_num",
        label: "ISERROR(EDATE(DATE(1900, 1, 15), -2)) [#NUM! -> TRUE]",
        formula: "ISERROR(EDATE(DATE(1900, 1, 15), -2))",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "iserror_na",
        label: "ISERROR(XMATCH(\"Missing\", cat)) [#N/A -> TRUE]",
        formula: "ISERROR(XMATCH(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"]))",
        expected: ParityExpected::Bool(true),
    },
    ParityCase {
        id: "iserror_num_val",
        label: "ISERROR(42) [number -> FALSE]",
        formula: "ISERROR(42)",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "iserror_err_text",
        label: "ISERROR(\"#DIV/0!\") [error-like text -> FALSE]",
        formula: "ISERROR(\"#DIV/0!\")",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "iserror_bool",
        label: "ISERROR(TRUE) [boolean -> FALSE]",
        formula: "ISERROR(TRUE)",
        expected: ParityExpected::Bool(false),
    },
    ParityCase {
        id: "na_trapped_iferror",
        label: "IFERROR(IF(FALSE, 10, NA()), 0) [trapped NA() -> 0]",
        formula: "IFERROR(IF(FALSE, 10, NA()), 0)",
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
        title: "Math & Aggregation Functions (14 Functions + Edge & Error Cases)",
        subtitle: "ABS, LN, EXP, MOD, SUM, AVERAGE, COUNT, MAX, MIN, MEDIAN, ROUND, ROUNDUP, ROUNDDOWN, SUMPRODUCT",
        cases: MATH_CASES,
    },
    ParityGroup {
        table_id: "LogicalParity",
        title: "Logical & Branching Functions (6 Functions + Edge & Error Cases)",
        subtitle: "IF, IFERROR, AND, OR, NOT, CHOOSE",
        cases: LOGICAL_CASES,
    },
    ParityGroup {
        table_id: "FinancialParity",
        title: "Financial & Return Functions (5 Functions + Edge & Error Cases)",
        subtitle: "NPV, XNPV, IRR, XIRR, PMT",
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
        title: "Date Construction & Decomposition Functions (7 Functions + Edge & Error Cases)",
        subtitle: "DATE, EDATE, EOMONTH, YEARFRAC, YEAR, MONTH, DAY",
        cases: DATE_CASES,
    },
    ParityGroup {
        table_id: "InformationParity",
        title: "Information Functions (2 Functions + Edge & Error Cases)",
        subtitle: "ISERROR, NA",
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
        subtitles: vec![group.subtitle.to_string()],
        currency: None,
        collapsible: false,
        placement: PlacementSpec::default(),
        columns: cols,
        rows,
    }
}

fn build_divergence_table() -> TableSpec {
    let cols = vec![
        build_descriptor_col("func_desc", "Function / Test"),
        build_measure_col_label("formula_val", "Excel Formula"),
        build_measure_col_label("expected_val", "f8 Expected"),
        build_measure_col_label("excel_quirk", "Live Excel Output"),
        build_descriptor_col("rationale", "Rationale"),
    ];

    let rows = DIVERGENCE_CASES
        .iter()
        .map(|case| {
            RowBuilder::descriptor(case.id, "func_desc", case.label)
                .cell_formula("formula_val", case.formula)
                .cell("expected_val", case.expected.to_cell_input())
                .cell_str("excel_quirk", case.excel_quirk)
                .cell_str("rationale", case.rationale)
                .build()
        })
        .collect();

    TableSpec {
        id: TableId::from("RuntimeDivergences"),
        title: "Pass 4b Runtime Domain & Solver Divergences (Live Formulas)".to_string(),
        subtitles: vec![
            "Live formulas where f8 enforces strict calendar dates, serial >= 1, discrete integers, sorted binary search, and exact IRR/XIRR roots over legacy Excel quirks"
                .to_string(),
        ],
        currency: None,
        collapsible: false,
        placement: PlacementSpec::default(),
        columns: cols,
        rows,
    }
}

/// Constructs the complete 37-function Excel parity verification model with pinned ground-truth constants.
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
        sheets: vec![
            SheetSpec {
                name: "ParityVerification".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: sheet_tables,
            },
            SheetSpec {
                name: "ExcelDivergences".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("RuntimeDivergences")],
            },
        ],
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
        build_measure_col_label("sparse_bool", "Sparse Bool"),
        build_measure_col_label("num_dup_asc", "Vector Dup Asc"),
        build_measure_col_label("num_dup_desc", "Vector Dup Desc"),
        build_measure_col_label("cf_lead_zero", "CF Lead Zero"),
        build_measure_col_label("cf_multi_root", "CF Multi Root"),
        build_measure_col_label("cf_rootless", "CF Rootless"),
        build_measure_col_label("cf_salvage", "CF Salvage"),
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
            .cell("sparse_bool", CellInput::null())
            .cell_num("num_dup_asc", 20.0)
            .cell_num("num_dup_desc", 40.0)
            .cell_num("cf_lead_zero", 0.0)
            .cell_num("cf_multi_root", -1600.0)
            .cell_num("cf_rootless", 100.0)
            .cell_num("cf_salvage", -1_000_000.0)
            .build(),
        RowBuilder::descriptor("r2", "row_id", "Row 2")
            .cell_num("num_a", 20.0)
            .cell_num("num_b", 3.0)
            .cell_num("num_desc", 30.0)
            .cell_str("category", "Beta")
            .cell_num("date", 45474.0) // 2024-07-01
            .cell_num("cash_flow", 300.0)
            .cell_formula("sparse", "IF(TRUE, 0, 1)")
            .cell("all_null", CellInput::null())
            .cell("sparse_bool", CellInput::boolean(true))
            .cell_num("num_dup_asc", 20.0)
            .cell_num("num_dup_desc", 40.0)
            .cell_num("cf_lead_zero", 0.0)
            .cell_num("cf_multi_root", 10000.0)
            .cell_num("cf_rootless", -10.0)
            .cell_num("cf_salvage", 1.0)
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
            .cell("sparse_bool", CellInput::null())
            .cell_num("num_dup_asc", 40.0)
            .cell_num("num_dup_desc", 20.0)
            .cell_num("cf_lead_zero", -100.0)
            .cell_num("cf_multi_root", -10000.0)
            .cell_num("cf_rootless", 100.0)
            .cell_num("cf_salvage", 0.0)
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
            .cell("sparse_bool", CellInput::null())
            .cell_num("num_dup_asc", 40.0)
            .cell_num("num_dup_desc", 20.0)
            .cell_num("cf_lead_zero", 110.0)
            .cell_num("cf_multi_root", 0.0)
            .cell_num("cf_rootless", 0.0)
            .cell_num("cf_salvage", 0.0)
            .build(),
    ];

    let seed_table = TableSpec {
        id: TableId::from("SeedData"),
        title: "Seed Data".to_string(),
        subtitles: vec!["Baseline inputs for ranges, lookups, and cash flows".to_string()],
        currency: None,
        collapsible: false,
        placement: PlacementSpec::default(),
        columns: seed_cols,
        rows: seed_rows,
    };

    let mut tables = Vec::with_capacity(2 + PARITY_GROUPS.len());
    tables.push(seed_table);
    for group in PARITY_GROUPS {
        tables.push(build_category_table(group));
    }
    tables.push(build_divergence_table());

    (manifest, tables)
}

fn assert_case_value(
    evaluated: &EvaluatedModel,
    table_id: &TableId,
    case_id: &str,
    expected: ParityExpected,
    formula_col: &ColId,
    expected_col: &ColId,
    expected_error_count: &mut usize,
) {
    let row_id = RowId::from(case_id);
    let actual = evaluated
        .get_value(table_id, &row_id, formula_col)
        .unwrap_or_else(|| {
            panic!("Missing evaluated formula_val for case '{case_id}' in table '{table_id}'")
        });

    match expected {
        ParityExpected::ExactNum(expected_num) => match actual {
            CellValue::Number(n) => {
                assert!(
                    n.to_bits() == expected_num.to_bits(),
                    "Exact numeric mismatch for '{case_id}' in '{table_id}': expected {expected_num:?} (bits={:#x}), got {n:?} (bits={:#x})",
                    expected_num.to_bits(),
                    n.to_bits(),
                );
            }
            CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty | CellValue::Error(_) => {
                panic!(
                    "Type mismatch for '{case_id}' in '{table_id}': expected Number({expected_num:?}), got {actual:?}"
                );
            }
        },
        ParityExpected::ApproxNum(expected_num) => match actual {
            CellValue::Number(n) => {
                let diff = (*n - expected_num).abs();
                let max_err = RELATIVE_TOLERANCE * expected_num.abs();
                assert!(
                    n.is_finite() && diff <= max_err,
                    "Approximate numeric mismatch for '{case_id}' in '{table_id}': expected {expected_num:.15}, got {n:.15} (abs_diff={diff:.3e}, max_allowed={max_err:.3e})",
                );
            }
            CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty | CellValue::Error(_) => {
                panic!(
                    "Type mismatch for '{case_id}' in '{table_id}': expected approx Number({expected_num:?}), got {actual:?}"
                );
            }
        },
        ParityExpected::Bool(expected_bool) => match actual {
            CellValue::Boolean(b) => {
                assert_eq!(
                    *b, expected_bool,
                    "Boolean mismatch for '{case_id}' in '{table_id}'"
                );
            }
            CellValue::Number(_) | CellValue::Text(_) | CellValue::Empty | CellValue::Error(_) => {
                panic!(
                    "Type mismatch for '{case_id}' in '{table_id}': expected Boolean({expected_bool}), got {actual:?}"
                );
            }
        },
        ParityExpected::Text(expected_text) => match actual {
            CellValue::Text(s) => {
                assert_eq!(
                    s.as_str(),
                    expected_text,
                    "Text mismatch for '{case_id}' in '{table_id}'"
                );
            }
            CellValue::Number(_)
            | CellValue::Boolean(_)
            | CellValue::Empty
            | CellValue::Error(_) => {
                panic!(
                    "Type mismatch for '{case_id}' in '{table_id}': expected Text({expected_text:?}), got {actual:?}"
                );
            }
        },
        ParityExpected::Error(expected_err) => {
            *expected_error_count += 1;
            match actual {
                CellValue::Error(actual_err) => {
                    assert_eq!(
                        *actual_err, expected_err,
                        "Domain error mismatch for '{case_id}' in '{table_id}'"
                    );
                }
                CellValue::Number(_)
                | CellValue::Text(_)
                | CellValue::Boolean(_)
                | CellValue::Empty => {
                    panic!(
                        "Expected error {expected_err} for '{case_id}' in '{table_id}', got {actual:?}"
                    );
                }
            }
            let recorded = evaluated.get_value(table_id, &row_id, expected_col);
            assert_eq!(
                recorded,
                Some(&CellValue::Text(expected_err.as_str().to_string())),
                "Error case '{case_id}' in '{table_id}' must store canonical error text in expected_val"
            );
        }
    }
}

/// Asserts that every parity case in `evaluated` matches its pinned Excel ground-truth constant,
/// every `parity_status` formula evaluates to `"PASS"`, every documented divergence case in
/// `RuntimeDivergences` evaluates to its mathematical/domain ground truth, and no unexpected domain errors exist.
pub fn assert_evaluated_parity(evaluated: &EvaluatedModel) {
    let formula_col = ColId::from("formula_val");
    let expected_col = ColId::from("expected_val");
    let status_col = ColId::from("parity_status");
    let quirk_col = ColId::from("excel_quirk");
    let rationale_col = ColId::from("rationale");
    let mut expected_error_count = 0usize;

    for group in PARITY_GROUPS {
        let table_id = TableId::from(group.table_id);
        for case in group.cases {
            assert_case_value(
                evaluated,
                &table_id,
                case.id,
                case.expected,
                &formula_col,
                &expected_col,
                &mut expected_error_count,
            );

            let row_id = RowId::from(case.id);
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

    let div_table_id = TableId::from("RuntimeDivergences");
    for case in DIVERGENCE_CASES {
        assert_case_value(
            evaluated,
            &div_table_id,
            case.id,
            case.expected,
            &formula_col,
            &expected_col,
            &mut expected_error_count,
        );

        let row_id = RowId::from(case.id);
        assert!(
            !case.excel_quirk.is_empty() && !case.rationale.is_empty(),
            "Divergence case '{}' must document non-empty excel_quirk and rationale",
            case.id
        );
        assert_eq!(
            evaluated.get_value(&div_table_id, &row_id, &quirk_col),
            Some(&CellValue::Text(case.excel_quirk.to_string())),
            "Mismatch in excel_quirk for '{}'",
            case.id
        );
        assert_eq!(
            evaluated.get_value(&div_table_id, &row_id, &rationale_col),
            Some(&CellValue::Text(case.rationale.to_string())),
            "Mismatch in rationale for '{}'",
            case.id
        );
    }

    let all_errors = evaluated.find_errors();
    assert_eq!(
        all_errors.len(),
        expected_error_count,
        "Unexpected domain error cells in parity model: {all_errors:?}"
    );
}
