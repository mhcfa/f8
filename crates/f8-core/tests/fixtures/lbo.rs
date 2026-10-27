//! Consolidated Institutional LBO Model test fixture.

use f8_core::model::format::{FormatPreset, SemanticStyle};
use f8_core::model::id::TableId;
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};

use super::{
    RowBuilder, build_descriptor_col, build_gutter_col, build_gutter_row, build_measure_col_label,
};

/// Assembles the complete 8-table, 2-sheet institutional LBO model specification.
#[must_use]
pub fn build_lbo_model(active_case: &str) -> (ProjectManifest, Vec<TableSpec>) {
    // -------------------------------------------------------------------------
    // Shared Timeline Columns & Multi-Tier Headers (Sheet: Model)
    // -------------------------------------------------------------------------
    let timeline_cols = vec![
        build_descriptor_col("line_item", "Line Item"),
        ColumnSpec::new("y0")
            .with_group("Project Titan LBO")
            .with_headers(["Transaction Close", "Close (Y0)"]),
        ColumnSpec::new("y1")
            .with_group("Project Titan LBO")
            .with_headers(["5-Year Investment Horizon", "2025 (Y1)"]),
        ColumnSpec::new("y2")
            .with_group("Project Titan LBO")
            .with_headers(["5-Year Investment Horizon", "2026 (Y2)"]),
        ColumnSpec::new("y3")
            .with_group("Project Titan LBO")
            .with_headers(["5-Year Investment Horizon", "2027 (Y3)"]),
        ColumnSpec::new("y4")
            .with_group("Project Titan LBO")
            .with_headers(["5-Year Investment Horizon", "2028 (Y4)"]),
        ColumnSpec::new("y5")
            .with_group("Project Titan LBO")
            .with_headers(["5-Year Investment Horizon", "2029 (Y5)"]),
    ];

    // -------------------------------------------------------------------------
    // 1. Scenarios Table (Sheet: Summary)
    // -------------------------------------------------------------------------
    let scenarios_table = TableSpec {
        id: TableId::from("Scenarios"),
        title: "Operating & Exit Scenarios".to_string(),
        subtitle: Some("Management Case Sensitivity Matrix".to_string()),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            build_descriptor_col("scenario_name", "Scenario Case"),
            build_measure_col_label("case_id", "Case ID"),
            build_measure_col_label("rev_growth", "Rev Growth"),
            build_measure_col_label("ebitda_margin", "EBITDA Margin"),
            build_measure_col_label("exit_multiple", "Exit Multiple"),
        ],
        rows: vec![
            RowBuilder::descriptor("base", "scenario_name", "Base Case (Management Plan)")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_str("case_id", "base")
                .cell_num("rev_growth", 0.08)
                .cell_num("ebitda_margin", 0.25)
                .cell_num_format("exit_multiple", 10.0, FormatPreset::Multiple, 1)
                .build(),
            RowBuilder::descriptor("bull", "scenario_name", "Bull Case (Upside Expansion)")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_str("case_id", "bull")
                .cell_num("rev_growth", 0.12)
                .cell_num("ebitda_margin", 0.28)
                .cell_num_format("exit_multiple", 12.0, FormatPreset::Multiple, 1)
                .build(),
            RowBuilder::descriptor("bear", "scenario_name", "Bear Case (Downside Contraction)")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_str("case_id", "bear")
                .cell_num("rev_growth", 0.03)
                .cell_num("ebitda_margin", 0.20)
                .cell_num_format("exit_multiple", 8.0, FormatPreset::Multiple, 1)
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 2. DealAssumptions Table (Sheet: Summary)
    // -------------------------------------------------------------------------
    let deal_assumptions_table = TableSpec {
        id: TableId::from("DealAssumptions"),
        title: "Transaction & Financing Assumptions".to_string(),
        subtitle: Some("Capital Structure, Debt Terms & Purchase Price".to_string()),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            build_descriptor_col("line_item", "Transaction Parameter"),
            build_measure_col_label("val", "Value"),
        ],
        rows: vec![
            // Scenario Selection
            RowSpec::section_header("Scenario Case Selection"),
            RowBuilder::line_item("active_case", "Active Scenario Case")
                .cell_str("val", active_case)
                .build(),
            RowBuilder::line_item("rev_growth", "Revenue Growth Rate")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_formula(
                    "val",
                    "XLOOKUP(DealAssumptions[\"active_case\", \"val\"], Scenarios[\"base\"..\"bear\", \"case_id\"], Scenarios[\"base\"..\"bear\", \"rev_growth\"])",
                )
                .build(),
            RowBuilder::line_item("ebitda_margin", "EBITDA Margin Target")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_formula(
                    "val",
                    "XLOOKUP(DealAssumptions[\"active_case\", \"val\"], Scenarios[\"base\"..\"bear\", \"case_id\"], Scenarios[\"base\"..\"bear\", \"ebitda_margin\"])",
                )
                .build(),
            RowBuilder::line_item("exit_multiple", "Exit Multiple (EV / EBITDA)")
                .format(FormatPreset::Multiple)
                .decimals(1)
                .cell_formula(
                    "val",
                    "XLOOKUP(DealAssumptions[\"active_case\", \"val\"], Scenarios[\"base\"..\"bear\", \"case_id\"], Scenarios[\"base\"..\"bear\", \"exit_multiple\"])",
                )
                .build(),
            RowBuilder::line_item("sponsor_hurdle", "Sponsor Target Hurdle Rate (SWITCH)")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_formula(
                    "val",
                    "SWITCH(DealAssumptions[\"active_case\", \"val\"], \"base\", 0.20, \"bull\", 0.25, \"bear\", 0.15, 0.20)",
                )
                .build(),
            build_gutter_row(),
            // Valuation & Leverage Drivers
            RowSpec::section_header("Valuation & Leverage Drivers"),
            RowBuilder::line_item("entry_ebitda", "LTM Entry EBITDA ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_num("val", 50.0)
                .build(),
            RowBuilder::line_item("entry_multiple", "Entry Multiple (EV / EBITDA)")
                .format(FormatPreset::Multiple)
                .decimals(1)
                .cell_num("val", 10.0)
                .build(),
            RowBuilder::line_item("senior_leverage", "Senior Debt Leverage (x EBITDA)")
                .format(FormatPreset::Multiple)
                .decimals(1)
                .cell_num("val", 3.0)
                .build(),
            RowBuilder::line_item("sub_leverage", "Subordinated Debt Leverage (x EBITDA)")
                .format(FormatPreset::Multiple)
                .decimals(1)
                .cell_num("val", 1.5)
                .build(),
            RowBuilder::line_item("senior_rate", "Senior Debt Interest Rate")
                .format(FormatPreset::Percentage)
                .decimals(2)
                .cell_num("val", 0.08)
                .build(),
            RowBuilder::line_item("sub_rate", "Subordinated Debt Interest Rate")
                .format(FormatPreset::Percentage)
                .decimals(2)
                .cell_num("val", 0.12)
                .build(),
            RowBuilder::line_item("tax_rate", "Corporate Tax Rate")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_num("val", 0.25)
                .build(),
            RowBuilder::line_item("capex_pct", "Capex (% Revenue)")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_num("val", 0.05)
                .build(),
            RowBuilder::line_item("discount_rate", "Sponsor Hurdle Rate (WACC)")
                .format(FormatPreset::Percentage)
                .decimals(1)
                .cell_num("val", 0.10)
                .build(),
            build_gutter_row(),
            // Sources & Uses Calculations
            RowSpec::section_header("Sources & Uses of Funds"),
            RowBuilder::line_item("entry_ev", "Target Enterprise Value ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .cell_formula(
                    "val",
                    "DealAssumptions[\"entry_ebitda\", \"val\"] * DealAssumptions[\"entry_multiple\", \"val\"]",
                )
                .build(),
            RowBuilder::line_item("senior_debt_entry", "Senior Debt Issued ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_formula(
                    "val",
                    "DealAssumptions[\"entry_ebitda\", \"val\"] * DealAssumptions[\"senior_leverage\", \"val\"]",
                )
                .build(),
            RowBuilder::line_item("sub_debt_entry", "Subordinated Debt Issued ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_formula(
                    "val",
                    "DealAssumptions[\"entry_ebitda\", \"val\"] * DealAssumptions[\"sub_leverage\", \"val\"]",
                )
                .build(),
            RowBuilder::line_item("total_debt_entry", "Total Debt Financed ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .cell_formula(
                    "val",
                    "DealAssumptions[\"senior_debt_entry\", \"val\"] + DealAssumptions[\"sub_debt_entry\", \"val\"]",
                )
                .build(),
            RowBuilder::line_item("sponsor_equity_entry", "Sponsor Equity Invested ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .highlight(true)
                .cell_formula(
                    "val",
                    "DealAssumptions[\"entry_ev\", \"val\"] - DealAssumptions[\"total_debt_entry\", \"val\"]",
                )
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 3. SourcesAndUses Table (Sheet: Summary) - Irregular 7-Column Layout
    // -------------------------------------------------------------------------
    let sources_and_uses_cols = vec![
        build_descriptor_col("source_item", "Sources of Financing"),
        ColumnSpec::new("source_val")
            .with_header("Sources ($M)")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
        ColumnSpec::new("source_pct")
            .with_header("% Total")
            .with_format(Some(FormatPreset::Percentage))
            .with_decimals(Some(1)),
        build_gutter_col(),
        build_descriptor_col("use_item", "Uses of Capital"),
        ColumnSpec::new("use_val")
            .with_header("Uses ($M)")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
        ColumnSpec::new("use_pct")
            .with_header("% Total")
            .with_format(Some(FormatPreset::Percentage))
            .with_decimals(Some(1)),
    ];

    let sources_and_uses_table = TableSpec {
        id: TableId::from("SourcesAndUses"),
        title: "Transaction Sources & Uses of Funds".to_string(),
        subtitle: Some(
            "7-Column Irregular Layout with Intra-Table Dividing Gutter".to_string(),
        ),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: sources_and_uses_cols,
        rows: vec![
            RowBuilder::new("senior_debt")
                .cell_str("source_item", "Senior Secured Term Loan")
                .cell_formula("source_val", "DealAssumptions[\"senior_debt_entry\", \"val\"]")
                .cell_formula(
                    "source_pct",
                    "SourcesAndUses[\"senior_debt\", \"source_val\"] / SourcesAndUses[\"total\", \"source_val\"]",
                )
                .cell_str("use_item", "Acquisition Target Enterprise Value")
                .cell_formula("use_val", "DealAssumptions[\"entry_ev\", \"val\"]")
                .cell_formula(
                    "use_pct",
                    "SourcesAndUses[\"senior_debt\", \"use_val\"] / SourcesAndUses[\"total\", \"use_val\"]",
                )
                .build(),
            RowBuilder::new("sub_debt")
                .cell_str("source_item", "Subordinated Mezzanine Notes")
                .cell_formula("source_val", "DealAssumptions[\"sub_debt_entry\", \"val\"]")
                .cell_formula(
                    "source_pct",
                    "SourcesAndUses[\"sub_debt\", \"source_val\"] / SourcesAndUses[\"total\", \"source_val\"]",
                )
                .cell_str("use_item", "Financing & Legal Advisory Fees")
                .cell_num("use_val", 0.0)
                .cell_num("use_pct", 0.0)
                .build(),
            RowBuilder::new("sponsor_equity")
                .cell_str("source_item", "Sponsor Equity Capital")
                .cell_formula("source_val", "DealAssumptions[\"sponsor_equity_entry\", \"val\"]")
                .cell_formula(
                    "source_pct",
                    "SourcesAndUses[\"sponsor_equity\", \"source_val\"] / SourcesAndUses[\"total\", \"source_val\"]",
                )
                .cell_str("use_item", "Minimum Balance Sheet Cash Buffer")
                .cell_num("use_val", 0.0)
                .cell_num("use_pct", 0.0)
                .build(),
            build_gutter_row(),
            RowBuilder::new("total")
                .cell_str("source_item", "Total Sources of Financing")
                .style(SemanticStyle::Total)
                .highlight(true)
                .cell_formula(
                    "source_val",
                    "SourcesAndUses[\"senior_debt\", \"source_val\"] + SourcesAndUses[\"sub_debt\", \"source_val\"] + SourcesAndUses[\"sponsor_equity\", \"source_val\"]",
                )
                .cell_formula(
                    "source_pct",
                    "SourcesAndUses[\"senior_debt\", \"source_pct\"] + SourcesAndUses[\"sub_debt\", \"source_pct\"] + SourcesAndUses[\"sponsor_equity\", \"source_pct\"]",
                )
                .cell_str("use_item", "Total Uses of Capital")
                .cell_formula(
                    "use_val",
                    "SourcesAndUses[\"senior_debt\", \"use_val\"] + SourcesAndUses[\"sub_debt\", \"use_val\"] + SourcesAndUses[\"sponsor_equity\", \"use_val\"]",
                )
                .cell_formula(
                    "use_pct",
                    "SourcesAndUses[\"senior_debt\", \"use_pct\"] + SourcesAndUses[\"sub_debt\", \"use_pct\"] + SourcesAndUses[\"sponsor_equity\", \"use_pct\"]",
                )
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 4. ReturnsSummary Table (Sheet: Summary)
    // -------------------------------------------------------------------------
    let returns_summary_table = TableSpec {
        id: TableId::from("ReturnsSummary"),
        title: "Exit Valuation & Sponsor Returns".to_string(),
        subtitle: Some("5-Year Exit Multiples, IRR & Waterfall Proceeds".to_string()),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            build_descriptor_col("line_item", "Investment Return Metric"),
            build_measure_col_label("val", "Metric Value"),
        ],
        rows: vec![
            RowSpec::section_header("Exit Valuation"),
            RowBuilder::line_item("exit_year", "Exit Fiscal Year")
                .format(FormatPreset::Number)
                .decimals(0)
                .cell_formula("val", "YEAR(Timeline[\"date\", \"y5\"])")
                .build(),
            RowBuilder::line_item("exit_multiple", "Exit EV / EBITDA Multiple")
                .format(FormatPreset::Multiple)
                .decimals(1)
                .cell_formula("val", "DealAssumptions[\"exit_multiple\", \"val\"]")
                .build(),
            RowBuilder::line_item("exit_ebitda", "Exit Year LTM EBITDA ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_formula("val", "Operations[\"ebitda\", \"y5\"]")
                .build(),
            RowBuilder::line_item("exit_ev", "Exit Enterprise Value ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .cell_formula(
                    "val",
                    "ReturnsSummary[\"exit_ebitda\", \"val\"] * ReturnsSummary[\"exit_multiple\", \"val\"]",
                )
                .build(),
            RowBuilder::line_item("exit_net_debt", "Less: Ending Net Debt ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_formula("val", "DebtSchedule[\"total_debt_end\", \"y5\"]")
                .build(),
            RowBuilder::line_item("exit_equity_proceeds", "Exit Sponsor Equity Value ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .cell_formula(
                    "val",
                    "ReturnsSummary[\"exit_ev\", \"val\"] - ReturnsSummary[\"exit_net_debt\", \"val\"]",
                )
                .build(),
            RowSpec::section_header("Sponsor Investment Returns"),
            RowBuilder::line_item("initial_equity", "Initial Sponsor Equity Invested ($M)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .cell_formula("val", "DealAssumptions[\"sponsor_equity_entry\", \"val\"]")
                .build(),
            RowBuilder::line_item("moic", "Multiple on Invested Capital (MOIC)")
                .format(FormatPreset::Multiple)
                .decimals(2)
                .style(SemanticStyle::Subtotal)
                .highlight(true)
                .cell_formula(
                    "val",
                    "ReturnsSummary[\"exit_equity_proceeds\", \"val\"] / ReturnsSummary[\"initial_equity\", \"val\"]",
                )
                .build(),
            RowBuilder::line_item("irr", "Internal Rate of Return (IRR)")
                .format(FormatPreset::Percentage)
                .decimals(2)
                .style(SemanticStyle::Total)
                .highlight(true)
                .cell_formula(
                    "val",
                    "XIRR(CashFlows[\"sponsor_cf\", \"y0\"..\"y5\"], Timeline[\"date\", \"y0\"..\"y5\"])",
                )
                .build(),
            RowBuilder::line_item("npv", "Net Present Value (NPV @ 10%)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .cell_formula(
                    "val",
                    "XNPV(DealAssumptions[\"discount_rate\", \"val\"], CashFlows[\"sponsor_cf\", \"y0\"..\"y5\"], Timeline[\"date\", \"y0\"..\"y5\"])",
                )
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 4. Timeline Table (Sheet: Model)
    // -------------------------------------------------------------------------
    let timeline_table = TableSpec {
        id: TableId::from("Timeline"),
        title: "Investment Timeline".to_string(),
        subtitle: Some("Annual Financial Calendar & Year Fractions".to_string()),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: timeline_cols.clone(),
        rows: vec![
            RowBuilder::line_item("date", "Period End Date")
                .format(FormatPreset::Date)
                .expr("EDATE(Timeline[\"date\", col.prev], 12)")
                .cell_formula("y0", "DATE(2024, 12, 31)")
                .build(),
            RowBuilder::line_item("year", "Calendar Year")
                .format(FormatPreset::Integer)
                .expr("YEAR(Timeline[\"date\", @col])")
                .build(),
            RowBuilder::line_item("days", "Period Elapsed Days")
                .format(FormatPreset::Number)
                .decimals(0)
                .expr("DAYS(Timeline[\"date\", @col], Timeline[\"date\", col.prev])")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("year_frac", "Year Fraction (30/360)")
                .format(FormatPreset::Number)
                .decimals(2)
                .expr("YEARFRAC(Timeline[\"date\", col.prev], Timeline[\"date\", @col], 0)")
                .cell_num("y0", 0.0)
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 5. Operations Table (Sheet: Model)
    // -------------------------------------------------------------------------
    let operations_table = TableSpec {
        id: TableId::from("Operations"),
        title: "Operating Forecast & Cash Flows".to_string(),
        subtitle: Some("($ in Millions)".to_string()),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: timeline_cols.clone(),
        rows: vec![
            RowSpec::section_header("Operating Forecast"),
            RowBuilder::line_item("revenue", "Revenue")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("Operations[\"revenue\", col.prev] * (1 + DealAssumptions[\"rev_growth\", \"val\"])")
                .cell_num("y0", 200.0)
                .build(),
            RowBuilder::line_item("ebitda", "EBITDA")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("Operations[\"revenue\", @col] * DealAssumptions[\"ebitda_margin\", \"val\"]")
                .cell_num("y0", 50.0)
                .build(),
            RowBuilder::line_item("depreciation", "Depreciation Expense")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("Operations[\"revenue\", @col] * 0.04")
                .cell_num("y0", 8.0)
                .build(),
            RowBuilder::line_item("ebit", "Operating Income (EBIT)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("Operations[\"ebitda\", @col] - Operations[\"depreciation\", @col]")
                .build(),
            RowSpec::section_header("Earnings & Taxes"),
            RowBuilder::line_item("interest_expense", "Debt Interest Expense")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("DebtSchedule[\"total_interest\", @col]")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("ebt", "Earnings Before Taxes (EBT)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("Operations[\"ebit\", @col] - Operations[\"interest_expense\", @col]")
                .build(),
            RowBuilder::line_item("tax", "Income Tax Provision")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("IF(Operations[\"ebt\", @col] > 0, Operations[\"ebt\", @col] * DealAssumptions[\"tax_rate\", \"val\"], 0)")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("net_income", "Net Income")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("Operations[\"ebt\", @col] - Operations[\"tax\", @col]")
                .build(),
            RowSpec::section_header("Cash Conversion"),
            RowBuilder::line_item("capex", "Capital Expenditures")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("Operations[\"revenue\", @col] * DealAssumptions[\"capex_pct\", \"val\"]")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("fcf_pre_debt", "Free Cash Flow (Pre-Debt)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .highlight(true)
                .expr("Operations[\"ebitda\", @col] - Operations[\"tax\", @col] - Operations[\"capex\", @col]")
                .cell_num("y0", 0.0)
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 6. DebtSchedule Table (Sheet: Model)
    // -------------------------------------------------------------------------
    let debt_schedule_table = TableSpec {
        id: TableId::from("DebtSchedule"),
        title: "Debt Schedule & Cash Sweep".to_string(),
        subtitle: Some("($ in Millions) - Senior Debt 5% Mandatory Amortization & 100% Cash Sweep".to_string()),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: timeline_cols.clone(),
        rows: vec![
            RowSpec::section_header("Senior Tranche (8.0% Rate)"),
            RowBuilder::line_item("senior_begin", "Senior Debt - Beginning")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("DebtSchedule[\"senior_end\", col.prev]")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("senior_interest", "Senior Debt Interest (8.0%)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("DebtSchedule[\"senior_begin\", @col] * DealAssumptions[\"senior_rate\", \"val\"] * Timeline[\"year_frac\", @col]")
                .cell_num("y0", 0.0)
                .build(),
            build_gutter_row(),
            RowSpec::section_header("Subordinated Tranche (12.0% Rate)"),
            RowBuilder::line_item("sub_begin", "Subordinated Debt - Beginning")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("DebtSchedule[\"sub_end\", col.prev]")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("sub_interest", "Subordinated Debt Interest (12.0%)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("DebtSchedule[\"sub_begin\", @col] * DealAssumptions[\"sub_rate\", \"val\"] * Timeline[\"year_frac\", @col]")
                .cell_num("y0", 0.0)
                .build(),
            build_gutter_row(),
            RowSpec::section_header("Debt Service & Sweep Mechanics"),
            RowBuilder::line_item("term_pmt_benchmark", "5-Year Full Amortization Benchmark (PMT)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("PMT(DealAssumptions[\"senior_rate\", \"val\"], 5, -DealAssumptions[\"senior_debt_entry\", \"val\"])")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("total_interest", "Total Debt Interest")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Subtotal)
                .expr("DebtSchedule[\"senior_interest\", @col] + DebtSchedule[\"sub_interest\", @col]")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("senior_mandatory", "Senior Mandatory Amortization (5%)")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("MIN(DebtSchedule[\"senior_begin\", @col], DealAssumptions[\"senior_debt_entry\", \"val\"] * 0.05)")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("cash_available_sweep", "Cash Flow Available for Sweep")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("MAX(0, Operations[\"fcf_pre_debt\", @col] - DebtSchedule[\"total_interest\", @col] - DebtSchedule[\"senior_mandatory\", @col])")
                .cell_num("y0", 0.0)
                .build(),
            RowBuilder::line_item("senior_sweep", "Senior Debt Cash Sweep")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("MIN(DebtSchedule[\"cash_available_sweep\", @col], DebtSchedule[\"senior_begin\", @col] - DebtSchedule[\"senior_mandatory\", @col])")
                .cell_num("y0", 0.0)
                .build(),
            build_gutter_row(),
            RowSpec::section_header("Outstanding Debt Balances"),
            RowBuilder::line_item("senior_end", "Senior Debt - Ending Balance")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("DebtSchedule[\"senior_begin\", @col] - DebtSchedule[\"senior_mandatory\", @col] - DebtSchedule[\"senior_sweep\", @col]")
                .cell_formula("y0", "DealAssumptions[\"senior_debt_entry\", \"val\"]")
                .build(),
            RowBuilder::line_item("sub_end", "Subordinated Debt - Ending Balance")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("DebtSchedule[\"sub_begin\", @col]")
                .cell_formula("y0", "DealAssumptions[\"sub_debt_entry\", \"val\"]")
                .build(),
            RowBuilder::line_item("total_debt_end", "Total Outstanding Debt")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .highlight(true)
                .expr("DebtSchedule[\"senior_end\", @col] + DebtSchedule[\"sub_end\", @col]")
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // 7. CashFlows Table (Sheet: Model)
    // -------------------------------------------------------------------------
    let cash_flows_table = TableSpec {
        id: TableId::from("CashFlows"),
        title: "Sponsor Equity Cash Flows".to_string(),
        subtitle: Some(
            "($ in Millions) - Initial Equity Investment & Exit Equity Proceeds".to_string(),
        ),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: timeline_cols,
        rows: vec![
            RowBuilder::line_item("sponsor_cf", "Sponsor Net Equity Cash Flow")
                .format(FormatPreset::Currency)
                .decimals(1)
                .style(SemanticStyle::Total)
                .highlight(true)
                .cell_formula("y0", "-DealAssumptions[\"sponsor_equity_entry\", \"val\"]")
                .cell_num("y1", 0.0)
                .cell_num("y2", 0.0)
                .cell_num("y3", 0.0)
                .cell_num("y4", 0.0)
                .cell_formula("y5", "ReturnsSummary[\"exit_equity_proceeds\", \"val\"]")
                .build(),
        ],
    };

    // -------------------------------------------------------------------------
    // Project Manifest (Sheets: Summary and Model)
    // -------------------------------------------------------------------------
    let manifest = ProjectManifest {
        model_name: "ProjectTitanLBO".to_string(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "Summary".to_string(),
                tab_color: Some("#1B365D".to_string()),
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    TableId::from("Scenarios"),
                    TableId::from("DealAssumptions"),
                    TableId::from("SourcesAndUses"),
                    TableId::from("ReturnsSummary"),
                ],
            },
            SheetSpec {
                name: "Model".to_string(),
                tab_color: Some("#003366".to_string()),
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    TableId::from("Timeline"),
                    TableId::from("Operations"),
                    TableId::from("DebtSchedule"),
                    TableId::from("CashFlows"),
                ],
            },
        ],
    };

    let tables = vec![
        scenarios_table,
        deal_assumptions_table,
        sources_and_uses_table,
        returns_summary_table,
        timeline_table,
        operations_table,
        debt_schedule_table,
        cash_flows_table,
    ];

    (manifest, tables)
}
