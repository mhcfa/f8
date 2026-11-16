#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::compiler::graph::{
    RangeDiscontinuity, ResolveError, resolve_cell_dependencies, resolve_table_ref,
};
use f8_core::compiler::ir::{CellGrid, CellId, SymbolTable, expand_table};
use f8_core::expr::ast::{
    ColEndpoint, ColSelector, RelativeOffset, RowEndpoint, RowSelector, TableRef, TableSelector,
};
use f8_core::model::id::{ColId, RowId, SheetId, TableId};
use f8_core::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec};
use indexmap::IndexMap;

fn build_test_symbols() -> (SymbolTable, TableSpec, TableSpec) {
    let mut symbols = SymbolTable::new();

    let table1 = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("line_item", "Line Item"),
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
            ColumnSpec::new("2026E"),
            ColumnSpec::new("2027E"),
        ],
        rows: vec![
            RowSpec::new("rev"),
            RowSpec::new("cogs"),
            RowSpec::new("sga"),
            RowSpec::new("ebit"),
        ],
    };

    // Table 2: Assumptions with different columns
    let table2 = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Key Assumptions".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("metric"),
            ColumnSpec::new("rate"),
            ColumnSpec::new("value"),
        ],
        rows: vec![RowSpec::new("tax"), RowSpec::new("inflation")],
    };

    symbols
        .register_table(SheetId::from("Model"), &table1)
        .expect("register table1 ok");
    symbols
        .register_table(SheetId::from("Summary"), &table2)
        .expect("register table2 ok");

    (symbols, table1, table2)
}

#[test]
fn test_scenario_1_resolving_discrete_coordinate_reference() {
    let (symbols, _, _) = build_test_symbols();

    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let table_ref = TableRef::new(
        "Assumptions",
        RowSelector::Key(RowId::from("tax")),
        ColSelector::Key(ColId::from("rate")),
    );

    let resolved = resolve_table_ref(&origin, &table_ref, &symbols).expect("resolves successfully");
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0], CellId::new("Assumptions", "tax", "rate"));
}

#[test]
fn test_scenario_2_resolving_active_column_reference() {
    let (symbols, _, _) = build_test_symbols();

    let origin = CellId::new("IncomeStatement", "ebit", "2024E");

    let mut grid = CellGrid::new();
    let mut row_cells = IndexMap::new();
    for col in &["2023A", "2024E", "2025E", "2026E", "2027E"] {
        row_cells.insert(ColId::from(*col), CellInput::number(100.0));
    }

    let is_table = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "IS".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
            ColumnSpec::new("2026E"),
            ColumnSpec::new("2027E"),
        ],
        rows: vec![
            RowSpec::new("rev").with_cells(row_cells.clone()),
            RowSpec::new("cogs").with_cells(row_cells.clone()),
            RowSpec::new("ebit")
                .with_expr("IncomeStatement[\"rev\", @col] - IncomeStatement[\"cogs\", @col]"),
        ],
    };

    let cells = expand_table(&symbols, &is_table).expect("expand ok");
    for cell in cells {
        grid.insert(cell).expect("insert ok");
    }

    let deps = resolve_cell_dependencies(&grid, &symbols).expect("resolution ok");

    let ebit_2024 = deps.get(&origin).expect("has origin deps");
    assert_eq!(ebit_2024.len(), 2);
    assert!(ebit_2024.contains(&CellId::new("IncomeStatement", "rev", "2024E")));
    assert!(ebit_2024.contains(&CellId::new("IncomeStatement", "cogs", "2024E")));
}

#[test]
fn test_scenario_3_cross_table_active_column_mismatch_rejected() {
    let (symbols, _, _) = build_test_symbols();

    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    // Assumptions has columns ["metric", "rate", "value"], NOT "2024E"
    let table_ref = TableRef::new(
        "Assumptions",
        RowSelector::Key(RowId::from("tax")),
        ColSelector::Active,
    );

    let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
    assert_eq!(
        err,
        ResolveError::active_column_mismatch(&origin, "Assumptions", "2024E")
    );
}

#[test]
fn test_scenario_4_resolving_relative_temporal_predecessor_offset() {
    let (symbols, _, _) = build_test_symbols();

    let origin = CellId::new("IncomeStatement", "rev", "2025E"); // index 2 of measures
    let table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Prev(1)),
    );

    let resolved = resolve_table_ref(&origin, &table_ref, &symbols).expect("resolves ok");
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0], CellId::new("IncomeStatement", "rev", "2024E"));
}

#[test]
fn test_scenario_5_temporal_offset_out_of_bounds_rejected() {
    use f8_core::compiler::ErrorDiagnostic;

    let (symbols, _, _) = build_test_symbols();

    // 2023A is at index 0 of measure columns; col.prev(1) stops at descriptor boundary
    let origin = CellId::new("IncomeStatement", "rev", "2023A");
    let table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Prev(1)),
    );

    let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
    assert_eq!(
        err,
        ResolveError::temporal_offset_out_of_bounds(&origin, TableId::from("IncomeStatement"), -1)
    );
    assert_eq!(
        err.to_string(),
        "Formula in cell 'IncomeStatement[\"rev\", \"2023A\"]' has temporal offset -1 which exceeds axis boundaries in table 'IncomeStatement'"
    );
    assert_eq!(
        err.diagnostic(),
        ErrorDiagnostic::ReferenceResolutionError {
            message: err.to_string(),
            coordinate: "IncomeStatement[\"rev\", \"2023A\"]".to_string(),
            target_table: "IncomeStatement".to_string(),
            row: None,
            col: None,
        }
    );

    // Also test forward offset out of bounds at last column (2027E, index 4 of measures)
    let origin_last = CellId::new("IncomeStatement", "rev", "2027E");
    let table_ref_next = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Next(1)),
    );

    let err_next = resolve_table_ref(&origin_last, &table_ref_next, &symbols).unwrap_err();
    assert_eq!(
        err_next,
        ResolveError::temporal_offset_out_of_bounds(
            &origin_last,
            TableId::from("IncomeStatement"),
            1
        )
    );
    assert_eq!(
        err_next.to_string(),
        "Formula in cell 'IncomeStatement[\"rev\", \"2027E\"]' has temporal offset +1 which exceeds axis boundaries in table 'IncomeStatement'"
    );
}

#[test]
fn test_scenario_6_resolving_closed_horizontal_column_range() {
    let (symbols, _, _) = build_test_symbols();

    let origin = CellId::new("IncomeStatement", "ebit", "2027E");
    let table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Range("2024E".into(), "2026E".into()),
    );

    let resolved = resolve_table_ref(&origin, &table_ref, &symbols).expect("resolves ok");
    assert_eq!(resolved.len(), 3);
    assert_eq!(resolved[0], CellId::new("IncomeStatement", "rev", "2024E"));
    assert_eq!(resolved[1], CellId::new("IncomeStatement", "rev", "2025E"));
    assert_eq!(resolved[2], CellId::new("IncomeStatement", "rev", "2026E"));
}

#[test]
fn test_scenario_7_inverted_range_slice_rejected() {
    let (symbols, _, _) = build_test_symbols();

    let origin = CellId::new("IncomeStatement", "ebit", "2027E");
    let table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Range("2026E".into(), "2024E".into()),
    );

    let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
    assert_eq!(
        err,
        ResolveError::inverted_range(&origin, "IncomeStatement", "2026E", "2024E")
    );

    // Row inverted range
    let row_table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Range("sga".into(), "rev".into()),
        ColSelector::Key(ColId::from("2024E")),
    );
    let row_err = resolve_table_ref(&origin, &row_table_ref, &symbols).unwrap_err();
    assert_eq!(
        row_err,
        ResolveError::inverted_range(&origin, "IncomeStatement", "sga", "rev")
    );
}

#[test]
fn test_scenario_8_resolving_relative_offsets_multi_step() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "ebit", "2026E");

    // col.prev(2) from 2026E -> 2024E
    let ref_prev2 = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Prev(2)),
    );
    let resolved_prev2 = resolve_table_ref(&origin, &ref_prev2, &symbols).expect("resolves prev 2");
    assert_eq!(resolved_prev2.len(), 1);
    assert_eq!(resolved_prev2[0].col, ColId::from("2024E"));

    // col.next(1) from 2026E -> 2027E
    let ref_next1 = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Next(1)),
    );
    let resolved_next1 = resolve_table_ref(&origin, &ref_next1, &symbols).expect("resolves next 1");
    assert_eq!(resolved_next1.len(), 1);
    assert_eq!(resolved_next1[0].col, ColId::from("2027E"));
}

#[test]
fn test_scenario_9_resolving_2d_block_range() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "ebit", "2027E");

    // 2 rows (cogs, sga) x 3 columns (2024E, 2025E, 2026E) = 6 cells
    let ref_2d = TableRef::new(
        "IncomeStatement",
        RowSelector::Range("cogs".into(), "sga".into()),
        ColSelector::Range("2024E".into(), "2026E".into()),
    );

    let resolved = resolve_table_ref(&origin, &ref_2d, &symbols).expect("resolves 2d block");
    assert_eq!(resolved.len(), 6);

    // Row-major order:
    // (cogs, 2024E), (cogs, 2025E), (cogs, 2026E)
    // (sga, 2024E), (sga, 2025E), (sga, 2026E)
    assert_eq!(resolved[0], CellId::new("IncomeStatement", "cogs", "2024E"));
    assert_eq!(resolved[1], CellId::new("IncomeStatement", "cogs", "2025E"));
    assert_eq!(resolved[2], CellId::new("IncomeStatement", "cogs", "2026E"));
    assert_eq!(resolved[3], CellId::new("IncomeStatement", "sga", "2024E"));
    assert_eq!(resolved[4], CellId::new("IncomeStatement", "sga", "2025E"));
    assert_eq!(resolved[5], CellId::new("IncomeStatement", "sga", "2026E"));
}

#[test]
fn test_scenario_10_rejecting_unknown_entities() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");

    // Unknown table
    let ref_unknown_table = TableRef::new(
        "NonExistent",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Key(ColId::from("2024E")),
    );
    assert_eq!(
        resolve_table_ref(&origin, &ref_unknown_table, &symbols).unwrap_err(),
        ResolveError::unknown_table(&origin, "NonExistent")
    );

    // Unknown row
    let ref_unknown_row = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("non_existent_row")),
        ColSelector::Key(ColId::from("2024E")),
    );
    assert_eq!(
        resolve_table_ref(&origin, &ref_unknown_row, &symbols).unwrap_err(),
        ResolveError::unknown_row(&origin, "IncomeStatement", "non_existent_row")
    );

    // Unknown column
    let ref_unknown_col = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Key(ColId::from("non_existent_col")),
    );
    assert_eq!(
        resolve_table_ref(&origin, &ref_unknown_col, &symbols).unwrap_err(),
        ResolveError::unknown_column(&origin, "IncomeStatement", "non_existent_col")
    );
}

#[test]
fn test_scenario_active_row_resolution_and_mismatch() {
    let (symbols, _, _) = build_test_symbols();

    // Valid active row: origin is (IncomeStatement, cogs, 2024E)
    let origin = CellId::new("IncomeStatement", "cogs", "2024E");
    let table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Active,
        ColSelector::Key(ColId::from("2024E")),
    );
    let resolved = resolve_table_ref(&origin, &table_ref, &symbols).expect("resolves active row");
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0], CellId::new("IncomeStatement", "cogs", "2024E"));

    // Cross-table mismatch: Assumptions only has rows ["tax", "inflation"], NOT "cogs"
    let mismatch_ref = TableRef::new(
        "Assumptions",
        RowSelector::Active,
        ColSelector::Key(ColId::from("rate")),
    );
    let err = resolve_table_ref(&origin, &mismatch_ref, &symbols).unwrap_err();
    assert_eq!(
        err,
        ResolveError::active_row_mismatch(&origin, "Assumptions", "cogs")
    );
}

#[test]
fn test_scenario_vertical_temporal_offsets_and_bounds() {
    let (symbols, _, _) = build_test_symbols();

    // IncomeStatement rows: ["rev", "cogs", "sga", "ebit"]
    // Origin at index 2 ("sga")
    let origin = CellId::new("IncomeStatement", "sga", "2024E");

    // row.prev(1) -> "cogs" (index 1)
    let ref_prev = TableRef::new(
        "IncomeStatement",
        RowSelector::Offset(RelativeOffset::Prev(1)),
        ColSelector::Key(ColId::from("2024E")),
    );
    let resolved_prev = resolve_table_ref(&origin, &ref_prev, &symbols).expect("resolves row.prev");
    assert_eq!(resolved_prev.len(), 1);
    assert_eq!(
        resolved_prev[0],
        CellId::new("IncomeStatement", "cogs", "2024E")
    );

    // row.next(1) -> "ebit" (index 3)
    let ref_next = TableRef::new(
        "IncomeStatement",
        RowSelector::Offset(RelativeOffset::Next(1)),
        ColSelector::Key(ColId::from("2024E")),
    );
    let resolved_next = resolve_table_ref(&origin, &ref_next, &symbols).expect("resolves row.next");
    assert_eq!(resolved_next.len(), 1);
    assert_eq!(
        resolved_next[0],
        CellId::new("IncomeStatement", "ebit", "2024E")
    );

    // Origin at index 0 ("rev"): row.prev(1) exceeds bounds
    let origin_first = CellId::new("IncomeStatement", "rev", "2024E");
    let err_prev = resolve_table_ref(&origin_first, &ref_prev, &symbols).unwrap_err();
    assert_eq!(
        err_prev,
        ResolveError::temporal_offset_out_of_bounds(
            &origin_first,
            TableId::from("IncomeStatement"),
            -1
        )
    );

    // Origin at index 3 ("ebit"): row.next(1) exceeds bounds
    let origin_last = CellId::new("IncomeStatement", "ebit", "2024E");
    let err_next = resolve_table_ref(&origin_last, &ref_next, &symbols).unwrap_err();
    assert_eq!(
        err_next,
        ResolveError::temporal_offset_out_of_bounds(
            &origin_last,
            TableId::from("IncomeStatement"),
            1
        )
    );
}

#[test]
fn test_scenario_row_ranges_and_col_next_happy_path() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "ebit", "2024E");

    // Row range "cogs".."ebit" -> ["cogs", "sga", "ebit"]
    let ref_range = TableRef::new(
        "IncomeStatement",
        RowSelector::Range("cogs".into(), "ebit".into()),
        ColSelector::Key(ColId::from("2024E")),
    );
    let resolved_range =
        resolve_table_ref(&origin, &ref_range, &symbols).expect("resolves row range");
    assert_eq!(resolved_range.len(), 3);
    assert_eq!(resolved_range[0].row, RowId::from("cogs"));
    assert_eq!(resolved_range[1].row, RowId::from("sga"));
    assert_eq!(resolved_range[2].row, RowId::from("ebit"));

    // ColSelector::Offset(RelativeOffset::Next(1)) happy path
    let ref_col_next = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Next(1)),
    );
    let resolved_col_next =
        resolve_table_ref(&origin, &ref_col_next, &symbols).expect("resolves col.next");
    assert_eq!(resolved_col_next.len(), 1);
    assert_eq!(resolved_col_next[0].col, ColId::from("2025E"));
}

#[test]
fn test_dynamic_expanding_and_prior_period_ranges_resolution_and_eval() {
    use f8_core::compiler::Compiler;
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};
    use f8_core::value::CellValue;

    let mut symbols = SymbolTable::new();
    let cols = vec![
        ColumnSpec::new("close"),
        ColumnSpec::new("2024E"),
        ColumnSpec::new("2025E"),
        ColumnSpec::new("2026E"),
    ];

    let mut sched_cells = IndexMap::new();
    sched_cells.insert(ColId::from("close"), CellInput::number(0.0));
    sched_cells.insert(ColId::from("2024E"), CellInput::number(10.0));
    sched_cells.insert(ColId::from("2025E"), CellInput::number(15.0));
    sched_cells.insert(ColId::from("2026E"), CellInput::number(20.0));

    let mut cum_cells = IndexMap::new();
    cum_cells.insert(ColId::from("close"), CellInput::number(0.0));

    let mut prior_cum_cells = IndexMap::new();
    prior_cum_cells.insert(ColId::from("close"), CellInput::number(0.0));
    prior_cum_cells.insert(ColId::from("2024E"), CellInput::number(0.0));

    let debt_table = TableSpec {
        id: TableId::from("Debt"),
        title: "Debt Schedule".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: cols,
        rows: vec![
            RowSpec::new("sched").with_cells(sched_cells),
            RowSpec::new("cum_sched")
                .with_cells(cum_cells)
                .with_expr("SUM(Debt[\"sched\", \"2024E\"..@col])"),
            RowSpec::new("prior_cum")
                .with_cells(prior_cum_cells)
                .with_expr("SUM(Debt[\"sched\", \"2024E\"..col.prev])"),
        ],
    };

    symbols
        .register_table(SheetId::from("Model"), &debt_table)
        .expect("register debt_table ok");

    // Scenario 4: Dynamic range inversion and out-of-bounds rejection
    let ref_prior_cum = TableRef::new(
        "Debt",
        RowSelector::Key(RowId::from("sched")),
        ColSelector::Range(
            ColEndpoint::from("2024E"),
            ColEndpoint::Offset(RelativeOffset::Prev(1)),
        ),
    );
    // Placed on "2024E": col.prev resolves to "close" (index 0) < "2024E" (index 1) -> InvertedRange
    let origin_2024 = CellId::new("Debt", "prior_cum", "2024E");
    let err_inv = resolve_table_ref(&origin_2024, &ref_prior_cum, &symbols).unwrap_err();
    assert_eq!(
        err_inv,
        ResolveError::inverted_range(&origin_2024, "Debt", "2024E", "close")
    );

    // Placed on "close": col.prev underflows -> TemporalOffsetOutOfBounds(-1)
    let origin_close = CellId::new("Debt", "prior_cum", "close");
    let err_oob = resolve_table_ref(&origin_close, &ref_prior_cum, &symbols).unwrap_err();
    assert_eq!(
        err_oob,
        ResolveError::temporal_offset_out_of_bounds(&origin_close, TableId::from("Debt"), -1)
    );

    // Dynamic row range resolution: "sched"..@row
    let ref_row_expand = TableRef::new(
        "Debt",
        RowSelector::Range(RowEndpoint::from("sched"), RowEndpoint::Active),
        ColSelector::Active,
    );
    let resolved_row =
        resolve_table_ref(&origin_2024, &ref_row_expand, &symbols).expect("resolves row expand");
    assert_eq!(resolved_row.len(), 3);
    assert_eq!(resolved_row[0], CellId::new("Debt", "sched", "2024E"));
    assert_eq!(resolved_row[1], CellId::new("Debt", "cum_sched", "2024E"));
    assert_eq!(resolved_row[2], CellId::new("Debt", "prior_cum", "2024E"));

    // Scenarios 2 & 3: Compile and evaluate full model
    let manifest = ProjectManifest {
        model_name: "DebtTest".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Model".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Debt")],
        }],
    };
    let compiled = Compiler::compile(&manifest, &[debt_table]).expect("compile ok");

    // Scenario 2: cum_sched evaluates to [0.0, 10.0, 25.0, 45.0]
    let debt_id = TableId::from("Debt");
    let cum_row = RowId::from("cum_sched");
    let prior_row = RowId::from("prior_cum");
    assert_eq!(
        compiled
            .cell_value(&debt_id, &cum_row, &ColId::from("close"))
            .unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        compiled
            .cell_value(&debt_id, &cum_row, &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(10.0)
    );
    assert_eq!(
        compiled
            .cell_value(&debt_id, &cum_row, &ColId::from("2025E"))
            .unwrap(),
        CellValue::Number(25.0)
    );
    assert_eq!(
        compiled
            .cell_value(&debt_id, &cum_row, &ColId::from("2026E"))
            .unwrap(),
        CellValue::Number(45.0)
    );

    // Scenario 3: prior_cum evaluates to 10.0 at 2025E and 25.0 at 2026E
    assert_eq!(
        compiled
            .cell_value(&debt_id, &prior_row, &ColId::from("close"))
            .unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        compiled
            .cell_value(&debt_id, &prior_row, &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        compiled
            .cell_value(&debt_id, &prior_row, &ColId::from("2025E"))
            .unwrap(),
        CellValue::Number(10.0)
    );
    assert_eq!(
        compiled
            .cell_value(&debt_id, &prior_row, &ColId::from("2026E"))
            .unwrap(),
        CellValue::Number(25.0)
    );
}

#[test]
fn test_active_table_selector_resolution() {
    let (symbols, _, _) = build_test_symbols();
    let origin_2024 = CellId::new("IncomeStatement", "ebit", "2024E");

    // 1. Scalar active table reference: @table["rev", @col]
    let ref_scalar = TableRef::new(
        TableSelector::Active,
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Active,
    );
    let resolved = resolve_table_ref(&origin_2024, &ref_scalar, &symbols).unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0], CellId::new("IncomeStatement", "rev", "2024E"));

    // 2. Relative offset active table reference: @table["rev", col.prev]
    let ref_prev = TableRef::new(
        TableSelector::Active,
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Prev(1)),
    );
    let resolved_prev = resolve_table_ref(&origin_2024, &ref_prev, &symbols).unwrap();
    assert_eq!(resolved_prev.len(), 1);
    assert_eq!(
        resolved_prev[0],
        CellId::new("IncomeStatement", "rev", "2023A")
    );

    // 3. Range slice active table reference: @table["rev".."cogs", @col]
    let ref_range = TableRef::new(
        TableSelector::Active,
        RowSelector::Range(RowEndpoint::from("rev"), RowEndpoint::from("cogs")),
        ColSelector::Active,
    );
    let resolved_range = resolve_table_ref(&origin_2024, &ref_range, &symbols).unwrap();
    assert_eq!(resolved_range.len(), 2);
    assert_eq!(
        resolved_range[0],
        CellId::new("IncomeStatement", "rev", "2024E")
    );
    assert_eq!(
        resolved_range[1],
        CellId::new("IncomeStatement", "cogs", "2024E")
    );
}

#[test]
fn test_active_table_selector_model_compilation_and_inspection() {
    use f8_core::compiler::Compiler;
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};
    use f8_core::value::CellValue;

    let table_active = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("y1"), ColumnSpec::new("y2")],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("y1", CellInput::number(100.0))
                .with_cell("y2", CellInput::number(200.0)),
            RowSpec::new("cogs")
                .with_cell("y1", CellInput::number(40.0))
                .with_cell("y2", CellInput::number(70.0)),
            RowSpec::new("gp").with_expr("@table[\"rev\", @col] - @table[\"cogs\", @col]"),
            RowSpec::new("sum_gp").with_expr("SUM(@table[\"rev\"..\"cogs\", @col])"),
            RowSpec::new("lag_rev")
                .with_cell("y1", CellInput::number(0.0))
                .with_cell("y2", CellInput::formula("@table[\"rev\", col.prev]")),
        ],
    };

    let table_explicit = TableSpec {
        id: TableId::from("IncomeStatementExplicit"),
        title: "Income Statement Explicit".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("y1"), ColumnSpec::new("y2")],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("y1", CellInput::number(100.0))
                .with_cell("y2", CellInput::number(200.0)),
            RowSpec::new("cogs")
                .with_cell("y1", CellInput::number(40.0))
                .with_cell("y2", CellInput::number(70.0)),
            RowSpec::new("gp").with_expr(
                "IncomeStatementExplicit[\"rev\", @col] - IncomeStatementExplicit[\"cogs\", @col]",
            ),
            RowSpec::new("sum_gp")
                .with_expr("SUM(IncomeStatementExplicit[\"rev\"..\"cogs\", @col])"),
            RowSpec::new("lag_rev")
                .with_cell("y1", CellInput::number(0.0))
                .with_cell(
                    "y2",
                    CellInput::formula("IncomeStatementExplicit[\"rev\", col.prev]"),
                ),
        ],
    };

    let manifest = ProjectManifest {
        model_name: "ActiveTableTest".into(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "Model".into(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("IncomeStatement")],
            },
            SheetSpec {
                name: "ExplicitModel".into(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("IncomeStatementExplicit")],
            },
        ],
    };

    let compiled =
        Compiler::compile(&manifest, &[table_active, table_explicit]).expect("compile ok");

    let is_id = TableId::from("IncomeStatement");
    let exp_id = TableId::from("IncomeStatementExplicit");

    // Pass 4: values evaluate identically
    assert_eq!(
        compiled
            .cell_value(&is_id, &RowId::from("gp"), &ColId::from("y1"))
            .unwrap(),
        compiled
            .cell_value(&exp_id, &RowId::from("gp"), &ColId::from("y1"))
            .unwrap()
    );
    assert_eq!(
        compiled
            .cell_value(&is_id, &RowId::from("gp"), &ColId::from("y1"))
            .unwrap(),
        CellValue::Number(60.0)
    );

    assert_eq!(
        compiled
            .cell_value(&is_id, &RowId::from("gp"), &ColId::from("y2"))
            .unwrap(),
        compiled
            .cell_value(&exp_id, &RowId::from("gp"), &ColId::from("y2"))
            .unwrap()
    );
    assert_eq!(
        compiled
            .cell_value(&is_id, &RowId::from("gp"), &ColId::from("y2"))
            .unwrap(),
        CellValue::Number(130.0)
    );

    assert_eq!(
        compiled
            .cell_value(&is_id, &RowId::from("sum_gp"), &ColId::from("y1"))
            .unwrap(),
        compiled
            .cell_value(&exp_id, &RowId::from("sum_gp"), &ColId::from("y1"))
            .unwrap()
    );
    assert_eq!(
        compiled
            .cell_value(&is_id, &RowId::from("sum_gp"), &ColId::from("y1"))
            .unwrap(),
        CellValue::Number(140.0)
    );

    assert_eq!(
        compiled
            .cell_value(&is_id, &RowId::from("lag_rev"), &ColId::from("y2"))
            .unwrap(),
        compiled
            .cell_value(&exp_id, &RowId::from("lag_rev"), &ColId::from("y2"))
            .unwrap()
    );
    assert_eq!(
        compiled
            .cell_value(&is_id, &RowId::from("lag_rev"), &ColId::from("y2"))
            .unwrap(),
        CellValue::Number(100.0)
    );

    // Pass 3 dependencies & inspect_cell: formula_expr preserves @table[...]
    let insp = compiled
        .inspect_cell(&is_id, &RowId::from("gp"), &ColId::from("y1"))
        .expect("cell inspection ok");
    assert_eq!(
        insp.formula_expr.as_deref(),
        Some("@table[\"rev\", @col] - @table[\"cogs\", @col]")
    );

    // Direct dependencies: resolves to IncomeStatement
    assert_eq!(insp.precedents.len(), 2);
    assert!(
        insp.precedents
            .iter()
            .any(|d| d.cell_id == CellId::new("IncomeStatement", "rev", "y1"))
    );
    assert!(
        insp.precedents
            .iter()
            .any(|d| d.cell_id == CellId::new("IncomeStatement", "cogs", "y1"))
    );
}

#[test]
fn test_contiguous_data_row_and_measure_col_ranges_with_surrounding_dividers() {
    use f8_core::compiler::Compiler;
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};
    use f8_core::value::CellValue;

    let table = TableSpec {
        id: TableId::from("T"),
        title: "Contiguous Sub-Block".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("line_item", "Line Item"),
            ColumnSpec::gutter(),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
            ColumnSpec::gutter(),
            ColumnSpec::new("2026E"),
        ],
        rows: vec![
            RowSpec::section_header("Revenue"),
            RowSpec::new("r1")
                .with_cell("line_item", "Product")
                .with_cell("2024E", 10.0)
                .with_cell("2025E", 20.0)
                .with_cell("2026E", 30.0),
            RowSpec::new("r2")
                .with_cell("line_item", "Services")
                .with_cell("2024E", 40.0)
                .with_cell("2025E", 50.0)
                .with_cell("2026E", 60.0),
            RowSpec::gutter(),
            RowSpec::new("tot")
                .with_cell("line_item", "2x2 Subtotal")
                .with_expr("SUM(@table[\"r1\"..\"r2\", \"2024E\"..\"2025E\"])"),
        ],
    };

    let manifest = ProjectManifest {
        model_name: "ContiguousRangeModel".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("T")],
        }],
    };

    let compiled = Compiler::compile(&manifest, &[table]).expect("compile ok");
    let t_id = TableId::from("T");
    assert_eq!(
        compiled
            .cell_value(&t_id, &RowId::from("tot"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(120.0)
    );

    let r1_2024 = compiled
        .inspect_cell(&t_id, &RowId::from("r1"), &ColId::from("2024E"))
        .unwrap();
    let r2_2025 = compiled
        .inspect_cell(&t_id, &RowId::from("r2"), &ColId::from("2025E"))
        .unwrap();
    let tot_2024 = compiled
        .inspect_cell(&t_id, &RowId::from("tot"), &ColId::from("2024E"))
        .unwrap();
    // Title (row 2), title gutter (row 3), header (row 4), section header (row 5), r1 (row 6), r2 (row 7)
    // Col B = line_item, Col C = gutter, Col D = 2024E, Col E = 2025E
    assert_eq!(r1_2024.physical_address, "'Main'!D6");
    assert_eq!(r2_2025.physical_address, "'Main'!E7");
    assert_eq!(tot_2024.excel_formula.as_deref(), Some("=SUM($D$6:$E$7)"));
    assert_eq!(tot_2024.precedents.len(), 4);
}

#[test]
fn test_vertical_range_on_single_descriptor_column_succeeds() {
    use f8_core::compiler::Compiler;
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};
    use f8_core::value::CellValue;

    let table = TableSpec {
        id: TableId::from("T"),
        title: "Descriptor Vertical Slice".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("line_item", "Line Item"),
            ColumnSpec::new("2024E"),
        ],
        rows: vec![
            RowSpec::new("r1")
                .with_cell("line_item", "COGS")
                .with_cell("2024E", 40.0),
            RowSpec::new("r2")
                .with_cell("line_item", "Rev")
                .with_cell("2024E", 100.0),
            RowSpec::new("r3")
                .with_cell("line_item", "OpEx")
                .with_cell("2024E", 25.0),
            RowSpec::gutter(),
            RowSpec::new("summary")
                .with_cell(
                    "line_item",
                    CellInput::formula("COUNTA(@table[\"r1\"..\"r2\", @col])"),
                )
                .with_expr(
                    "XLOOKUP(\"Rev\", @table[\"r1\"..\"r3\", \"line_item\"], @table[\"r1\"..\"r3\", @col])",
                ),
        ],
    };

    let manifest = ProjectManifest {
        model_name: "DescriptorVerticalModel".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("T")],
        }],
    };

    let compiled = Compiler::compile(&manifest, &[table]).expect("compile ok");
    let t_id = TableId::from("T");
    assert_eq!(
        compiled
            .cell_value(&t_id, &RowId::from("summary"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(100.0)
    );
    assert_eq!(
        compiled
            .cell_value(&t_id, &RowId::from("summary"), &ColId::from("line_item"))
            .unwrap(),
        CellValue::Number(2.0)
    );
}

#[test]
fn test_row_range_spanning_gutter_rejected() {
    use f8_core::compiler::graph::GraphError;
    use f8_core::compiler::{Compiler, CompilerError, ErrorDiagnostic};
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};

    let table = TableSpec {
        id: TableId::from("T"),
        title: "Row Gutter".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E")],
        rows: vec![
            RowSpec::new("r1").with_cell("2024E", 10.0),
            RowSpec::gutter(),
            RowSpec::new("r2").with_cell("2024E", 20.0),
            RowSpec::new("tot").with_expr("SUM(T[\"r1\"..\"r2\", @col])"),
        ],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &table)
        .unwrap();

    let origin = CellId::new("T", "tot", "2024E");
    let table_ref = TableRef::new(
        "T",
        RowSelector::Range("r1".into(), "r2".into()),
        ColSelector::Active,
    );
    let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
    assert_eq!(
        err,
        ResolveError::discontinuous_range(
            &origin,
            TableId::from("T"),
            "r1".to_string(),
            "r2".to_string(),
            RangeDiscontinuity::RowGutter,
        )
    );
    assert_eq!(
        err.to_string(),
        "Formula in cell 'T[\"tot\", \"2024E\"]' specifies range 'r1'..'r2' in table 'T' spanning a gutter row"
    );
    assert_eq!(
        err.diagnostic(),
        ErrorDiagnostic::ReferenceResolutionError {
            message: err.to_string(),
            coordinate: "T[\"tot\", \"2024E\"]".to_string(),
            target_table: "T".to_string(),
            row: None,
            col: None,
        }
    );

    let manifest = ProjectManifest {
        model_name: "RowGutterFail".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("T")],
        }],
    };
    let compile_err = Compiler::compile(&manifest, &[table]).unwrap_err();
    assert!(matches!(
        compile_err,
        CompilerError::Graph(GraphError::Resolve(ResolveError::DiscontinuousRange {
            discontinuity: RangeDiscontinuity::RowGutter,
            ..
        }))
    ));
}

#[test]
fn test_row_range_spanning_section_header_rejected_including_untaken_branch() {
    use f8_core::compiler::graph::GraphError;
    use f8_core::compiler::{Compiler, CompilerError};
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};

    for expr_str in [
        "SUM(T[\"r1\"..\"r2\", @col])",
        "IF(TRUE, 1, SUM(T[\"r1\"..\"r2\", @col]))",
    ] {
        let table = TableSpec {
            id: TableId::from("T"),
            title: "Section Header".into(),
            subtitle: None,
            currency: None,
            collapsible: false,
            placement: f8_core::model::PlacementSpec::default(),
            columns: vec![ColumnSpec::new("2024E")],
            rows: vec![
                RowSpec::new("r1").with_cell("2024E", 10.0),
                RowSpec::section_header("OpEx"),
                RowSpec::new("r2").with_cell("2024E", 20.0),
                RowSpec::new("tot").with_expr(expr_str),
            ],
        };

        let manifest = ProjectManifest {
            model_name: "SectionHeaderFail".into(),
            theme: Default::default(),
            sheets: vec![SheetSpec {
                name: "Main".into(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("T")],
            }],
        };

        let err = Compiler::compile(&manifest, &[table]).unwrap_err();
        let CompilerError::Graph(GraphError::Resolve(resolve_err)) = err else {
            panic!("expected Pass 3 DiscontinuousRange error, got {err:?}");
        };
        assert_eq!(
            resolve_err,
            ResolveError::discontinuous_range(
                &CellId::new("T", "tot", "2024E"),
                TableId::from("T"),
                "r1".to_string(),
                "r2".to_string(),
                RangeDiscontinuity::SectionHeader {
                    title: "OpEx".to_string(),
                },
            )
        );
        assert!(
            resolve_err
                .to_string()
                .contains("spanning section header 'OpEx'")
        );
    }
}

#[test]
fn test_col_range_spanning_gutter_rejected() {
    let table = TableSpec {
        id: TableId::from("T"),
        title: "Column Gutter".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2024E"),
            ColumnSpec::gutter(),
            ColumnSpec::new("2025E"),
        ],
        rows: vec![
            RowSpec::new("r1")
                .with_cell("2024E", 10.0)
                .with_cell("2025E", 20.0),
        ],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &table)
        .unwrap();

    let origin = CellId::new("T", "r1", "2025E");
    for col_sel in [
        ColSelector::Range("2024E".into(), "2025E".into()),
        ColSelector::Range(
            ColEndpoint::Offset(RelativeOffset::Prev(1)),
            ColEndpoint::Active,
        ),
    ] {
        let table_ref = TableRef::new("T", RowSelector::Key(RowId::from("r1")), col_sel);
        let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
        assert_eq!(
            err,
            ResolveError::discontinuous_range(
                &origin,
                TableId::from("T"),
                "2024E".to_string(),
                "2025E".to_string(),
                RangeDiscontinuity::ColumnGutter,
            )
        );
        assert!(err.to_string().contains("spanning a gutter column"));
    }
}

#[test]
fn test_col_range_including_descriptor_endpoint_or_intermediate_rejected() {
    use f8_core::compiler::ErrorDiagnostic;

    let table = TableSpec {
        id: TableId::from("T"),
        title: "Descriptor Column Range".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("desc", "Desc"),
            ColumnSpec::new("m1"),
            ColumnSpec::descriptor("notes", "Notes"),
            ColumnSpec::new("m2"),
        ],
        rows: vec![RowSpec::new("r1")],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &table)
        .unwrap();

    let origin = CellId::new("T", "r1", "m2");
    let cases = [
        (
            ColSelector::Range("desc".into(), "m1".into()),
            "desc",
            "m1",
            "desc",
        ),
        (
            ColSelector::Range("desc".into(), "desc".into()),
            "desc",
            "desc",
            "desc",
        ),
        (
            ColSelector::Range("m1".into(), "m2".into()),
            "m1",
            "m2",
            "notes",
        ),
        (
            ColSelector::Range(
                ColEndpoint::Offset(RelativeOffset::Prev(1)),
                ColEndpoint::Active,
            ),
            "m1",
            "m2",
            "notes",
        ),
        (
            ColSelector::Range("m1".into(), "notes".into()),
            "m1",
            "notes",
            "notes",
        ),
    ];

    for (col_sel, expected_start, expected_end, expected_desc_col) in cases {
        let table_ref = TableRef::new("T", RowSelector::Key(RowId::from("r1")), col_sel);
        let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
        assert_eq!(
            err,
            ResolveError::discontinuous_range(
                &origin,
                TableId::from("T"),
                expected_start.to_string(),
                expected_end.to_string(),
                RangeDiscontinuity::DescriptorColumn {
                    column: ColId::from(expected_desc_col),
                },
            )
        );
        assert!(
            err.to_string()
                .contains(&format!("spanning descriptor column '{expected_desc_col}'"))
        );
        assert_eq!(
            err.diagnostic(),
            ErrorDiagnostic::ReferenceResolutionError {
                message: err.to_string(),
                coordinate: "T[\"r1\", \"m2\"]".to_string(),
                target_table: "T".to_string(),
                row: None,
                col: Some(expected_desc_col.to_string()),
            }
        );
    }
}

#[test]
fn test_inverted_range_precedes_discontinuity_and_first_discontinuity_order() {
    let table = TableSpec {
        id: TableId::from("T"),
        title: "Ordering".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("desc", "Desc"),
            ColumnSpec::new("m1"),
            ColumnSpec::gutter(),
            ColumnSpec::descriptor("notes", "Notes"),
            ColumnSpec::new("m2"),
        ],
        rows: vec![
            RowSpec::new("r1"),
            RowSpec::gutter(),
            RowSpec::section_header("OpEx"),
            RowSpec::new("r2"),
        ],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &table)
        .unwrap();

    let origin = CellId::new("T", "r1", "m1");

    // Scenario 7: Inverted range precedes discontinuity
    let inv_row_ref = TableRef::new(
        "T",
        RowSelector::Range("r2".into(), "r1".into()),
        ColSelector::Key(ColId::from("m1")),
    );
    assert_eq!(
        resolve_table_ref(&origin, &inv_row_ref, &symbols).unwrap_err(),
        ResolveError::inverted_range(&origin, "T", "r2", "r1")
    );

    let inv_col_ref = TableRef::new(
        "T",
        RowSelector::Key(RowId::from("r1")),
        ColSelector::Range("m1".into(), "desc".into()),
    );
    assert_eq!(
        resolve_table_ref(&origin, &inv_col_ref, &symbols).unwrap_err(),
        ResolveError::inverted_range(&origin, "T", "m1", "desc")
    );

    // Multiple discontinuities report first in declared table order
    let multi_row_ref = TableRef::new(
        "T",
        RowSelector::Range("r1".into(), "r2".into()),
        ColSelector::Key(ColId::from("m1")),
    );
    assert_eq!(
        resolve_table_ref(&origin, &multi_row_ref, &symbols).unwrap_err(),
        ResolveError::discontinuous_range(
            &origin,
            TableId::from("T"),
            "r1".to_string(),
            "r2".to_string(),
            RangeDiscontinuity::RowGutter,
        )
    );

    let multi_col_ref = TableRef::new(
        "T",
        RowSelector::Key(RowId::from("r1")),
        ColSelector::Range("m1".into(), "m2".into()),
    );
    assert_eq!(
        resolve_table_ref(&origin, &multi_col_ref, &symbols).unwrap_err(),
        ResolveError::discontinuous_range(
            &origin,
            TableId::from("T"),
            "m1".to_string(),
            "m2".to_string(),
            RangeDiscontinuity::ColumnGutter,
        )
    );
}

#[test]
fn test_cross_table_col_offset_at_origin_boundary_and_range() {
    use f8_core::compiler::Compiler;
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};
    use f8_core::value::CellValue;

    let history = TableSpec {
        id: TableId::from("History"),
        title: "History".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2022A"),
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
        ],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("2022A", 100.0)
                .with_cell("2023A", 150.0)
                .with_cell("2024E", CellInput::formula("Forecast[\"rev\", col.next]")),
        ],
    };

    let forecast = TableSpec {
        id: TableId::from("Forecast"),
        title: "Forecast".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E"), ColumnSpec::new("2025E")],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("2024E", CellInput::formula("History[\"rev\", col.prev]"))
                .with_cell("2025E", 300.0),
            RowSpec::new("cum_hist")
                .with_cell(
                    "2024E",
                    CellInput::formula("SUM(History[\"rev\", \"2022A\"..col.prev])"),
                )
                .with_cell("2025E", 0.0),
        ],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &history)
        .unwrap();
    symbols
        .register_table(SheetId::from("Main"), &forecast)
        .unwrap();

    let forecast_origin = CellId::new("Forecast", "rev", "2024E");
    let ref_hist_prev = TableRef::new(
        "History",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Prev(1)),
    );
    assert_eq!(
        resolve_table_ref(&forecast_origin, &ref_hist_prev, &symbols)
            .unwrap()
            .cells(),
        &[CellId::new("History", "rev", "2023A")]
    );

    let ref_hist_slice = TableRef::new(
        "History",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Range(
            ColEndpoint::from("2022A"),
            ColEndpoint::Offset(RelativeOffset::Prev(1)),
        ),
    );
    assert_eq!(
        resolve_table_ref(&forecast_origin, &ref_hist_slice, &symbols)
            .unwrap()
            .cells(),
        &[
            CellId::new("History", "rev", "2022A"),
            CellId::new("History", "rev", "2023A"),
        ]
    );

    let history_origin = CellId::new("History", "rev", "2024E");
    let ref_forecast_next = TableRef::new(
        "Forecast",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Next(1)),
    );
    assert_eq!(
        resolve_table_ref(&history_origin, &ref_forecast_next, &symbols)
            .unwrap()
            .cells(),
        &[CellId::new("Forecast", "rev", "2025E")]
    );

    let manifest = ProjectManifest {
        model_name: "CrossTableBoundary".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("History"), TableId::from("Forecast")],
        }],
    };

    let compiled = Compiler::compile(&manifest, &[history, forecast]).expect("compile ok");
    let hist_id = TableId::from("History");
    let fcst_id = TableId::from("Forecast");

    assert_eq!(
        compiled
            .cell_value(&fcst_id, &RowId::from("rev"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(150.0)
    );
    assert_eq!(
        compiled
            .cell_value(&fcst_id, &RowId::from("cum_hist"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(250.0)
    );
    assert_eq!(
        compiled
            .cell_value(&hist_id, &RowId::from("rev"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(300.0)
    );

    assert_eq!(
        compiled
            .inspect_cell(&hist_id, &RowId::from("rev"), &ColId::from("2023A"))
            .unwrap()
            .physical_address,
        "'Main'!C4"
    );
    assert_eq!(
        compiled
            .inspect_cell(&fcst_id, &RowId::from("rev"), &ColId::from("2025E"))
            .unwrap()
            .physical_address,
        "'Main'!C9"
    );

    let fcst_rev_2024 = compiled
        .inspect_cell(&fcst_id, &RowId::from("rev"), &ColId::from("2024E"))
        .unwrap();
    assert_eq!(fcst_rev_2024.excel_formula.as_deref(), Some("=C4"));
    assert_eq!(fcst_rev_2024.precedents.len(), 1);
    assert_eq!(
        fcst_rev_2024.precedents[0].cell_id,
        CellId::new("History", "rev", "2023A")
    );

    let fcst_cum_2024 = compiled
        .inspect_cell(&fcst_id, &RowId::from("cum_hist"), &ColId::from("2024E"))
        .unwrap();
    assert_eq!(
        fcst_cum_2024.excel_formula.as_deref(),
        Some("=SUM($B$4:C$4)")
    );
    assert_eq!(fcst_cum_2024.precedents.len(), 2);

    let hist_rev_2024 = compiled
        .inspect_cell(&hist_id, &RowId::from("rev"), &ColId::from("2024E"))
        .unwrap();
    assert_eq!(hist_rev_2024.excel_formula.as_deref(), Some("=C9"));
    assert_eq!(hist_rev_2024.precedents.len(), 1);
    assert_eq!(
        hist_rev_2024.precedents[0].cell_id,
        CellId::new("Forecast", "rev", "2025E")
    );
}

#[test]
fn test_cross_table_col_offset_steps_along_target_measure_columns() {
    use f8_core::compiler::Compiler;
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};
    use f8_core::value::CellValue;

    let detail = TableSpec {
        id: TableId::from("Detail"),
        title: "Detail".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2022A"),
            ColumnSpec::descriptor("notes", "Notes"),
            ColumnSpec::gutter(),
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
        ],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("2022A", 80.0)
                .with_cell("notes", "audited")
                .with_cell("2023A", 120.0)
                .with_cell("2024E", 160.0),
        ],
    };

    let summary = TableSpec {
        id: TableId::from("Summary"),
        title: "Summary".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2022A"), ColumnSpec::new("2024E")],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("2022A", 80.0)
                .with_cell("2024E", CellInput::formula("Detail[\"rev\", col.prev]")),
            RowSpec::new("rev_lag2")
                .with_cell("2022A", 0.0)
                .with_cell("2024E", CellInput::formula("Detail[\"rev\", col.prev(2)]")),
        ],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &detail)
        .unwrap();
    symbols
        .register_table(SheetId::from("Main"), &summary)
        .unwrap();

    let origin = CellId::new("Summary", "rev", "2024E");
    let ref_prev1 = TableRef::new(
        "Detail",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Prev(1)),
    );
    let ref_prev2 = TableRef::new(
        "Detail",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Offset(RelativeOffset::Prev(2)),
    );

    assert_eq!(
        resolve_table_ref(&origin, &ref_prev1, &symbols)
            .unwrap()
            .cells(),
        &[CellId::new("Detail", "rev", "2023A")]
    );
    assert_eq!(
        resolve_table_ref(&origin, &ref_prev2, &symbols)
            .unwrap()
            .cells(),
        &[CellId::new("Detail", "rev", "2022A")]
    );

    let manifest = ProjectManifest {
        model_name: "TargetMeasureStepping".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Detail"), TableId::from("Summary")],
        }],
    };

    let compiled = Compiler::compile(&manifest, &[detail, summary]).expect("compile ok");
    let sum_id = TableId::from("Summary");

    assert_eq!(
        compiled
            .cell_value(&sum_id, &RowId::from("rev"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(120.0)
    );
    assert_eq!(
        compiled
            .cell_value(&sum_id, &RowId::from("rev_lag2"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(80.0)
    );

    // Detail on Main: Col B = 2022A, Col C = notes, Col D = gutter, Col E = 2023A, Col F = 2024E; rev = row 5
    let sum_rev = compiled
        .inspect_cell(&sum_id, &RowId::from("rev"), &ColId::from("2024E"))
        .unwrap();
    assert_eq!(sum_rev.excel_formula.as_deref(), Some("=E5"));

    let sum_lag2 = compiled
        .inspect_cell(&sum_id, &RowId::from("rev_lag2"), &ColId::from("2024E"))
        .unwrap();
    assert_eq!(sum_lag2.excel_formula.as_deref(), Some("=B5"));
}

#[test]
fn test_cross_table_row_offset_steps_along_target_data_rows() {
    use f8_core::compiler::Compiler;
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};
    use f8_core::value::CellValue;

    let schedule = TableSpec {
        id: TableId::from("Schedule"),
        title: "Schedule".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E")],
        rows: vec![
            RowSpec::new("r0").with_cell("2024E", 10.0),
            RowSpec::section_header("Sec"),
            RowSpec::new("r1").with_cell("2024E", 20.0),
            RowSpec::gutter(),
            RowSpec::new("r2").with_cell("2024E", 30.0),
            RowSpec::new("r3").with_cell("2024E", 40.0),
        ],
    };

    let driver_scalar = TableSpec {
        id: TableId::from("DriverTable"),
        title: "Drivers".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E")],
        rows: vec![
            RowSpec::new("r1").with_expr("Schedule[row.prev, @col]"),
            RowSpec::new("r3").with_expr("Schedule[row.prev, @col]"),
        ],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &schedule)
        .unwrap();
    symbols
        .register_table(SheetId::from("Main"), &driver_scalar)
        .unwrap();

    let origin_r1 = CellId::new("DriverTable", "r1", "2024E");
    let origin_r3 = CellId::new("DriverTable", "r3", "2024E");
    let ref_row_prev = TableRef::new(
        "Schedule",
        RowSelector::Offset(RelativeOffset::Prev(1)),
        ColSelector::Active,
    );
    let ref_row_next = TableRef::new(
        "Schedule",
        RowSelector::Offset(RelativeOffset::Next(1)),
        ColSelector::Active,
    );
    let ref_row_range = TableRef::new(
        "Schedule",
        RowSelector::Range(
            RowEndpoint::from("r2"),
            RowEndpoint::Offset(RelativeOffset::Prev(1)),
        ),
        ColSelector::Active,
    );

    assert_eq!(
        resolve_table_ref(&origin_r1, &ref_row_prev, &symbols)
            .unwrap()
            .cells(),
        &[CellId::new("Schedule", "r0", "2024E")]
    );
    assert_eq!(
        resolve_table_ref(&origin_r1, &ref_row_next, &symbols)
            .unwrap()
            .cells(),
        &[CellId::new("Schedule", "r2", "2024E")]
    );
    assert_eq!(
        resolve_table_ref(&origin_r3, &ref_row_prev, &symbols)
            .unwrap()
            .cells(),
        &[CellId::new("Schedule", "r2", "2024E")]
    );
    assert_eq!(
        resolve_table_ref(&origin_r3, &ref_row_range, &symbols)
            .unwrap()
            .cells(),
        &[CellId::new("Schedule", "r2", "2024E")]
    );

    let manifest = ProjectManifest {
        model_name: "CrossTableRowOffset".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Schedule"), TableId::from("DriverTable")],
        }],
    };

    let compiled_scalar =
        Compiler::compile(&manifest, &[schedule.clone(), driver_scalar]).expect("compile ok");
    let drv_id = TableId::from("DriverTable");
    assert_eq!(
        compiled_scalar
            .cell_value(&drv_id, &RowId::from("r1"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(10.0)
    );
    assert_eq!(
        compiled_scalar
            .cell_value(&drv_id, &RowId::from("r3"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(30.0)
    );
    // Schedule rows on Main (no col headers): Title (2), title gutter (3), r0 (4), Sec (5), r1 (6), gutter (7), r2 (8), r3 (9)
    let sched_id = TableId::from("Schedule");
    assert_eq!(
        compiled_scalar
            .inspect_cell(&sched_id, &RowId::from("r0"), &ColId::from("2024E"))
            .unwrap()
            .physical_address,
        "'Main'!B4"
    );
    assert_eq!(
        compiled_scalar
            .inspect_cell(&sched_id, &RowId::from("r2"), &ColId::from("2024E"))
            .unwrap()
            .physical_address,
        "'Main'!B8"
    );
    assert_eq!(
        compiled_scalar
            .inspect_cell(&drv_id, &RowId::from("r1"), &ColId::from("2024E"))
            .unwrap()
            .excel_formula
            .as_deref(),
        Some("=B4")
    );
    assert_eq!(
        compiled_scalar
            .inspect_cell(&drv_id, &RowId::from("r3"), &ColId::from("2024E"))
            .unwrap()
            .excel_formula
            .as_deref(),
        Some("=B8")
    );

    let driver_range = TableSpec {
        id: TableId::from("DriverTable"),
        title: "Drivers".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E")],
        rows: vec![
            RowSpec::new("r1").with_expr("Schedule[row.prev, @col]"),
            RowSpec::new("r3").with_expr("SUM(Schedule[\"r2\"..row.prev, @col])"),
        ],
    };
    let compiled_range =
        Compiler::compile(&manifest, &[schedule, driver_range]).expect("compile ok");
    assert_eq!(
        compiled_range
            .cell_value(&drv_id, &RowId::from("r3"), &ColId::from("2024E"))
            .unwrap(),
        CellValue::Number(30.0)
    );
    assert_eq!(
        compiled_range
            .inspect_cell(&drv_id, &RowId::from("r3"), &ColId::from("2024E"))
            .unwrap()
            .excel_formula
            .as_deref(),
        Some("=SUM(B$8:B8)")
    );
}

#[test]
fn test_cross_table_offset_active_mismatch_rejected() {
    use f8_core::compiler::graph::GraphError;
    use f8_core::compiler::{Compiler, CompilerError, ErrorDiagnostic};
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};

    let assumptions = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Assumptions".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("val")],
        rows: vec![RowSpec::new("tax").with_cell("val", 0.21)],
    };

    let forecast_base = TableSpec {
        id: TableId::from("Forecast"),
        title: "Forecast".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E")],
        rows: vec![RowSpec::new("ebitda").with_cell("2024E", 100.0)],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &assumptions)
        .unwrap();
    symbols
        .register_table(SheetId::from("Main"), &forecast_base)
        .unwrap();

    let origin = CellId::new("Forecast", "ebitda", "2024E");

    for col_sel in [
        ColSelector::Offset(RelativeOffset::Prev(1)),
        ColSelector::Offset(RelativeOffset::Next(1)),
        ColSelector::Range(
            ColEndpoint::from("val"),
            ColEndpoint::Offset(RelativeOffset::Prev(1)),
        ),
        ColSelector::Range(
            ColEndpoint::Offset(RelativeOffset::Prev(1)),
            ColEndpoint::from("val"),
        ),
    ] {
        let table_ref = TableRef::new("Assumptions", RowSelector::Key(RowId::from("tax")), col_sel);
        let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
        assert_eq!(
            err,
            ResolveError::active_column_mismatch(&origin, "Assumptions", "2024E")
        );
        assert_eq!(
            err.diagnostic(),
            ErrorDiagnostic::ReferenceResolutionError {
                message: err.to_string(),
                coordinate: "Forecast[\"ebitda\", \"2024E\"]".to_string(),
                target_table: "Assumptions".to_string(),
                row: None,
                col: Some("2024E".to_string()),
            }
        );
    }

    for row_sel in [
        RowSelector::Offset(RelativeOffset::Prev(1)),
        RowSelector::Offset(RelativeOffset::Next(1)),
        RowSelector::Range(
            RowEndpoint::from("tax"),
            RowEndpoint::Offset(RelativeOffset::Prev(1)),
        ),
        RowSelector::Range(
            RowEndpoint::Offset(RelativeOffset::Prev(1)),
            RowEndpoint::from("tax"),
        ),
    ] {
        let table_ref = TableRef::new("Assumptions", row_sel, ColSelector::Key(ColId::from("val")));
        let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
        assert_eq!(
            err,
            ResolveError::active_row_mismatch(&origin, "Assumptions", "ebitda")
        );
        assert_eq!(
            err.diagnostic(),
            ErrorDiagnostic::ReferenceResolutionError {
                message: err.to_string(),
                coordinate: "Forecast[\"ebitda\", \"2024E\"]".to_string(),
                target_table: "Assumptions".to_string(),
                row: Some("ebitda".to_string()),
                col: None,
            }
        );
    }

    let manifest = ProjectManifest {
        model_name: "CrossTableMismatch".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Assumptions"), TableId::from("Forecast")],
        }],
    };

    let forecast_col_mismatch = TableSpec {
        rows: vec![RowSpec::new("ebitda").with_expr("Assumptions[\"tax\", col.prev]")],
        ..forecast_base.clone()
    };
    let err_col = Compiler::compile(&manifest, &[assumptions.clone(), forecast_col_mismatch])
        .expect_err("expected ActiveColumnMismatch");
    assert!(matches!(
        err_col,
        CompilerError::Graph(GraphError::Resolve(ResolveError::ActiveColumnMismatch {
            ref target_table,
            ref column,
            ..
        })) if target_table.as_str() == "Assumptions" && column.as_str() == "2024E"
    ));

    let forecast_row_mismatch = TableSpec {
        rows: vec![RowSpec::new("ebitda").with_expr("Assumptions[row.prev, \"val\"]")],
        ..forecast_base
    };
    let err_row = Compiler::compile(&manifest, &[assumptions, forecast_row_mismatch])
        .expect_err("expected ActiveRowMismatch");
    assert!(matches!(
        err_row,
        CompilerError::Graph(GraphError::Resolve(ResolveError::ActiveRowMismatch {
            ref target_table,
            ref row,
            ..
        })) if target_table.as_str() == "Assumptions" && row.as_str() == "ebitda"
    ));
}

#[test]
fn test_cross_table_offset_target_bounds_descriptor_anchor_and_inverted_range() {
    use f8_core::compiler::ErrorDiagnostic;

    let target = TableSpec {
        id: TableId::from("Target"),
        title: "Target".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("label", "Label"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
        ],
        rows: vec![RowSpec::new("r1"), RowSpec::new("r2")],
    };

    // Origin has earlier column "2023A" and earlier row "r0" so origin-stepping would not underflow,
    // plus both descriptor and measure variants of "label".
    let origin_table = TableSpec {
        id: TableId::from("Origin"),
        title: "Origin".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::descriptor("label", "Label"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
        ],
        rows: vec![RowSpec::new("r0"), RowSpec::new("r1"), RowSpec::new("r2")],
    };

    let origin_measure_label = TableSpec {
        id: TableId::from("OriginMeasureLabel"),
        title: "Origin Measure Label".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::new("label"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
        ],
        rows: vec![RowSpec::new("r1")],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Main"), &target)
        .unwrap();
    symbols
        .register_table(SheetId::from("Main"), &origin_table)
        .unwrap();
    symbols
        .register_table(SheetId::from("Main"), &origin_measure_label)
        .unwrap();

    // 1. Origin["r1", "2024E"] -> Target["r1", col.prev]: "2024E" is Target's first measure column
    let origin_2024 = CellId::new("Origin", "r1", "2024E");
    let ref_col_prev = TableRef::new(
        "Target",
        RowSelector::Key(RowId::from("r1")),
        ColSelector::Offset(RelativeOffset::Prev(1)),
    );
    let err_col_oob = resolve_table_ref(&origin_2024, &ref_col_prev, &symbols).unwrap_err();
    assert_eq!(
        err_col_oob,
        ResolveError::temporal_offset_out_of_bounds(&origin_2024, TableId::from("Target"), -1)
    );
    assert_eq!(
        err_col_oob.to_string(),
        "Formula in cell 'Origin[\"r1\", \"2024E\"]' has temporal offset -1 which exceeds axis boundaries in table 'Target'"
    );
    assert_eq!(
        err_col_oob.diagnostic(),
        ErrorDiagnostic::ReferenceResolutionError {
            message: err_col_oob.to_string(),
            coordinate: "Origin[\"r1\", \"2024E\"]".to_string(),
            target_table: "Target".to_string(),
            row: None,
            col: None,
        }
    );

    // 2. Origin["r1", "2024E"] -> Target[row.prev, "2024E"]: "r1" is Target's first data row
    let ref_row_prev = TableRef::new(
        "Target",
        RowSelector::Offset(RelativeOffset::Prev(1)),
        ColSelector::Key(ColId::from("2024E")),
    );
    let err_row_oob = resolve_table_ref(&origin_2024, &ref_row_prev, &symbols).unwrap_err();
    assert_eq!(
        err_row_oob,
        ResolveError::temporal_offset_out_of_bounds(&origin_2024, TableId::from("Target"), -1)
    );

    // 3. Forward bounds in Target: "2025E" is Target's last measure col, "r2" is Target's last data row
    let origin_2025 = CellId::new("Origin", "r1", "2025E");
    let ref_col_next = TableRef::new(
        "Target",
        RowSelector::Key(RowId::from("r1")),
        ColSelector::Offset(RelativeOffset::Next(1)),
    );
    let err_col_next = resolve_table_ref(&origin_2025, &ref_col_next, &symbols).unwrap_err();
    assert_eq!(
        err_col_next,
        ResolveError::temporal_offset_out_of_bounds(&origin_2025, TableId::from("Target"), 1)
    );
    assert_eq!(
        err_col_next.to_string(),
        "Formula in cell 'Origin[\"r1\", \"2025E\"]' has temporal offset +1 which exceeds axis boundaries in table 'Target'"
    );

    let origin_r2 = CellId::new("Origin", "r2", "2024E");
    let ref_row_next = TableRef::new(
        "Target",
        RowSelector::Offset(RelativeOffset::Next(1)),
        ColSelector::Key(ColId::from("2024E")),
    );
    assert_eq!(
        resolve_table_ref(&origin_r2, &ref_row_next, &symbols).unwrap_err(),
        ResolveError::temporal_offset_out_of_bounds(&origin_r2, TableId::from("Target"), 1)
    );

    // 4. Origin["r1", "label"] (both descriptor and measure in Origin) -> Target["r1", col.prev]:
    // "label" is AxisRole::Descriptor in Target -> TemporalOffsetOutOfBounds
    for label_origin in [
        CellId::new("Origin", "r1", "label"),
        CellId::new("OriginMeasureLabel", "r1", "label"),
    ] {
        let err_desc = resolve_table_ref(&label_origin, &ref_col_prev, &symbols).unwrap_err();
        assert_eq!(
            err_desc,
            ResolveError::temporal_offset_out_of_bounds(&label_origin, TableId::from("Target"), -1)
        );
        assert_eq!(
            err_desc.diagnostic(),
            ErrorDiagnostic::ReferenceResolutionError {
                message: err_desc.to_string(),
                coordinate: label_origin.to_string(),
                target_table: "Target".to_string(),
                row: None,
                col: None,
            }
        );
    }

    // 5. Origin["r1", "2025E"] -> Target["r1", col.prev.."label"]:
    // col.prev resolves to Target.columns[1] ("2024E") > Target.columns[0] ("label") -> InvertedRange
    let ref_inverted = TableRef::new(
        "Target",
        RowSelector::Key(RowId::from("r1")),
        ColSelector::Range(
            ColEndpoint::Offset(RelativeOffset::Prev(1)),
            ColEndpoint::from("label"),
        ),
    );
    assert_eq!(
        resolve_table_ref(&origin_2025, &ref_inverted, &symbols).unwrap_err(),
        ResolveError::inverted_range(&origin_2025, "Target", "2024E", "label")
    );

    // 6. Multi-reference expressions where HistoryA succeeds and Target fails (underflow, overflow,
    // or descriptor anchor in Target) must disambiguate target_table = "Target".
    use f8_core::compiler::graph::GraphError;
    use f8_core::compiler::{Compiler, CompilerError};
    use f8_core::model::manifest::{ProjectManifest, SheetSpec};

    let history_a = TableSpec {
        id: TableId::from("HistoryA"),
        title: "History A".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::new("label"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
            ColumnSpec::new("2026E"),
        ],
        rows: vec![
            RowSpec::new("r0")
                .with_cell("2023A", 1.0)
                .with_cell("label", 2.0)
                .with_cell("2024E", 3.0)
                .with_cell("2025E", 4.0)
                .with_cell("2026E", 5.0),
            RowSpec::new("r1")
                .with_cell("2023A", 10.0)
                .with_cell("label", 20.0)
                .with_cell("2024E", 30.0)
                .with_cell("2025E", 40.0)
                .with_cell("2026E", 50.0),
            RowSpec::new("r2")
                .with_cell("2023A", 11.0)
                .with_cell("label", 21.0)
                .with_cell("2024E", 31.0)
                .with_cell("2025E", 41.0)
                .with_cell("2026E", 51.0),
            RowSpec::new("r3")
                .with_cell("2023A", 12.0)
                .with_cell("label", 22.0)
                .with_cell("2024E", 32.0)
                .with_cell("2025E", 42.0)
                .with_cell("2026E", 52.0),
        ],
    };

    let target_full = TableSpec {
        id: TableId::from("Target"),
        title: "Target".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("label", "Label"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
        ],
        rows: vec![
            RowSpec::new("r1")
                .with_cell("label", "One")
                .with_cell("2024E", 10.0)
                .with_cell("2025E", 20.0),
            RowSpec::new("r2")
                .with_cell("label", "Two")
                .with_cell("2024E", 30.0)
                .with_cell("2025E", 40.0),
        ],
    };

    let multi_cases = [
        (
            "r1",
            "2024E",
            "HistoryA[\"r1\", col.prev] + Target[\"r1\", col.prev]",
            -1,
        ),
        (
            "r1",
            "2025E",
            "HistoryA[\"r1\", col.next] + Target[\"r1\", col.next]",
            1,
        ),
        (
            "r1",
            "2024E",
            "HistoryA[row.prev, \"2024E\"] + Target[row.prev, \"2024E\"]",
            -1,
        ),
        (
            "r2",
            "2024E",
            "HistoryA[row.next, \"2024E\"] + Target[row.next, \"2024E\"]",
            1,
        ),
        (
            "r1",
            "label",
            "HistoryA[\"r1\", col.prev] + Target[\"r1\", col.prev]",
            -1,
        ),
    ];

    let manifest = ProjectManifest {
        model_name: "MultiTableDisambiguation".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                TableId::from("HistoryA"),
                TableId::from("Target"),
                TableId::from("Origin"),
            ],
        }],
    };

    for (fail_row, fail_col, expr_str, expected_offset) in multi_cases {
        let mut r1_cells = IndexMap::new();
        r1_cells.insert(ColId::from("2023A"), CellInput::number(1.0));
        r1_cells.insert(ColId::from("label"), CellInput::number(2.0));
        r1_cells.insert(ColId::from("2024E"), CellInput::number(3.0));
        r1_cells.insert(ColId::from("2025E"), CellInput::number(4.0));

        let mut r2_cells = r1_cells.clone();
        if fail_row == "r1" {
            r1_cells.insert(ColId::from(fail_col), CellInput::formula(expr_str));
        } else {
            r2_cells.insert(ColId::from(fail_col), CellInput::formula(expr_str));
        }

        let origin_multi = TableSpec {
            id: TableId::from("Origin"),
            title: "Origin".into(),
            subtitle: None,
            currency: None,
            collapsible: false,
            placement: f8_core::model::PlacementSpec::default(),
            columns: vec![
                ColumnSpec::new("2023A"),
                ColumnSpec::new("label"),
                ColumnSpec::new("2024E"),
                ColumnSpec::new("2025E"),
            ],
            rows: vec![
                RowSpec::new("r1").with_cells(r1_cells),
                RowSpec::new("r2").with_cells(r2_cells),
            ],
        };

        let compile_err = Compiler::compile(
            &manifest,
            &[history_a.clone(), target_full.clone(), origin_multi],
        )
        .expect_err("expected TemporalOffsetOutOfBounds in Target");

        let expected_origin = CellId::new("Origin", fail_row, fail_col);
        let expected_resolve = ResolveError::temporal_offset_out_of_bounds(
            &expected_origin,
            TableId::from("Target"),
            expected_offset,
        );
        assert_eq!(
            compile_err.diagnostic(),
            ErrorDiagnostic::ReferenceResolutionError {
                message: expected_resolve.to_string(),
                coordinate: expected_origin.to_string(),
                target_table: "Target".to_string(),
                row: None,
                col: None,
            }
        );
        let CompilerError::Graph(GraphError::Resolve(resolve_err)) = compile_err else {
            panic!("expected Graph(Resolve(_)), got {compile_err:?}");
        };
        assert_eq!(resolve_err, expected_resolve);
    }
}

#[test]
fn test_direct_ast_zero_and_overflow_relative_offsets_rejected_in_resolver() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "cogs", "2025E");

    for bad_col_offset in [
        RelativeOffset::Prev(0),
        RelativeOffset::Next(0),
        RelativeOffset::Prev(u32::MAX),
        RelativeOffset::Next(u32::MAX),
    ] {
        let table_ref = TableRef::new(
            "IncomeStatement",
            RowSelector::Key(RowId::from("rev")),
            ColSelector::Offset(bad_col_offset),
        );
        let err = resolve_table_ref(&origin, &table_ref, &symbols)
            .expect_err("zero or > i32::MAX col offset must fail resolution");
        assert!(
            matches!(err, ResolveError::TemporalOffsetOutOfBounds { .. }),
            "expected TemporalOffsetOutOfBounds, got {err:?}"
        );
    }

    for bad_row_offset in [
        RelativeOffset::Prev(0),
        RelativeOffset::Next(0),
        RelativeOffset::Prev(u32::MAX),
        RelativeOffset::Next(u32::MAX),
    ] {
        let table_ref = TableRef::new(
            "IncomeStatement",
            RowSelector::Offset(bad_row_offset),
            ColSelector::Key(ColId::from("2025E")),
        );
        let err = resolve_table_ref(&origin, &table_ref, &symbols)
            .expect_err("zero or > i32::MAX row offset must fail resolution");
        assert!(
            matches!(err, ResolveError::TemporalOffsetOutOfBounds { .. }),
            "expected TemporalOffsetOutOfBounds, got {err:?}"
        );
    }
}
