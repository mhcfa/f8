#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::compiler::graph::{ResolveError, resolve_cell_dependencies, resolve_table_ref};
use finx::compiler::ir::{CellGrid, CellId, SymbolTable, expand_table};
use finx::expr::ast::{ColSelector, RowSelector, SpecialCol, SpecialRow, TableRef};
use finx::model::id::{ColGroupId, ColId, RowGroupId, RowId, SheetId, TableId};
use finx::model::table::{
    AxisRole, CellInput, ColumnGroup, ColumnSpec, RowGroup, RowGroupChild, RowSpec, TableSpec,
};
use indexmap::IndexMap;

fn build_test_symbols() -> (SymbolTable, TableSpec, TableSpec) {
    let mut symbols = SymbolTable::new();

    // Table 1: IncomeStatement with forecast group
    let forecast_group = ColumnGroup {
        id: ColGroupId::from("forecast"),
        label: "Forecast".into(),
        columns: vec![
            ColId::from("2024E"),
            ColId::from("2025E"),
            ColId::from("2026E"),
        ],
    };

    let expenses_group = RowGroup {
        id: RowGroupId::from("expenses"),
        label: "Expenses".into(),
        children: vec![
            RowGroupChild::Row(RowId::from("cogs")),
            RowGroupChild::Row(RowId::from("sga")),
        ],
    };

    let table1 = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".into(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
            ColumnSpec::new("2026E"),
            ColumnSpec::new("2027E"),
        ],
        column_groups: vec![forecast_group],
        row_groups: vec![expenses_group],
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
        columns: vec![
            ColumnSpec::new("metric"),
            ColumnSpec::new("rate"),
            ColumnSpec::new("value"),
        ],
        column_groups: vec![],
        row_groups: vec![],
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
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
            ColumnSpec::new("2026E"),
            ColumnSpec::new("2027E"),
        ],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            RowSpec {
                id: RowId::from("rev"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: None,
                cells: row_cells.clone(),
            },
            RowSpec {
                id: RowId::from("cogs"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: None,
                cells: row_cells.clone(),
            },
            RowSpec {
                id: RowId::from("ebit"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: Some(
                    "IncomeStatement[\"rev\", @col] - IncomeStatement[\"cogs\", @col]".into(),
                ),
                cells: IndexMap::new(),
            },
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

    let origin = CellId::new("IncomeStatement", "rev", "2025E"); // index 2
    let table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Special(SpecialCol::Prev(1)),
    );

    let resolved = resolve_table_ref(&origin, &table_ref, &symbols).expect("resolves ok");
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0], CellId::new("IncomeStatement", "rev", "2024E"));
}

#[test]
fn test_scenario_5_temporal_offset_out_of_bounds_rejected() {
    let (symbols, _, _) = build_test_symbols();

    // 2023A is at index 0
    let origin = CellId::new("IncomeStatement", "rev", "2023A");
    let table_ref = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Special(SpecialCol::Prev(1)),
    );

    let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
    assert_eq!(
        err,
        ResolveError::temporal_offset_out_of_bounds(&origin, -1)
    );

    // Also test forward offset out of bounds at last column (2027E, index 4)
    let origin_last = CellId::new("IncomeStatement", "rev", "2027E");
    let table_ref_next = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Special(SpecialCol::Next(1)),
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
fn test_scenario_8_resolving_column_group_and_milestone_selectors() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "ebit", "2027E");

    // cols.forecast
    let ref_group = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Special(SpecialCol::Group(ColGroupId::from("forecast"))),
    );
    let resolved_group = resolve_table_ref(&origin, &ref_group, &symbols).expect("resolves group");
    assert_eq!(resolved_group.len(), 3);
    assert_eq!(resolved_group[0].col, ColId::from("2024E"));
    assert_eq!(resolved_group[1].col, ColId::from("2025E"));
    assert_eq!(resolved_group[2].col, ColId::from("2026E"));

    // cols.forecast.first
    let ref_first = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Special(SpecialCol::GroupFirst(ColGroupId::from("forecast"))),
    );
    let resolved_first = resolve_table_ref(&origin, &ref_first, &symbols).expect("resolves first");
    assert_eq!(resolved_first.len(), 1);
    assert_eq!(resolved_first[0].col, ColId::from("2024E"));

    // cols.forecast.last
    let ref_last = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Special(SpecialCol::GroupLast(ColGroupId::from("forecast"))),
    );
    let resolved_last = resolve_table_ref(&origin, &ref_last, &symbols).expect("resolves last");
    assert_eq!(resolved_last.len(), 1);
    assert_eq!(resolved_last[0].col, ColId::from("2026E"));

    // rows.expenses
    let ref_rows = TableRef::new(
        "IncomeStatement",
        RowSelector::Special(SpecialRow::Group(RowGroupId::from("expenses"))),
        ColSelector::Key(ColId::from("2024E")),
    );
    let resolved_rows =
        resolve_table_ref(&origin, &ref_rows, &symbols).expect("resolves row group");
    assert_eq!(resolved_rows.len(), 2);
    assert_eq!(resolved_rows[0].row, RowId::from("cogs"));
    assert_eq!(resolved_rows[1].row, RowId::from("sga"));
}

#[test]
fn test_scenario_9_resolving_2d_block_range() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "ebit", "2027E");

    // 2 rows (cogs, sga) x 3 columns (2024E, 2025E, 2026E) = 6 cells
    let ref_2d = TableRef::new(
        "IncomeStatement",
        RowSelector::Special(SpecialRow::Group(RowGroupId::from("expenses"))),
        ColSelector::Special(SpecialCol::Group(ColGroupId::from("forecast"))),
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

    // Unknown column group
    let ref_unknown_col_group = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Special(SpecialCol::Group(ColGroupId::from("ghost_group"))),
    );
    assert_eq!(
        resolve_table_ref(&origin, &ref_unknown_col_group, &symbols).unwrap_err(),
        ResolveError::unknown_column_group(&origin, "IncomeStatement", "ghost_group")
    );

    // Unknown row group
    let ref_unknown_row_group = TableRef::new(
        "IncomeStatement",
        RowSelector::Special(SpecialRow::Group(RowGroupId::from("ghost_group"))),
        ColSelector::Key(ColId::from("2024E")),
    );
    assert_eq!(
        resolve_table_ref(&origin, &ref_unknown_row_group, &symbols).unwrap_err(),
        ResolveError::unknown_row_group(&origin, "IncomeStatement", "ghost_group")
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
        RowSelector::Special(SpecialRow::Prev(1)),
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
        RowSelector::Special(SpecialRow::Next(1)),
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
fn test_scenario_row_milestones_and_happy_path_ranges() {
    let (symbols, _, _) = build_test_symbols();
    let origin = CellId::new("IncomeStatement", "ebit", "2024E");

    // rows.expenses.first -> "cogs"
    let ref_first = TableRef::new(
        "IncomeStatement",
        RowSelector::Special(SpecialRow::GroupFirst(RowGroupId::from("expenses"))),
        ColSelector::Key(ColId::from("2024E")),
    );
    let resolved_first = resolve_table_ref(&origin, &ref_first, &symbols).expect("resolves first");
    assert_eq!(resolved_first.len(), 1);
    assert_eq!(resolved_first[0].row, RowId::from("cogs"));

    // rows.expenses.last -> "sga"
    let ref_last = TableRef::new(
        "IncomeStatement",
        RowSelector::Special(SpecialRow::GroupLast(RowGroupId::from("expenses"))),
        ColSelector::Key(ColId::from("2024E")),
    );
    let resolved_last = resolve_table_ref(&origin, &ref_last, &symbols).expect("resolves last");
    assert_eq!(resolved_last.len(), 1);
    assert_eq!(resolved_last[0].row, RowId::from("sga"));

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

    // SpecialCol::Next happy path
    let ref_col_next = TableRef::new(
        "IncomeStatement",
        RowSelector::Key(RowId::from("rev")),
        ColSelector::Special(SpecialCol::Next(1)),
    );
    let resolved_col_next =
        resolve_table_ref(&origin, &ref_col_next, &symbols).expect("resolves col.next");
    assert_eq!(resolved_col_next.len(), 1);
    assert_eq!(resolved_col_next[0].col, ColId::from("2025E"));
}
