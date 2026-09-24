//! Dedicated evaluation test suite for Public Trading Comps model fixture.
//!
//! Validates exact mathematical precision of:
//! - Entity row calculations (`market_cap`, `ev`, `ebitda_margin`, multiples) broadcasting via `@row`.
//! - Summary benchmark rows (`mean`, `median`, `max`, `min`) aggregating vertical slices with Excel parity.
//! - Secondary `TargetValuation` benchmarking table resolving cross-table references and implied valuations.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::Compiler;
use f8_core::model::id::{ColId, RowId, TableId};
use f8_core::value::CellValue;

mod fixtures;

#[test]
fn test_trading_comps_mathematical_precision_and_target_valuation() {
    let (manifest, tables) = fixtures::trading_comps::build_trading_comps_model();

    let evaluated = Compiler::evaluate(&manifest, &tables)
        .expect("Trading comps model must compile and evaluate without errors");

    let errors = evaluated.find_errors();
    assert!(
        errors.is_empty(),
        "Expected zero evaluation errors in trading comps model, found: {errors:?}"
    );

    let comps_id = TableId::from("Comps");
    let target_id = TableId::from("TargetValuation");

    let get_num = |t: &TableId, r: &str, c: &str| -> f64 {
        match evaluated.get_value(t, &RowId::from(r), &ColId::from(c)) {
            Some(CellValue::Number(n)) => *n,
            other => panic!("Expected number at {t:?}[{r}, {c}], got {other:?}"),
        }
    };

    let get_text = |t: &TableId, r: &str, c: &str| -> String {
        match evaluated.get_value(t, &RowId::from(r), &ColId::from(c)) {
            Some(CellValue::Text(s)) => s.clone(),
            other => panic!("Expected text at {t:?}[{r}, {c}], got {other:?}"),
        }
    };

    // -------------------------------------------------------------------------
    // 1. Verify Entity Descriptors and Input Values
    // -------------------------------------------------------------------------
    assert_eq!(get_text(&comps_id, "snow", "company"), "Snowflake Inc.");
    assert_eq!(get_text(&comps_id, "snow", "ticker"), "SNOW");
    assert_eq!(get_num(&comps_id, "snow", "share_price"), 150.0);
    assert_eq!(get_num(&comps_id, "snow", "shares_out"), 330.0);

    // -------------------------------------------------------------------------
    // 2. Verify Column-Vector Formulas Broadcasting with @row
    // -------------------------------------------------------------------------
    // Snowflake (snow)
    let snow_mcap = get_num(&comps_id, "snow", "market_cap");
    assert!((snow_mcap - 49_500.0).abs() < 1e-6);
    let snow_ev = get_num(&comps_id, "snow", "ev");
    assert!((snow_ev - 46_000.0).abs() < 1e-6); // 49500 + 0 - 3500
    let snow_margin = get_num(&comps_id, "snow", "ebitda_margin");
    assert!((snow_margin - 350.0 / 3200.0).abs() < 1e-6);
    let snow_ev_rev = get_num(&comps_id, "snow", "ev_rev");
    assert!((snow_ev_rev - 14.375).abs() < 1e-6); // 46000 / 3200
    let snow_ev_ebitda = get_num(&comps_id, "snow", "ev_ebitda");
    assert!((snow_ev_ebitda - 46_000.0 / 350.0).abs() < 1e-6);
    let snow_pe = get_num(&comps_id, "snow", "pe");
    assert!((snow_pe - 49_500.0 / (350.0 * 0.75)).abs() < 1e-6);

    // Datadog (ddog)
    let ddog_mcap = get_num(&comps_id, "ddog", "market_cap");
    assert!((ddog_mcap - 39_000.0).abs() < 1e-6);
    let ddog_ev = get_num(&comps_id, "ddog", "ev");
    assert!((ddog_ev - 36_200.0).abs() < 1e-6); // 39000 + 0 - 2800
    let ddog_ev_rev = get_num(&comps_id, "ddog", "ev_rev");
    assert!((ddog_ev_rev - 36_200.0 / 2600.0).abs() < 1e-6);
    let ddog_ev_ebitda = get_num(&comps_id, "ddog", "ev_ebitda");
    assert!((ddog_ev_ebitda - 36_200.0 / 600.0).abs() < 1e-6);

    // MongoDB (mdb)
    let mdb_mcap = get_num(&comps_id, "mdb", "market_cap");
    assert!((mdb_mcap - 21_000.0).abs() < 1e-6);
    let mdb_ev = get_num(&comps_id, "mdb", "ev");
    assert!((mdb_ev - 19_100.0).abs() < 1e-6);
    let mdb_pe = get_num(&comps_id, "mdb", "pe");
    assert!((mdb_pe - 100.0).abs() < 1e-6); // 21000 / (280 * 0.75) = 21000 / 210

    // Cloudflare (net)
    let net_mcap = get_num(&comps_id, "net", "market_cap");
    assert!((net_mcap - 28_900.0).abs() < 1e-6);
    let net_ev = get_num(&comps_id, "net", "ev");
    assert!((net_ev - 28_700.0).abs() < 1e-6); // 28900 + 1200 - 1400

    // CrowdStrike (crwd)
    let crwd_mcap = get_num(&comps_id, "crwd", "market_cap");
    assert!((crwd_mcap - 75_950.0).abs() < 1e-6);
    let crwd_ev = get_num(&comps_id, "crwd", "ev");
    assert!((crwd_ev - 73_000.0).abs() < 1e-6); // 75950 + 750 - 3700

    // Elastic (estc)
    let estc_mcap = get_num(&comps_id, "estc", "market_cap");
    assert!((estc_mcap - 10_500.0).abs() < 1e-6);
    let estc_ev = get_num(&comps_id, "estc", "ev");
    assert!((estc_ev - 10_070.0).abs() < 1e-6); // 10500 + 570 - 1000
    let estc_margin = get_num(&comps_id, "estc", "ebitda_margin");
    assert!((estc_margin - 0.15).abs() < 1e-6); // 195 / 1300

    // -------------------------------------------------------------------------
    // 3. Verify Vertical Summary Benchmark Rows (AVERAGE, MEDIAN, MIN, MAX)
    // -------------------------------------------------------------------------
    // Share Price Benchmarks
    let sp_mean = get_num(&comps_id, "mean", "share_price");
    assert!((sp_mean - 175.0).abs() < 1e-6);
    let sp_median = get_num(&comps_id, "median", "share_price");
    assert!((sp_median - 135.0).abs() < 1e-6); // (120 + 150) / 2
    let sp_min = get_num(&comps_id, "min", "share_price");
    assert!((sp_min - 85.0).abs() < 1e-6);
    let sp_max = get_num(&comps_id, "max", "share_price");
    assert!((sp_max - 310.0).abs() < 1e-6);

    // Market Cap Benchmarks
    let mc_mean = get_num(&comps_id, "mean", "market_cap");
    assert!((mc_mean - 37_475.0).abs() < 1e-6);
    let mc_median = get_num(&comps_id, "median", "market_cap");
    assert!((mc_median - 33_950.0).abs() < 1e-6); // (28900 + 39000) / 2

    // EV / Rev Benchmarks
    let ev_rev_median = get_num(&comps_id, "median", "ev_rev");
    let expected_ev_rev_median = (36_200.0 / 2600.0 + 14.375) / 2.0; // (ddog + snow) / 2
    assert!((ev_rev_median - expected_ev_rev_median).abs() < 1e-6);
    let ev_rev_min = get_num(&comps_id, "min", "ev_rev");
    assert!((ev_rev_min - 10_070.0 / 1300.0).abs() < 1e-6); // estc

    // EV / EBITDA Benchmarks
    let ev_ebitda_median = get_num(&comps_id, "median", "ev_ebitda");
    let expected_ev_ebitda_median = (19_100.0 / 280.0 + 73_000.0 / 900.0) / 2.0; // (mdb + crwd) / 2
    assert!((ev_ebitda_median - expected_ev_ebitda_median).abs() < 1e-6);

    // -------------------------------------------------------------------------
    // 4. Verify Target Valuation Benchmarking Table
    // -------------------------------------------------------------------------
    // Cross-table peer median references
    let target_ev_rev_peer = get_num(&target_id, "peer_median_ev_rev", "val");
    assert_eq!(target_ev_rev_peer, ev_rev_median);

    let target_ev_ebitda_peer = get_num(&target_id, "peer_median_ev_ebitda", "val");
    assert_eq!(target_ev_ebitda_peer, ev_ebitda_median);

    // Implied valuations
    let implied_ev_rev = get_num(&target_id, "implied_ev_rev", "val");
    let expected_implied_ev_rev = 250.0 * expected_ev_rev_median;
    assert!((implied_ev_rev - expected_implied_ev_rev).abs() < 1e-6);

    let implied_ev_ebitda = get_num(&target_id, "implied_ev_ebitda", "val");
    let expected_implied_ev_ebitda = 45.0 * expected_ev_ebitda_median;
    assert!((implied_ev_ebitda - expected_implied_ev_ebitda).abs() < 1e-6);

    let implied_ev_blended = get_num(&target_id, "implied_ev_blended", "val");
    let expected_blended = (expected_implied_ev_rev + expected_implied_ev_ebitda) / 2.0;
    assert!((implied_ev_blended - expected_blended).abs() < 1e-6);

    let implied_equity_val = get_num(&target_id, "implied_equity_val", "val");
    let expected_equity = expected_blended - 20.0 + 15.0; // Blended EV - Debt + Cash
    assert!((implied_equity_val - expected_equity).abs() < 1e-6);
}
