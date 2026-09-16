#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::expr::{
    BinaryOp, ColSelector, Expr, ParseError, RowSelector, SpecialCol, SpecialRow, TableRef,
};
use finx::model::id::{ColGroupId, RowGroupId, RowId, TableId};

#[test]
fn test_complex_formula_expression_parsing() {
    // Standard margin formula: (IncomeStatement.revenue[@] - IncomeStatement.cogs[@]) / IncomeStatement.revenue[@]
    let input =
        "(IncomeStatement.revenue[@] - IncomeStatement.cogs[@]) / IncomeStatement.revenue[@]";
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
                        Expr::Reference(TableRef::dot(
                            "IncomeStatement",
                            "revenue",
                            ColSelector::Active
                        ))
                    );
                    assert_eq!(
                        *sub_right,
                        Expr::Reference(TableRef::dot(
                            "IncomeStatement",
                            "cogs",
                            ColSelector::Active
                        ))
                    );
                }
                other => panic!("Expected subtraction in numerator, got {other:?}"),
            }
            assert_eq!(
                *right,
                Expr::Reference(TableRef::dot(
                    "IncomeStatement",
                    "revenue",
                    ColSelector::Active
                ))
            );
        }
        other => panic!("Expected division at root, got {other:?}"),
    }
}

#[test]
fn test_nested_functions_and_logical_operators() {
    // IF(AND(a.b[@] > 0, a.c[@] <= 100), SUM(Opex["r_and_d".."g_and_a", @]), null)
    let input = "IF(AND(a.b[@] > 0, a.c[@] <= 100), SUM(Opex[\"r_and_d\"..\"g_and_a\", @]), null)";
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
                Expr::Reference(TableRef::bracket(
                    TableId::new("Opex"),
                    RowSelector::Range(RowId::new("r_and_d"), RowId::new("g_and_a")),
                    ColSelector::Active,
                ))
            );
        } else {
            panic!("Expected SUM function call");
        }

        // Third arg: null
        assert_eq!(args[2], Expr::null());
    } else {
        panic!("Expected root IF call");
    }
}

#[test]
fn test_temporal_offsets() {
    let input = "BalanceSheet.cash[col.prev] + CashFlow.net_change[@]";
    let expr = Expr::parse(input).expect("Should parse temporal offset formula");

    assert_eq!(
        expr,
        Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Reference(TableRef::dot(
                TableId::new("BalanceSheet"),
                RowId::new("cash"),
                ColSelector::Special(SpecialCol::Prev(1)),
            ))),
            right: Box::new(Expr::Reference(TableRef::dot(
                TableId::new("CashFlow"),
                RowId::new("net_change"),
                ColSelector::Active,
            ))),
        }
    );
}

#[test]
fn test_all_special_col_and_row_variants() {
    let formulas = [
        (
            "Table.row[col.prev]",
            ColSelector::Special(SpecialCol::Prev(1)),
        ),
        (
            "Table.row[col.prev(5)]",
            ColSelector::Special(SpecialCol::Prev(5)),
        ),
        (
            "Table.row[col.next]",
            ColSelector::Special(SpecialCol::Next(1)),
        ),
        (
            "Table.row[col.next(2)]",
            ColSelector::Special(SpecialCol::Next(2)),
        ),
        (
            "Table.row[cols.forecast]",
            ColSelector::Special(SpecialCol::Group(ColGroupId::new("forecast"))),
        ),
        (
            "Table.row[cols.forecast.first]",
            ColSelector::Special(SpecialCol::GroupFirst(ColGroupId::new("forecast"))),
        ),
        (
            "Table.row[cols.forecast.last]",
            ColSelector::Special(SpecialCol::GroupLast(ColGroupId::new("forecast"))),
        ),
    ];

    for (formula, expected_col) in formulas {
        let parsed = Expr::parse(formula).expect("Should parse");
        assert_eq!(
            parsed,
            Expr::Reference(TableRef::dot(
                TableId::new("Table"),
                RowId::new("row"),
                expected_col
            ))
        );
    }

    let row_formulas = [
        (
            "Table[rows.opex, @]",
            RowSelector::Special(SpecialRow::Group(RowGroupId::new("opex"))),
        ),
        (
            "Table[rows.opex.first, @]",
            RowSelector::Special(SpecialRow::GroupFirst(RowGroupId::new("opex"))),
        ),
        (
            "Table[rows.opex.last, @]",
            RowSelector::Special(SpecialRow::GroupLast(RowGroupId::new("opex"))),
        ),
    ];

    for (formula, expected_row) in row_formulas {
        let parsed = Expr::parse(formula).expect("Should parse");
        assert_eq!(
            parsed,
            Expr::Reference(TableRef::bracket(
                TableId::new("Table"),
                expected_row,
                ColSelector::Active
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
    let bare_key = Expr::parse("Table.row[unquoted_col]").unwrap_err();
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
}
