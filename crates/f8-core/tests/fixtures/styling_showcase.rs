//! Comprehensive styling and formatting showcase fixture.
//!
//! Exercises every visual styling option, layout element, border configuration,
//! number format preset, currency denomination, and cell override for the
//! default Corporate Navy theme in a single inspectable workbook.

use std::path::PathBuf;

use f8_core::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use f8_core::model::id::TableId;
use f8_core::model::manifest::{FreezePanes, ProjectManifest, SheetSpec};
use f8_core::model::table::{
    CellInput, ColumnSpec, DetailedCellInput, RowSpec, SectionHeaderRowSpec, TableSpec,
    ValueLiteral,
};

use super::{
    RowBuilder, build_descriptor_col, build_gutter_col, build_gutter_row, build_measure_col_label,
};

/// Builds the complete styling and formatting showcase workbook model.
///
/// Contains two sheets:
/// 1. `Styling_Showcase`: Five stacked tables demonstrating accounting borders,
///    hierarchical section headers/outlines, number formats, currencies, and cell features.
/// 2. `Freeze_Panes_Demo`: A multi-month operational matrix demonstrating freeze panes
///    pinned at row 4, column C with custom tab accent color.
pub fn build_styling_showcase_model() -> (ProjectManifest, Vec<TableSpec>) {
    // =========================================================================
    // Table 1: Institutional Accounting Borders & Semantic Styles
    // =========================================================================
    let t1_cols = vec![
        build_descriptor_col("line_item", "Financial Metric / Line Item"),
        ColumnSpec::new("actual").with_headers(["Operating Performance", "2025A Actual"]),
        ColumnSpec::new("budget").with_headers(["Operating Performance", "2026B Budget"]),
        ColumnSpec::new("variance").with_headers(["Operating Performance", "Variance ($)"]),
        build_gutter_col(),
        ColumnSpec::new("division_total")
            .with_headers(["Consolidated Summary", "Division Total"])
            .with_style(Some(SemanticStyle::Subtotal))
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(0)),
        ColumnSpec::new("consolidated_total")
            .with_headers(["Consolidated Summary", "Consolidated Total"])
            .with_style(Some(SemanticStyle::Total))
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(0)),
    ];

    let t1_rows = vec![
        RowBuilder::line_item("product_sales", "Commercial Product Revenue")
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("actual", 1_250_000.0)
            .cell_num("budget", 1_400_000.0)
            .cell_formula(
                "variance",
                "Styles_And_Borders[\"product_sales\", \"budget\"] - Styles_And_Borders[\"product_sales\", \"actual\"]",
            )
            .cell_num("division_total", 1_400_000.0)
            .cell_formula(
                "consolidated_total",
                "Styles_And_Borders[\"product_sales\", \"actual\"] + Styles_And_Borders[\"product_sales\", \"division_total\"]",
            )
            .build(),
        RowBuilder::line_item("service_contracts", "Professional Services & Maintenance")
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("actual", 450_000.0)
            .cell_num("budget", 500_000.0)
            .cell_formula(
                "variance",
                "Styles_And_Borders[\"service_contracts\", \"budget\"] - Styles_And_Borders[\"service_contracts\", \"actual\"]",
            )
            .cell_num("division_total", 500_000.0)
            .cell_formula(
                "consolidated_total",
                "Styles_And_Borders[\"service_contracts\", \"actual\"] + Styles_And_Borders[\"service_contracts\", \"division_total\"]",
            )
            .build(),
        RowBuilder::line_item("subtotal_revenue", "Total Gross Revenue")
            .style(SemanticStyle::Subtotal)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_formula(
                "actual",
                "Styles_And_Borders[\"product_sales\", \"actual\"] + Styles_And_Borders[\"service_contracts\", \"actual\"]",
            )
            .cell_formula(
                "budget",
                "Styles_And_Borders[\"product_sales\", \"budget\"] + Styles_And_Borders[\"service_contracts\", \"budget\"]",
            )
            .cell_formula(
                "variance",
                "Styles_And_Borders[\"subtotal_revenue\", \"budget\"] - Styles_And_Borders[\"subtotal_revenue\", \"actual\"]",
            )
            .cell_formula(
                "division_total",
                "Styles_And_Borders[\"product_sales\", \"division_total\"] + Styles_And_Borders[\"service_contracts\", \"division_total\"]",
            )
            .cell_formula(
                "consolidated_total",
                "Styles_And_Borders[\"subtotal_revenue\", \"actual\"] + Styles_And_Borders[\"subtotal_revenue\", \"division_total\"]",
            )
            .build(),
        build_gutter_row(),
        RowBuilder::line_item("cogs_materials", "Direct Raw Materials")
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("actual", -680_000.0)
            .cell_num("budget", -720_000.0)
            .cell_formula(
                "variance",
                "Styles_And_Borders[\"cogs_materials\", \"budget\"] - Styles_And_Borders[\"cogs_materials\", \"actual\"]",
            )
            .cell_num("division_total", -720_000.0)
            .cell_formula(
                "consolidated_total",
                "Styles_And_Borders[\"cogs_materials\", \"actual\"] + Styles_And_Borders[\"cogs_materials\", \"division_total\"]",
            )
            .build(),
        RowBuilder::line_item("cogs_logistics", "Freight & Distribution Logistics")
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("actual", -120_000.0)
            .cell_num("budget", -130_000.0)
            .cell_formula(
                "variance",
                "Styles_And_Borders[\"cogs_logistics\", \"budget\"] - Styles_And_Borders[\"cogs_logistics\", \"actual\"]",
            )
            .cell_num("division_total", -130_000.0)
            .cell_formula(
                "consolidated_total",
                "Styles_And_Borders[\"cogs_logistics\", \"actual\"] + Styles_And_Borders[\"cogs_logistics\", \"division_total\"]",
            )
            .build(),
        RowBuilder::line_item("total_margin", "Gross Margin (4-Sided Grand Total Box)")
            .style(SemanticStyle::Total)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_formula(
                "actual",
                "Styles_And_Borders[\"subtotal_revenue\", \"actual\"] + Styles_And_Borders[\"cogs_materials\", \"actual\"] + Styles_And_Borders[\"cogs_logistics\", \"actual\"]",
            )
            .cell_formula(
                "budget",
                "Styles_And_Borders[\"subtotal_revenue\", \"budget\"] + Styles_And_Borders[\"cogs_materials\", \"budget\"] + Styles_And_Borders[\"cogs_logistics\", \"budget\"]",
            )
            .cell_formula(
                "variance",
                "Styles_And_Borders[\"total_margin\", \"budget\"] - Styles_And_Borders[\"total_margin\", \"actual\"]",
            )
            .cell_formula(
                "division_total",
                "Styles_And_Borders[\"subtotal_revenue\", \"division_total\"] + Styles_And_Borders[\"cogs_materials\", \"division_total\"] + Styles_And_Borders[\"cogs_logistics\", \"division_total\"]",
            )
            .cell_formula(
                "consolidated_total",
                "Styles_And_Borders[\"total_margin\", \"actual\"] + Styles_And_Borders[\"total_margin\", \"division_total\"]",
            )
            .build(),
        build_gutter_row(),
        RowBuilder::line_item("highlight_ebitda", "Operating EBITDA (Visual Highlight)")
            .highlight(true)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell(
                "actual",
                CellInput::Detailed(DetailedCellInput {
                    value: Some(ValueLiteral::Number(620_000.0)),
                    note: Some("Audited EBITDA benchmark with non-recurring add-backs.".to_string()),
                    ..Default::default()
                }),
            )
            .cell_num("budget", 710_000.0)
            .cell_formula(
                "variance",
                "Styles_And_Borders[\"highlight_ebitda\", \"budget\"] - Styles_And_Borders[\"highlight_ebitda\", \"actual\"]",
            )
            .cell_num("division_total", 710_000.0)
            .cell_formula(
                "consolidated_total",
                "Styles_And_Borders[\"highlight_ebitda\", \"actual\"] + Styles_And_Borders[\"highlight_ebitda\", \"division_total\"]",
            )
            .build(),
        RowBuilder::line_item("zero_line", "Extraordinary Restructuring Charges")
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("actual", 0.0)
            .cell_num("budget", 0.0)
            .cell_formula(
                "variance",
                "Styles_And_Borders[\"zero_line\", \"budget\"] - Styles_And_Borders[\"zero_line\", \"actual\"]",
            )
            .cell_num("division_total", 0.0)
            .cell_num("consolidated_total", 0.0)
            .build(),
    ];

    let table_borders = TableSpec {
        id: TableId::from("Styles_And_Borders"),
        title: "Institutional Accounting Borders & Semantic Styles".to_string(),
        subtitle: Some(
            "Demonstrating data, subtotal, and total styles with 1D and 2D accounting borders, visual highlights, and input vs formula color coding".to_string(),
        ),
        currency: Some(CurrencyCode::Usd),
        collapsible: false,
        columns: t1_cols,
        rows: t1_rows,
    };

    // =========================================================================
    // Table 2: Hierarchical Sections & Collapsible Outlines
    // =========================================================================
    let t2_cols = vec![
        build_descriptor_col("line_item", "Division / Department"),
        ColumnSpec::new("q1_act").with_headers([
            "Commercial Operations",
            "Quarterly Performance",
            "Q1 Actual",
        ]),
        ColumnSpec::new("q2_act").with_headers([
            "Commercial Operations",
            "Quarterly Performance",
            "Q2 Actual",
        ]),
        ColumnSpec::new("q3_act").with_headers([
            "Commercial Operations",
            "Quarterly Performance",
            "Q3 Actual",
        ]),
        ColumnSpec::new("q4_act").with_headers([
            "Commercial Operations",
            "Quarterly Performance",
            "Q4 Actual",
        ]),
        build_gutter_col(),
        ColumnSpec::new("fy_sum")
            .with_headers(["Consolidated Reporting", "Full Year Total"])
            .with_style(Some(SemanticStyle::Total))
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(0)),
    ];

    let t2_rows = vec![
        RowSpec::section_header("Commercial Operations"),
        SectionHeaderRowSpec::new("North America Division")
            .with_indent(Some(1))
            .into(),
        // North America Division - Tier 3 Deep Indentation Sub-Accounts
        RowBuilder::line_item("na_strategic", "Strategic Enterprise Accounts")
            .indent(3)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("q1_act", 100_000.0)
            .cell_num("q2_act", 105_000.0)
            .cell_num("q3_act", 115_000.0)
            .cell_num("q4_act", 125_000.0)
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"na_strategic\", \"q1_act\"] + Hierarchical_Outlines[\"na_strategic\", \"q2_act\"] + Hierarchical_Outlines[\"na_strategic\", \"q3_act\"] + Hierarchical_Outlines[\"na_strategic\", \"q4_act\"]",
            )
            .build(),
        RowBuilder::line_item("na_midmarket", "Commercial Mid-Market Accounts")
            .indent(3)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("q1_act", 50_000.0)
            .cell_num("q2_act", 55_000.0)
            .cell_num("q3_act", 60_000.0)
            .cell_num("q4_act", 65_000.0)
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"na_midmarket\", \"q1_act\"] + Hierarchical_Outlines[\"na_midmarket\", \"q2_act\"] + Hierarchical_Outlines[\"na_midmarket\", \"q3_act\"] + Hierarchical_Outlines[\"na_midmarket\", \"q4_act\"]",
            )
            .build(),
        RowBuilder::line_item("na_direct", "Direct Accounts Sub-Rollup")
            .indent(2)
            .style(SemanticStyle::Subtotal)
            .format(FormatPreset::Currency)
            .decimals(0)
            .expr("Hierarchical_Outlines[\"na_strategic\", @col] + Hierarchical_Outlines[\"na_midmarket\", @col]")
            .build(),
        RowBuilder::line_item("na_partners", "Solution Partner Network")
            .indent(2)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("q1_act", 50_000.0)
            .cell_num("q2_act", 55_000.0)
            .cell_num("q3_act", 60_000.0)
            .cell_num("q4_act", 65_000.0)
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"na_partners\", \"q1_act\"] + Hierarchical_Outlines[\"na_partners\", \"q2_act\"] + Hierarchical_Outlines[\"na_partners\", \"q3_act\"] + Hierarchical_Outlines[\"na_partners\", \"q4_act\"]",
            )
            .build(),
        RowBuilder::line_item("na_subtotal", "North America Subtotal")
            .indent(1)
            .style(SemanticStyle::Subtotal)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_formula(
                "q1_act",
                "Hierarchical_Outlines[\"na_direct\", \"q1_act\"] + Hierarchical_Outlines[\"na_partners\", \"q1_act\"]",
            )
            .cell_formula(
                "q2_act",
                "Hierarchical_Outlines[\"na_direct\", \"q2_act\"] + Hierarchical_Outlines[\"na_partners\", \"q2_act\"]",
            )
            .cell_formula(
                "q3_act",
                "Hierarchical_Outlines[\"na_direct\", \"q3_act\"] + Hierarchical_Outlines[\"na_partners\", \"q3_act\"]",
            )
            .cell_formula(
                "q4_act",
                "Hierarchical_Outlines[\"na_direct\", \"q4_act\"] + Hierarchical_Outlines[\"na_partners\", \"q4_act\"]",
            )
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"na_subtotal\", \"q1_act\"] + Hierarchical_Outlines[\"na_subtotal\", \"q2_act\"] + Hierarchical_Outlines[\"na_subtotal\", \"q3_act\"] + Hierarchical_Outlines[\"na_subtotal\", \"q4_act\"]",
            )
            .build(),
        build_gutter_row(),
        SectionHeaderRowSpec::new("International Operations").with_indent(Some(1)).into(),
        // International Operations
        RowBuilder::line_item("intl_emea", "EMEA Corporate Accounts")
            .indent(2)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("q1_act", 80_000.0)
            .cell_num("q2_act", 85_000.0)
            .cell_num("q3_act", 90_000.0)
            .cell_num("q4_act", 95_000.0)
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"intl_emea\", \"q1_act\"] + Hierarchical_Outlines[\"intl_emea\", \"q2_act\"] + Hierarchical_Outlines[\"intl_emea\", \"q3_act\"] + Hierarchical_Outlines[\"intl_emea\", \"q4_act\"]",
            )
            .build(),
        RowBuilder::line_item("intl_apac", "APAC Regional Growth")
            .indent(2)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("q1_act", 40_000.0)
            .cell_num("q2_act", 45_000.0)
            .cell_num("q3_act", 50_000.0)
            .cell_num("q4_act", 55_000.0)
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"intl_apac\", \"q1_act\"] + Hierarchical_Outlines[\"intl_apac\", \"q2_act\"] + Hierarchical_Outlines[\"intl_apac\", \"q3_act\"] + Hierarchical_Outlines[\"intl_apac\", \"q4_act\"]",
            )
            .build(),
        RowBuilder::line_item("intl_subtotal", "International Subtotal")
            .indent(1)
            .style(SemanticStyle::Subtotal)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_formula(
                "q1_act",
                "Hierarchical_Outlines[\"intl_emea\", \"q1_act\"] + Hierarchical_Outlines[\"intl_apac\", \"q1_act\"]",
            )
            .cell_formula(
                "q2_act",
                "Hierarchical_Outlines[\"intl_emea\", \"q2_act\"] + Hierarchical_Outlines[\"intl_apac\", \"q2_act\"]",
            )
            .cell_formula(
                "q3_act",
                "Hierarchical_Outlines[\"intl_emea\", \"q3_act\"] + Hierarchical_Outlines[\"intl_apac\", \"q3_act\"]",
            )
            .cell_formula(
                "q4_act",
                "Hierarchical_Outlines[\"intl_emea\", \"q4_act\"] + Hierarchical_Outlines[\"intl_apac\", \"q4_act\"]",
            )
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"intl_subtotal\", \"q1_act\"] + Hierarchical_Outlines[\"intl_subtotal\", \"q2_act\"] + Hierarchical_Outlines[\"intl_subtotal\", \"q3_act\"] + Hierarchical_Outlines[\"intl_subtotal\", \"q4_act\"]",
            )
            .build(),
        build_gutter_row(),
        RowBuilder::line_item("commercial_total", "Total Commercial Revenue")
            .style(SemanticStyle::Total)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_formula(
                "q1_act",
                "Hierarchical_Outlines[\"na_subtotal\", \"q1_act\"] + Hierarchical_Outlines[\"intl_subtotal\", \"q1_act\"]",
            )
            .cell_formula(
                "q2_act",
                "Hierarchical_Outlines[\"na_subtotal\", \"q2_act\"] + Hierarchical_Outlines[\"intl_subtotal\", \"q2_act\"]",
            )
            .cell_formula(
                "q3_act",
                "Hierarchical_Outlines[\"na_subtotal\", \"q3_act\"] + Hierarchical_Outlines[\"intl_subtotal\", \"q3_act\"]",
            )
            .cell_formula(
                "q4_act",
                "Hierarchical_Outlines[\"na_subtotal\", \"q4_act\"] + Hierarchical_Outlines[\"intl_subtotal\", \"q4_act\"]",
            )
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"commercial_total\", \"q1_act\"] + Hierarchical_Outlines[\"commercial_total\", \"q2_act\"] + Hierarchical_Outlines[\"commercial_total\", \"q3_act\"] + Hierarchical_Outlines[\"commercial_total\", \"q4_act\"]",
            )
            .build(),
        build_gutter_row(),
        RowSpec::section_header("Operating Expenses & Overhead"),
        // Operating Expenses
        RowBuilder::line_item("opex_rnd", "Product Engineering & R&D")
            .indent(1)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("q1_act", 45_000.0)
            .cell_num("q2_act", 48_000.0)
            .cell_num("q3_act", 50_000.0)
            .cell_num("q4_act", 52_000.0)
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"opex_rnd\", \"q1_act\"] + Hierarchical_Outlines[\"opex_rnd\", \"q2_act\"] + Hierarchical_Outlines[\"opex_rnd\", \"q3_act\"] + Hierarchical_Outlines[\"opex_rnd\", \"q4_act\"]",
            )
            .build(),
        RowBuilder::line_item("opex_sales", "Sales Operations & Marketing")
            .indent(1)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("q1_act", 30_000.0)
            .cell_num("q2_act", 32_000.0)
            .cell_num("q3_act", 35_000.0)
            .cell_num("q4_act", 38_000.0)
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"opex_sales\", \"q1_act\"] + Hierarchical_Outlines[\"opex_sales\", \"q2_act\"] + Hierarchical_Outlines[\"opex_sales\", \"q3_act\"] + Hierarchical_Outlines[\"opex_sales\", \"q4_act\"]",
            )
            .build(),
        RowBuilder::line_item("opex_admin", "General Administrative & Legal")
            .indent(1)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("q1_act", 15_000.0)
            .cell_num("q2_act", 15_500.0)
            .cell_num("q3_act", 16_000.0)
            .cell_num("q4_act", 16_500.0)
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"opex_admin\", \"q1_act\"] + Hierarchical_Outlines[\"opex_admin\", \"q2_act\"] + Hierarchical_Outlines[\"opex_admin\", \"q3_act\"] + Hierarchical_Outlines[\"opex_admin\", \"q4_act\"]",
            )
            .build(),
        build_gutter_row(),
        RowBuilder::line_item("opex_total", "Total Operating Expenses")
            .style(SemanticStyle::Total)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_formula(
                "q1_act",
                "Hierarchical_Outlines[\"opex_rnd\", \"q1_act\"] + Hierarchical_Outlines[\"opex_sales\", \"q1_act\"] + Hierarchical_Outlines[\"opex_admin\", \"q1_act\"]",
            )
            .cell_formula(
                "q2_act",
                "Hierarchical_Outlines[\"opex_rnd\", \"q2_act\"] + Hierarchical_Outlines[\"opex_sales\", \"q2_act\"] + Hierarchical_Outlines[\"opex_admin\", \"q2_act\"]",
            )
            .cell_formula(
                "q3_act",
                "Hierarchical_Outlines[\"opex_rnd\", \"q3_act\"] + Hierarchical_Outlines[\"opex_sales\", \"q3_act\"] + Hierarchical_Outlines[\"opex_admin\", \"q3_act\"]",
            )
            .cell_formula(
                "q4_act",
                "Hierarchical_Outlines[\"opex_rnd\", \"q4_act\"] + Hierarchical_Outlines[\"opex_sales\", \"q4_act\"] + Hierarchical_Outlines[\"opex_admin\", \"q4_act\"]",
            )
            .cell_formula(
                "fy_sum",
                "Hierarchical_Outlines[\"opex_total\", \"q1_act\"] + Hierarchical_Outlines[\"opex_total\", \"q2_act\"] + Hierarchical_Outlines[\"opex_total\", \"q3_act\"] + Hierarchical_Outlines[\"opex_total\", \"q4_act\"]",
            )
            .build(),
    ];

    let table_hierarchical = TableSpec {
        id: TableId::from("Hierarchical_Outlines"),
        title: "Hierarchical Sections & Collapsible Outline Grouping".to_string(),
        subtitle: Some(
            "Demonstrating nested SectionHeaderRow tiers with native Excel outline grouping levels 1 and 2".to_string(),
        ),
        currency: Some(CurrencyCode::Usd),
        collapsible: false,
        columns: t2_cols,
        rows: t2_rows,
    };

    // =========================================================================
    // Table 3: Number Format Presets & Precision Matrix
    // =========================================================================
    let t3_cols = vec![
        build_descriptor_col("line_item", "Format Preset & Precision Tier"),
        build_measure_col_label("pos_val", "Positive Value"),
        build_measure_col_label("neg_val", "Negative (Parens)"),
        build_measure_col_label("zero_val", "Zero Balance (Dash)"),
        build_measure_col_label("formula_val", "Formula Calculation"),
    ];

    let t3_rows = vec![
        RowBuilder::line_item("curr_0dec", "Currency (USD, 0 Decimals - Default)")
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("pos_val", 1_250_000.0)
            .cell_num("neg_val", -1_250_000.0)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"curr_0dec\", \"pos_val\"] + Format_Presets_And_Decimals[\"curr_0dec\", \"neg_val\"]",
            )
            .build(),
        RowBuilder::line_item("curr_2dec", "Currency (USD, 2 Decimals)")
            .format(FormatPreset::Currency)
            .decimals(2)
            .cell_num("pos_val", 1_250_000.50)
            .cell_num("neg_val", -450_250.75)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"curr_2dec\", \"pos_val\"] * 0.10",
            )
            .build(),
        RowBuilder::line_item("num_0dec", "Number (0 Decimals - Default)")
            .format(FormatPreset::Number)
            .decimals(0)
            .cell_num("pos_val", 75_000.0)
            .cell_num("neg_val", -25_000.0)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"num_0dec\", \"pos_val\"] + Format_Presets_And_Decimals[\"num_0dec\", \"neg_val\"]",
            )
            .build(),
        RowBuilder::line_item("num_1dec", "Number (1 Decimal)")
            .format(FormatPreset::Number)
            .decimals(1)
            .cell_num("pos_val", 12_345.6)
            .cell_num("neg_val", -7_890.4)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"num_1dec\", \"pos_val\"] - Format_Presets_And_Decimals[\"num_1dec\", \"neg_val\"]",
            )
            .build(),
        RowBuilder::line_item("num_2dec", "Number (2 Decimals)")
            .format(FormatPreset::Number)
            .decimals(2)
            .cell_num("pos_val", 89_123.45)
            .cell_num("neg_val", -12_345.67)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"num_2dec\", \"pos_val\"] + Format_Presets_And_Decimals[\"num_2dec\", \"neg_val\"]",
            )
            .build(),
        RowBuilder::line_item("num_4dec", "Number (4 Decimals)")
            .format(FormatPreset::Number)
            .decimals(4)
            .cell_num("pos_val", 0.1234)
            .cell_num("neg_val", -0.0567)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"num_4dec\", \"pos_val\"] / 2.0",
            )
            .build(),
        RowBuilder::line_item("pct_0dec", "Percentage (0 Decimals)")
            .format(FormatPreset::Percentage)
            .decimals(0)
            .cell_num("pos_val", 0.25)
            .cell_num("neg_val", -0.10)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"pct_0dec\", \"pos_val\"] + Format_Presets_And_Decimals[\"pct_0dec\", \"neg_val\"]",
            )
            .build(),
        RowBuilder::line_item("pct_1dec", "Percentage (1 Decimal - Default)")
            .format(FormatPreset::Percentage)
            .decimals(1)
            .cell_num("pos_val", 0.175)
            .cell_num("neg_val", -0.042)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"pct_1dec\", \"pos_val\"] * 2.0",
            )
            .build(),
        RowBuilder::line_item("pct_2dec", "Percentage (2 Decimals)")
            .format(FormatPreset::Percentage)
            .decimals(2)
            .cell_num("pos_val", 0.0825)
            .cell_num("neg_val", -0.0175)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"pct_2dec\", \"pos_val\"] - Format_Presets_And_Decimals[\"pct_2dec\", \"neg_val\"]",
            )
            .build(),
        RowBuilder::line_item("mult_1dec", "Multiple (1 Decimal - Default)")
            .format(FormatPreset::Multiple)
            .decimals(1)
            .cell_num("pos_val", 8.5)
            .cell_num("neg_val", -1.2)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"mult_1dec\", \"pos_val\"] * 1.5",
            )
            .build(),
        RowBuilder::line_item("mult_2dec", "Multiple (2 Decimals)")
            .format(FormatPreset::Multiple)
            .decimals(2)
            .cell_num("pos_val", 14.25)
            .cell_num("neg_val", -2.75)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"mult_2dec\", \"pos_val\"] - Format_Presets_And_Decimals[\"mult_2dec\", \"neg_val\"]",
            )
            .build(),
        RowBuilder::line_item("bps_0dec", "Basis Points (0 Decimals - Default)")
            .format(FormatPreset::BasisPoints)
            .decimals(0)
            .cell_num("pos_val", 250.0)
            .cell_num("neg_val", -75.0)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"bps_0dec\", \"pos_val\"] + Format_Presets_And_Decimals[\"bps_0dec\", \"neg_val\"]",
            )
            .build(),
        RowBuilder::line_item("bps_1dec", "Basis Points (1 Decimal)")
            .format(FormatPreset::BasisPoints)
            .decimals(1)
            .cell_num("pos_val", 187.5)
            .cell_num("neg_val", -32.5)
            .cell_num("zero_val", 0.0)
            .cell_formula(
                "formula_val",
                "Format_Presets_And_Decimals[\"bps_1dec\", \"pos_val\"] * 0.5",
            )
            .build(),
        RowBuilder::line_item("date_std", "Date (ISO 8601 Calendar)")
            .format(FormatPreset::Date)
            .cell_str("pos_val", "2026-12-31")
            .cell_str("neg_val", "2026-06-30")
            .cell_str("zero_val", "2026-01-01")
            .cell_str("formula_val", "2026-09-30")
            .build(),
        RowBuilder::line_item("text_std", "Text (Literal String & Annotations)")
            .format(FormatPreset::Text)
            .cell_str("pos_val", "Investment Grade (AAA)")
            .cell_str("neg_val", "Speculative Grade (BB)")
            .cell_str("zero_val", "Unrated")
            .cell_str("formula_val", "Audited & Reconciled")
            .build(),
    ];

    let table_formats = TableSpec {
        id: TableId::from("Format_Presets_And_Decimals"),
        title: "Number Format Presets & Precision Matrix".to_string(),
        subtitle: Some(
            "Demonstrating all 7 format presets across positive, negative (parentheses), zero (dash), and decimal precisions".to_string(),
        ),
        currency: None,
        collapsible: false,
        columns: t3_cols,
        rows: t3_rows,
    };

    // =========================================================================
    // Table 4: Multi-Currency Global Denominations
    // =========================================================================
    let t4_cols = vec![
        build_descriptor_col("line_item", "Currency Code & Regional Entity"),
        build_descriptor_col("symbol_type", "Symbol Code & Quoting"),
        build_gutter_col(),
        build_measure_col_label("pos_val", "Positive Balance"),
        build_measure_col_label("neg_val", "Deficit Balance (Parens)"),
        build_measure_col_label("zero_val", "Zero Balance (Dash)"),
    ];

    let currencies = [
        (
            "curr_usd",
            "USD - US Dollar (United States)",
            "$ (ASCII)",
            CurrencyCode::Usd,
            1_000_000.0,
        ),
        (
            "curr_eur",
            "EUR - Euro (Eurozone)",
            "€ (Unicode)",
            CurrencyCode::Eur,
            1_000_000.0,
        ),
        (
            "curr_gbp",
            "GBP - British Pound (United Kingdom)",
            "£ (Unicode)",
            CurrencyCode::Gbp,
            1_000_000.0,
        ),
        (
            "curr_jpy",
            "JPY - Japanese Yen (Japan)",
            "¥ (Unicode)",
            CurrencyCode::Jpy,
            100_000_000.0,
        ),
        (
            "curr_chf",
            "CHF - Swiss Franc (Switzerland)",
            "CHF (Quoted Alpha-3)",
            CurrencyCode::Chf,
            1_000_000.0,
        ),
        (
            "curr_cad",
            "CAD - Canadian Dollar (Canada)",
            "C$ (Quoted Prefix)",
            CurrencyCode::Cad,
            1_000_000.0,
        ),
        (
            "curr_aud",
            "AUD - Australian Dollar (Australia)",
            "A$ (Quoted Prefix)",
            CurrencyCode::Aud,
            1_000_000.0,
        ),
        (
            "curr_cny",
            "CNY - Chinese Yuan (China)",
            "¥ (Unicode)",
            CurrencyCode::Cny,
            10_000_000.0,
        ),
        (
            "curr_hkd",
            "HKD - Hong Kong Dollar (Hong Kong)",
            "HK$ (Quoted Prefix)",
            CurrencyCode::Hkd,
            10_000_000.0,
        ),
        (
            "curr_sgd",
            "SGD - Singapore Dollar (Singapore)",
            "S$ (Quoted Prefix)",
            CurrencyCode::Sgd,
            1_000_000.0,
        ),
        (
            "curr_sek",
            "SEK - Swedish Krona (Sweden)",
            "kr (Quoted Suffix)",
            CurrencyCode::Sek,
            10_000_000.0,
        ),
        (
            "curr_nok",
            "NOK - Norwegian Krone (Norway)",
            "kr (Quoted Suffix)",
            CurrencyCode::Nok,
            10_000_000.0,
        ),
        (
            "curr_nzd",
            "NZD - New Zealand Dollar (New Zealand)",
            "NZ$ (Quoted Prefix)",
            CurrencyCode::Nzd,
            1_000_000.0,
        ),
        (
            "curr_inr",
            "INR - Indian Rupee (India)",
            "₹ (Unicode)",
            CurrencyCode::Inr,
            10_000_000.0,
        ),
        (
            "curr_brl",
            "BRL - Brazilian Real (Brazil)",
            "R$ (Quoted Prefix)",
            CurrencyCode::Brl,
            5_000_000.0,
        ),
    ];

    let mut t4_rows = Vec::new();
    for (idx, (id, label, sym_type, code, amount)) in currencies.into_iter().enumerate() {
        if idx == 4 {
            t4_rows.push(build_gutter_row());
        }
        let mut row = RowBuilder::line_item(id, label);
        row = row.format(FormatPreset::Currency).decimals(0);
        row = row.cell_str("symbol_type", sym_type);

        // Custom currency per row
        row = row.currency(code);
        row = row.cell_num("pos_val", amount);
        row = row.cell_num("neg_val", -amount);
        row = row.cell_num("zero_val", 0.0);
        t4_rows.push(row.build());
    }

    let table_currencies = TableSpec {
        id: TableId::from("Currency_Denominations"),
        title: "Multi-Currency Global Denominations".to_string(),
        subtitle: Some(
            "Demonstrating all 15 supported ISO 4217 currency codes with syntax-safe escaping, negative parentheses, and zero dashes".to_string(),
        ),
        currency: None,
        collapsible: false,
        columns: t4_cols,
        rows: t4_rows,
    };

    // =========================================================================
    // Table 5: Cell Payloads, Typographic Styles & Audit Annotations
    // =========================================================================
    let t5_cols = vec![
        build_descriptor_col("line_item", "Feature / Presentation Rule"),
        build_measure_col_label("sample_val", "Sample Value"),
        build_descriptor_col("eval_type", "AST Payload Type"),
        build_descriptor_col("style_rule", "Applied Color & Format Rule"),
        build_descriptor_col("notes", "Audit Note / Description"),
    ];

    let t5_rows = vec![
        RowBuilder::line_item("input_blue", "Hardcoded Assumption Input")
            .cell(
                "sample_val",
                CellInput::Detailed(DetailedCellInput {
                    value: Some(ValueLiteral::Number(500_000.0)),
                    format: Some(FormatPreset::Currency),
                    decimals: Some(0),
                    currency: Some(CurrencyCode::Usd),
                    ..Default::default()
                }),
            )
            .cell_str("eval_type", "f64 Numeric Literal")
            .cell_str("style_rule", "Font Color: Input Blue (#0000FF)")
            .cell_str("notes", "Direct numeric input entered by financial modeler")
            .build(),
        RowBuilder::line_item("formula_black", "Calculated Formula Result")
            .cell(
                "sample_val",
                CellInput::Detailed(DetailedCellInput {
                    expr: Some("Cell_Features_And_Notes[\"input_blue\", \"sample_val\"] * 1.20".to_string()),
                    format: Some(FormatPreset::Currency),
                    decimals: Some(0),
                    currency: Some(CurrencyCode::Usd),
                    ..Default::default()
                }),
            )
            .cell_str("eval_type", "Live Formula AST Expression")
            .cell_str("style_rule", "Font Color: Formula Black (#000000)")
            .cell_str("notes", "Evaluated formula lowered into native Excel math")
            .build(),
        RowBuilder::line_item("highlight_item", "Visual Highlight Emphasis")
            .cell(
                "sample_val",
                CellInput::Detailed(DetailedCellInput {
                    value: Some(ValueLiteral::Number(0.425)),
                    highlight: Some(true),
                    format: Some(FormatPreset::Percentage),
                    decimals: Some(1),
                    ..Default::default()
                }),
            )
            .cell_str("eval_type", "f64 Percentage Literal")
            .cell_str("style_rule", "Fill: Yellow (#FEF3C7), Font: Amber (#92400E)")
            .cell_str("notes", "Critical benchmark metric exceeding target threshold")
            .build(),
        RowBuilder::line_item("commented_item", "Cell With Audit Note Comment")
            .cell(
                "sample_val",
                CellInput::Detailed(DetailedCellInput {
                    value: Some(ValueLiteral::Number(10_000_000.0)),
                    format: Some(FormatPreset::Currency),
                    decimals: Some(0),
                    currency: Some(CurrencyCode::Usd),
                    note: Some("Institutional Memo: Sign-off approved by Investment Committee on 2026-09-15.".to_string()),
                    ..Default::default()
                }),
            )
            .cell_str("eval_type", "f64 Currency with Comment")
            .cell_str("style_rule", "Data Format + Excel Red Comment Corner Indicator")
            .cell_str("notes", "Hover cursor over cell in Excel to view native comment box")
            .build(),
        RowBuilder::line_item("text_string", "String Text Literal")
            .cell_str("sample_val", "Active Status")
            .cell_str("eval_type", "String Text Constant")
            .cell_str("style_rule", "Format Preset: Text (@), Descriptor Gray Font")
            .cell_str("notes", "Categorical classification string payload")
            .build(),
        RowBuilder::line_item("bool_true", "Boolean Literal True")
            .cell("sample_val", CellInput::boolean(true))
            .cell_str("eval_type", "Boolean Constant")
            .cell_str("style_rule", "Type: Boolean True")
            .cell_str("notes", "Debt covenant compliance check passed")
            .build(),
        RowBuilder::line_item("bool_false", "Boolean Literal False")
            .cell("sample_val", CellInput::boolean(false))
            .cell_str("eval_type", "Boolean Constant")
            .cell_str("style_rule", "Type: Boolean False")
            .cell_str("notes", "Material adverse change condition trigger inactive")
            .build(),
        RowBuilder::line_item("null_empty", "Null / Blank Cell")
            .cell("sample_val", CellInput::null())
            .cell_str("eval_type", "Null Literal")
            .cell_str("style_rule", "Type: Empty / Null Cell")
            .cell_str("notes", "Optional field with no assigned value rendered clean")
            .build(),
    ];

    let table_features = TableSpec {
        id: TableId::from("Cell_Features_And_Notes"),
        title: "Cell Payloads, Typographic Styles & Audit Annotations".to_string(),
        subtitle: Some(
            "Demonstrating input vs formula font coloring, literal types, cell comments, and style overrides".to_string(),
        ),
        currency: None,
        collapsible: false,
        columns: t5_cols,
        rows: t5_rows,
    };

    // =========================================================================
    // Table 6: Freeze Panes & Operational Matrix (Sheet 2)
    // =========================================================================
    let t6_cols = vec![
        build_descriptor_col("line_item", "Operating Metric / Month"),
        build_measure_col_label("m01", "Month 01"),
        build_measure_col_label("m02", "Month 02"),
        build_measure_col_label("m03", "Month 03"),
        build_measure_col_label("m04", "Month 04"),
        build_measure_col_label("m05", "Month 05"),
        build_measure_col_label("m06", "Month 06"),
        ColumnSpec::new("h1_total")
            .with_header("H1 Total")
            .with_style(Some(SemanticStyle::Total)),
    ];

    let t6_rows = vec![
        RowBuilder::line_item("subscribers", "Active Subscriber Base (k)")
            .format(FormatPreset::Number)
            .decimals(1)
            .cell_num("m01", 120.5)
            .cell_num("m02", 124.0)
            .cell_num("m03", 128.2)
            .cell_num("m04", 132.8)
            .cell_num("m05", 137.5)
            .cell_num("m06", 142.0)
            .cell_formula(
                "h1_total",
                "Freeze_Panes_Sample[\"subscribers\", \"m06\"]",
            )
            .build(),
        RowBuilder::line_item("arpu", "Average Revenue Per User ($)")
            .format(FormatPreset::Currency)
            .decimals(2)
            .cell_num("m01", 45.00)
            .cell_num("m02", 45.25)
            .cell_num("m03", 45.50)
            .cell_num("m04", 45.75)
            .cell_num("m05", 46.00)
            .cell_num("m06", 46.50)
            .cell_formula(
                "h1_total",
                "Freeze_Panes_Sample[\"arpu\", \"m06\"]",
            )
            .build(),
        RowBuilder::line_item("monthly_arr", "Monthly Recurring Revenue ($k)")
            .style(SemanticStyle::Subtotal)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_formula("m01", "Freeze_Panes_Sample[\"subscribers\", \"m01\"] * Freeze_Panes_Sample[\"arpu\", \"m01\"]")
            .cell_formula("m02", "Freeze_Panes_Sample[\"subscribers\", \"m02\"] * Freeze_Panes_Sample[\"arpu\", \"m02\"]")
            .cell_formula("m03", "Freeze_Panes_Sample[\"subscribers\", \"m03\"] * Freeze_Panes_Sample[\"arpu\", \"m03\"]")
            .cell_formula("m04", "Freeze_Panes_Sample[\"subscribers\", \"m04\"] * Freeze_Panes_Sample[\"arpu\", \"m04\"]")
            .cell_formula("m05", "Freeze_Panes_Sample[\"subscribers\", \"m05\"] * Freeze_Panes_Sample[\"arpu\", \"m05\"]")
            .cell_formula("m06", "Freeze_Panes_Sample[\"subscribers\", \"m06\"] * Freeze_Panes_Sample[\"arpu\", \"m06\"]")
            .cell_formula(
                "h1_total",
                "Freeze_Panes_Sample[\"monthly_arr\", \"m01\"] + Freeze_Panes_Sample[\"monthly_arr\", \"m02\"] + Freeze_Panes_Sample[\"monthly_arr\", \"m03\"] + Freeze_Panes_Sample[\"monthly_arr\", \"m04\"] + Freeze_Panes_Sample[\"monthly_arr\", \"m05\"] + Freeze_Panes_Sample[\"monthly_arr\", \"m06\"]",
            )
            .build(),
        RowBuilder::line_item("churn_pct", "Monthly Churn Rate (%)")
            .format(FormatPreset::Percentage)
            .decimals(2)
            .cell_num("m01", 0.0125)
            .cell_num("m02", 0.0120)
            .cell_num("m03", 0.0118)
            .cell_num("m04", 0.0115)
            .cell_num("m05", 0.0110)
            .cell_num("m06", 0.0105)
            .cell_formula("h1_total", "Freeze_Panes_Sample[\"churn_pct\", \"m06\"]")
            .build(),
        RowBuilder::line_item("gross_burn", "Operational Cash Burn ($k)")
            .style(SemanticStyle::Total)
            .format(FormatPreset::Currency)
            .decimals(0)
            .cell_num("m01", -2100.0)
            .cell_num("m02", -2050.0)
            .cell_num("m03", -1980.0)
            .cell_num("m04", -1920.0)
            .cell_num("m05", -1850.0)
            .cell_num("m06", -1790.0)
            .cell_formula(
                "h1_total",
                "Freeze_Panes_Sample[\"gross_burn\", \"m01\"] + Freeze_Panes_Sample[\"gross_burn\", \"m02\"] + Freeze_Panes_Sample[\"gross_burn\", \"m03\"] + Freeze_Panes_Sample[\"gross_burn\", \"m04\"] + Freeze_Panes_Sample[\"gross_burn\", \"m05\"] + Freeze_Panes_Sample[\"gross_burn\", \"m06\"]",
            )
            .build(),
    ];

    let table_freeze = TableSpec {
        id: TableId::from("Freeze_Panes_Sample"),
        title: "Freeze Panes & Operational Matrix".to_string(),
        subtitle: Some(
            "Freeze pane locked at Row 4, Column C (col index 2), with tab accent color #2C4D75"
                .to_string(),
        ),
        currency: Some(CurrencyCode::Usd),
        collapsible: false,
        columns: t6_cols,
        rows: t6_rows,
    };

    // =========================================================================
    // Manifest Definition
    // =========================================================================
    let manifest = ProjectManifest {
        model_name: "f8_Styling_Showcase".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![
            SheetSpec {
                name: "Styling_Showcase".to_string(),
                tab_color: Some("#1B365D".to_string()),
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    PathBuf::from("tables/Styles_And_Borders.json"),
                    PathBuf::from("tables/Hierarchical_Outlines.json"),
                    PathBuf::from("tables/Format_Presets_And_Decimals.json"),
                    PathBuf::from("tables/Currency_Denominations.json"),
                    PathBuf::from("tables/Cell_Features_And_Notes.json"),
                ],
            },
            SheetSpec {
                name: "Freeze_Panes_Demo".to_string(),
                tab_color: Some("#2C4D75".to_string()),
                freeze_panes: Some(FreezePanes { row: 4, col: 2 }),
                show_gridlines: true,
                tables: vec![PathBuf::from("tables/Freeze_Panes_Sample.json")],
            },
        ],
    };

    let tables = vec![
        table_borders,
        table_hierarchical,
        table_formats,
        table_currencies,
        table_features,
        table_freeze,
    ];

    (manifest, tables)
}
