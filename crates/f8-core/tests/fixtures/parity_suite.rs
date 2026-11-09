//! Canonical Excel calculation parity test fixture covering all 44 registered standard functions,
//! domain error semantics (`#DIV/0!`, `#VALUE!`, `#NUM!`, `#N/A`, `#REF!`), and calculation edge cases.
//!
//! Organizes tests into 6 vertical tables on a single worksheet:
//! 1. `SeedData`: Independent inputs (numbers, categories, dates, cash flows) for ranges and lookups.
//! 2. `MathParity`: 17 standard math and aggregation functions + edge and error cases.
//! 3. `LogicalParity`: 9 logical, branching, and error-handling functions + edge and error cases.
//! 4. `FinancialParity`: 7 discounting, return, and loan amortization functions + edge and error cases.
//! 5. `LookupParity`: 3 modern lookup and index functions + edge and error cases.
//! 6. `DateParity`: 8 serial date construction, decomposition, and difference functions + edge and error cases.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test fixture")]

use f8_core::Compiler;
use f8_core::model::id::{ColId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError};

use super::{RowBuilder, build_descriptor_col, build_measure_col_label};

/// Grading mode for an Excel calculation parity test case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParityGrade {
    /// Exact equality (`=`) for integers, dates, strings, booleans, counts, and lookups.
    Exact,
    /// Relative tolerance (`<= 1e-9 * ABS(f8_val)`) for floating-point and iterative financial functions.
    RelativeTolerance,
    /// Domain error parity (`COUNTIFS(formula_val, f8_val) = 1`) verifying the exact `ExcelError` variant.
    Error(ExcelError),
}

/// Definition of a single parity test case.
struct ParityCase {
    id: &'static str,
    label: &'static str,
    formula: &'static str,
    grade: ParityGrade,
}

/// Constructs the complete 44-function Excel parity verification model with edge and error cases.
#[must_use]
pub fn build_parity_model() -> (ProjectManifest, Vec<TableSpec>) {
    let manifest = ProjectManifest {
        model_name: "ExcelParityVerification".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "ParityVerification".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                TableId::from("SeedData"),
                TableId::from("MathParity"),
                TableId::from("LogicalParity"),
                TableId::from("FinancialParity"),
                TableId::from("LookupParity"),
                TableId::from("DateParity"),
            ],
        }],
    };

    // -------------------------------------------------------------------------
    // 1. SeedData Table: Inputs for range aggregations, lookups, dates, cash flows
    // -------------------------------------------------------------------------
    let seed_cols = vec![
        build_descriptor_col("row_id", "Row"),
        build_measure_col_label("num_a", "Vector A"),
        build_measure_col_label("num_b", "Vector B"),
        build_measure_col_label("category", "Category"),
        build_measure_col_label("date", "Date"),
        build_measure_col_label("cash_flow", "Cash Flow"),
    ];

    let seed_rows = vec![
        RowBuilder::descriptor("r1", "row_id", "Row 1")
            .cell_num("num_a", 10.0)
            .cell_num("num_b", 2.0)
            .cell_str("category", "Alpha")
            .cell_num("date", 45292.0) // 2024-01-01
            .cell_num("cash_flow", -1000.0)
            .build(),
        RowBuilder::descriptor("r2", "row_id", "Row 2")
            .cell_num("num_a", 20.0)
            .cell_num("num_b", 3.0)
            .cell_str("category", "Beta")
            .cell_num("date", 45474.0) // 2024-07-01
            .cell_num("cash_flow", 300.0)
            .build(),
        RowBuilder::descriptor("r3", "row_id", "Row 3")
            .cell_num("num_a", 30.0)
            .cell_num("num_b", 4.0)
            .cell_str("category", "Alpha")
            .cell_num("date", 45658.0) // 2025-01-01
            .cell_num("cash_flow", 400.0)
            .build(),
        RowBuilder::descriptor("r4", "row_id", "Row 4")
            .cell_num("num_a", 40.0)
            .cell_num("num_b", 5.0)
            .cell_str("category", "Gamma")
            .cell_num("date", 46023.0) // 2026-01-01
            .cell_num("cash_flow", 600.0)
            .build(),
    ];

    let seed_table = TableSpec {
        id: TableId::from("SeedData"),
        title: "Seed Data".to_string(),
        subtitle: Some("Baseline inputs for ranges, lookups, and cash flows".to_string()),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: seed_cols,
        rows: seed_rows,
    };

    // -------------------------------------------------------------------------
    // 2. Parity Cases Definition (All 44 Standard Functions + Edge & Error Cases)
    // -------------------------------------------------------------------------
    let math_cases = [
        // Core 17 functions
        ParityCase {
            id: "abs",
            label: "ABS(-42.5)",
            formula: "ABS(-42.5)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "sum",
            label: "SUM(num_a)",
            formula: "SUM(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "average",
            label: "AVERAGE(num_a)",
            formula: "AVERAGE(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "count",
            label: "COUNT(num_a)",
            formula: "COUNT(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "counta",
            label: "COUNTA(category)",
            formula: "COUNTA(SeedData[\"r1\"..\"r4\", \"category\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "max",
            label: "MAX(num_a)",
            formula: "MAX(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "min",
            label: "MIN(num_a)",
            formula: "MIN(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "median",
            label: "MEDIAN(num_a) [even count]",
            formula: "MEDIAN(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "round",
            label: "ROUND(123.456, 2)",
            formula: "ROUND(123.456, 2)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "roundup",
            label: "ROUNDUP(123.451, 2)",
            formula: "ROUNDUP(123.451, 2)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "rounddown",
            label: "ROUNDDOWN(123.459, 2)",
            formula: "ROUNDDOWN(123.459, 2)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "sumproduct",
            label: "SUMPRODUCT(num_a, num_b)",
            formula: "SUMPRODUCT(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"num_b\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "sumifs",
            label: "SUMIFS(num_a, cat, \"Alpha\")",
            formula: "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "countifs",
            label: "COUNTIFS(cat, \"Alpha\")",
            formula: "COUNTIFS(SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "averageifs",
            label: "AVERAGEIFS(num_a, cat, \"Alpha\")",
            formula: "AVERAGEIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "minifs",
            label: "MINIFS(num_a, cat, \"Alpha\")",
            formula: "MINIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "maxifs",
            label: "MAXIFS(num_a, cat, \"Alpha\")",
            formula: "MAXIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            grade: ParityGrade::Exact,
        },
        // Edge cases
        ParityCase {
            id: "sum_coercion_edge",
            label: "SUM(cat_range, TRUE, 5) [range text ignored, scalar bool coerced]",
            formula: "SUM(SeedData[\"r1\"..\"r4\", \"category\"], TRUE, 5)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "count_text_edge",
            label: "COUNT(category) [non-numeric range]",
            formula: "COUNT(SeedData[\"r1\"..\"r4\", \"category\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "median_odd_edge",
            label: "MEDIAN(r1..r3) [odd count]",
            formula: "MEDIAN(SeedData[\"r1\"..\"r3\", \"num_a\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "round_neg_digits",
            label: "ROUND(1250, -2) [negative digits]",
            formula: "ROUND(1250, -2)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "round_half_away",
            label: "ROUND(-2.5, 0) [halfway away from zero]",
            formula: "ROUND(-2.5, 0)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "roundup_negative",
            label: "ROUNDUP(-123.451, 2) [negative away from zero]",
            formula: "ROUNDUP(-123.451, 2)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "rounddown_negative",
            label: "ROUNDDOWN(-123.459, 2) [negative toward zero]",
            formula: "ROUNDDOWN(-123.459, 2)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "sumifs_wildcard_rel",
            label: "SUMIFS(num_a, cat, \"A*\", num_a, \">15\") [wildcard + relational]",
            formula: "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"A*\", SeedData[\"r1\"..\"r4\", \"num_a\"], \">15\")",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "minifs_empty_match",
            label: "MINIFS(num_a, cat, \"Missing\") [empty match -> 0]",
            formula: "MINIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Missing\")",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "maxifs_empty_match",
            label: "MAXIFS(num_a, cat, \"Missing\") [empty match -> 0]",
            formula: "MAXIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Missing\")",
            grade: ParityGrade::Exact,
        },
        // Error cases
        ParityCase {
            id: "div0_arith_err",
            label: "10 / 0 [#DIV/0!]",
            formula: "10 / 0",
            grade: ParityGrade::Error(ExcelError::Div0),
        },
        ParityCase {
            id: "average_empty_err",
            label: "AVERAGE(category) [no numeric cells -> #DIV/0!]",
            formula: "AVERAGE(SeedData[\"r1\"..\"r4\", \"category\"])",
            grade: ParityGrade::Error(ExcelError::Div0),
        },
        ParityCase {
            id: "averageifs_empty_err",
            label: "AVERAGEIFS(num_a, cat, \"Missing\") [no matches -> #DIV/0!]",
            formula: "AVERAGEIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Missing\")",
            grade: ParityGrade::Error(ExcelError::Div0),
        },
        ParityCase {
            id: "abs_text_err",
            label: "ABS(\"invalid\") [text coercion -> #VALUE!]",
            formula: "ABS(\"invalid\")",
            grade: ParityGrade::Error(ExcelError::Value),
        },
        ParityCase {
            id: "sumifs_dim_err",
            label: "SUMIFS(r1..r4, r1..r2, \"Alpha\") [shape mismatch -> #VALUE!]",
            formula: "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r2\", \"category\"], \"Alpha\")",
            grade: ParityGrade::Error(ExcelError::Value),
        },
        ParityCase {
            id: "pow_domain_err",
            label: "(-4) ^ 0.5 [negative fractional power -> #NUM!]",
            formula: "(-4) ^ 0.5",
            grade: ParityGrade::Error(ExcelError::Num),
        },
        ParityCase {
            id: "median_empty_err",
            label: "MEDIAN(category) [no numeric cells -> #NUM!]",
            formula: "MEDIAN(SeedData[\"r1\"..\"r4\", \"category\"])",
            grade: ParityGrade::Error(ExcelError::Num),
        },
    ];

    let logical_cases = [
        // Core 9 functions
        ParityCase {
            id: "if_fn",
            label: "IF(10 > 5, 100, 200)",
            formula: "IF(10 > 5, 100, 200)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "ifs_fn",
            label: "IFS(10 < 5, 1, 10 > 5, 2)",
            formula: "IFS(10 < 5, 1, 10 > 5, 2)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "iferror",
            label: "IFERROR(10 / 0, 999)",
            formula: "IFERROR(10 / 0, 999)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "ifna",
            label: "IFNA(XLOOKUP(\"missing\", ...), 888)",
            formula: "IFNA(XLOOKUP(\"missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"]), 888)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "and_fn",
            label: "AND(10 > 5, 20 > 10)",
            formula: "AND(10 > 5, 20 > 10)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "or_fn",
            label: "OR(10 < 5, 20 > 10)",
            formula: "OR(10 < 5, 20 > 10)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "not_fn",
            label: "NOT(10 < 5)",
            formula: "NOT(10 < 5)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "choose_fn",
            label: "CHOOSE(2, 100, 200, 300)",
            formula: "CHOOSE(2, 100, 200, 300)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "switch_fn",
            label: "SWITCH(2, 1, 100, 2, 200, 3, 300)",
            formula: "SWITCH(2, 1, 100, 2, 200, 3, 300)",
            grade: ParityGrade::Exact,
        },
        // Edge cases
        ParityCase {
            id: "if_lazy_shortcircuit",
            label: "IF(TRUE, 42, 10 / 0) [lazy branch short-circuit]",
            formula: "IF(TRUE, 42, 10 / 0)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "if_default_false",
            label: "IF(10 < 5, 100) [omitted false branch -> FALSE]",
            formula: "IF(10 < 5, 100)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "type_rank_num_text",
            label: "100 < \"1\" [Number < Text ranking]",
            formula: "100 < \"1\"",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "type_rank_text_bool",
            label: "\"zzz\" < FALSE [Text < Boolean ranking]",
            formula: "\"zzz\" < FALSE",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "case_insensitive_eq",
            label: "\"Alpha\" = \"alpha\" [case-insensitive equality]",
            formula: "\"Alpha\" = \"alpha\"",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "switch_default_edge",
            label: "SWITCH(99, 1, 100, 2, 200, 999) [default fallback]",
            formula: "SWITCH(99, 1, 100, 2, 200, 999)",
            grade: ParityGrade::Exact,
        },
        // Error cases
        ParityCase {
            id: "ifna_passthrough_div0",
            label: "IFNA(10 / 0, 888) [non-#N/A error bubbles -> #DIV/0!]",
            formula: "IFNA(10 / 0, 888)",
            grade: ParityGrade::Error(ExcelError::Div0),
        },
        ParityCase {
            id: "ifs_no_match_err",
            label: "IFS(10 < 5, 1, 20 < 10, 2) [no true branch -> #N/A]",
            formula: "IFS(10 < 5, 1, 20 < 10, 2)",
            grade: ParityGrade::Error(ExcelError::Na),
        },
        ParityCase {
            id: "switch_no_match_err",
            label: "SWITCH(99, 1, 100, 2, 200) [unmatched without default -> #N/A]",
            formula: "SWITCH(99, 1, 100, 2, 200)",
            grade: ParityGrade::Error(ExcelError::Na),
        },
        ParityCase {
            id: "choose_oob_err",
            label: "CHOOSE(5, 100, 200, 300) [index out of bounds -> #VALUE!]",
            formula: "CHOOSE(5, 100, 200, 300)",
            grade: ParityGrade::Error(ExcelError::Value),
        },
    ];

    let financial_cases = [
        // Core 7 functions
        ParityCase {
            id: "npv",
            label: "NPV(0.1, cash_flows)",
            formula: "NPV(0.1, SeedData[\"r2\"..\"r4\", \"cash_flow\"])",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "xnpv",
            label: "XNPV(0.1, cash_flows, dates)",
            formula: "XNPV(0.1, SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "irr",
            label: "IRR(cash_flows)",
            formula: "IRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"])",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "xirr",
            label: "XIRR(cash_flows, dates)",
            formula: "XIRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "pmt",
            label: "PMT(0.06/12, 36, -10000)",
            formula: "PMT(0.06 / 12, 36, -10000)",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "ipmt",
            label: "IPMT(0.06/12, 1, 36, -10000)",
            formula: "IPMT(0.06 / 12, 1, 36, -10000)",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "ppmt",
            label: "PPMT(0.06/12, 1, 36, -10000)",
            formula: "PPMT(0.06 / 12, 1, 36, -10000)",
            grade: ParityGrade::RelativeTolerance,
        },
        // Edge cases
        ParityCase {
            id: "irr_custom_guess",
            label: "IRR(cash_flows, 0.25) [custom initial guess]",
            formula: "IRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], 0.25)",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "pmt_zero_rate",
            label: "PMT(0, 10, -1000) [zero interest rate]",
            formula: "PMT(0, 10, -1000)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "pmt_annuity_due_fv",
            label: "PMT(0.06/12, 36, -10000, 2000, 1) [fv + type=1]",
            formula: "PMT(0.06 / 12, 36, -10000, 2000, 1)",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "ipmt_mid_period",
            label: "IPMT(0.06/12, 18, 36, -10000) [mid-schedule period]",
            formula: "IPMT(0.06 / 12, 18, 36, -10000)",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "ppmt_mid_period",
            label: "PPMT(0.06/12, 18, 36, -10000) [mid-schedule period]",
            formula: "PPMT(0.06 / 12, 18, 36, -10000)",
            grade: ParityGrade::RelativeTolerance,
        },
        // Error cases
        ParityCase {
            id: "irr_no_sign_change_err",
            label: "IRR(r2..r4) [all positive cash flows -> #NUM!]",
            formula: "IRR(SeedData[\"r2\"..\"r4\", \"cash_flow\"])",
            grade: ParityGrade::Error(ExcelError::Num),
        },
        ParityCase {
            id: "xirr_no_sign_change_err",
            label: "XIRR(r2..r4, dates) [all positive cash flows -> #NUM!]",
            formula: "XIRR(SeedData[\"r2\"..\"r4\", \"cash_flow\"], SeedData[\"r2\"..\"r4\", \"date\"])",
            grade: ParityGrade::Error(ExcelError::Num),
        },
        ParityCase {
            id: "xnpv_invalid_rate_err",
            label: "XNPV(-1.0, cash_flows, dates) [rate <= -1 -> #NUM!]",
            formula: "XNPV(-1.0, SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
            grade: ParityGrade::Error(ExcelError::Num),
        },
        ParityCase {
            id: "ipmt_per_oob_err",
            label: "IPMT(0.06/12, 40, 36, -10000) [per > nper -> #NUM!]",
            formula: "IPMT(0.06 / 12, 40, 36, -10000)",
            grade: ParityGrade::Error(ExcelError::Num),
        },
        ParityCase {
            id: "ppmt_per_zero_err",
            label: "PPMT(0.06/12, 0, 36, -10000) [per < 1 -> #NUM!]",
            formula: "PPMT(0.06 / 12, 0, 36, -10000)",
            grade: ParityGrade::Error(ExcelError::Num),
        },
    ];

    let lookup_cases = [
        // Core 3 functions
        ParityCase {
            id: "xlookup",
            label: "XLOOKUP(\"Beta\", cat, num_a)",
            formula: "XLOOKUP(\"Beta\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "xmatch",
            label: "XMATCH(\"Beta\", cat)",
            formula: "XMATCH(\"Beta\", SeedData[\"r1\"..\"r4\", \"category\"])",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "index_fn",
            label: "INDEX(num_a, 3)",
            formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], 3)",
            grade: ParityGrade::Exact,
        },
        // Edge cases
        ParityCase {
            id: "xlookup_fallback_edge",
            label: "XLOOKUP(\"Missing\", cat, num_a, -1) [if_not_found fallback]",
            formula: "XLOOKUP(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], -1)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "xlookup_reverse_edge",
            label: "XLOOKUP(\"Alpha\", cat, num_a, _, 0, -1) [reverse search]",
            formula: "XLOOKUP(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], _, 0, -1)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "xlookup_wildcard_edge",
            label: "XLOOKUP(\"G*\", cat, num_a, _, 2) [wildcard match_mode=2]",
            formula: "XLOOKUP(\"G*\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], _, 2)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "xmatch_next_smaller",
            label: "XMATCH(25, num_a, -1) [exact or next smaller]",
            formula: "XMATCH(25, SeedData[\"r1\"..\"r4\", \"num_a\"], -1)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "index_2d_edge",
            label: "INDEX(num_a..num_b, 2, 2) [2D matrix lookup]",
            formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"..\"num_b\"], 2, 2)",
            grade: ParityGrade::Exact,
        },
        // Error cases
        ParityCase {
            id: "xlookup_missing_err",
            label: "XLOOKUP(\"Missing\", cat, num_a) [unmatched -> #N/A]",
            formula: "XLOOKUP(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"])",
            grade: ParityGrade::Error(ExcelError::Na),
        },
        ParityCase {
            id: "xmatch_missing_err",
            label: "XMATCH(\"Missing\", cat) [unmatched -> #N/A]",
            formula: "XMATCH(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"])",
            grade: ParityGrade::Error(ExcelError::Na),
        },
        ParityCase {
            id: "index_oob_err",
            label: "INDEX(num_a, 5) [row out of bounds -> #REF!]",
            formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], 5)",
            grade: ParityGrade::Error(ExcelError::Ref),
        },
        ParityCase {
            id: "xlookup_mode_err",
            label: "XLOOKUP(\"Alpha\", cat, num_a, _, 99) [invalid match_mode -> #VALUE!]",
            formula: "XLOOKUP(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], _, 99)",
            grade: ParityGrade::Error(ExcelError::Value),
        },
    ];

    let date_cases = [
        // Core 8 functions
        ParityCase {
            id: "date_fn",
            label: "DATE(2024, 6, 15)",
            formula: "DATE(2024, 6, 15)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "edate",
            label: "EDATE(DATE(2024, 1, 31), 1) [leap year Feb clamp]",
            formula: "EDATE(DATE(2024, 1, 31), 1)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "eomonth",
            label: "EOMONTH(DATE(2024, 1, 15), 1)",
            formula: "EOMONTH(DATE(2024, 1, 15), 1)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "yearfrac",
            label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0) [US 30/360]",
            formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0)",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "year_fn",
            label: "YEAR(DATE(2024, 6, 15))",
            formula: "YEAR(DATE(2024, 6, 15))",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "month_fn",
            label: "MONTH(DATE(2024, 6, 15))",
            formula: "MONTH(DATE(2024, 6, 15))",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "day_fn",
            label: "DAY(DATE(2024, 6, 15))",
            formula: "DAY(DATE(2024, 6, 15))",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "days_fn",
            label: "DAYS(DATE(2024, 6, 15), DATE(2024, 6, 1))",
            formula: "DAYS(DATE(2024, 6, 15), DATE(2024, 6, 1))",
            grade: ParityGrade::Exact,
        },
        // Edge cases
        ParityCase {
            id: "lotus_leap_day_bug",
            label: "DATE(1900, 2, 29) [1900 leap day bug serial 60]",
            formula: "DATE(1900, 2, 29)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "days_across_1900_bug",
            label: "DAYS(DATE(1900, 3, 1), DATE(1900, 2, 28)) [spans serial 60]",
            formula: "DAYS(DATE(1900, 3, 1), DATE(1900, 2, 28))",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "date_month_overflow",
            label: "DATE(2024, 14, 1) [month overflow to 2025-02-01]",
            formula: "DATE(2024, 14, 1)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "edate_nonleap_clamp",
            label: "EDATE(DATE(2025, 1, 31), 1) [non-leap Feb 28 clamp]",
            formula: "EDATE(DATE(2025, 1, 31), 1)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "eomonth_neg_offset",
            label: "EOMONTH(DATE(2024, 3, 15), -1) [negative month offset]",
            formula: "EOMONTH(DATE(2024, 3, 15), -1)",
            grade: ParityGrade::Exact,
        },
        ParityCase {
            id: "yearfrac_act_act",
            label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 1) [Actual/Actual]",
            formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 1)",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "yearfrac_act_360",
            label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 2) [Actual/360]",
            formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 2)",
            grade: ParityGrade::RelativeTolerance,
        },
        ParityCase {
            id: "yearfrac_act_365",
            label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 3) [Actual/365]",
            formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 3)",
            grade: ParityGrade::RelativeTolerance,
        },
        // Error cases
        ParityCase {
            id: "yearfrac_basis_err",
            label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 5) [basis > 4 -> #NUM!]",
            formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 5)",
            grade: ParityGrade::Error(ExcelError::Num),
        },
        ParityCase {
            id: "date_year_oob_err",
            label: "DATE(10000, 1, 1) [year >= 10000 -> #NUM!]",
            formula: "DATE(10000, 1, 1)",
            grade: ParityGrade::Error(ExcelError::Num),
        },
        ParityCase {
            id: "days_text_err",
            label: "DAYS(\"invalid\", DATE(2024, 1, 1)) [text coercion -> #VALUE!]",
            formula: "DAYS(\"invalid\", DATE(2024, 1, 1))",
            grade: ParityGrade::Error(ExcelError::Value),
        },
    ];

    // Helper closure to build a category table given its cases
    let build_category_table = |table_id: &str,
                                title: &str,
                                subtitle: &str,
                                cases: &[ParityCase]|
     -> TableSpec {
        let cols = vec![
            build_descriptor_col("func_desc", "Function / Test"),
            build_measure_col_label("formula_val", "Excel Formula"),
            build_measure_col_label("f8_val", "f8 Evaluated"),
            build_measure_col_label("parity_status", "Parity Status"),
        ];

        let mut rows = Vec::with_capacity(cases.len());
        for case in cases {
            let check_expr = match case.grade {
                ParityGrade::Exact => format!(
                    "IF(IFERROR({table_id}[@row, \"formula_val\"] = {table_id}[@row, \"f8_val\"], FALSE), \"PASS\", \"FAIL\")"
                ),
                ParityGrade::RelativeTolerance => format!(
                    "IF(IFERROR(ABS({table_id}[@row, \"formula_val\"] - {table_id}[@row, \"f8_val\"]) <= 0.000000001 * ABS({table_id}[@row, \"f8_val\"]), FALSE), \"PASS\", \"FAIL\")"
                ),
                ParityGrade::Error(_) => format!(
                    "IF(COUNTIFS({table_id}[@row, \"formula_val\"], {table_id}[@row, \"f8_val\"]) = 1, \"PASS\", \"FAIL\")"
                ),
            };

            let row = RowBuilder::descriptor(case.id, "func_desc", case.label)
                .cell_formula("formula_val", case.formula)
                .cell_num("f8_val", 0.0) // Temporary placeholder before evaluation
                .cell_formula("parity_status", &check_expr)
                .build();
            rows.push(row);
        }

        TableSpec {
            id: TableId::from(table_id),
            title: title.to_string(),
            subtitle: Some(subtitle.to_string()),
            currency: None,
            collapsible: false,
            placement: f8_core::model::PlacementSpec::default(),
            columns: cols,
            rows,
        }
    };

    let math_table = build_category_table(
        "MathParity",
        "Math & Aggregation Functions (17 Functions + Edge & Error Cases)",
        "ABS, SUM, AVERAGE, COUNT, COUNTA, MAX, MIN, MEDIAN, ROUND, ROUNDUP, ROUNDDOWN, SUMPRODUCT, *IFS",
        &math_cases,
    );
    let logical_table = build_category_table(
        "LogicalParity",
        "Logical & Branching Functions (9 Functions + Edge & Error Cases)",
        "IF, IFS, IFERROR, IFNA, AND, OR, NOT, CHOOSE, SWITCH",
        &logical_cases,
    );
    let financial_table = build_category_table(
        "FinancialParity",
        "Financial & Return Functions (7 Functions + Edge & Error Cases)",
        "NPV, XNPV, IRR, XIRR, PMT, IPMT, PPMT",
        &financial_cases,
    );
    let lookup_table = build_category_table(
        "LookupParity",
        "Lookup & Reference Functions (3 Functions + Edge & Error Cases)",
        "XLOOKUP, XMATCH, INDEX",
        &lookup_cases,
    );
    let date_table = build_category_table(
        "DateParity",
        "Date Construction & Difference Functions (8 Functions + Edge & Error Cases)",
        "DATE, EDATE, EOMONTH, YEARFRAC, YEAR, MONTH, DAY, DAYS",
        &date_cases,
    );

    let all_case_groups: [&[ParityCase]; 5] = [
        &math_cases,
        &logical_cases,
        &financial_cases,
        &lookup_cases,
        &date_cases,
    ];

    let initial_tables = vec![
        seed_table,
        math_table,
        logical_table,
        financial_table,
        lookup_table,
        date_table,
    ];

    let mut final_tables = initial_tables.clone();

    // Evaluate initial tables to capture f8's evaluated values for formula_val
    let evaluated = Compiler::evaluate(&manifest, &initial_tables)
        .expect("Initial parity model must evaluate cleanly");

    for (table, cases) in final_tables[1..].iter_mut().zip(all_case_groups.iter()) {
        let t_id = &table.id;
        for (row, case) in table.rows.iter_mut().zip(cases.iter()) {
            if let RowSpec::Data(d) = row {
                let r_id = &d.id;
                let eval_val = evaluated
                    .get_value(t_id, r_id, &ColId::from("formula_val"))
                    .expect("Evaluated cell must exist");

                let literal_input = match (case.grade, eval_val) {
                    (ParityGrade::Error(expected_err), CellValue::Error(actual_err)) => {
                        assert_eq!(
                            *actual_err, expected_err,
                            "Case '{}' in '{}' produced wrong ExcelError",
                            case.id, t_id
                        );
                        CellInput::string(actual_err.as_str())
                    }
                    (ParityGrade::Error(expected_err), other) => {
                        panic!(
                            "Case '{}' in '{}' expected error {expected_err}, got {other:?}",
                            case.id, t_id
                        );
                    }
                    (ParityGrade::Exact | ParityGrade::RelativeTolerance, CellValue::Number(n)) => {
                        CellInput::number(*n)
                    }
                    (ParityGrade::Exact, CellValue::Boolean(b)) => CellInput::boolean(*b),
                    (ParityGrade::Exact, CellValue::Text(s)) => CellInput::string(s),
                    (ParityGrade::Exact | ParityGrade::RelativeTolerance, other) => {
                        panic!(
                            "Case '{}' in '{}' expected normal value for {:?}, got {other:?}",
                            case.id, t_id, case.grade
                        );
                    }
                };
                d.cells.insert(ColId::from("f8_val"), literal_input);
            }
        }
    }

    (manifest, final_tables)
}
