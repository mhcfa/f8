#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::model::{
    ColGroupId, ColId, ColumnGroup, FormatPreset, FreezePanes, MatchArmSpec, MatchSelector,
    ModelError, ProjectManifest, RowGroup, RowGroupChild, RowGroupId, RowId, RowSpec, SheetSpec,
    TableId, TableSpec, ValueLiteral,
};
use indexmap::IndexMap;
use std::path::PathBuf;

#[test]
fn test_scenario_1_deserialize_valid_manifest() {
    let json_data = r#"{
        "model_name": "LBO_Simple",
        "theme": "corporate_navy",
        "sheets": [
            {
                "name": "Summary",
                "tab_color": "navy",
                "tables": ["tables/assumptions.json"]
            },
            {
                "name": "Model",
                "tab_color": "forest_green",
                "freeze_panes": { "row": 2, "col": 1 },
                "show_gridlines": true,
                "tables": ["tables/income_statement.json"]
            }
        ]
    }"#;

    let manifest: ProjectManifest = serde_json::from_str(json_data).unwrap();
    assert_eq!(manifest.model_name, "LBO_Simple");
    assert_eq!(manifest.theme, "corporate_navy");
    assert_eq!(manifest.sheets.len(), 2);
    assert_eq!(manifest.sheets[0].name, "Summary");
    assert!(manifest.sheets[0].show_gridlines);
    assert_eq!(
        manifest.sheets[1].freeze_panes,
        Some(FreezePanes { row: 2, col: 1 })
    );
    assert!(manifest.validate().is_ok());
}

#[test]
fn test_manifest_duplicate_sheet_name_rejected() {
    let manifest = ProjectManifest {
        model_name: "Test".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![PathBuf::from("tables/a.json")],
            },
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![PathBuf::from("tables/b.json")],
            },
        ],
    };

    let err = manifest.validate().unwrap_err();
    assert!(matches!(err, ModelError::DuplicateSheetName { sheet } if sheet == "Model"));
}

#[test]
fn test_scenario_2_deserialize_and_validate_assumptions_table() {
    let json_data = r#"{
        "id": "Assumptions",
        "title": "Model Assumptions",
        "columns": ["value"],
        "rows": [
            {
                "id": "tax_rate",
                "label": "Corporate Tax Rate",
                "format": "percentage",
                "value": [0.25],
                "note": "Federal + blended state rate"
            },
            {
                "id": "exit_multiple",
                "label": "Exit EV / EBITDA Multiple",
                "format": "multiple",
                "value": [10.0]
            }
        ]
    }"#;

    let table: TableSpec = serde_json::from_str(json_data).unwrap();
    assert_eq!(table.id.as_str(), "Assumptions");
    assert_eq!(table.columns, vec![ColId::from("value")]);
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows[0].format, Some(FormatPreset::Percentage));
    assert_eq!(table.rows[0].value, Some(vec![ValueLiteral::Number(0.25)]));
    assert!(table.validate().is_ok());
}

#[test]
fn test_scenario_2_deserialize_and_validate_income_statement_example() {
    let json_data = r#"{
        "id": "IncomeStatement",
        "title": "Income Statement",
        "columns": ["2023A", "2024E", "2025E", "2026E", "2027E"],
        "column_groups": [
            {
                "id": "historical",
                "label": "Historical Actuals",
                "columns": ["2023A"]
            },
            {
                "id": "forecast",
                "label": "Forecast Projections",
                "columns": ["2024E", "2025E", "2026E", "2027E"]
            }
        ],
        "rows": [
            {
                "id": "revenue",
                "label": "Total Revenue",
                "format": "currency",
                "note": "Actuals from SEC 10-K; forecast assumes 8% constant growth",
                "match": {
                    "2023A": { "value": [100000.0] },
                    "cols.forecast": { "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08" }
                }
            },
            {
                "id": "cogs",
                "label": "Cost of Goods Sold",
                "format": "currency",
                "expr": "IncomeStatement[\"revenue\", @] * 0.45"
            },
            {
                "id": "gross_profit",
                "label": "Gross Profit",
                "format": "currency_total",
                "expr": "IncomeStatement[\"revenue\", @] - IncomeStatement[\"cogs\", @]"
            },
            {
                "id": "tax_expense",
                "label": "Income Tax Provision",
                "format": "currency",
                "expr": "IncomeStatement[\"gross_profit\", @] * Assumptions[\"tax_rate\", \"value\"]"
            }
        ]
    }"#;

    let table: TableSpec = serde_json::from_str(json_data).unwrap();
    assert!(table.validate().is_ok());
}

#[test]
fn test_scenario_3_reject_row_with_multiple_payloads() {
    let mut table = sample_table();
    table.rows[0].value = Some(vec![ValueLiteral::Number(1.0), ValueLiteral::Number(2.0)]);
    table.rows[0].expr = Some("IncomeStatement[\"revenue\", @]".to_string());

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::MutuallyExclusivePayload { table: t, row: r }
        if t.as_str() == "TestTable" && r.as_str() == "row_1"
    ));
}

#[test]
fn test_scenario_3_reject_row_with_zero_payloads() {
    let mut table = sample_table();
    table.rows[0].expr = None;
    table.rows[0].value = None;
    table.rows[0].match_arms = None;

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::MutuallyExclusivePayload { table: t, row: r }
        if t.as_str() == "TestTable" && r.as_str() == "row_1"
    ));
}

#[test]
fn test_scenario_4_reject_non_exhaustive_match_partitioning() {
    let mut table = sample_table();
    let mut arms = IndexMap::new();
    arms.insert(
        "col_1".to_string(),
        MatchArmSpec {
            value: Some(vec![ValueLiteral::Number(100.0)]),
            expr: None,
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);

    let err = table.validate().unwrap_err();
    match err {
        ModelError::UncoveredColumns {
            table: t,
            row: r,
            missing,
        } => {
            assert_eq!(t.as_str(), "TestTable");
            assert_eq!(r.as_str(), "row_1");
            assert_eq!(missing, vec![ColId::from("col_2")]);
        }
        other => panic!("expected UncoveredColumns, got {other:?}"),
    }
}

#[test]
fn test_scenario_5_reject_overlapping_match_selectors() {
    let mut table = sample_table();
    table.columns = vec![
        ColId::from("2021A"),
        ColId::from("2022A"),
        ColId::from("2023A"),
    ];

    let mut arms = IndexMap::new();
    arms.insert(
        "2021A..2022A".to_string(),
        MatchArmSpec {
            value: Some(vec![ValueLiteral::Number(1.0), ValueLiteral::Number(2.0)]),
            expr: None,
            note: None,
            source: None,
        },
    );
    arms.insert(
        "2022A..2023A".to_string(),
        MatchArmSpec {
            value: Some(vec![ValueLiteral::Number(2.0), ValueLiteral::Number(3.0)]),
            expr: None,
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::OverlappingMatchArm { table: _, row: _, column }
        if column.as_str() == "2022A"
    ));
}

#[test]
fn test_scenario_6_reject_mismatched_value_array_length() {
    let mut table = sample_table();
    // Table has 2 columns, but value only has 1
    table.rows[0].expr = None;
    table.rows[0].value = Some(vec![ValueLiteral::Number(42.0)]);

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::DimensionMismatch {
            table: _,
            row: _,
            expected: 2,
            actual: 1
        }
    ));
}

#[test]
fn test_scenario_7_reject_non_contiguous_column_group() {
    let mut table = sample_table();
    table.columns = vec![
        ColId::from("c1"),
        ColId::from("c2"),
        ColId::from("c3"),
        ColId::from("c4"),
    ];
    table.column_groups = vec![ColumnGroup {
        id: ColGroupId::from("broken_group"),
        label: "Broken".to_string(),
        columns: vec![ColId::from("c1"), ColId::from("c3")],
    }];

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::NonContiguousColumnGroup { table: _, group }
        if group == ColGroupId::from("broken_group")
    ));
}

#[test]
fn test_scenario_7_reject_overlapping_column_groups() {
    let mut table = sample_table();
    table.columns = vec![ColId::from("c1"), ColId::from("c2"), ColId::from("c3")];
    table.column_groups = vec![
        ColumnGroup {
            id: ColGroupId::from("g1"),
            label: "Group 1".to_string(),
            columns: vec![ColId::from("c1"), ColId::from("c2")],
        },
        ColumnGroup {
            id: ColGroupId::from("g2"),
            label: "Group 2".to_string(),
            columns: vec![ColId::from("c2"), ColId::from("c3")],
        },
    ];

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::OverlappingColumnGroup { column, .. }
        if column.as_str() == "c2"
    ));
}

#[test]
fn test_scenario_8_parse_and_validate_complex_nested_row_groups() {
    let json_data = r#"{
        "id": "BalanceSheet",
        "title": "Balance Sheet",
        "columns": ["2024E"],
        "row_groups": [
            {
                "id": "assets",
                "label": "Assets",
                "children": [
                    {
                        "id": "current_assets",
                        "label": "Current Assets",
                        "children": ["cash", "accounts_receivable", "inventory"]
                    },
                    {
                        "id": "non_current_assets",
                        "label": "Non-Current Assets",
                        "children": ["ppe", "goodwill", "intangibles"]
                    }
                ]
            }
        ],
        "rows": [
            { "id": "cash", "label": "Cash", "value": [100.0] },
            { "id": "accounts_receivable", "label": "AR", "value": [50.0] },
            { "id": "inventory", "label": "Inventory", "value": [30.0] },
            { "id": "total_current_assets", "label": "Total Current Assets", "expr": "180" },
            { "id": "ppe", "label": "PPE", "value": [200.0] },
            { "id": "goodwill", "label": "Goodwill", "value": [40.0] },
            { "id": "intangibles", "label": "Intangibles", "value": [10.0] },
            { "id": "total_non_current_assets", "label": "Total Non-Current Assets", "expr": "250" },
            { "id": "total_assets", "label": "Total Assets", "expr": "430" }
        ]
    }"#;

    let table: TableSpec = serde_json::from_str(json_data).unwrap();
    // Non-contiguous because total_current_assets is between inventory and ppe!
    // Assets descendant rows: [cash, accounts_receivable, inventory, ppe, goodwill, intangibles]
    // In table.rows, total_current_assets sits between inventory and ppe, so descendant rows of 'assets' are NOT contiguous!
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::NonContiguousRowGroup { table: _, group }
        if group == RowGroupId::from("assets")
    ));
}

#[test]
fn test_valid_hierarchical_row_groups_contiguous() {
    let json_data = r#"{
        "id": "BalanceSheet",
        "title": "Balance Sheet",
        "columns": ["2024E"],
        "row_groups": [
            {
                "id": "current_assets",
                "label": "Current Assets",
                "children": ["cash", "accounts_receivable", "inventory"]
            },
            {
                "id": "non_current_assets",
                "label": "Non-Current Assets",
                "children": ["ppe", "goodwill", "intangibles"]
            }
        ],
        "rows": [
            { "id": "cash", "label": "Cash", "value": [100.0] },
            { "id": "accounts_receivable", "label": "AR", "value": [50.0] },
            { "id": "inventory", "label": "Inventory", "value": [30.0] },
            { "id": "total_current_assets", "label": "Total Current Assets", "expr": "180" },
            { "id": "ppe", "label": "PPE", "value": [200.0] },
            { "id": "goodwill", "label": "Goodwill", "value": [40.0] },
            { "id": "intangibles", "label": "Intangibles", "value": [10.0] },
            { "id": "total_non_current_assets", "label": "Total Non-Current Assets", "expr": "250" }
        ]
    }"#;

    let table: TableSpec = serde_json::from_str(json_data).unwrap();
    assert!(table.validate().is_ok());
}

#[test]
fn test_match_selector_parsing() {
    assert_eq!(
        MatchSelector::parse("2024E").unwrap(),
        MatchSelector::Single(ColId::from("2024E"))
    );
    assert_eq!(
        MatchSelector::parse("2021A..2023A").unwrap(),
        MatchSelector::Range {
            start: ColId::from("2021A"),
            end: ColId::from("2023A"),
        }
    );
    assert_eq!(
        MatchSelector::parse("cols.forecast").unwrap(),
        MatchSelector::Group(ColGroupId::from("forecast"))
    );
}

#[test]
fn test_format_presets() {
    let presets = [
        FormatPreset::Currency,
        FormatPreset::CurrencyTotal,
        FormatPreset::Percentage,
        FormatPreset::Multiple,
        FormatPreset::Integer,
        FormatPreset::Decimal,
        FormatPreset::BasisPoints,
        FormatPreset::Date,
        FormatPreset::Text,
    ];

    for preset in presets {
        let serialized = serde_json::to_string(&preset).unwrap();
        let deserialized: FormatPreset = serde_json::from_str(&serialized).unwrap();
        assert_eq!(preset, deserialized);
    }
}

#[test]
fn test_value_literal_null_and_mixed() {
    let json_data = r#"[123, 45.67, "sample", null]"#;
    let values: Vec<ValueLiteral> = serde_json::from_str(json_data).unwrap();
    assert_eq!(values.len(), 4);
    assert_eq!(values[0], ValueLiteral::Number(123.0));
    assert_eq!(values[1], ValueLiteral::Number(45.67));
    assert_eq!(values[2], ValueLiteral::String("sample".to_string()));
    assert_eq!(values[3], ValueLiteral::Null);

    let reserialized = serde_json::to_string(&values).unwrap();
    assert_eq!(reserialized, r#"[123.0,45.67,"sample",null]"#);
}

fn sample_table() -> TableSpec {
    TableSpec {
        id: TableId::from("TestTable"),
        title: "Test Table".to_string(),
        columns: vec![ColId::from("col_1"), ColId::from("col_2")],
        column_groups: Vec::new(),
        row_groups: Vec::new(),
        rows: vec![RowSpec {
            id: RowId::from("row_1"),
            label: "Row 1".to_string(),
            format: Some(FormatPreset::Decimal),
            note: None,
            source: None,
            value: None,
            expr: Some("1.0".to_string()),
            match_arms: None,
        }],
    }
}

#[test]
fn test_reject_empty_columns() {
    let mut table = sample_table();
    table.columns.clear();
    let err = table.validate().unwrap_err();
    assert!(matches!(err, ModelError::EmptyColumns { table } if table.as_str() == "TestTable"));
}

#[test]
fn test_reject_empty_rows() {
    let mut table = sample_table();
    table.rows.clear();
    let err = table.validate().unwrap_err();
    assert!(matches!(err, ModelError::EmptyRows { table } if table.as_str() == "TestTable"));
}

#[test]
fn test_reject_duplicate_column_id() {
    let mut table = sample_table();
    table.columns = vec![ColId::from("col_1"), ColId::from("col_1")];
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::DuplicateColumnId { column, .. } if column.as_str() == "col_1")
    );
}

#[test]
fn test_reject_duplicate_row_id() {
    let mut table = sample_table();
    table.rows.push(RowSpec {
        id: RowId::from("row_1"),
        label: "Row 1 Duplicate".to_string(),
        format: None,
        note: None,
        source: None,
        value: None,
        expr: Some("2.0".to_string()),
        match_arms: None,
    });
    let err = table.validate().unwrap_err();
    assert!(matches!(err, ModelError::DuplicateRowId { row, .. } if row.as_str() == "row_1"));
}

#[test]
fn test_reject_duplicate_column_group_id() {
    let mut table = sample_table();
    table.column_groups = vec![
        ColumnGroup {
            id: ColGroupId::from("cg1"),
            label: "Group 1".to_string(),
            columns: vec![ColId::from("col_1")],
        },
        ColumnGroup {
            id: ColGroupId::from("cg1"),
            label: "Group 1 Duplicate".to_string(),
            columns: vec![ColId::from("col_2")],
        },
    ];
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::DuplicateColumnGroupId { group, .. } if group.as_str() == "cg1")
    );
}

#[test]
fn test_reject_duplicate_row_group_id() {
    let mut table = sample_table();
    table.rows.push(RowSpec {
        id: RowId::from("row_2"),
        label: "Row 2".to_string(),
        format: None,
        note: None,
        source: None,
        value: None,
        expr: Some("2.0".to_string()),
        match_arms: None,
    });
    table.row_groups = vec![
        RowGroup {
            id: RowGroupId::from("rg1"),
            label: "Group 1".to_string(),
            children: vec![RowGroupChild::Row(RowId::from("row_1"))],
        },
        RowGroup {
            id: RowGroupId::from("rg1"),
            label: "Group 1 Duplicate".to_string(),
            children: vec![RowGroupChild::Row(RowId::from("row_2"))],
        },
    ];
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::DuplicateRowGroupId { group, .. } if group.as_str() == "rg1")
    );
}

#[test]
fn test_reject_unknown_column_in_column_group() {
    let mut table = sample_table();
    table.column_groups = vec![ColumnGroup {
        id: ColGroupId::from("cg1"),
        label: "Group 1".to_string(),
        columns: vec![ColId::from("col_unknown")],
    }];
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::UnknownColumn { column, .. } if column.as_str() == "col_unknown")
    );
}

#[test]
fn test_reject_empty_column_group() {
    let mut table = sample_table();
    table.column_groups = vec![ColumnGroup {
        id: ColGroupId::from("cg1"),
        label: "Empty Group".to_string(),
        columns: Vec::new(),
    }];
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::NonContiguousColumnGroup { group, .. } if group.as_str() == "cg1")
    );
}

#[test]
fn test_reject_unknown_row_in_row_group() {
    let mut table = sample_table();
    table.row_groups = vec![RowGroup {
        id: RowGroupId::from("rg1"),
        label: "Group 1".to_string(),
        children: vec![RowGroupChild::Row(RowId::from("row_unknown"))],
    }];
    let err = table.validate().unwrap_err();
    assert!(matches!(err, ModelError::UnknownRow { row, .. } if row.as_str() == "row_unknown"));
}

#[test]
fn test_reject_empty_row_group() {
    let mut table = sample_table();
    table.row_groups = vec![RowGroup {
        id: RowGroupId::from("rg1"),
        label: "Empty Group".to_string(),
        children: Vec::new(),
    }];
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::NonContiguousRowGroup { group, .. } if group.as_str() == "rg1")
    );
}

#[test]
fn test_reject_overlapping_row_groups() {
    let mut table = sample_table();
    table.row_groups = vec![
        RowGroup {
            id: RowGroupId::from("rg1"),
            label: "Group 1".to_string(),
            children: vec![RowGroupChild::Row(RowId::from("row_1"))],
        },
        RowGroup {
            id: RowGroupId::from("rg2"),
            label: "Group 2".to_string(),
            children: vec![RowGroupChild::Row(RowId::from("row_1"))],
        },
    ];
    let err = table.validate().unwrap_err();
    assert!(matches!(err, ModelError::OverlappingRowGroup { row, .. } if row.as_str() == "row_1"));
}

#[test]
fn test_reject_cyclic_row_group() {
    let mut table = sample_table();
    table.row_groups = vec![RowGroup {
        id: RowGroupId::from("rg1"),
        label: "Group 1".to_string(),
        children: vec![RowGroupChild::Group(RowGroup {
            id: RowGroupId::from("rg1"),
            label: "Group 1 Nested Duplicate".to_string(),
            children: vec![RowGroupChild::Row(RowId::from("row_1"))],
        })],
    }];
    let err = table.validate().unwrap_err();
    assert!(matches!(err, ModelError::CyclicRowGroup { group, .. } if group.as_str() == "rg1"));
}

#[test]
fn test_reject_match_arm_multiple_payloads() {
    let mut table = sample_table();
    let mut arms = IndexMap::new();
    arms.insert(
        "col_1..col_2".to_string(),
        MatchArmSpec {
            value: Some(vec![ValueLiteral::Number(1.0), ValueLiteral::Number(2.0)]),
            expr: Some("1.0".to_string()),
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::MutuallyExclusiveArmPayload { arm, .. } if arm == "col_1..col_2")
    );
}

#[test]
fn test_reject_match_arm_zero_payloads() {
    let mut table = sample_table();
    let mut arms = IndexMap::new();
    arms.insert(
        "col_1..col_2".to_string(),
        MatchArmSpec {
            value: None,
            expr: None,
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::MutuallyExclusiveArmPayload { arm, .. } if arm == "col_1..col_2")
    );
}

#[test]
fn test_reject_empty_match_arms() {
    let mut table = sample_table();
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(IndexMap::new());
    let err = table.validate().unwrap_err();
    assert!(matches!(err, ModelError::UncoveredColumns { missing, .. } if missing.len() == 2));
}

#[test]
fn test_reject_match_arm_dimension_mismatch() {
    let mut table = sample_table();
    let mut arms = IndexMap::new();
    arms.insert(
        "col_1..col_2".to_string(),
        MatchArmSpec {
            value: Some(vec![ValueLiteral::Number(1.0)]),
            expr: None,
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::DimensionMismatch {
            expected: 2,
            actual: 1,
            ..
        }
    ));
}

#[test]
fn test_reject_invalid_match_selector_inverted_range() {
    let mut table = sample_table();
    let mut arms = IndexMap::new();
    arms.insert(
        "col_2..col_1".to_string(),
        MatchArmSpec {
            expr: Some("1.0".to_string()),
            value: None,
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::InvalidMatchSelector { selector, .. } if selector == "col_2..col_1")
    );
}

#[test]
fn test_reject_invalid_match_selector_syntax() {
    let mut table = sample_table();
    let mut arms = IndexMap::new();
    arms.insert(
        "cols.".to_string(),
        MatchArmSpec {
            expr: Some("1.0".to_string()),
            value: None,
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::InvalidMatchSelector { selector, .. } if selector == "cols.")
    );
}

#[test]
fn test_reject_unknown_column_in_match_selector() {
    let mut table = sample_table();
    let mut arms = IndexMap::new();
    arms.insert(
        "col_unknown".to_string(),
        MatchArmSpec {
            expr: Some("1.0".to_string()),
            value: None,
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::UnknownColumn { column, .. } if column.as_str() == "col_unknown")
    );
}

#[test]
fn test_reject_unknown_column_group_in_match_selector() {
    let mut table = sample_table();
    let mut arms = IndexMap::new();
    arms.insert(
        "cols.unknown_group".to_string(),
        MatchArmSpec {
            expr: Some("1.0".to_string()),
            value: None,
            note: None,
            source: None,
        },
    );
    table.rows[0].expr = None;
    table.rows[0].match_arms = Some(arms);
    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::UnknownColumnGroup { group, .. } if group.as_str() == "unknown_group")
    );
}
