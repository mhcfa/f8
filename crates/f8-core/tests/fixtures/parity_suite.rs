//! Canonical Excel calculation parity test fixture covering all 44 registered standard functions.
//!
//! Organizes tests into 6 vertical tables on a single worksheet:
//! 1. `SeedData`: Independent inputs (numbers, categories, dates, cash flows) for ranges and lookups.
//! 2. `MathParity`: 17 standard math and aggregation functions.
//! 3. `LogicalParity`: 9 logical, branching, and error-handling functions.
//! 4. `FinancialParity`: 7 discounting, return, and loan amortization functions.
//! 5. `LookupParity`: 3 modern lookup and index functions.
//! 6. `DateParity`: 8 serial date construction, decomposition, and difference functions.

use f8_core::Compiler;
use f8_core::model::id::{ColId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, RowSpec, TableSpec};
use f8_core::value::CellValue;

use super::{RowBuilder, build_descriptor_col, build_measure_col_label};

/// Definition of a single parity test case.
struct ParityCase {
    id: &'static str,
    label: &'static str,
    formula: &'static str,
    is_boolean: bool,
}

/// Constructs the complete 44-function Excel parity verification model.
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
    // 2. Parity Cases Definition (All 44 Standard Functions)
    // -------------------------------------------------------------------------
    let math_cases = [
        ParityCase {
            id: "abs",
            label: "ABS(-42.5)",
            formula: "ABS(-42.5)",
            is_boolean: false,
        },
        ParityCase {
            id: "sum",
            label: "SUM(num_a)",
            formula: "SUM(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "average",
            label: "AVERAGE(num_a)",
            formula: "AVERAGE(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "count",
            label: "COUNT(num_a)",
            formula: "COUNT(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "counta",
            label: "COUNTA(category)",
            formula: "COUNTA(SeedData[\"r1\"..\"r4\", \"category\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "max",
            label: "MAX(num_a)",
            formula: "MAX(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "min",
            label: "MIN(num_a)",
            formula: "MIN(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "median",
            label: "MEDIAN(num_a)",
            formula: "MEDIAN(SeedData[\"r1\"..\"r4\", \"num_a\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "round",
            label: "ROUND(123.456, 2)",
            formula: "ROUND(123.456, 2)",
            is_boolean: false,
        },
        ParityCase {
            id: "roundup",
            label: "ROUNDUP(123.451, 2)",
            formula: "ROUNDUP(123.451, 2)",
            is_boolean: false,
        },
        ParityCase {
            id: "rounddown",
            label: "ROUNDDOWN(123.459, 2)",
            formula: "ROUNDDOWN(123.459, 2)",
            is_boolean: false,
        },
        ParityCase {
            id: "sumproduct",
            label: "SUMPRODUCT(num_a, num_b)",
            formula: "SUMPRODUCT(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"num_b\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "sumifs",
            label: "SUMIFS(num_a, cat, \"Alpha\")",
            formula: "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            is_boolean: false,
        },
        ParityCase {
            id: "countifs",
            label: "COUNTIFS(cat, \"Alpha\")",
            formula: "COUNTIFS(SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            is_boolean: false,
        },
        ParityCase {
            id: "averageifs",
            label: "AVERAGEIFS(num_a, cat, \"Alpha\")",
            formula: "AVERAGEIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            is_boolean: false,
        },
        ParityCase {
            id: "minifs",
            label: "MINIFS(num_a, cat, \"Alpha\")",
            formula: "MINIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            is_boolean: false,
        },
        ParityCase {
            id: "maxifs",
            label: "MAXIFS(num_a, cat, \"Alpha\")",
            formula: "MAXIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"Alpha\")",
            is_boolean: false,
        },
    ];

    let logical_cases = [
        ParityCase {
            id: "if_fn",
            label: "IF(10 > 5, 100, 200)",
            formula: "IF(10 > 5, 100, 200)",
            is_boolean: false,
        },
        ParityCase {
            id: "ifs_fn",
            label: "IFS(10 < 5, 1, 10 > 5, 2)",
            formula: "IFS(10 < 5, 1, 10 > 5, 2)",
            is_boolean: false,
        },
        ParityCase {
            id: "iferror",
            label: "IFERROR(10 / 0, 999)",
            formula: "IFERROR(10 / 0, 999)",
            is_boolean: false,
        },
        ParityCase {
            id: "ifna",
            label: "IFNA(XLOOKUP(\"missing\", ...), 888)",
            formula: "IFNA(XLOOKUP(\"missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"]), 888)",
            is_boolean: false,
        },
        ParityCase {
            id: "and_fn",
            label: "AND(10 > 5, 20 > 10)",
            formula: "AND(10 > 5, 20 > 10)",
            is_boolean: true,
        },
        ParityCase {
            id: "or_fn",
            label: "OR(10 < 5, 20 > 10)",
            formula: "OR(10 < 5, 20 > 10)",
            is_boolean: true,
        },
        ParityCase {
            id: "not_fn",
            label: "NOT(10 < 5)",
            formula: "NOT(10 < 5)",
            is_boolean: true,
        },
        ParityCase {
            id: "choose_fn",
            label: "CHOOSE(2, 100, 200, 300)",
            formula: "CHOOSE(2, 100, 200, 300)",
            is_boolean: false,
        },
        ParityCase {
            id: "switch_fn",
            label: "SWITCH(2, 1, 100, 2, 200, 3, 300)",
            formula: "SWITCH(2, 1, 100, 2, 200, 3, 300)",
            is_boolean: false,
        },
    ];

    let financial_cases = [
        ParityCase {
            id: "npv",
            label: "NPV(0.1, cash_flows)",
            formula: "NPV(0.1, SeedData[\"r2\"..\"r4\", \"cash_flow\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "xnpv",
            label: "XNPV(0.1, cash_flows, dates)",
            formula: "XNPV(0.1, SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "irr",
            label: "IRR(cash_flows)",
            formula: "IRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "xirr",
            label: "XIRR(cash_flows, dates)",
            formula: "XIRR(SeedData[\"r1\"..\"r4\", \"cash_flow\"], SeedData[\"r1\"..\"r4\", \"date\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "pmt",
            label: "PMT(0.06/12, 36, -10000)",
            formula: "PMT(0.06 / 12, 36, -10000)",
            is_boolean: false,
        },
        ParityCase {
            id: "ipmt",
            label: "IPMT(0.06/12, 1, 36, -10000)",
            formula: "IPMT(0.06 / 12, 1, 36, -10000)",
            is_boolean: false,
        },
        ParityCase {
            id: "ppmt",
            label: "PPMT(0.06/12, 1, 36, -10000)",
            formula: "PPMT(0.06 / 12, 1, 36, -10000)",
            is_boolean: false,
        },
    ];

    let lookup_cases = [
        ParityCase {
            id: "xlookup",
            label: "XLOOKUP(\"Beta\", cat, num_a)",
            formula: "XLOOKUP(\"Beta\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "xmatch",
            label: "XMATCH(\"Beta\", cat)",
            formula: "XMATCH(\"Beta\", SeedData[\"r1\"..\"r4\", \"category\"])",
            is_boolean: false,
        },
        ParityCase {
            id: "index_fn",
            label: "INDEX(num_a, 3)",
            formula: "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], 3)",
            is_boolean: false,
        },
    ];

    let date_cases = [
        ParityCase {
            id: "date_fn",
            label: "DATE(2024, 6, 15)",
            formula: "DATE(2024, 6, 15)",
            is_boolean: false,
        },
        ParityCase {
            id: "edate",
            label: "EDATE(DATE(2024, 1, 31), 1)",
            formula: "EDATE(DATE(2024, 1, 31), 1)",
            is_boolean: false,
        },
        ParityCase {
            id: "eomonth",
            label: "EOMONTH(DATE(2024, 1, 15), 1)",
            formula: "EOMONTH(DATE(2024, 1, 15), 1)",
            is_boolean: false,
        },
        ParityCase {
            id: "yearfrac",
            label: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0)",
            formula: "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0)",
            is_boolean: false,
        },
        ParityCase {
            id: "year_fn",
            label: "YEAR(DATE(2024, 6, 15))",
            formula: "YEAR(DATE(2024, 6, 15))",
            is_boolean: false,
        },
        ParityCase {
            id: "month_fn",
            label: "MONTH(DATE(2024, 6, 15))",
            formula: "MONTH(DATE(2024, 6, 15))",
            is_boolean: false,
        },
        ParityCase {
            id: "day_fn",
            label: "DAY(DATE(2024, 6, 15))",
            formula: "DAY(DATE(2024, 6, 15))",
            is_boolean: false,
        },
        ParityCase {
            id: "days_fn",
            label: "DAYS(DATE(2024, 6, 15), DATE(2024, 6, 1))",
            formula: "DAYS(DATE(2024, 6, 15), DATE(2024, 6, 1))",
            is_boolean: false,
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
            let check_expr = if case.is_boolean {
                format!(
                    "IF({table_id}[@row, \"formula_val\"] = {table_id}[@row, \"f8_val\"], \"PASS\", \"FAIL\")"
                )
            } else {
                format!(
                    "IF(ABS({table_id}[@row, \"formula_val\"] - {table_id}[@row, \"f8_val\"]) < 0.0001, \"PASS\", \"FAIL\")"
                )
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
        "Math & Aggregation Functions (17)",
        "ABS, SUM, AVERAGE, COUNT, COUNTA, MAX, MIN, MEDIAN, ROUND, ROUNDUP, ROUNDDOWN, SUMPRODUCT, *IFS",
        &math_cases,
    );
    let logical_table = build_category_table(
        "LogicalParity",
        "Logical & Branching Functions (9)",
        "IF, IFS, IFERROR, IFNA, AND, OR, NOT, CHOOSE, SWITCH",
        &logical_cases,
    );
    let financial_table = build_category_table(
        "FinancialParity",
        "Financial & Return Functions (7)",
        "NPV, XNPV, IRR, XIRR, PMT, IPMT, PPMT",
        &financial_cases,
    );
    let lookup_table = build_category_table(
        "LookupParity",
        "Lookup & Modern Array Functions (3)",
        "XLOOKUP, XMATCH, INDEX",
        &lookup_cases,
    );
    let date_table = build_category_table(
        "DateParity",
        "Date Construction & Difference Functions (8)",
        "DATE, EDATE, EOMONTH, YEARFRAC, YEAR, MONTH, DAY, DAYS",
        &date_cases,
    );

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
    if let Ok(evaluated) = Compiler::evaluate(&manifest, &initial_tables) {
        // Populate f8_val with the concrete evaluated values as static literals
        for table in &mut final_tables[1..] {
            let t_id = &table.id;
            for row in &mut table.rows {
                if let RowSpec::Data(d) = row {
                    let r_id = &d.id;
                    let eval_val = evaluated
                        .get_value(t_id, r_id, &ColId::from("formula_val"))
                        .unwrap_or(&CellValue::Empty);

                    let literal_input = match eval_val {
                        CellValue::Number(n) => CellInput::number(*n),
                        CellValue::Boolean(b) => CellInput::boolean(*b),
                        CellValue::Text(s) => CellInput::string(s),
                        _ => CellInput::number(0.0),
                    };
                    d.cells.insert(ColId::from("f8_val"), literal_input);
                }
            }
        }
    }

    (manifest, final_tables)
}
