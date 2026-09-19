#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::model::{
    AxisRole, CellInput, ColGroupId, ColId, ColumnGroup, ColumnSpec, DetailedCellInput,
    FormatPreset, FreezePanes, ModelError, ProjectManifest, RowGroup, RowGroupChild, RowGroupId,
    RowId, RowSpec, SheetSpec, TableId, TableSpec, ValueLiteral,
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
        "columns": [
            { "id": "parameter", "role": "descriptor", "label": "Parameter" },
            { "id": "value", "role": "measure", "label": "Value" }
        ],
        "rows": [
            {
                "id": "tax_rate",
                "format": "percentage",
                "note": "Federal + blended state rate",
                "cells": {
                    "parameter": "Corporate Tax Rate",
                    "value": 0.25
                }
            },
            {
                "id": "exit_multiple",
                "format": "multiple",
                "cells": {
                    "parameter": "Exit EV / EBITDA Multiple",
                    "value": 10.0
                }
            }
        ]
    }"#;

    let table: TableSpec = serde_json::from_str(json_data).unwrap();
    assert_eq!(table.id.as_str(), "Assumptions");
    assert_eq!(table.columns.len(), 2);
    assert_eq!(table.columns[0].id.as_str(), "parameter");
    assert_eq!(table.columns[0].role, AxisRole::Descriptor);
    assert_eq!(table.columns[1].id.as_str(), "value");
    assert_eq!(table.columns[1].role, AxisRole::Measure);
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows[0].format, Some(FormatPreset::Percentage));
    assert_eq!(
        table.rows[0].cells.get(&ColId::from("value")),
        Some(&CellInput::number(0.25))
    );
    assert!(table.validate().is_ok());
}

#[test]
fn test_scenario_2_deserialize_and_validate_income_statement_example() {
    let json_data = r#"{
        "id": "IncomeStatement",
        "title": "Income Statement",
        "columns": [
            { "id": "line_item", "role": "descriptor", "label": "Line Item" },
            { "id": "2023A", "label": "2023A" },
            { "id": "2024E", "label": "2024E" },
            { "id": "2025E", "label": "2025E" },
            { "id": "2026E", "label": "2026E" },
            { "id": "2027E", "label": "2027E" }
        ],
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
                "format": "currency",
                "note": "Actuals from SEC 10-K; forecast assumes 8% constant growth",
                "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08",
                "cells": {
                    "line_item": "Total Revenue",
                    "2023A": 100000.0
                }
            },
            {
                "id": "cogs",
                "format": "currency",
                "expr": "IncomeStatement[\"revenue\", @col] * 0.45",
                "cells": {
                    "line_item": "Cost of Goods Sold"
                }
            },
            {
                "id": "gross_profit",
                "format": "currency",
                "expr": "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
                "cells": {
                    "line_item": "Gross Profit"
                }
            },
            {
                "id": "tax_expense",
                "format": "currency",
                "expr": "IncomeStatement[\"gross_profit\", @col] * Assumptions[\"tax_rate\", \"value\"]",
                "cells": {
                    "line_item": "Income Tax Provision"
                }
            }
        ]
    }"#;

    let table: TableSpec = serde_json::from_str(json_data).unwrap();
    assert_eq!(table.columns.len(), 6);
    assert_eq!(table.columns[0].role, AxisRole::Descriptor);
    assert_eq!(table.columns[1].role, AxisRole::Measure);
    assert_eq!(table.rows.len(), 4);
    assert!(table.validate().is_ok());
}

#[test]
fn test_reject_row_specifying_unknown_cell_column() {
    let mut table = sample_table();
    table.rows[0]
        .cells
        .insert(ColId::from("nonexistent_col"), CellInput::number(1.0));

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::UnknownCellColumn { table: t, row: r, column: c }
        if t.as_str() == "TestTable" && r.as_str() == "row_1" && c.as_str() == "nonexistent_col"
    ));
}

#[test]
fn test_reject_detailed_cell_with_mutually_exclusive_payload() {
    let mut table = sample_table();
    table.rows[0].cells.insert(
        ColId::from("col_1"),
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(100.0)),
            expr: Some("1 + 1".to_string()),
            ..Default::default()
        }),
    );

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::MutuallyExclusiveCellPayload { table: t, row: r, column: c }
        if t.as_str() == "TestTable" && r.as_str() == "row_1" && c.as_str() == "col_1"
    ));
}

#[test]
fn test_reject_detailed_cell_with_empty_payload() {
    let mut table = sample_table();
    table.rows[0].cells.insert(
        ColId::from("col_1"),
        CellInput::Detailed(DetailedCellInput {
            value: None,
            expr: None,
            format: Some(FormatPreset::Currency),
            ..Default::default()
        }),
    );

    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::EmptyCellPayload { table: t, row: r, column: c }
        if t.as_str() == "TestTable" && r.as_str() == "row_1" && c.as_str() == "col_1"
    ));
}

#[test]
fn test_scenario_7_reject_non_contiguous_column_group() {
    let mut table = sample_table();
    table.columns = vec![
        ColumnSpec::new("c1"),
        ColumnSpec::new("c2"),
        ColumnSpec::new("c3"),
    ];
    table.column_groups = vec![ColumnGroup {
        id: ColGroupId::from("cg1"),
        label: "Group 1".to_string(),
        columns: vec![ColId::from("c1"), ColId::from("c3")],
    }];

    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::NonContiguousColumnGroup { group, .. } if group.as_str() == "cg1")
    );
}

#[test]
fn test_scenario_7_reject_overlapping_column_groups() {
    let mut table = sample_table();
    table.columns = vec![
        ColumnSpec::new("c1"),
        ColumnSpec::new("c2"),
        ColumnSpec::new("c3"),
    ];
    table.column_groups = vec![
        ColumnGroup {
            id: ColGroupId::from("cg1"),
            label: "Group 1".to_string(),
            columns: vec![ColId::from("c1"), ColId::from("c2")],
        },
        ColumnGroup {
            id: ColGroupId::from("cg2"),
            label: "Group 2".to_string(),
            columns: vec![ColId::from("c2"), ColId::from("c3")],
        },
    ];

    let err = table.validate().unwrap_err();
    assert!(
        matches!(err, ModelError::OverlappingColumnGroup { column, .. } if column.as_str() == "c2")
    );
}

#[test]
fn test_scenario_8_parse_and_validate_complex_nested_row_groups() {
    let json_data = r#"{
        "id": "IncomeStatement",
        "title": "Income Statement",
        "columns": [
            { "id": "2024E" },
            { "id": "2025E" }
        ],
        "row_groups": [
            {
                "id": "operating_expenses",
                "label": "Operating Expenses",
                "children": [
                    {
                        "id": "r_and_d_group",
                        "label": "Research & Development",
                        "children": ["salaries", "lab_supplies"]
                    },
                    "s_and_m",
                    "g_and_a"
                ]
            }
        ],
        "rows": [
            { "id": "salaries", "expr": "100" },
            { "id": "lab_supplies", "expr": "20" },
            { "id": "s_and_m", "expr": "50" },
            { "id": "g_and_a", "expr": "30" }
        ]
    }"#;

    let table: TableSpec = serde_json::from_str(json_data).unwrap();
    assert_eq!(table.row_groups.len(), 1);
    assert_eq!(table.rows.len(), 4);
    assert!(table.validate().is_ok());
}

#[test]
fn test_valid_hierarchical_row_groups_contiguous() {
    let mut table = sample_table();
    table.rows = vec![
        RowSpec {
            id: RowId::from("r1"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: Some("1.0".to_string()),
            cells: IndexMap::new(),
        },
        RowSpec {
            id: RowId::from("r2"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: Some("2.0".to_string()),
            cells: IndexMap::new(),
        },
        RowSpec {
            id: RowId::from("r3"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: Some("3.0".to_string()),
            cells: IndexMap::new(),
        },
    ];

    table.row_groups = vec![RowGroup {
        id: RowGroupId::from("rg_root"),
        label: "Root Group".to_string(),
        children: vec![
            RowGroupChild::Row(RowId::from("r1")),
            RowGroupChild::Group(RowGroup {
                id: RowGroupId::from("rg_nested"),
                label: "Nested Group".to_string(),
                children: vec![
                    RowGroupChild::Row(RowId::from("r2")),
                    RowGroupChild::Row(RowId::from("r3")),
                ],
            }),
        ],
    }];

    assert!(table.validate().is_ok());
}

#[test]
fn test_format_presets() {
    let presets = [
        FormatPreset::Currency,
        FormatPreset::Number,
        FormatPreset::Percentage,
        FormatPreset::Multiple,
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
    let json_data = r#"[123.0, 45.67, "sample", true, null]"#;
    let values: Vec<ValueLiteral> = serde_json::from_str(json_data).unwrap();
    assert_eq!(values.len(), 5);
    assert_eq!(values[0], ValueLiteral::Number(123.0));
    assert_eq!(values[1], ValueLiteral::Number(45.67));
    assert_eq!(values[2], ValueLiteral::String("sample".to_string()));
    assert_eq!(values[3], ValueLiteral::Boolean(true));
    assert_eq!(values[4], ValueLiteral::Null);

    let reserialized = serde_json::to_string(&values).unwrap();
    assert_eq!(reserialized, r#"[123.0,45.67,"sample",true,null]"#);
}

fn sample_table() -> TableSpec {
    TableSpec {
        id: TableId::from("TestTable"),
        title: "Test Table".to_string(),
        columns: vec![ColumnSpec::new("col_1"), ColumnSpec::new("col_2")],
        column_groups: Vec::new(),
        row_groups: Vec::new(),
        rows: vec![RowSpec {
            id: RowId::from("row_1"),
            role: AxisRole::Measure,
            format: Some(FormatPreset::Number),
            note: None,
            source: None,
            expr: Some("1.0".to_string()),
            cells: IndexMap::new(),
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
    table.columns = vec![ColumnSpec::new("col_1"), ColumnSpec::new("col_1")];
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
        role: AxisRole::Measure,
        format: None,
        note: None,
        source: None,
        expr: Some("2.0".to_string()),
        cells: IndexMap::new(),
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
        role: AxisRole::Measure,
        format: None,
        note: None,
        source: None,
        expr: Some("2.0".to_string()),
        cells: IndexMap::new(),
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
