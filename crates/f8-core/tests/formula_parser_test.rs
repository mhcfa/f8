#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::expr::{
    BinaryOp, BlankCheck, CallArg, ColEndpoint, ColSelector, CriteriaComparator, Expr, ParseError,
    RelativeOffset, RowEndpoint, RowSelector, TableRef, TableSelector,
};
use f8_core::model::id::{ColId, RowId, TableId};

#[test]
fn test_complex_formula_expression_parsing() {
    // Standard margin formula: (IncomeStatement["revenue", @col] - IncomeStatement["cogs", @col]) / IncomeStatement["revenue", @col]
    let input = "(IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]) / IncomeStatement[\"revenue\", @col]";
    let expr = Expr::parse(input).expect("Should parse valid margin formula");

    let Expr::Binary { op, left, right } = expr else {
        panic!("Expected division at root, got {expr:?}");
    };
    assert_eq!(op, BinaryOp::Div);
    let Expr::Binary {
        op: sub_op,
        left: sub_left,
        right: sub_right,
    } = *left
    else {
        panic!("Expected subtraction in numerator, got {left:?}");
    };
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
    assert_eq!(
        *right,
        Expr::Reference(TableRef::new(
            "IncomeStatement",
            RowSelector::Key(RowId::new("revenue")),
            ColSelector::Active
        ))
    );
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
        if let CallArg::Expr(Expr::FunctionCall {
            name: and_name,
            args: and_args,
        }) = &args[0]
        {
            assert_eq!(and_name, "AND");
            assert_eq!(and_args.len(), 2);
        } else {
            panic!("Expected AND function call");
        }

        // Second arg: SUM(...)
        if let CallArg::Expr(Expr::FunctionCall {
            name: sum_name,
            args: sum_args,
        }) = &args[1]
        {
            assert_eq!(sum_name, "SUM");
            assert_eq!(sum_args.len(), 1);
            assert_eq!(
                sum_args[0],
                CallArg::Expr(Expr::Reference(TableRef::new(
                    TableId::new("Opex"),
                    RowSelector::Range("r_and_d".into(), "g_and_a".into()),
                    ColSelector::Active,
                )))
            );
        } else {
            panic!("Expected SUM function call");
        }

        // Third arg: omitted argument (_)
        assert_eq!(args[2], CallArg::Omitted);
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
    let ParseError::UnquotedCoordinateKey { found, .. } = bare_key else {
        panic!("Expected UnquotedCoordinateKey, got {bare_key:?}");
    };
    assert_eq!(found, "unquoted_col");

    // Lowercase function
    let lowercase = Expr::parse("sum(1, 2)").unwrap_err();
    let ParseError::InvalidFunctionName { name, .. } = lowercase else {
        panic!("Expected InvalidFunctionName, got {lowercase:?}");
    };
    assert_eq!(name, "sum");

    // Trailing junk
    let trailing = Expr::parse("1 + 2 garbage").unwrap_err();
    let ParseError::TrailingInput { trailing, .. } = trailing else {
        panic!("Expected TrailingInput, got {trailing:?}");
    };
    assert_eq!(trailing, "garbage");

    // Reject legacy dot-access
    let dot_err = Expr::parse("IncomeStatement.revenue[@col]").unwrap_err();
    let ParseError::UnexpectedToken { found, offset, .. } = dot_err else {
        panic!("Expected UnexpectedToken for dot-access, got {dot_err:?}");
    };
    assert_eq!(found, ".");
    assert_eq!(offset, 15);

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
                args: vec![CallArg::Expr(Expr::Reference(TableRef::new(
                    "Debt",
                    RowSelector::Key(RowId::new("amort")),
                    ColSelector::Range(ColEndpoint::from("2024E"), ColEndpoint::Active),
                )))],
            },
        ),
        (
            "SUM(Debt[\"amort\", \"2024E\"..col.prev])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![CallArg::Expr(Expr::Reference(TableRef::new(
                    "Debt",
                    RowSelector::Key(RowId::new("amort")),
                    ColSelector::Range(
                        ColEndpoint::from("2024E"),
                        ColEndpoint::Offset(RelativeOffset::Prev(1)),
                    ),
                )))],
            },
        ),
        (
            "SUM(Debt[\"amort\", col.prev(2)..@col])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![CallArg::Expr(Expr::Reference(TableRef::new(
                    "Debt",
                    RowSelector::Key(RowId::new("amort")),
                    ColSelector::Range(
                        ColEndpoint::Offset(RelativeOffset::Prev(2)),
                        ColEndpoint::Active,
                    ),
                )))],
            },
        ),
        (
            "SUM(Cohort[\"c1\"..@row, @col])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![CallArg::Expr(Expr::Reference(TableRef::new(
                    "Cohort",
                    RowSelector::Range(RowEndpoint::from("c1"), RowEndpoint::Active),
                    ColSelector::Active,
                )))],
            },
        ),
        (
            "SUM(Cohort[row.prev(2)..row.next, @col..col.next(2)])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![CallArg::Expr(Expr::Reference(TableRef::new(
                    "Cohort",
                    RowSelector::Range(
                        RowEndpoint::Offset(RelativeOffset::Prev(2)),
                        RowEndpoint::Offset(RelativeOffset::Next(1)),
                    ),
                    ColSelector::Range(
                        ColEndpoint::Active,
                        ColEndpoint::Offset(RelativeOffset::Next(2)),
                    ),
                )))],
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

#[test]
fn test_active_table_selector_parsing_and_roundtrip() {
    let cases = vec![
        (
            "@table[\"rev\", @col] - @table[\"cogs\", @col]",
            Expr::Binary {
                op: BinaryOp::Sub,
                left: Box::new(Expr::Reference(TableRef::new(
                    TableSelector::Active,
                    RowSelector::Key(RowId::new("rev")),
                    ColSelector::Active,
                ))),
                right: Box::new(Expr::Reference(TableRef::new(
                    TableSelector::Active,
                    RowSelector::Key(RowId::new("cogs")),
                    ColSelector::Active,
                ))),
            },
        ),
        (
            "@table[\"rev\", col.prev]",
            Expr::Reference(TableRef::new(
                TableSelector::Active,
                RowSelector::Key(RowId::new("rev")),
                ColSelector::Offset(RelativeOffset::Prev(1)),
            )),
        ),
        (
            "SUM(@table[\"rev\"..\"cogs\", @col])",
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![CallArg::Expr(Expr::Reference(TableRef::new(
                    TableSelector::Active,
                    RowSelector::Range(RowEndpoint::from("rev"), RowEndpoint::from("cogs")),
                    ColSelector::Active,
                )))],
            },
        ),
    ];

    for (formula, expected) in cases {
        let parsed = Expr::parse(formula).expect("should parse valid @table formula");
        assert_eq!(parsed, expected);
        let displayed = parsed.to_string();
        assert_eq!(displayed, formula);
        assert_eq!(Expr::parse(&displayed).unwrap(), expected);
    }
}

#[test]
fn test_active_table_selector_invalid_positions_rejected() {
    // Scenario 2: Invalid @table positions rejected
    let invalid_formulas = [
        "@table",
        "@table(\"rev\")",
        "IncomeStatement[@table, @col]",
        "IncomeStatement[\"rev\", @table]",
        "[\"rev\", @col]",
    ];

    for formula in invalid_formulas {
        let err = Expr::parse(formula).expect_err(&format!("'{formula}' must fail parsing"));
        assert!(
            matches!(err, ParseError::UnexpectedToken { .. }),
            "expected UnexpectedToken for '{formula}', got {err:?}"
        );
    }
}

#[test]
fn test_call_arg_confinement_and_invalid_underscore_syntax() {
    let xlookup_formula =
        "XLOOKUP(100, R1[\"k1\"..\"k2\", @col], R2[\"v1\"..\"v2\", @col], _, 0, 1)";
    let parsed_xlookup = Expr::parse(xlookup_formula).expect("valid omitted slot in XLOOKUP");
    assert_eq!(parsed_xlookup.to_string(), xlookup_formula);
    let Expr::FunctionCall { name, args } = &parsed_xlookup else {
        panic!("expected FunctionCall, got {parsed_xlookup:?}");
    };
    assert_eq!(name, "XLOOKUP");
    assert_eq!(args.len(), 6);
    assert_eq!(args[0], CallArg::from(Expr::number(100.0)));
    assert_eq!(args[3], CallArg::Omitted);
    assert_eq!(args[3].to_string(), "_");

    let if_formula = "IF(TRUE, 10, _)";
    let parsed_if = Expr::parse(if_formula).expect("valid omitted slot in IF");
    assert_eq!(parsed_if.to_string(), if_formula);
    let Expr::FunctionCall { args: if_args, .. } = &parsed_if else {
        panic!("expected FunctionCall, got {parsed_if:?}");
    };
    assert_eq!(if_args[2], CallArg::Omitted);

    let invalid_underscore = ["_", "1 + _", "IF(TRUE, 10, _ + 1)", "IF(TRUE, 10, (_))"];
    for formula in invalid_underscore {
        let err = Expr::parse(formula).expect_err(&format!("'{formula}' must fail parsing"));
        assert!(
            matches!(err, ParseError::UnexpectedToken { .. }),
            "expected UnexpectedToken for '{formula}', got {err:?}"
        );
    }
}

#[test]
fn test_cluster_a1_scenario_1_grammar_strictness() {
    // 1. Unary '+' is retired
    for formula in ["+10", "+TRUE", "+\"abc\"", "1 + +2", "SUM(+1)"] {
        let err = Expr::parse(formula).expect_err(&format!("'{formula}' must fail parsing"));
        assert!(
            matches!(err, ParseError::UnexpectedToken { ref found, .. } if found == "+"),
            "expected UnexpectedToken('+') for '{formula}', got {err:?}"
        );
    }

    // 2. Unparenthesized chained comparisons are non-associative
    let chained_cases = [
        ("1 = 1 = TRUE", "="),
        ("TRUE = 1 = 1", "="),
        ("1 < 2 < 3", "<"),
        ("FALSE = FALSE = FALSE", "="),
        ("1 <= 2 >= 3", ">="),
        ("1 <> 2 = 3", "="),
    ];
    for (formula, expected_op) in chained_cases {
        let err = Expr::parse(formula).expect_err(&format!("'{formula}' must fail parsing"));
        assert!(
            matches!(err, ParseError::ChainedComparison { operator, .. } if operator == expected_op),
            "expected ChainedComparison('{expected_op}') for '{formula}', got {err:?}"
        );
    }

    // 3. Invalid relative offsets (< 1, fractional, or > i32::MAX) are rejected
    for formula in [
        "Table[row.prev(0), @col]",
        "Table[@row, col.next(0)]",
        "Table[@row, col.prev(5000000000)]",
        "Table[@row, col.prev(-1)]",
        "Table[@row, col.prev(1.5)]",
        "SUM(Table[@row, col.prev(0)..@col])",
    ] {
        let err = Expr::parse(formula).expect_err(&format!("'{formula}' must fail parsing"));
        assert!(
            matches!(err, ParseError::UnexpectedToken { .. }),
            "expected UnexpectedToken for '{formula}', got {err:?}"
        );
    }

    // 4. Valid binary '+', unary '-', parenthesized comparisons, positive integer offsets, ISBLANK, and NA() succeed
    for valid in [
        "1 + 2",
        "-10",
        "(1 = 1) = TRUE",
        "TRUE = (1 = 1)",
        "Table[@row, col.prev]",
        "Table[@row, col.prev(2)]",
        "ISBLANK(Table[\"r1\", @col])",
        "NA()",
    ] {
        let parsed = Expr::parse(valid).unwrap_or_else(|e| panic!("'{valid}' should parse: {e}"));
        let displayed = parsed.to_string();
        assert_eq!(displayed, valid, "Display roundtrip mismatch for '{valid}'");
        assert_eq!(Expr::parse(&displayed).unwrap(), parsed);
    }
}

#[test]
fn test_cluster_c_scenario_1_criteria_grammar_roundtrip_and_diagnostics() {
    use f8_core::compiler::{CompilerError, ErrorDiagnostic};

    let roundtrip_cases = [
        "COUNTIFS(T[\"r1\"..\"r4\", @col], > 15)",
        "COUNTIFS(T[\"r1\"..\"r4\", @col], >= -10)",
        "COUNTIFS(T[\"r1\"..\"r4\", @col], <= Summary[\"cutoff\", @col])",
        "COUNTIFS(T[\"r1\"..\"r4\", @col], <> \"Alpha\")",
        "COUNTIFS(T[\"r1\"..\"r4\", @col], <> (T[\"a\", @col] > T[\"b\", @col]))",
        "COUNTIFS(T[\"r1\"..\"r4\", @col], T[\"a\", @col] > T[\"b\", @col])",
        "COUNTIFS(T[\"r1\"..\"r4\", @col], BLANK)",
        "COUNTIFS(T[\"r1\"..\"r4\", @col], NOT_BLANK)",
        "COUNTIFS(BLANK[\"r1\"..\"r4\", @col], NOT_BLANK[\"r1\", @col])",
        "ERROR.TYPE(1 / 0)",
    ];

    for formula in roundtrip_cases {
        let parsed = Expr::parse(formula).unwrap_or_else(|e| panic!("'{formula}' must parse: {e}"));
        let displayed = parsed.to_string();
        assert_eq!(displayed, formula, "Display mismatch for '{formula}'");
        let reparsed = Expr::parse(&displayed).unwrap();
        assert_eq!(reparsed, parsed, "Round-trip AST mismatch for '{formula}'");
    }

    // Programmatic relational Comparator AST parenthesizes operand in Display for round-trip equality
    let programmatic = Expr::FunctionCall {
        name: "COUNTIFS".into(),
        args: vec![
            CallArg::Expr(Expr::Reference(TableRef::new(
                TableSelector::from("T"),
                RowSelector::Range(RowEndpoint::from("r1"), RowEndpoint::from("r2")),
                ColSelector::Active,
            ))),
            CallArg::Comparator {
                op: CriteriaComparator::NotEq,
                expr: Expr::Binary {
                    op: BinaryOp::Gt,
                    left: Box::new(Expr::number(10.0)),
                    right: Box::new(Expr::number(5.0)),
                },
            },
            CallArg::BlankCheck(BlankCheck::Blank),
        ],
    };
    let rendered = programmatic.to_string();
    assert_eq!(
        rendered,
        "COUNTIFS(T[\"r1\"..\"r2\", @col], <> (10 > 5), BLANK)"
    );
    assert_eq!(Expr::parse(&rendered).unwrap(), programmatic);

    // Redundant '=' prefix fails with RedundantCriteriaEquals and exact token_offset
    let redundant_eq = "COUNTIFS(T[\"r1\"..\"r2\", @col], = 10)";
    let expected_offset = redundant_eq.find('=').unwrap();
    let parse_err = Expr::parse(redundant_eq).unwrap_err();
    assert_eq!(
        parse_err,
        ParseError::RedundantCriteriaEquals {
            offset: expected_offset,
        }
    );
    let comp_err = CompilerError::Ir(f8_core::compiler::ir::IrError::FormulaParse {
        table: TableId::from("T"),
        row: Some(RowId::from("r1")),
        column: Some(ColId::from("c1")),
        source: parse_err,
    });
    assert_eq!(
        comp_err.diagnostic(),
        ErrorDiagnostic::FormulaParseError {
            message: comp_err.to_string(),
            table: "T".into(),
            row: Some("r1".into()),
            col: Some("c1".into()),
            token_offset: Some(expected_offset),
        }
    );

    // Unparenthesized trailing relational operator after comparator raises ChainedComparison
    let chained =
        Expr::parse("COUNTIFS(T[\"r1\"..\"r2\", @col], <> T[\"a\", @col] = T[\"b\", @col])")
            .unwrap_err();
    assert!(matches!(
        chained,
        ParseError::ChainedComparison { operator: "=", .. }
    ));

    // Lowercase blank/not_blank and malformed dotted function names fail parsing
    for bad in [
        "COUNTIFS(T[\"r1\"..\"r2\", @col], blank)",
        "COUNTIFS(T[\"r1\"..\"r2\", @col], not_blank)",
        "ERROR . TYPE(1)",
        "ERROR. TYPE(1)",
        "ERROR .TYPE(1)",
        "ERROR..TYPE(1)",
        "error.type(1)",
        "ERROR.type(1)",
        "> 5",
        "BLANK",
        "NOT_BLANK",
    ] {
        assert!(Expr::parse(bad).is_err(), "expected parse error on '{bad}'");
    }
}
