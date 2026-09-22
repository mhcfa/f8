#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::path::PathBuf;

use f8_core::model::{
    AxisRole, CellInput, ColId, ColumnSpec, CurrencyCode, DetailedCellInput, FormatPreset,
    FreezePanes, ModelError, ProjectManifest, RowSpec, SectionHeaderRowSpec, SemanticStyle,
    SheetSpec, TableId, TableSpec, ValueLiteral,
};

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
            { "kind": "data", "id": "parameter", "role": "descriptor", "headers": ["Parameter"] },
            { "kind": "data", "id": "value", "role": "measure", "headers": ["Value"] }
        ],
        "rows": [
            {
                "kind": "data",
                "id": "tax_rate",
                "format": "percentage",
                "note": "Federal + blended state rate",
                "cells": {
                    "parameter": "Corporate Tax Rate",
                    "value": 0.25
                }
            },
            {
                "kind": "data",
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
    assert_eq!(table.columns[0].id().map(|i| i.as_str()), Some("parameter"));
    assert_eq!(table.columns[0].role(), Some(AxisRole::Descriptor));
    assert_eq!(table.columns[1].id().map(|i| i.as_str()), Some("value"));
    assert_eq!(table.columns[1].role(), Some(AxisRole::Measure));
    assert_eq!(table.rows.len(), 2);
    if let RowSpec::Data(d) = &table.rows[0] {
        assert_eq!(d.format, Some(FormatPreset::Percentage));
        assert_eq!(
            d.cells.get(&ColId::from("value")),
            Some(&CellInput::number(0.25))
        );
    } else {
        panic!("expected Data row");
    }
    assert!(table.validate().is_ok());
}

#[test]
fn test_scenario_2_deserialize_and_validate_income_statement_example() {
    let json_data = r#"{
        "id": "IncomeStatement",
        "title": "Income Statement",
        "columns": [
            { "kind": "data", "id": "line_item", "role": "descriptor", "headers": ["Line Item"] },
            { "kind": "data", "id": "2023A", "headers": ["Historical Actuals", "2023A"] },
            { "kind": "data", "id": "2024E", "headers": ["Forecast Projections", "2024E"] },
            { "kind": "data", "id": "2025E", "headers": ["Forecast Projections", "2025E"] },
            { "kind": "data", "id": "2026E", "headers": ["Forecast Projections", "2026E"] },
            { "kind": "data", "id": "2027E", "headers": ["Forecast Projections", "2027E"] }
        ],
        "rows": [
            {
                "kind": "data",
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
                "kind": "data",
                "id": "cogs",
                "format": "currency",
                "expr": "IncomeStatement[\"revenue\", @col] * 0.45",
                "cells": {
                    "line_item": "Cost of Goods Sold"
                }
            },
            {
                "kind": "data",
                "id": "gross_profit",
                "format": "currency",
                "expr": "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
                "cells": {
                    "line_item": "Gross Profit"
                }
            },
            {
                "kind": "data",
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
    assert_eq!(table.columns[0].role(), Some(AxisRole::Descriptor));
    assert_eq!(table.columns[1].role(), Some(AxisRole::Measure));
    assert_eq!(table.rows.len(), 4);
    assert!(table.validate().is_ok());
}

#[test]
fn test_reject_row_specifying_unknown_cell_column() {
    let mut table = sample_table();
    table.rows[0] = table.rows[0]
        .clone()
        .with_cell("nonexistent_col", CellInput::number(1.0));

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
    table.rows[0] = table.rows[0].clone().with_cell(
        "col_1",
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
    table.rows[0] = table.rows[0].clone().with_cell(
        "col_1",
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
fn test_reject_indent_on_measure_cell() {
    let mut table = sample_table();
    table.columns = vec![
        ColumnSpec::descriptor("desc", "Description"),
        ColumnSpec::new("c1"),
    ];
    table.rows = vec![RowSpec::new("r1").with_cell(
        "c1",
        DetailedCellInput {
            value: Some(ValueLiteral::Number(100.0)),
            indent: Some(1),
            ..Default::default()
        },
    )];
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidIndentOnMeasureCell {
            table: t,
            row: r,
            column: c,
            indent: 1,
        }
        if t.as_str() == "TestTable" && r.as_str() == "r1" && c.as_str() == "c1"
    ));
}

#[test]
fn test_reject_primary_descriptor_deserialization() {
    let json = r#""primary_descriptor""#;
    assert!(serde_json::from_str::<AxisRole>(json).is_err());
}

#[test]
fn test_multi_tier_column_headers_serde() {
    let col = ColumnSpec::new("2024E").with_headers(["Forecast", "2024E"]);
    let json = serde_json::to_string(&col).unwrap();
    let deserialized: ColumnSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(col, deserialized);
    assert_eq!(
        deserialized.headers(),
        Some(&["Forecast".to_string(), "2024E".to_string()][..])
    );
}

#[test]
fn test_tagged_spec_enums_serde() {
    let gutter_col = ColumnSpec::gutter();
    let json_gutter_col = serde_json::to_string(&gutter_col).unwrap();
    assert_eq!(json_gutter_col, r#"{"kind":"gutter"}"#);
    let deser_col: ColumnSpec = serde_json::from_str(&json_gutter_col).unwrap();
    assert_eq!(gutter_col, deser_col);

    let sec_header: RowSpec = SectionHeaderRowSpec::new("Operating Expenses")
        .with_indent(Some(1))
        .into();
    let json_sec = serde_json::to_string(&sec_header).unwrap();
    assert!(json_sec.contains(r#""kind":"section_header""#));
    let deser_row: RowSpec = serde_json::from_str(&json_sec).unwrap();
    assert_eq!(sec_header, deser_row);

    let gutter_row = RowSpec::gutter();
    let json_gutter_row = serde_json::to_string(&gutter_row).unwrap();
    assert_eq!(json_gutter_row, r#"{"kind":"gutter"}"#);
    let deser_grow: RowSpec = serde_json::from_str(&json_gutter_row).unwrap();
    assert_eq!(gutter_row, deser_grow);
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
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![ColumnSpec::new("col_1"), ColumnSpec::new("col_2")],
        rows: vec![
            RowSpec::new("row_1")
                .with_format(Some(FormatPreset::Number))
                .with_expr("1.0"),
        ],
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
    table.rows.push(RowSpec::new("row_1").with_expr("2.0"));
    let err = table.validate().unwrap_err();
    assert!(matches!(err, ModelError::DuplicateRowId { row, .. } if row.as_str() == "row_1"));
}

#[test]
fn test_semantic_style_serde() {
    let data: SemanticStyle = serde_json::from_str(r#""data""#).unwrap();
    assert_eq!(data, SemanticStyle::Data);

    let subtotal: SemanticStyle = serde_json::from_str(r#""subtotal""#).unwrap();
    assert_eq!(subtotal, SemanticStyle::Subtotal);

    let total: SemanticStyle = serde_json::from_str(r#""total""#).unwrap();
    assert_eq!(total, SemanticStyle::Total);

    assert_eq!(
        serde_json::to_string(&SemanticStyle::Data).unwrap(),
        r#""data""#
    );
    assert_eq!(
        serde_json::to_string(&SemanticStyle::Subtotal).unwrap(),
        r#""subtotal""#
    );
    assert_eq!(
        serde_json::to_string(&SemanticStyle::Total).unwrap(),
        r#""total""#
    );
}

#[test]
fn test_scenario_5_reject_non_numeric_preset_with_decimals_column() {
    let mut table = sample_table();
    table.columns[0] = table.columns[0]
        .clone()
        .with_format(Some(FormatPreset::Date))
        .with_decimals(Some(2));

    let err = table.validate().unwrap_err();
    match err {
        ModelError::InvalidPrecisionForPreset {
            context,
            preset,
            decimals,
            ..
        } => {
            assert!(context.contains("column 'col_1'"));
            assert_eq!(preset, FormatPreset::Date);
            assert_eq!(decimals, 2);
        }
        other => panic!("expected InvalidPrecisionForPreset, got {other:?}"),
    }
}

#[test]
fn test_scenario_5_reject_non_numeric_preset_with_decimals_row() {
    let mut table = sample_table();
    table.rows[0] = table.rows[0]
        .clone()
        .with_format(Some(FormatPreset::Text))
        .with_decimals(Some(0));

    let err = table.validate().unwrap_err();
    match err {
        ModelError::InvalidPrecisionForPreset {
            context,
            preset,
            decimals,
            ..
        } => {
            assert!(context.contains("row 'row_1'"));
            assert_eq!(preset, FormatPreset::Text);
            assert_eq!(decimals, 0);
        }
        other => panic!("expected InvalidPrecisionForPreset, got {other:?}"),
    }
}

#[test]
fn test_scenario_5_reject_non_numeric_preset_with_decimals_cell() {
    let mut table = sample_table();
    table.rows[0] = table.rows[0].clone().with_cell(
        "col_1",
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::String("abc".to_string())),
            expr: None,
            format: Some(FormatPreset::Text),
            decimals: Some(2),
            style: None,
            highlight: None,
            note: None,
            source: None,
            currency: None,
            indent: None,
        }),
    );

    let err = table.validate().unwrap_err();
    match err {
        ModelError::InvalidPrecisionForPreset {
            context,
            preset,
            decimals,
            ..
        } => {
            assert!(context.contains("row 'row_1', column 'col_1'"));
            assert_eq!(preset, FormatPreset::Text);
            assert_eq!(decimals, 2);
        }
        other => panic!("expected InvalidPrecisionForPreset, got {other:?}"),
    }
}

#[test]
fn test_table_with_semantic_formatting_json_deserialization() {
    let json_data = r#"{
        "id": "FormattingTest",
        "title": "Formatting Test Table",
        "columns": [
            { "kind": "data", "id": "q1", "role": "measure", "style": "subtotal", "highlight": true, "decimals": 1 }
        ],
        "rows": [
            {
                "kind": "data",
                "id": "net_income",
                "role": "measure",
                "style": "total",
                "highlight": true,
                "decimals": 2,
                "cells": {
                    "q1": {
                        "value": 123.45,
                        "style": "data",
                        "highlight": false,
                        "decimals": 4
                    }
                }
            }
        ]
    }"#;

    let table: TableSpec = serde_json::from_str(json_data).unwrap();
    if let ColumnSpec::Data(c) = &table.columns[0] {
        assert_eq!(c.style, Some(SemanticStyle::Subtotal));
        assert_eq!(c.highlight, Some(true));
        assert_eq!(c.decimals, Some(1));
    } else {
        panic!("expected Data column");
    }

    if let RowSpec::Data(r) = &table.rows[0] {
        assert_eq!(r.style, Some(SemanticStyle::Total));
        assert_eq!(r.highlight, Some(true));
        assert_eq!(r.decimals, Some(2));

        let cell = r.cells.get(&ColId::from("q1")).unwrap();
        assert_eq!(cell.style(), Some(SemanticStyle::Data));
        assert_eq!(cell.highlight(), Some(false));
        assert_eq!(cell.decimals(), Some(4));
    } else {
        panic!("expected Data row");
    }
    assert!(table.validate().is_ok());
}

#[test]
fn test_currency_code_metadata_and_excel_escaping() {
    assert_eq!(CurrencyCode::Usd.symbol(), "$");
    assert_eq!(CurrencyCode::Usd.excel_symbol(), "$");
    assert_eq!(CurrencyCode::Eur.symbol(), "€");
    assert_eq!(CurrencyCode::Eur.excel_symbol(), "€");
    assert_eq!(CurrencyCode::Gbp.symbol(), "£");
    assert_eq!(CurrencyCode::Gbp.excel_symbol(), "£");
    assert_eq!(CurrencyCode::Jpy.symbol(), "¥");
    assert_eq!(CurrencyCode::Jpy.excel_symbol(), "¥");
    assert_eq!(CurrencyCode::Cny.symbol(), "¥");
    assert_eq!(CurrencyCode::Cny.excel_symbol(), "¥");
    assert_eq!(CurrencyCode::Chf.symbol(), "CHF");
    assert_eq!(CurrencyCode::Chf.excel_symbol(), "\"CHF\"");
    assert_eq!(CurrencyCode::Cad.symbol(), "C$");
    assert_eq!(CurrencyCode::Cad.excel_symbol(), "\"C$\"");
    assert_eq!(CurrencyCode::Aud.symbol(), "A$");
    assert_eq!(CurrencyCode::Aud.excel_symbol(), "\"A$\"");
    assert_eq!(CurrencyCode::Hkd.symbol(), "HK$");
    assert_eq!(CurrencyCode::Hkd.excel_symbol(), "\"HK$\"");
    assert_eq!(CurrencyCode::Sgd.symbol(), "S$");
    assert_eq!(CurrencyCode::Sgd.excel_symbol(), "\"S$\"");
    assert_eq!(CurrencyCode::Sek.symbol(), "kr");
    assert_eq!(CurrencyCode::Sek.excel_symbol(), "\"kr\"");
    assert_eq!(CurrencyCode::Nok.symbol(), "kr");
    assert_eq!(CurrencyCode::Nok.excel_symbol(), "\"kr\"");
    assert_eq!(CurrencyCode::Nzd.symbol(), "NZ$");
    assert_eq!(CurrencyCode::Nzd.excel_symbol(), "\"NZ$\"");
    assert_eq!(CurrencyCode::Inr.symbol(), "₹");
    assert_eq!(CurrencyCode::Inr.excel_symbol(), "\"₹\"");
    assert_eq!(CurrencyCode::Brl.symbol(), "R$");
    assert_eq!(CurrencyCode::Brl.excel_symbol(), "\"R$\"");
}

#[test]
fn test_scenario_8_reject_currency_on_non_currency_presets() {
    // Column rejection
    let mut table = sample_table();
    table.columns[0] = table.columns[0]
        .clone()
        .with_format(Some(FormatPreset::Percentage))
        .with_currency(Some(CurrencyCode::Usd));
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidCurrencyForPreset {
            preset: FormatPreset::Percentage,
            currency: CurrencyCode::Usd,
            ..
        }
    ));

    // Row rejection
    let mut table = sample_table();
    table.rows[0] = table.rows[0]
        .clone()
        .with_format(Some(FormatPreset::Date))
        .with_currency(Some(CurrencyCode::Eur));
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidCurrencyForPreset {
            preset: FormatPreset::Date,
            currency: CurrencyCode::Eur,
            ..
        }
    ));

    // Cell rejection
    let mut table = sample_table();
    table.rows[0] = table.rows[0].clone().with_cell(
        "col_1",
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(10.0)),
            format: Some(FormatPreset::Number),
            currency: Some(CurrencyCode::Gbp),
            ..Default::default()
        }),
    );
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidCurrencyForPreset {
            preset: FormatPreset::Number,
            currency: CurrencyCode::Gbp,
            ..
        }
    ));
}

#[test]
fn test_scenario_9_reject_invalid_currency_serde() {
    let json_data = r#"{
        "id": "Test",
        "title": "Test",
        "currency": "bitcoin",
        "columns": [{ "kind": "data", "id": "c1" }],
        "rows": [{ "kind": "data", "id": "r1" }]
    }"#;
    let err = serde_json::from_str::<TableSpec>(json_data).unwrap_err();
    assert!(err.to_string().contains("unknown variant"));
}

#[test]
fn test_scenario_11_table_subtitle_validation() {
    // Empty subtitle
    let mut table = sample_table();
    table.subtitle = Some("".to_string());
    assert!(matches!(
        table.validate().unwrap_err(),
        ModelError::EmptyTableSubtitle { .. }
    ));

    // Whitespace subtitle
    let mut table = sample_table();
    table.subtitle = Some("   \t\n  ".to_string());
    assert!(matches!(
        table.validate().unwrap_err(),
        ModelError::EmptyTableSubtitle { .. }
    ));

    // Valid subtitle
    let mut table = sample_table();
    table.subtitle = Some("($ in thousands)".to_string());
    assert!(table.validate().is_ok());
}

#[test]
fn test_scenario_12_reject_precision_exceeding_max_bounds() {
    // Column with decimals > 14
    let mut table = sample_table();
    table.columns[0] = table.columns[0].clone().with_decimals(Some(15));
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::PrecisionOutOfBounds {
            decimals: 15,
            max: 14,
            ..
        }
    ));

    // Row with decimals > 14
    let mut table = sample_table();
    table.rows[0] = table.rows[0].clone().with_decimals(Some(18));
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::PrecisionOutOfBounds {
            decimals: 18,
            max: 14,
            ..
        }
    ));

    // Cell with decimals > 14
    let mut table = sample_table();
    table.rows[0] = table.rows[0].clone().with_cell(
        "col_1",
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(1.23)),
            decimals: Some(15),
            ..Default::default()
        }),
    );
    let err = table.validate().unwrap_err();
    assert!(matches!(
        err,
        ModelError::PrecisionOutOfBounds {
            decimals: 15,
            max: 14,
            ..
        }
    ));
}

#[test]
fn test_scenario_13_table_collapsible_serde() {
    // 1. Default table has collapsible == false and omits field from JSON
    let table = TableSpec::new("T1", "Table 1");
    assert!(!table.collapsible);

    let serialized = serde_json::to_string(&table).unwrap();
    assert!(
        !serialized.contains("collapsible"),
        "collapsible: false should be skipped during serialization"
    );

    // 2. Deserializing JSON without collapsible defaults to false
    let deserialized: TableSpec = serde_json::from_str(&serialized).unwrap();
    assert!(!deserialized.collapsible);

    // 3. Builder method with_collapsible(true) sets field
    let collapsible_table = table.with_collapsible(true);
    assert!(collapsible_table.collapsible);

    // 4. Serializing collapsible: true includes the field
    let coll_json = serde_json::to_string(&collapsible_table).unwrap();
    assert!(
        coll_json.contains(r#""collapsible":true"#),
        "collapsible: true must be present in JSON"
    );

    // 5. Deserializing JSON with "collapsible": true yields collapsible == true
    let deserialized_coll: TableSpec = serde_json::from_str(&coll_json).unwrap();
    assert!(deserialized_coll.collapsible);
}
