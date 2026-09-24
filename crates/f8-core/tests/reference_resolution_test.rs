#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::compiler::graph::{ResolveError, resolve_cell_dependencies, resolve_table_ref};
use f8_core::compiler::ir::{CellGrid, CellId, SymbolTable, expand_table};
use f8_core::expr::ast::{ColSelector, RelativeOffset, RowSelector, TableRef};
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
        ResolveError::temporal_offset_out_of_bounds(&origin, -1)
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
        ResolveError::temporal_offset_out_of_bounds(&origin_last, 1)
    );
}

#[test]
fn test_scenario_6_resolving_closed_horizontal_column_range() {
    let (symbols, _, _) = build_test_symbols();

    let origin = CellId::new("IncomeStatement", "ebit", "2027E");
    let table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Range(ColId::from("2024E"), ColId::from("2026E")),
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
        ColSelector::Range(ColId::from("2026E"), ColId::from("2024E")),
    );

    let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
    assert_eq!(
        err,
        ResolveError::inverted_range(&origin, "IncomeStatement", "2026E", "2024E")
    );

    // Row inverted range
    let row_table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Range(RowId::from("sga"), RowId::from("rev")),
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
        RowSelector::Range(RowId::from("cogs"), RowId::from("sga")),
        ColSelector::Range(ColId::from("2024E"), ColId::from("2026E")),
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
        ResolveError::temporal_offset_out_of_bounds(&origin_first, -1)
    );

    // Origin at index 3 ("ebit"): row.next(1) exceeds bounds
    let origin_last = CellId::new("IncomeStatement", "ebit", "2024E");
    let err_next = resolve_table_ref(&origin_last, &ref_next, &symbols).unwrap_err();
    assert_eq!(
        err_next,
        ResolveError::temporal_offset_out_of_bounds(&origin_last, 1)
    );
}

#[test]
fn test_scenario_row_ranges_and_col_next_happy_path() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "ebit", "2024E");

    // Row range "cogs".."ebit" -> ["cogs", "sga", "ebit"]
    let ref_range = TableRef::new(
        "IncomeStatement",
        RowSelector::Range(RowId::from("cogs"), RowId::from("ebit")),
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
