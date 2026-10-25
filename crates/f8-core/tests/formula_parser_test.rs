#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::expr::{
    BinaryOp, ColEndpoint, ColSelector, Expr, ParseError, RelativeOffset, RowEndpoint, RowSelector,
    TableRef,
};
use f8_core::model::id::{ColId, RowId, TableId};

#[test]
fn test_complex_formula_expression_parsing() {
    // Standard margin formula: (IncomeStatement["revenue", @col] - IncomeStatement["cogs", @col]) / IncomeStatement["revenue", @col]
    let input = "(IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]) / IncomeStatement[\"revenue\", @col]";
    let expr = Expr::parse(input).expect("Should parse valid margin formula");

    match expr {
        Expr::Binary { op, left, right } => {
            assert_eq!(op, BinaryOp::Div);
            match *left {
                Expr::Binary {
                    op: sub_op,
                    left: sub_left,
                    right: sub_right,
                } => {
                    assert_eq!(sub_op, BinaryOp::Sub);
                    assert_eq!(
                        *sub_left,
                        Expr::Reference(TableRef::new(
                            "IncomeStatement",
                            RowSelector::Key(RowId::new("revenue")),
                            ColSelector::Active
                        ))
                    );
                    assert_eq!(
                        *sub_right,
                        Expr::Reference(TableRef::new(
                            "IncomeStatement",
                            RowSelector::Key(RowId::new("cogs")),
                            ColSelector::Active
                        ))
                    );
                }
                other => panic!("Expected subtraction in numerator, got {other:?}"),
            }
            assert_eq!(
                *right,
                Expr::Reference(TableRef::new(
                    "IncomeStatement",
                    RowSelector::Key(RowId::new("revenue")),
                    ColSelector::Active
                ))
            );
        }
        other => panic!("Expected division at root, got {other:?}"),
    }
}

#[test]
fn test_nested_functions_and_logical_operators() {
    // IF(AND(a["b", @col] > 0, a["c", @col] <= 100), SUM(Opex["r_and_d".."g_and_a", @col]), _)
    let input = "IF(AND(a[\"b\", @col] > 0, a[\"c\", @col] <= 100), SUM(Opex[\"r_and_d\"..\"g_and_a\", @col]), _)";
    let expr = Expr::parse(input).expect("Should parse nested logical expressions");

    if let Expr::FunctionCall { name, args } = expr {
        assert_eq!(name, "IF");
        assert_eq!(args.len(), 3);

        // First arg: AND(...)
        if let Expr::FunctionCall {
            name: and_name,
            args: and_args,
        } = &args[0]
        {
            assert_eq!(and_name, "AND");
            assert_eq!(and_args.len(), 2);
        } else {
            panic!("Expected AND function call");
        }

        // Second arg: SUM(...)
        if let Expr::FunctionCall {
            name: sum_name,
            args: sum_args,
        } = &args[1]
        {
            assert_eq!(sum_name, "SUM");
            assert_eq!(sum_args.len(), 1);
            assert_eq!(
                sum_args[0],
                Expr::Reference(TableRef::new(
                    TableId::new("Opex"),
                    RowSelector::Range("r_and_d".into(), "g_and_a".into()),
                    ColSelector::Active,
                ))
            );
        } else {
            panic!("Expected SUM function call");
        }

        // Third arg: omitted argument (_)
        assert_eq!(args[2], Expr::Omitted);
    } else {
        panic!("Expected root IF call");
    }
}

#[test]
fn test_temporal_offsets() {
    let input = "BalanceSheet[\"cash\", col.prev] + CashFlow[\"net_change\", @col]";
    let expr = Expr::parse(input).expect("Should parse temporal offset formula");

    assert_eq!(
        expr,
        Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Key(RowId::new("cash")),
                ColSelector::Offset(RelativeOffset::Prev(1)),
            ))),
            right: Box::new(Expr::Reference(TableRef::new(
                TableId::new("CashFlow"),
                RowSelector::Key(RowId::new("net_change")),
                ColSelector::Active,
            ))),
        }
    );
}

#[test]
fn test_all_special_col_and_row_variants() {
    let formulas = [
        (
            "Table[\"row\", col.prev]",
            ColSelector::Offset(RelativeOffset::Prev(1)),
        ),
        (
            "Table[\"row\", col.prev(5)]",
            ColSelector::Offset(RelativeOffset::Prev(5)),
        ),
        (
            "Table[\"row\", col.next]",
            ColSelector::Offset(RelativeOffset::Next(1)),
        ),
        (
            "Table[\"row\", col.next(2)]",
            ColSelector::Offset(RelativeOffset::Next(2)),
        ),
    ];

    for (formula, expected_col) in formulas {
        let parsed = Expr::parse(formula).expect("Should parse");
        assert_eq!(
            parsed,
            Expr::Reference(TableRef::new(
                TableId::new("Table"),
                RowSelector::Key(RowId::new("row")),
                expected_col
            ))
        );
    }

    let row_formulas = [
        ("Table[@row, \"col\"]", RowSelector::Active),
        (
            "Table[row.prev, \"col\"]",
            RowSelector::Offset(RelativeOffset::Prev(1)),
        ),
        (
            "Table[row.prev(5), \"col\"]",
            RowSelector::Offset(RelativeOffset::Prev(5)),
        ),
        (
            "Table[row.next, \"col\"]",
            RowSelector::Offset(RelativeOffset::Next(1)),
        ),
        (
            "Table[row.next(2), \"col\"]",
            RowSelector::Offset(RelativeOffset::Next(2)),
        ),
    ];

    for (formula, expected_row) in row_formulas {
        let parsed = Expr::parse(formula).expect("Should parse");
        assert_eq!(
            parsed,
            Expr::Reference(TableRef::new(
                TableId::new("Table"),
                expected_row,
                ColSelector::Key(ColId::new("col"))
            ))
        );
    }
}

#[test]
fn test_syntax_errors_actionable_diagnostics() {
    // Empty formula
    assert_eq!(
        Expr::parse("   ").unwrap_err(),
        ParseError::UnexpectedEof {
            expected: "expression"
        }
    );

    // Unclosed parenthesis
    let unclosed = Expr::parse("(1 + 2").unwrap_err();
    assert_eq!(unclosed, ParseError::UnexpectedEof { expected: "')'" });

    // Bare coordinate key
    let bare_key = Expr::parse("Table[\"row\", unquoted_col]").unwrap_err();
    match bare_key {
        ParseError::UnquotedCoordinateKey { found, .. } => assert_eq!(found, "unquoted_col"),
        other => panic!("Expected UnquotedCoordinateKey, got {other:?}"),
    }

    // Lowercase function
    let lowercase = Expr::parse("sum(1, 2)").unwrap_err();
    match lowercase {
        ParseError::InvalidFunctionName { name, .. } => assert_eq!(name, "sum"),
        other => panic!("Expected InvalidFunctionName, got {other:?}"),
    }

    // Trailing junk
    let trailing = Expr::parse("1 + 2 garbage").unwrap_err();
    match trailing {
        ParseError::TrailingInput { trailing, .. } => assert_eq!(trailing, "garbage"),
        other => panic!("Expected TrailingInput, got {other:?}"),
    }

    // Reject legacy dot-access
    let dot_err = Expr::parse("IncomeStatement.revenue[@col]").unwrap_err();
    match dot_err {
        ParseError::UnexpectedToken { found, offset, .. } => {
            assert_eq!(found, ".");
            assert_eq!(offset, 15);
        }
        other => panic!("Expected UnexpectedToken for dot-access, got {other:?}"),
    }

    // Reject retired bare '@'
    let bare_at_err = Expr::parse("IncomeStatement[\"revenue\", @]").unwrap_err();
    assert_eq!(bare_at_err, ParseError::RetiredBareAt { offset: 27 });
}

#[test]
fn test_dynamic_range_slice_parsing_and_display_roundtrip() {
    let cases = [
        (
            "SUM(Debt[\"amort\", \"2024E\"..@col])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![Expr::Reference(TableRef::new(
                    "Debt",
                    RowSelector::Key(RowId::new("amort")),
                    ColSelector::Range(ColEndpoint::from("2024E"), ColEndpoint::Active),
                ))],
            },
        ),
        (
            "SUM(Debt[\"amort\", \"2024E\"..col.prev])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![Expr::Reference(TableRef::new(
                    "Debt",
                    RowSelector::Key(RowId::new("amort")),
                    ColSelector::Range(
                        ColEndpoint::from("2024E"),
                        ColEndpoint::Offset(RelativeOffset::Prev(1)),
                    ),
                ))],
            },
        ),
        (
            "SUM(Debt[\"amort\", col.prev(2)..@col])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![Expr::Reference(TableRef::new(
                    "Debt",
                    RowSelector::Key(RowId::new("amort")),
                    ColSelector::Range(
                        ColEndpoint::Offset(RelativeOffset::Prev(2)),
                        ColEndpoint::Active,
                    ),
                ))],
            },
        ),
        (
            "SUM(Cohort[\"c1\"..@row, @col])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![Expr::Reference(TableRef::new(
                    "Cohort",
                    RowSelector::Range(RowEndpoint::from("c1"), RowEndpoint::Active),
                    ColSelector::Active,
                ))],
            },
        ),
        (
            "SUM(Cohort[row.prev(2)..row.next, @col..col.next(2)])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![Expr::Reference(TableRef::new(
                    "Cohort",
                    RowSelector::Range(
                        RowEndpoint::Offset(RelativeOffset::Prev(2)),
                        RowEndpoint::Offset(RelativeOffset::Next(1)),
                    ),
                    ColSelector::Range(
                        ColEndpoint::Active,
                        ColEndpoint::Offset(RelativeOffset::Next(2)),
                    ),
                ))],
            },
        ),
    ];

    for (formula, expected) in cases {
        let parsed = Expr::parse(formula).expect("should parse dynamic range formula");
        assert_eq!(parsed, expected);
        let displayed = parsed.to_string();
        assert_eq!(displayed, formula);
        assert_eq!(Expr::parse(&displayed).unwrap(), expected);
    }

    // Chained ranges and open-ended slices must fail to parse
    assert!(Expr::parse("SUM(Debt[\"amort\", \"2024E\"..\"2025E\"..@col])").is_err());
    assert!(Expr::parse("SUM(Debt[\"amort\", \"2024E\"..])").is_err());
    assert!(Expr::parse("SUM(Debt[\"amort\", ..@col])").is_err());
}
