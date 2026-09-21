//! Consolidated Three-Statement Financial Model test fixture.

use std::path::PathBuf;

use finx::model::format::{FormatPreset, SemanticStyle};
use finx::model::id::TableId;
use finx::model::manifest::{ProjectManifest, SheetSpec};
use finx::model::table::{ColumnSpec, RowSpec, TableSpec};

use super::{RowBuilder, build_descriptor_col, build_measure_col_label};

/// Constructs the canonical 4-table Three-Statement financial model.
#[must_use]
pub fn build_three_statement_model() -> (ProjectManifest, Vec<TableSpec>) {
    // -------------------------------------------------------------------------
    // Shared Timeline Columns & Multi-Tier Headers
    // -------------------------------------------------------------------------
    let timeline_cols = vec![
        build_descriptor_col("line_item", "Line Item"),
        ColumnSpec::new("y2023a").with_headers(["Historical Actuals", "2023A"]),
        ColumnSpec::new("y2024e").with_headers(["Forecast Projections", "2024E"]),
        ColumnSpec::new("y2025e").with_headers(["Forecast Projections", "2025E"]),
        ColumnSpec::new("y2026e").with_headers(["Forecast Projections", "2026E"]),
    ];

    // -------------------------------------------------------------------------
    // 1. Assumptions Table
    // -------------------------------------------------------------------------
    let assumptions_table = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Model Assumptions".to_string(),
        subtitle: Some("Key Operating Drivers & Macro Parameters".to_string()),
        currency: None,
        collapsible: false,
        columns: vec![
            build_descriptor_col("line_item", "Parameter"),
            build_measure_col_label("val", "Value"),
        ],
        rows: vec![
            RowBuilder::line_item("tax_rate", "Corporate Tax Rate")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_num("val", 0.20)
                .build(),
            RowBuilder::line_item("rev_growth", "Revenue Growth Rate")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_num("val", 0.15)
                .build(),
            RowBuilder::line_item("cogs_pct", "Cost of Goods Sold (% Revenue)")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_num("val", 0.40)
                .build(),
            RowBuilder::line_item("opex_salaries", "Salaries & SG&A Overhead ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_num("val", 20.0)
                .build(),
            RowBuilder::line_item("depreciation", "Depreciation Expense ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_num("val", 10.0)
                .build(),
            RowBuilder::line_item("capex", "Capital Expenditures ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_num("val", 15.0)
                .build(),
            RowBuilder::line_item("wc_pct_rev", "Working Capital (% Revenue)")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_num("val", 0.10)
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 2. Income Statement Table
    // -------------------------------------------------------------------------
    let income_statement_table = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".to_string(),
        subtitle: Some("($ in Millions)".to_string()),
        currency: None,
        collapsible: false,
        columns: timeline_cols.clone(),
        rows: vec![
            RowBuilder::line_item("revenue", "Revenue")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("IncomeStatement[\"revenue\", col.prev] * (1.0 + Assumptions[\"rev_growth\", \"val\"])")
                .cell_num("y2023a", 100.0)
                .build(),
            RowBuilder::line_item("cogs", "Cost of Goods Sold")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("IncomeStatement[\"revenue\", @col] * Assumptions[\"cogs_pct\", \"val\"]")
                .build(),
            RowBuilder::line_item("gross_profit", "Gross Profit")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]")
                .build(),
            RowSpec::section_header("Operating Expenses"),
            RowBuilder::line_item("opex", "Salaries & SG&A Overhead")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("Assumptions[\"opex_salaries\", \"val\"]")
                .build(),
            RowBuilder::line_item("depreciation", "Depreciation Expense")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("Assumptions[\"depreciation\", \"val\"]")
                .build(),
            RowBuilder::line_item("operating_income", "Operating Income (EBIT)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("IncomeStatement[\"gross_profit\", @col] - IncomeStatement[\"opex\", @col] - IncomeStatement[\"depreciation\", @col]")
                .build(),
            RowBuilder::line_item("tax", "Income Tax Provision")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("IncomeStatement[\"operating_income\", @col] * Assumptions[\"tax_rate\", \"val\"]")
                .build(),
            RowBuilder::line_item("net_income", "Net Income")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .highlight(true)
                .expr("IncomeStatement[\"operating_income\", @col] - IncomeStatement[\"tax\", @col]")
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 3. Cash Flow Table
    // -------------------------------------------------------------------------
    let cash_flow_table = TableSpec {
        id: TableId::from("CashFlow"),
        title: "Cash Flow Statement".to_string(),
        subtitle: Some("($ in Millions)".to_string()),
        currency: None,
        collapsible: false,
        columns: timeline_cols.clone(),
        rows: vec![
            RowSpec::section_header("Operating Activities"),
            RowBuilder::line_item("net_income", "Net Income")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("IncomeStatement[\"net_income\", @col]")
                .build(),
            RowBuilder::line_item("depreciation", "Depreciation Addback")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("Assumptions[\"depreciation\", \"val\"]")
                .build(),
            RowBuilder::line_item("change_in_wc", "Change in Working Capital")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("(IncomeStatement[\"revenue\", @col] - IncomeStatement[\"revenue\", col.prev]) * Assumptions[\"wc_pct_rev\", \"val\"]")
                .cell_num("y2023a", 0.0)
                .build(),
            RowBuilder::line_item("operating_cf", "Cash Flow from Operations")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("CashFlow[\"net_income\", @col] + CashFlow[\"depreciation\", @col] - CashFlow[\"change_in_wc\", @col]")
                .build(),
            RowSpec::section_header("Investing Activities"),
            RowBuilder::line_item("capex", "Capital Expenditures")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("Assumptions[\"capex\", \"val\"]")
                .build(),
            RowBuilder::line_item("investing_cf", "Cash Flow from Investing")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("0.0 - CashFlow[\"capex\", @col]")
                .build(),
            RowBuilder::line_item("net_cash_flow", "Net Change in Cash")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("CashFlow[\"operating_cf\", @col] + CashFlow[\"investing_cf\", @col]")
                .build(),
            RowBuilder::line_item("ending_cash", "Ending Cash Balance")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .highlight(true)
                .expr("CashFlow[\"ending_cash\", col.prev] + CashFlow[\"net_cash_flow\", @col]")
                .cell_num("y2023a", 50.0)
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 4. Balance Sheet Table
    // -------------------------------------------------------------------------
    let balance_sheet_table = TableSpec {
        id: TableId::from("BalanceSheet"),
        title: "Balance Sheet".to_string(),
        subtitle: Some("($ in Millions)".to_string()),
        currency: None,
        collapsible: false,
        columns: timeline_cols,
        rows: vec![
            // Assets
            RowSpec::section_header("Assets"),
            RowBuilder::line_item("cash", "Cash & Cash Equivalents")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("CashFlow[\"ending_cash\", @col]")
                .build(),
            RowBuilder::line_item("working_capital", "Operating Working Capital")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("IncomeStatement[\"revenue\", @col] * Assumptions[\"wc_pct_rev\", \"val\"]")
                .build(),
            RowBuilder::line_item("ppe", "Property, Plant & Equipment (Net)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("BalanceSheet[\"ppe\", col.prev] + Assumptions[\"capex\", \"val\"] - Assumptions[\"depreciation\", \"val\"]")
                .cell_num("y2023a", 40.0)
                .build(),
            RowBuilder::line_item("total_assets", "Total Assets")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .highlight(true)
                .expr("BalanceSheet[\"cash\", @col] + BalanceSheet[\"working_capital\", @col] + BalanceSheet[\"ppe\", @col]")
                .build(),
            // Liabilities & Equity
            RowSpec::section_header("Liabilities & Stockholders' Equity"),
            RowBuilder::line_item("debt", "Total Outstanding Debt")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("0.0")
                .build(),
            RowBuilder::line_item("equity_capital", "Common Stock & Contributed Capital")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("BalanceSheet[\"equity_capital\", col.prev]")
                .cell_num("y2023a", 76.0)
                .build(),
            RowBuilder::line_item("retained_earnings", "Retained Earnings")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("BalanceSheet[\"retained_earnings\", col.prev] + IncomeStatement[\"net_income\", @col]")
                .cell_num("y2023a", 24.0)
                .build(),
            RowBuilder::line_item("total_liabilities_and_equity", "Total Liabilities & Equity")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .highlight(true)
                .expr("BalanceSheet[\"debt\", @col] + BalanceSheet[\"equity_capital\", @col] + BalanceSheet[\"retained_earnings\", @col]")
                .build(),
            // Balance Check: Total Assets - (Total Liabilities + Equity) == 0.0
            RowBuilder::line_item("balance_check", "Balance Check (Assets - L&E)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("BalanceSheet[\"total_assets\", @col] - BalanceSheet[\"total_liabilities_and_equity\", @col]")
                .build(),
        ],
    };

    let manifest = ProjectManifest {
        model_name: "ThreeStatementFinancialModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: Some("#1B365D".to_string()),
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                PathBuf::from("tables/assumptions.json"),
                PathBuf::from("tables/income_statement.json"),
                PathBuf::from("tables/cash_flow.json"),
                PathBuf::from("tables/balance_sheet.json"),
            ],
        }],
    };

    let tables = vec![
        assumptions_table,
        income_statement_table,
        cash_flow_table,
        balance_sheet_table,
    ];

    (manifest, tables)
}
