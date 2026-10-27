//! Institutional Public Trading Comparables ("Trading Comps") model fixture.
//!
//! Models peer SaaS companies with multi-tier asymmetric column headers,
//! intra-table gutters, vertical calculation broadcasting with `@row` coordinates,
//! vertical range summary benchmark aggregations (`MEDIAN`, `AVERAGE`, `MIN`, `MAX`),
//! and a secondary stacked target valuation benchmarking table.

use indexmap::IndexMap;

use f8_core::model::format::{FormatPreset, SemanticStyle};
use f8_core::model::id::{ColId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec};

use super::{
    RowBuilder, build_descriptor_col, build_gutter_col, build_gutter_row, build_measure_col_label,
};

/// Measure columns present in the Peer Trading Comps table.
const COMPS_MEASURE_COLS: &[&str] = &[
    "share_price",
    "shares_out",
    "market_cap",
    "cash",
    "debt",
    "ev",
    "revenue",
    "ebitda",
    "ebitda_margin",
    "ev_rev",
    "ev_ebitda",
    "pe",
];

/// Data payload defining a peer company in the `Comps` table.
#[derive(Debug, Clone, Copy)]
struct PeerData<'a> {
    id: &'a str,
    company: &'a str,
    ticker: &'a str,
    share_price: f64,
    shares_out: f64,
    cash: f64,
    debt: f64,
    revenue: f64,
    ebitda: f64,
}

/// Helper constructing a peer entity row in the `Comps` table.
fn make_peer_row(peer: PeerData<'_>) -> RowSpec {
    let mut cells = IndexMap::new();
    cells.insert(ColId::from("company"), CellInput::string(peer.company));
    cells.insert(ColId::from("ticker"), CellInput::string(peer.ticker));
    cells.insert(
        ColId::from("share_price"),
        CellInput::number(peer.share_price),
    );
    cells.insert(
        ColId::from("shares_out"),
        CellInput::number(peer.shares_out),
    );
    cells.insert(ColId::from("cash"), CellInput::number(peer.cash));
    cells.insert(ColId::from("debt"), CellInput::number(peer.debt));
    cells.insert(ColId::from("revenue"), CellInput::number(peer.revenue));
    cells.insert(ColId::from("ebitda"), CellInput::number(peer.ebitda));

    RowSpec::new(peer.id).with_cells(cells)
}

/// Helper constructing a summary benchmark row aggregating vertical slices across peers.
fn make_summary_row(id: &str, label: &str, func: &str, style: SemanticStyle) -> RowSpec {
    let mut cells = IndexMap::new();
    cells.insert(ColId::from("company"), CellInput::string(label));
    cells.insert(ColId::from("ticker"), CellInput::string("—"));
    for &col in COMPS_MEASURE_COLS {
        let formula = format!("{func}(Comps[\"snow\"..\"estc\", @col])");
        cells.insert(ColId::from(col), CellInput::formula(formula));
    }
    RowSpec::new(id).with_style(Some(style)).with_cells(cells)
}

/// Constructs the complete 2-table Public Trading Comparables model.
#[must_use]
pub fn build_trading_comps_model() -> (ProjectManifest, Vec<TableSpec>) {
    // =========================================================================
    // Table 1: Peer Trading Comps Table (`Comps`)
    // =========================================================================
    let comps_cols = vec![
        build_descriptor_col("company", "Company"),
        build_descriptor_col("ticker", "Ticker"),
        build_gutter_col(),
        // Market Data (3 cols)
        ColumnSpec::new("share_price")
            .with_group("Market Data")
            .with_header("Share Price")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(2)),
        ColumnSpec::new("shares_out")
            .with_group("Market Data")
            .with_header("Shares Out")
            .with_format(Some(FormatPreset::Number))
            .with_decimals(Some(1)),
        ColumnSpec::new("market_cap")
            .with_group("Market Data")
            .with_header("Market Cap")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1))
            .with_expr("Comps[@row, \"share_price\"] * Comps[@row, \"shares_out\"]"),
        build_gutter_col(),
        // Balance Sheet (3 cols)
        ColumnSpec::new("cash")
            .with_group("Balance Sheet")
            .with_header("Cash")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
        ColumnSpec::new("debt")
            .with_group("Balance Sheet")
            .with_header("Total Debt")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
        ColumnSpec::new("ev")
            .with_group("Balance Sheet")
            .with_header("Enterprise Value")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1))
            .with_expr(
                "Comps[@row, \"market_cap\"] + Comps[@row, \"debt\"] - Comps[@row, \"cash\"]",
            ),
        build_gutter_col(),
        // Operating Metrics (3 cols)
        ColumnSpec::new("revenue")
            .with_group("Operating Metrics")
            .with_header("LTM Revenue")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
        ColumnSpec::new("ebitda")
            .with_group("Operating Metrics")
            .with_header("LTM EBITDA")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
        ColumnSpec::new("ebitda_margin")
            .with_group("Operating Metrics")
            .with_header("EBITDA Margin")
            .with_format(Some(FormatPreset::Percentage))
            .with_decimals(Some(1))
            .with_expr("Comps[@row, \"ebitda\"] / Comps[@row, \"revenue\"]"),
        build_gutter_col(),
        // Valuation Multiples (3 cols)
        ColumnSpec::new("ev_rev")
            .with_group("Valuation Multiples")
            .with_header("EV / Rev")
            .with_format(Some(FormatPreset::Multiple))
            .with_decimals(Some(1))
            .with_expr("Comps[@row, \"ev\"] / Comps[@row, \"revenue\"]"),
        ColumnSpec::new("ev_ebitda")
            .with_group("Valuation Multiples")
            .with_header("EV / EBITDA")
            .with_format(Some(FormatPreset::Multiple))
            .with_decimals(Some(1))
            .with_expr("Comps[@row, \"ev\"] / Comps[@row, \"ebitda\"]"),
        ColumnSpec::new("pe")
            .with_group("Valuation Multiples")
            .with_header("P / E")
            .with_format(Some(FormatPreset::Multiple))
            .with_decimals(Some(1))
            .with_expr("Comps[@row, \"market_cap\"] / (Comps[@row, \"ebitda\"] * 0.75)"),
    ];

    let comps_rows = vec![
        make_peer_row(PeerData {
            id: "snow",
            company: "Snowflake Inc.",
            ticker: "SNOW",
            share_price: 150.00,
            shares_out: 330.0,
            cash: 3_500.0,
            debt: 0.0,
            revenue: 3_200.0,
            ebitda: 350.0,
        }),
        make_peer_row(PeerData {
            id: "ddog",
            company: "Datadog Inc.",
            ticker: "DDOG",
            share_price: 120.00,
            shares_out: 325.0,
            cash: 2_800.0,
            debt: 0.0,
            revenue: 2_600.0,
            ebitda: 600.0,
        }),
        make_peer_row(PeerData {
            id: "mdb",
            company: "MongoDB Inc.",
            ticker: "MDB",
            share_price: 280.00,
            shares_out: 75.0,
            cash: 1_900.0,
            debt: 0.0,
            revenue: 1_850.0,
            ebitda: 280.0,
        }),
        make_peer_row(PeerData {
            id: "net",
            company: "Cloudflare Inc.",
            ticker: "NET",
            share_price: 85.00,
            shares_out: 340.0,
            cash: 1_400.0,
            debt: 1_200.0,
            revenue: 1_650.0,
            ebitda: 210.0,
        }),
        make_peer_row(PeerData {
            id: "crwd",
            company: "CrowdStrike Holdings",
            ticker: "CRWD",
            share_price: 310.00,
            shares_out: 245.0,
            cash: 3_700.0,
            debt: 750.0,
            revenue: 3_450.0,
            ebitda: 900.0,
        }),
        make_peer_row(PeerData {
            id: "estc",
            company: "Elastic N.V.",
            ticker: "ESTC",
            share_price: 105.00,
            shares_out: 100.0,
            cash: 1_000.0,
            debt: 570.0,
            revenue: 1_300.0,
            ebitda: 195.0,
        }),
        build_gutter_row(),
        make_summary_row("mean", "Mean", "AVERAGE", SemanticStyle::Subtotal),
        make_summary_row("median", "Median", "MEDIAN", SemanticStyle::Subtotal),
        make_summary_row("max", "High", "MAX", SemanticStyle::Data),
        make_summary_row("min", "Low", "MIN", SemanticStyle::Data),
    ];

    let comps_table = TableSpec {
        id: TableId::from("Comps"),
        title: "Public Trading Comparables (SaaS Peers)".to_string(),
        subtitle: Some("Enterprise Value and Operational Trading Multiples ($ in Millions, except share price)".to_string()),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: comps_cols,
        rows: comps_rows,
    };

    // =========================================================================
    // Table 2: Target Valuation Benchmarking Table (`TargetValuation`)
    // =========================================================================
    let target_val_cols = vec![
        build_descriptor_col("metric", "Valuation Metric"),
        build_measure_col_label("val", "Value"),
    ];

    let target_val_rows = vec![
        RowBuilder::descriptor("target_rev", "metric", "Target LTM Revenue")
            .format(FormatPreset::Currency)
            .decimals(1)
            .cell_num("val", 250.0)
            .build(),
        RowBuilder::descriptor("target_ebitda", "metric", "Target LTM EBITDA")
            .format(FormatPreset::Currency)
            .decimals(1)
            .cell_num("val", 45.0)
            .build(),
        RowBuilder::descriptor("target_debt", "metric", "Target Total Debt")
            .format(FormatPreset::Currency)
            .decimals(1)
            .cell_num("val", 20.0)
            .build(),
        RowBuilder::descriptor("target_cash", "metric", "Target Cash & Equivalents")
            .format(FormatPreset::Currency)
            .decimals(1)
            .cell_num("val", 15.0)
            .build(),
        build_gutter_row(),
        RowBuilder::descriptor("peer_median_ev_rev", "metric", "Peer Median EV / LTM Rev")
            .format(FormatPreset::Multiple)
            .decimals(1)
            .cell_formula("val", "Comps[\"median\", \"ev_rev\"]")
            .build(),
        RowBuilder::descriptor(
            "peer_median_ev_ebitda",
            "metric",
            "Peer Median EV / LTM EBITDA",
        )
        .format(FormatPreset::Multiple)
        .decimals(1)
        .cell_formula("val", "Comps[\"median\", \"ev_ebitda\"]")
        .build(),
        build_gutter_row(),
        RowBuilder::descriptor("implied_ev_rev", "metric", "Implied EV (EV / Rev)")
            .format(FormatPreset::Currency)
            .decimals(1)
            .cell_formula(
                "val",
                "TargetValuation[\"target_rev\", \"val\"] * TargetValuation[\"peer_median_ev_rev\", \"val\"]",
            )
            .build(),
        RowBuilder::descriptor("implied_ev_ebitda", "metric", "Implied EV (EV / EBITDA)")
            .format(FormatPreset::Currency)
            .decimals(1)
            .cell_formula(
                "val",
                "TargetValuation[\"target_ebitda\", \"val\"] * TargetValuation[\"peer_median_ev_ebitda\", \"val\"]",
            )
            .build(),
        RowBuilder::descriptor(
            "implied_ev_blended",
            "metric",
            "Implied Enterprise Value (Blended)",
        )
        .format(FormatPreset::Currency)
        .decimals(1)
        .style(SemanticStyle::Subtotal)
        .cell_formula(
            "val",
            "AVERAGE(TargetValuation[\"implied_ev_rev\"..\"implied_ev_ebitda\", \"val\"])",
        )
        .build(),
        RowBuilder::descriptor("implied_equity_val", "metric", "Implied Equity Value")
            .format(FormatPreset::Currency)
            .decimals(1)
            .style(SemanticStyle::Total)
            .cell_formula(
                "val",
                "TargetValuation[\"implied_ev_blended\", \"val\"] - TargetValuation[\"target_debt\", \"val\"] + TargetValuation[\"target_cash\", \"val\"]",
            )
            .build(),
    ];

    let target_val_table = TableSpec {
        id: TableId::from("TargetValuation"),
        title: "Target Valuation Benchmarking".to_string(),
        subtitle: Some(
            "Implied Enterprise and Equity Valuation Based on Peer Medians ($ in Millions)"
                .to_string(),
        ),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: target_val_cols,
        rows: target_val_rows,
    };

    let manifest = ProjectManifest {
        model_name: "TradingCompsAndTargetValuation".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Trading_Comps".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Comps"), TableId::from("TargetValuation")],
        }],
    };

    (manifest, vec![comps_table, target_val_table])
}
