#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::model::{
    AxisRole, CellInput, ColId, ColumnSpec, CurrencyCode, DetailedCellInput, FormatPreset,
    FreezePanes, ModelError, ProjectManifest, RowSpec, SectionHeaderRowSpec, SemanticStyle,
    SheetSpec, TableId, TableSpec, ThemeSpec, ValueLiteral,
};

#[test]
fn test_scenario_1_deserialize_valid_manifest() {
    let json_data = r##"{
        "model_name": "LBO_Simple",
        "theme": "corporate_navy",
        "sheets": [
            {
                "name": "Summary",
                "tab_color": "#00356B",
                "tables": ["Assumptions"]
            },
            {
                "name": "Model",
                "tab_color": "#228B22",
                "freeze_panes": { "row": 2, "col": 1 },
                "show_gridlines": true,
                "tables": ["IncomeStatement"]
            }
        ]
    }"##;

    let manifest: ProjectManifest = serde_json::from_str(json_data).unwrap();
    assert_eq!(manifest.model_name, "LBO_Simple");
    assert_eq!(
        manifest.theme,
        ThemeSpec::Name("corporate_navy".to_string())
    );
    assert_eq!(manifest.sheets.len(), 2);
    assert_eq!(manifest.sheets[0].name, "Summary");
    assert_eq!(
        manifest.sheets[0].tables,
        vec![TableId::from("Assumptions")]
    );
    assert!(!manifest.sheets[0].show_gridlines);
    assert!(manifest.sheets[1].show_gridlines);
    assert_eq!(
        manifest.sheets[1].freeze_panes,
        Some(FreezePanes { row: 2, col: 1 })
    );
    assert_eq!(
        manifest.sheets[1].tables,
        vec![TableId::from("IncomeStatement")]
    );
    assert!(manifest.validate().is_ok());
}

#[test]
fn test_manifest_duplicate_sheet_name_rejected() {
    let manifest = ProjectManifest {
        model_name: "Test".to_string(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                layout: Default::default(),
                tables: vec![TableId::from("a")],
            },
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                layout: Default::default(),
                tables: vec![TableId::from("b")],
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
                "cells": {
                    "parameter": { "value": "Corporate Tax Rate", "note": "Federal + blended state rate" },
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
            d.cells.get(&ColId::from("parameter")),
            Some(&CellInput::Detailed(DetailedCellInput {
                value: Some(ValueLiteral::String("Corporate Tax Rate".to_string())),
                note: Some("Federal + blended state rate".to_string()),
                ..Default::default()
            }))
        );
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
                "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08",
                "cells": {
                    "line_item": { "value": "Total Revenue", "note": "Actuals from SEC 10-K; forecast assumes 8% constant growth" },
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
    let col = ColumnSpec::new("2024E")
        .with_group("Forecast")
        .with_headers(["Year 1", "2024E"]);
    let json = serde_json::to_string(&col).unwrap();
    let deserialized: ColumnSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(col, deserialized);
    assert_eq!(deserialized.group(), Some("Forecast"));
    assert_eq!(
        deserialized.headers(),
        Some(&["Year 1".to_string(), "2024E".to_string()][..])
    );
    assert_eq!(deserialized.header_depth(), 3);
    assert_eq!(deserialized.max_header_chars(), 6);
    assert_eq!(
        ColumnSpec::new("c_group_only")
            .with_group("Summary")
            .header_depth(),
        2
    );
    assert_eq!(ColumnSpec::gutter().header_depth(), 0);
    assert_eq!(ColumnSpec::gutter().max_header_chars(), 0);
}

#[test]
fn test_reject_empty_or_whitespace_column_group() {
    for invalid_group in ["", "   "] {
        let mut table = TableSpec::new("TestTable", "Title");
        table.columns = vec![ColumnSpec::new("c1").with_group(invalid_group)];
        table.rows = vec![RowSpec::new("r1")];
        let err = table.validate().unwrap_err();
        assert!(matches!(
            err,
            ModelError::EmptyColumnGroup { table: t, column: c }
            if t.as_str() == "TestTable" && c.as_str() == "c1"
        ));
    }
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
        subtitles: Vec::new(),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
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
    assert!(data.is_data());
    assert!(!data.uses_bold_font());

    let bold: SemanticStyle = serde_json::from_str(r#""bold""#).unwrap();
    assert_eq!(bold, SemanticStyle::Bold);
    assert!(bold.is_bold());
    assert!(bold.uses_bold_font());

    let subtotal: SemanticStyle = serde_json::from_str(r#""subtotal""#).unwrap();
    assert_eq!(subtotal, SemanticStyle::Subtotal);
    assert!(subtotal.is_subtotal());
    assert!(subtotal.uses_bold_font());

    let total: SemanticStyle = serde_json::from_str(r#""total""#).unwrap();
    assert_eq!(total, SemanticStyle::Total);
    assert!(total.is_total());
    assert!(total.uses_bold_font());

    assert_eq!(
        serde_json::to_string(&SemanticStyle::Data).unwrap(),
        r#""data""#
    );
    assert_eq!(
        serde_json::to_string(&SemanticStyle::Bold).unwrap(),
        r#""bold""#
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
    let ModelError::InvalidPrecisionForPreset {
        context,
        preset,
        decimals,
        ..
    } = err
    else {
        panic!("expected InvalidPrecisionForPreset, got {err:?}");
    };
    assert!(context.contains("column 'col_1'"));
    assert_eq!(preset, FormatPreset::Date);
    assert_eq!(decimals, 2);
}

#[test]
fn test_scenario_5_reject_non_numeric_preset_with_decimals_row() {
    let mut table = sample_table();
    table.rows[0] = table.rows[0]
        .clone()
        .with_format(Some(FormatPreset::Text))
        .with_decimals(Some(0));

    let err = table.validate().unwrap_err();
    let ModelError::InvalidPrecisionForPreset {
        context,
        preset,
        decimals,
        ..
    } = err
    else {
        panic!("expected InvalidPrecisionForPreset, got {err:?}");
    };
    assert!(context.contains("row 'row_1'"));
    assert_eq!(preset, FormatPreset::Text);
    assert_eq!(decimals, 0);

    table.rows[0] = table.rows[0]
        .clone()
        .with_format(Some(FormatPreset::Integer))
        .with_decimals(Some(0));
    let err = table.validate().unwrap_err();
    let ModelError::InvalidPrecisionForPreset {
        context,
        preset,
        decimals,
        ..
    } = err
    else {
        panic!("expected InvalidPrecisionForPreset, got {err:?}");
    };
    assert!(context.contains("row 'row_1'"));
    assert_eq!(preset, FormatPreset::Integer);
    assert_eq!(decimals, 0);
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
            shade: None,
            note: None,
            currency: None,
            indent: None,
        }),
    );

    let err = table.validate().unwrap_err();
    let ModelError::InvalidPrecisionForPreset {
        context,
        preset,
        decimals,
        ..
    } = err
    else {
        panic!("expected InvalidPrecisionForPreset, got {err:?}");
    };
    assert!(context.contains("row 'row_1', column 'col_1'"));
    assert_eq!(preset, FormatPreset::Text);
    assert_eq!(decimals, 2);
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
    let table = sample_table().with_subtitle("");
    assert!(matches!(
        table.validate().unwrap_err(),
        ModelError::EmptyTableSubtitle { .. }
    ));

    // Whitespace subtitle
    let table = sample_table().with_subtitle("   \t\n  ");
    assert!(matches!(
        table.validate().unwrap_err(),
        ModelError::EmptyTableSubtitle { .. }
    ));

    // Multi-line subtitle with empty/whitespace line
    let table = sample_table().with_subtitles(["Valid line", "   "]);
    assert!(matches!(
        table.validate().unwrap_err(),
        ModelError::EmptyTableSubtitle { .. }
    ));

    // Valid single and multi-line subtitles
    let table = sample_table().with_subtitle("($ in thousands)");
    assert!(table.validate().is_ok());
    assert_eq!(table.subtitles, vec!["($ in thousands)".to_string()]);

    let table_multi = sample_table().with_subtitles(["($ in Millions)", "Audited FY22-FY24"]);
    assert!(table_multi.validate().is_ok());
    assert_eq!(
        table_multi.subtitles,
        vec![
            "($ in Millions)".to_string(),
            "Audited FY22-FY24".to_string()
        ]
    );

    // Empty subtitles omitted when serialized
    let empty_json = serde_json::to_value(sample_table()).unwrap();
    assert!(empty_json.get("subtitles").is_none());
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

#[test]
fn test_scenario_14_table_placement_serde() {
    use f8_core::model::{DEFAULT_HORIZONTAL_TABLE_GAP, DEFAULT_VERTICAL_TABLE_GAP, PlacementSpec};

    // 1. Default table has placement == Stack { gap: 2 } and omits "placement" from JSON
    let table = TableSpec::new("T1", "Table 1");
    assert_eq!(table.placement, PlacementSpec::stack());
    assert!(table.placement.is_default());

    let serialized = serde_json::to_string(&table).unwrap();
    assert!(
        !serialized.contains("placement"),
        "default placement should be skipped during serialization"
    );

    let deserialized: TableSpec = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.placement, PlacementSpec::stack());

    // 2. Custom stack gap serializes and deserializes
    let stack_gap_table = table
        .clone()
        .with_placement(PlacementSpec::stack_with_gap(4));
    assert!(!stack_gap_table.placement.is_default());
    let stack_json = serde_json::to_string(&stack_gap_table).unwrap();
    assert!(stack_json.contains(r#""placement":{"kind":"stack","gap":4}"#));
    let deser_stack: TableSpec = serde_json::from_str(&stack_json).unwrap();
    assert_eq!(deser_stack.placement, PlacementSpec::stack_with_gap(4));

    // 3. Below placement with default and explicit gap
    let below_default: PlacementSpec = serde_json::from_str(r#"{"kind":"below","target":"T1"}"#)
        .expect("deserialize below with default gap");
    assert_eq!(
        below_default,
        PlacementSpec::below_with_gap("T1", DEFAULT_VERTICAL_TABLE_GAP)
    );
    assert_eq!(below_default, PlacementSpec::below("T1"));

    let below_custom =
        TableSpec::new("T2", "Table 2").with_placement(PlacementSpec::below_with_gap("T1", 0));
    let below_json = serde_json::to_string(&below_custom).unwrap();
    let deser_below: TableSpec = serde_json::from_str(&below_json).unwrap();
    assert_eq!(
        deser_below.placement,
        PlacementSpec::below_with_gap("T1", 0)
    );

    // 4. RightOf placement with default and explicit gap
    let right_default: PlacementSpec = serde_json::from_str(r#"{"kind":"right_of","target":"T1"}"#)
        .expect("deserialize right_of with default gap");
    assert_eq!(
        right_default,
        PlacementSpec::right_of_with_gap("T1", DEFAULT_HORIZONTAL_TABLE_GAP)
    );
    assert_eq!(right_default, PlacementSpec::right_of("T1"));

    let right_custom =
        TableSpec::new("T3", "Table 3").with_placement(PlacementSpec::right_of_with_gap("T1", 3));
    let right_json = serde_json::to_string(&right_custom).unwrap();
    let deser_right: TableSpec = serde_json::from_str(&right_json).unwrap();
    assert_eq!(
        deser_right.placement,
        PlacementSpec::right_of_with_gap("T1", 3)
    );
}

#[test]
fn test_detailed_cell_input_rejects_unknown_source_field() {
    let json_data = r#"{
        "value": 100.0,
        "source": { "doc": "10-K" }
    }"#;
    let result: Result<DetailedCellInput, _> = serde_json::from_str(json_data);
    assert!(
        result.is_err(),
        "DetailedCellInput must reject unknown 'source' field due to deny_unknown_fields"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("unknown field `source`"));
}

#[test]
fn test_manifest_sheet_name_and_option_invariants_rejected() {
    let make_manifest = |sheet: SheetSpec| ProjectManifest {
        model_name: "Test".to_string(),
        theme: Default::default(),
        sheets: vec![sheet],
    };

    let base_sheet = |name: &str| SheetSpec {
        name: name.to_string(),
        tab_color: None,
        freeze_panes: None,
        show_gridlines: true,
        layout: Default::default(),
        tables: vec![TableId::from("T1")],
    };

    for invalid_name in [
        "",
        "A_sheet_name_that_exceeds_31_chr",
        "Bad:Name",
        "Bad\\Name",
        "Bad/Name",
        "Bad?Name",
        "Bad*Name",
        "Bad[Name",
        "Bad]Name",
        "'LeadingQuote",
        "TrailingQuote'",
        "History",
        "history",
    ] {
        let err = make_manifest(base_sheet(invalid_name))
            .validate()
            .unwrap_err();
        assert!(
            matches!(err, ModelError::InvalidSheetName { ref sheet, .. } if sheet == invalid_name),
            "expected InvalidSheetName for {invalid_name:?}, got {err:?}"
        );
    }

    // Valid sheet with internal single quote and spaces is allowed
    assert!(
        make_manifest(base_sheet("John's Cash Flow"))
            .validate()
            .is_ok()
    );

    // UTF-16 code unit boundary (31 UTF-16 units max: surrogate pairs count as 2 units)
    assert!(
        make_manifest(base_sheet(&"€".repeat(31)))
            .validate()
            .is_ok(),
        "31 BMP chars (93 UTF-8 bytes, 31 UTF-16 units) must be valid"
    );
    assert!(
        make_manifest(base_sheet(&format!("{}A", "😀".repeat(15))))
            .validate()
            .is_ok(),
        "15 astral chars + 'A' (31 UTF-16 units) must be valid"
    );
    let astral_32 = "😀".repeat(16);
    assert!(matches!(
        make_manifest(base_sheet(&astral_32)).validate().unwrap_err(),
        ModelError::InvalidSheetName { ref sheet, .. } if sheet == &astral_32
    ));

    // Empty manifest.sheets rejected
    let empty_sheets_manifest = ProjectManifest {
        model_name: "EmptySheets".to_string(),
        theme: Default::default(),
        sheets: vec![],
    };
    assert!(
        empty_sheets_manifest.validate().is_err(),
        "empty manifest.sheets must fail validation"
    );

    // Case-insensitive duplicate sheet names rejected
    let dup_manifest = ProjectManifest {
        model_name: "Test".to_string(),
        theme: Default::default(),
        sheets: vec![base_sheet("Sheet1"), base_sheet("sheet1")],
    };
    let dup_err = dup_manifest.validate().unwrap_err();
    assert!(matches!(
        dup_err,
        ModelError::DuplicateSheetName { ref sheet } if sheet == "sheet1"
    ));

    // Invalid tab_color hex rejected in Pass 1
    for bad_color in ["navy", "#GGG", "#12345", "1B365D"] {
        let mut sheet = base_sheet("Model");
        sheet.tab_color = Some(bad_color.to_string());
        let err = make_manifest(sheet).validate().unwrap_err();
        assert!(
            matches!(
                err,
                ModelError::InvalidSheetTabColor { ref sheet, ref color }
                    if sheet == "Model" && color == bad_color
            ),
            "expected InvalidSheetTabColor for {bad_color}, got {err:?}"
        );
    }

    // Out-of-bounds freeze_panes rejected in Pass 1
    for (row, col) in [(1_048_576, 0), (0, 16_384), (0, 70_000)] {
        let mut sheet = base_sheet("Model");
        sheet.freeze_panes = Some(FreezePanes { row, col });
        let err = make_manifest(sheet).validate().unwrap_err();
        assert!(
            matches!(
                err,
                ModelError::InvalidFreezePanes {
                    ref sheet,
                    row: r,
                    col: c,
                    ..
                } if sheet == "Model" && r == row && c == col
            ),
            "expected InvalidFreezePanes for ({row}, {col}), got {err:?}"
        );
    }
}

#[test]
fn test_table_id_and_finite_cell_number_invariants_rejected() {
    let make_table = |id: &str| {
        let mut t = TableSpec::new(id, "Title");
        t.columns.push(ColumnSpec::new("c1"));
        t.rows.push(RowSpec::new("r1").with_cell("c1", 1.0));
        t
    };

    for bad_id in [
        "",
        "Income Statement",
        "123",
        "1e5",
        "2E10",
        "TRUE",
        "FALSE",
        "_",
    ] {
        let err = make_table(bad_id).validate().unwrap_err();
        assert!(
            matches!(err, ModelError::InvalidTableId { ref table } if table.as_str() == bad_id),
            "expected InvalidTableId for {bad_id:?}, got {err:?}"
        );
    }

    for valid_id in ["IncomeStatement", "3Statement", "_internal", "T_1"] {
        assert!(
            make_table(valid_id).validate().is_ok(),
            "expected valid TableId for {valid_id:?}"
        );
    }

    for non_finite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut t1 = make_table("T1");
        t1.rows = vec![
            RowSpec::new("r1")
                .with_cell("c1", CellInput::Literal(ValueLiteral::Number(non_finite))),
        ];
        let err1 = t1.validate().unwrap_err();
        assert!(matches!(
            err1,
            ModelError::NonFiniteCellNumber {
                ref table,
                ref row,
                ref column,
                ..
            } if table.as_str() == "T1" && row.as_str() == "r1" && column.as_str() == "c1"
        ));

        let mut t2 = make_table("T1");
        t2.rows = vec![RowSpec::new("r1").with_cell(
            "c1",
            CellInput::Detailed(DetailedCellInput {
                value: Some(ValueLiteral::Number(non_finite)),
                ..Default::default()
            }),
        )];
        let err2 = t2.validate().unwrap_err();
        assert!(matches!(err2, ModelError::NonFiniteCellNumber { .. }));
    }

    // Empty or whitespace-only RowId and ColId rejected in Pass 1
    for bad_axis_id in ["", "   ", "\t\n"] {
        let mut bad_col_tbl = TableSpec::new("T1", "Title");
        bad_col_tbl.columns.push(ColumnSpec::new(bad_axis_id));
        bad_col_tbl
            .rows
            .push(RowSpec::new("r1").with_cell(bad_axis_id, 1.0));
        assert!(
            bad_col_tbl.validate().is_err(),
            "expected empty/whitespace ColId {bad_axis_id:?} to fail validation"
        );

        let mut bad_row_tbl = TableSpec::new("T1", "Title");
        bad_row_tbl.columns.push(ColumnSpec::new("c1"));
        bad_row_tbl
            .rows
            .push(RowSpec::new(bad_axis_id).with_cell("c1", 1.0));
        assert!(
            bad_row_tbl.validate().is_err(),
            "expected empty/whitespace RowId {bad_axis_id:?} to fail validation"
        );
    }

    // Cross-table DuplicateTableId rejected across Compiler entrypoints and theme validated uniformly
    let manifest = ProjectManifest {
        model_name: "DupModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            tables: vec![TableId::from("T1")],
        }],
    };
    let mut dup_a = make_table("T1");
    dup_a.rows = vec![RowSpec::new("r1").with_cell("c1", 1.0)];
    let mut dup_b = make_table("T1");
    dup_b.rows = vec![RowSpec::new("r2").with_cell("c1", 2.0)];
    let dup_err = f8_core::compiler::Compiler::compile(&manifest, &[dup_a, dup_b]).unwrap_err();
    assert!(
        matches!(
            dup_err,
            f8_core::compiler::CompilerError::Model(ModelError::DuplicateTableId { ref table })
                if table.as_str() == "T1"
        ),
        "expected DuplicateTableId, got {dup_err:?}"
    );

    let bad_theme_manifest = ProjectManifest {
        model_name: "BadTheme".to_string(),
        theme: f8_core::model::ThemeSpec::Name("nonexistent_theme".to_string()),
        sheets: manifest.sheets.clone(),
    };
    assert!(matches!(
        f8_core::compiler::Compiler::evaluate(&bad_theme_manifest, &[make_table("T1")])
            .unwrap_err(),
        f8_core::compiler::CompilerError::Model(ModelError::ThemeNotFound { .. })
    ));

    // ValueLiteral::Display normalizes -0.0 to "0"
    assert_eq!(ValueLiteral::Number(-0.0).to_string(), "0");
}

#[test]
fn test_detailed_cell_explicit_null_value_serde_and_validation() {
    use f8_core::model::Shade;

    // 1. Direct DetailedCellInput deserialization preserves explicit JSON null as Some(ValueLiteral::Null)
    let detailed: DetailedCellInput =
        serde_json::from_str(r#"{ "value": null, "note": "Excluded: NM", "shade": "none" }"#)
            .unwrap();
    assert_eq!(detailed.value, Some(ValueLiteral::Null));
    assert_eq!(detailed.note.as_deref(), Some("Excluded: NM"));
    assert_eq!(detailed.shade, Some(Shade::None));

    // 2. JSON round-trip of DetailedCellInput { value: Some(ValueLiteral::Null), .. } is lossless
    let roundtrip_json = serde_json::to_string(&detailed).unwrap();
    let roundtripped: DetailedCellInput = serde_json::from_str(&roundtrip_json).unwrap();
    assert_eq!(roundtripped, detailed);

    // 3. Untagged CellInput preserves explicit null inside Detailed objects (with and without metadata)
    let cell_with_note: CellInput =
        serde_json::from_str(r#"{ "value": null, "note": "Excluded" }"#).unwrap();
    assert_eq!(
        cell_with_note,
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Null),
            note: Some("Excluded".to_string()),
            ..Default::default()
        })
    );
    let cell_bare_obj: CellInput = serde_json::from_str(r#"{ "value": null }"#).unwrap();
    assert_eq!(
        cell_bare_obj,
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Null),
            ..Default::default()
        })
    );
    let cell_bare_null: CellInput = serde_json::from_str("null").unwrap();
    assert_eq!(cell_bare_null, CellInput::Literal(ValueLiteral::Null));

    // 4. TableSpec deserialized from JSON with explicit null + note passes Pass 1 validation
    let valid_json = r#"{
        "id": "PeerComps",
        "title": "Peer Comps",
        "columns": [
            { "kind": "data", "id": "company", "role": "descriptor", "headers": ["Company"] },
            { "kind": "data", "id": "ev_fcf", "headers": ["EV / FCF"] }
        ],
        "rows": [
            {
                "kind": "data",
                "id": "orcl",
                "cells": {
                    "company": "Oracle",
                    "ev_fcf": { "value": null, "note": "Excluded: negative FCF" }
                }
            }
        ]
    }"#;
    let valid_table: TableSpec = serde_json::from_str(valid_json).unwrap();
    assert!(valid_table.validate().is_ok());

    // 5. Absent 'value' still deserializes as None and triggers EmptyCellPayload
    let missing_json = r#"{
        "id": "PeerComps",
        "title": "Peer Comps",
        "columns": [{ "kind": "data", "id": "c1", "headers": ["C1"] }],
        "rows": [{ "kind": "data", "id": "r1", "cells": { "c1": { "note": "Missing payload" } } }]
    }"#;
    let missing_table: TableSpec = serde_json::from_str(missing_json).unwrap();
    assert!(matches!(
        missing_table.validate().unwrap_err(),
        ModelError::EmptyCellPayload { .. }
    ));

    // 6. Explicit 'value': null alongside 'expr' triggers MutuallyExclusiveCellPayload
    let conflict_json = r#"{
        "id": "PeerComps",
        "title": "Peer Comps",
        "columns": [{ "kind": "data", "id": "c1", "headers": ["C1"] }],
        "rows": [{
            "kind": "data",
            "id": "r1",
            "cells": { "c1": { "value": null, "expr": "1 + 1" } }
        }]
    }"#;
    let conflict_table: TableSpec = serde_json::from_str(conflict_json).unwrap();
    assert!(matches!(
        conflict_table.validate().unwrap_err(),
        ModelError::MutuallyExclusiveCellPayload { .. }
    ));

    // 7. Non-literal value payload still fails deserialization
    assert!(serde_json::from_str::<DetailedCellInput>(r#"{ "value": [1, 2] }"#).is_err());
    assert!(serde_json::from_str::<DetailedCellInput>(r#"{ "value": { "a": 1 } }"#).is_err());
}

#[test]
fn test_sheet_layout_spec_serde_roundtrip_and_pass1_validation() {
    use f8_core::compiler::{CompilerError, ErrorDiagnostic};
    use f8_core::model::SheetLayoutSpec;

    // 1. Omitted layout defaults to Auto and is omitted during serialization (Scenario 1)
    let default_json = r#"{
        "model_name": "DefaultLayout",
        "sheets": [
            { "name": "Summary", "tables": ["T1"] }
        ]
    }"#;
    let default_manifest: ProjectManifest = serde_json::from_str(default_json).unwrap();
    assert_eq!(default_manifest.sheets[0].layout, SheetLayoutSpec::Auto);
    assert!(default_manifest.sheets[0].layout.is_default());
    assert!(default_manifest.validate().is_ok());

    let serialized_default = serde_json::to_string(&default_manifest.sheets[0]).unwrap();
    assert!(
        !serialized_default.contains("layout"),
        "Auto layout must be omitted when serialized: {serialized_default}"
    );

    // 2. Grid without col_width and Grid with explicit col_width round-trip cleanly
    let grid_json = r#"{
        "model_name": "GridLayout",
        "sheets": [
            { "name": "DefaultGrid", "layout": { "kind": "grid" }, "tables": ["T1"] },
            { "name": "SizedGrid", "layout": { "kind": "grid", "col_width": 14.5 }, "tables": ["T2"] },
            { "name": "MinBound", "layout": { "kind": "grid", "col_width": 1.0 }, "tables": ["T3"] },
            { "name": "MaxBound", "layout": { "kind": "grid", "col_width": 255.0 }, "tables": ["T4"] }
        ]
    }"#;
    let grid_manifest: ProjectManifest = serde_json::from_str(grid_json).unwrap();
    assert_eq!(grid_manifest.sheets[0].layout, SheetLayoutSpec::grid());
    assert_eq!(
        grid_manifest.sheets[1].layout,
        SheetLayoutSpec::grid_with_col_width(14.5)
    );
    assert_eq!(
        grid_manifest.sheets[2].layout,
        SheetLayoutSpec::grid_with_col_width(1.0)
    );
    assert_eq!(
        grid_manifest.sheets[3].layout,
        SheetLayoutSpec::grid_with_col_width(255.0)
    );
    assert!(grid_manifest.validate().is_ok());

    let grid_default_str = serde_json::to_string(&grid_manifest.sheets[0].layout).unwrap();
    assert_eq!(grid_default_str, r#"{"kind":"grid"}"#);
    let grid_sized_str = serde_json::to_string(&grid_manifest.sheets[1].layout).unwrap();
    assert_eq!(grid_sized_str, r#"{"kind":"grid","col_width":14.5}"#);

    // 3. Unknown fields on SheetLayoutSpec fail deserialization (#[serde(deny_unknown_fields)])
    assert!(
        serde_json::from_str::<SheetLayoutSpec>(r#"{"kind":"auto","col_width":12.0}"#).is_err()
    );
    assert!(serde_json::from_str::<SheetLayoutSpec>(r#"{"kind":"auto","extra":true}"#).is_err());
    assert!(serde_json::from_str::<SheetLayoutSpec>(r#"{"kind":"grid","extra":1}"#).is_err());
    assert!(serde_json::from_str::<SheetLayoutSpec>(r#"{"kind":"unknown"}"#).is_err());

    // 4. Pass 1 rejects out-of-bounds and non-finite col_width (Scenario 7)
    for bad_width in [
        0.0,
        0.5,
        -5.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        256.0,
    ] {
        let bad_manifest = ProjectManifest {
            model_name: "BadWidth".to_string(),
            theme: Default::default(),
            sheets: vec![SheetSpec {
                name: "Dashboard".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::grid_with_col_width(bad_width),
                tables: vec![TableId::from("T1")],
            }],
        };
        let err = bad_manifest.validate().unwrap_err();
        assert!(
            matches!(err, ModelError::InvalidSheetColWidth { ref sheet, .. } if sheet == "Dashboard"),
            "Expected InvalidSheetColWidth for {bad_width:?}, got {err:?}"
        );
        let diag = CompilerError::Model(err).diagnostic();
        assert!(
            matches!(
                diag,
                ErrorDiagnostic::SpecificationError { table: None, .. }
            ),
            "Expected SpecificationError diagnostic with table: None, got {diag:?}"
        );
    }
}
