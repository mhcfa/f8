use crate::expr::ast::{
    BinaryOp, ColSelector, Expr, Literal, RowSelector, SpecialCol, SpecialRow, TableRef, UnaryOp,
};
use crate::expr::error::ParseError;
use crate::model::id::{ColGroupId, ColId, RowGroupId, RowId};
use winnow::ascii::{digit1, multispace0};
use winnow::combinator::opt;
use winnow::error::{ContextError, ErrMode};
use winnow::prelude::*;
use winnow::token::{literal, take_while};

/// Parses a raw formula expression string into an [`Expr`] AST.
pub fn parse_expr_str(input: &str) -> Result<Expr, ParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "expression",
        });
    }

    let orig = input;
    let mut cursor = input;
    skip_ws(&mut cursor);

    let expr = parse_expression(&mut cursor, orig, 0)?;

    skip_ws(&mut cursor);
    if !cursor.is_empty() {
        let offset = orig.len() - cursor.len();
        return Err(ParseError::TrailingInput {
            trailing: cursor.to_string(),
            offset,
        });
    }

    Ok(expr)
}

fn skip_ws(input: &mut &str) {
    let _ = multispace0::<_, ContextError>.parse_next(input);
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn has_ident_boundary(input: &str, keyword_len: usize) -> bool {
    if input.len() == keyword_len {
        true
    } else {
        let next_char = input[keyword_len..].chars().next();
        match next_char {
            Some(c) => !is_ident_continue(c),
            None => true,
        }
    }
}

fn parse_expression(input: &mut &str, orig: &str, min_bp: u8) -> Result<Expr, ParseError> {
    skip_ws(input);
    let mut left = parse_prefix(input, orig)?;

    loop {
        skip_ws(input);
        if input.is_empty() {
            break;
        }

        let saved = *input;
        let Some((op, op_prec, is_right)) = peek_binary_op(input) else {
            break;
        };

        if op_prec < min_bp {
            *input = saved;
            break;
        }

        // Consume operator
        consume_binary_op(input, op);

        let next_bp = if is_right { op_prec } else { op_prec + 1 };
        let right = parse_expression(input, orig, next_bp)?;
        left = Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        };
    }

    Ok(left)
}

fn parse_prefix(input: &mut &str, orig: &str) -> Result<Expr, ParseError> {
    skip_ws(input);
    if input.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "expression",
        });
    }

    // Unary prefix: + or -
    if input.starts_with('+') {
        *input = &input[1..];
        let expr = parse_expression(input, orig, UnaryOp::Pos.precedence())?;
        return Ok(Expr::Unary {
            op: UnaryOp::Pos,
            expr: Box::new(expr),
        });
    }

    if input.starts_with('-') {
        *input = &input[1..];
        let expr = parse_expression(input, orig, UnaryOp::Neg.precedence())?;
        return Ok(Expr::Unary {
            op: UnaryOp::Neg,
            expr: Box::new(expr),
        });
    }

    // Parenthesized expression
    if input.starts_with('(') {
        *input = &input[1..];
        let expr = parse_expression(input, orig, 0)?;
        skip_ws(input);
        if input.is_empty() {
            return Err(ParseError::UnexpectedEof { expected: "')'" });
        }
        if input.starts_with(')') {
            *input = &input[1..];
            return Ok(expr);
        }
        let offset = orig.len() - input.len();
        return Err(ParseError::UnexpectedToken {
            expected: "')'",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset,
        });
    }

    // Number literal
    if input.starts_with(|c: char| c.is_ascii_digit()) {
        return parse_number(input, orig);
    }

    // String literal
    if input.starts_with('"') {
        let s = parse_string_literal(input, orig)?;
        return Ok(Expr::Literal(Literal::String(s)));
    }

    // Null literal or identifier (function call or table reference)
    if input.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        let offset = orig.len() - input.len();
        let ident = parse_identifier(input);

        // Check for null keyword with identifier boundary
        if ident == "null" {
            return Ok(Expr::Literal(Literal::Null));
        }

        skip_ws(input);

        // Function call: Identifier ( ... )
        if input.starts_with('(') {
            return parse_function_call(ident, input, orig, offset);
        }

        // Dot access: Identifier . row [ col ]
        if input.starts_with('.') {
            return parse_dot_access(ident, input, orig);
        }

        // Bracket access: Identifier [ row , col ]
        if input.starts_with('[') {
            return parse_bracket_access(ident, input, orig);
        }

        return Err(ParseError::UnexpectedToken {
            expected: "function call '(', dot access '.', or bracket access '['",
            found: ident,
            offset,
        });
    }

    let offset = orig.len() - input.len();
    let found = input
        .chars()
        .next()
        .map_or("".to_string(), |c| c.to_string());
    Err(ParseError::UnexpectedToken {
        expected: "expression",
        found,
        offset,
    })
}

fn parse_number(input: &mut &str, orig: &str) -> Result<Expr, ParseError> {
    let offset = orig.len() - input.len();
    let res: Result<&str, ErrMode<ContextError>> = (digit1, opt((literal("."), digit1)))
        .take()
        .parse_next(input);

    match res {
        Ok(raw) => match raw.parse::<f64>() {
            Ok(n) => Ok(Expr::Literal(Literal::Number(n))),
            Err(_) => Err(ParseError::InvalidNumber {
                raw: raw.to_string(),
                offset,
            }),
        },
        Err(_) => Err(ParseError::UnexpectedToken {
            expected: "numeric literal",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset,
        }),
    }
}

fn parse_string_literal(input: &mut &str, orig: &str) -> Result<String, ParseError> {
    let offset = orig.len() - input.len();
    if input.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "opening quote '\"'",
        });
    }
    if !input.starts_with('"') {
        return Err(ParseError::UnexpectedToken {
            expected: "opening quote '\"'",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset,
        });
    }

    *input = &input[1..];
    let mut out = String::new();
    let mut chars = input.char_indices();
    let mut end_idx = None;

    while let Some((idx, c)) = chars.next() {
        if c == '\\' {
            if let Some((_, next_c)) = chars.next() {
                out.push(next_c);
            } else {
                return Err(ParseError::UnexpectedEof {
                    expected: "escaped character",
                });
            }
        } else if c == '"' {
            end_idx = Some(idx);
            break;
        } else {
            out.push(c);
        }
    }

    if let Some(idx) = end_idx {
        *input = &input[idx + 1..];
        Ok(out)
    } else {
        Err(ParseError::UnexpectedEof {
            expected: "closing quote '\"'",
        })
    }
}

fn parse_identifier(input: &mut &str) -> String {
    let res: Result<&str, ErrMode<ContextError>> = (
        take_while(1.., |c: char| c.is_ascii_alphabetic() || c == '_'),
        take_while(0.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
    )
        .take()
        .parse_next(input);

    match res {
        Ok(s) => s.to_string(),
        Err(_) => String::new(),
    }
}

fn parse_argument_list(input: &mut &str, orig: &str) -> Result<Vec<Expr>, ParseError> {
    skip_ws(input);
    if input.starts_with(')') {
        return Ok(Vec::new());
    }

    let mut args = Vec::new();
    loop {
        skip_ws(input);
        if input.is_empty() {
            return Err(ParseError::UnexpectedEof {
                expected: "expression or ')'",
            });
        }
        let arg = parse_expression(input, orig, 0)?;
        args.push(arg);

        skip_ws(input);
        if input.starts_with(',') {
            *input = &input[1..];
            skip_ws(input);
            if input.starts_with(')') {
                let err_offset = orig.len() - input.len();
                return Err(ParseError::UnexpectedToken {
                    expected: "expression after ','",
                    found: ")".to_string(),
                    offset: err_offset,
                });
            }
            continue;
        }

        if input.starts_with(')') {
            break;
        }

        if input.is_empty() {
            return Err(ParseError::UnexpectedEof {
                expected: "',' or ')'",
            });
        }

        let err_offset = orig.len() - input.len();
        return Err(ParseError::UnexpectedToken {
            expected: "',' or ')'",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset: err_offset,
        });
    }

    Ok(args)
}

fn parse_function_call(
    name: String,
    input: &mut &str,
    orig: &str,
    offset: usize,
) -> Result<Expr, ParseError> {
    // Validate uppercase function name
    let is_valid = !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');

    if !is_valid {
        return Err(ParseError::InvalidFunctionName { name, offset });
    }

    // Consume '('
    *input = &input[1..];
    let args = parse_argument_list(input, orig)?;

    // Consume ')'
    skip_ws(input);
    if input.is_empty() {
        return Err(ParseError::UnexpectedEof { expected: "')'" });
    }
    if !input.starts_with(')') {
        let err_offset = orig.len() - input.len();
        return Err(ParseError::UnexpectedToken {
            expected: "')'",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset: err_offset,
        });
    }
    *input = &input[1..];

    Ok(Expr::FunctionCall { name, args })
}

fn parse_dot_access(table: String, input: &mut &str, orig: &str) -> Result<Expr, ParseError> {
    // Consume '.'
    *input = &input[1..];
    skip_ws(input);

    let offset = orig.len() - input.len();
    let row_name = parse_identifier(input);
    if row_name.is_empty() {
        return Err(ParseError::UnexpectedToken {
            expected: "row identifier",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset,
        });
    }

    skip_ws(input);
    if input.is_empty() {
        return Err(ParseError::UnexpectedEof { expected: "'['" });
    }
    if !input.starts_with('[') {
        let err_offset = orig.len() - input.len();
        return Err(ParseError::UnexpectedToken {
            expected: "'['",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset: err_offset,
        });
    }
    *input = &input[1..];

    skip_ws(input);
    let col = parse_col_spec(input, orig)?;

    skip_ws(input);
    if input.is_empty() {
        return Err(ParseError::UnexpectedEof { expected: "']'" });
    }
    if !input.starts_with(']') {
        let err_offset = orig.len() - input.len();
        return Err(ParseError::UnexpectedToken {
            expected: "']'",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset: err_offset,
        });
    }
    *input = &input[1..];

    Ok(Expr::Reference(TableRef::dot(table, row_name, col)))
}

fn parse_bracket_access(table: String, input: &mut &str, orig: &str) -> Result<Expr, ParseError> {
    // Consume '['
    *input = &input[1..];
    skip_ws(input);

    let row = parse_row_spec(input, orig)?;

    skip_ws(input);
    if input.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "',' separating row and column selectors",
        });
    }
    if !input.starts_with(',') {
        let err_offset = orig.len() - input.len();
        return Err(ParseError::UnexpectedToken {
            expected: "',' separating row and column selectors",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset: err_offset,
        });
    }
    *input = &input[1..];

    skip_ws(input);
    let col = parse_col_spec(input, orig)?;

    skip_ws(input);
    if input.is_empty() {
        return Err(ParseError::UnexpectedEof { expected: "']'" });
    }
    if !input.starts_with(']') {
        let err_offset = orig.len() - input.len();
        return Err(ParseError::UnexpectedToken {
            expected: "']'",
            found: input
                .chars()
                .next()
                .map_or("".to_string(), |c| c.to_string()),
            offset: err_offset,
        });
    }
    *input = &input[1..];

    Ok(Expr::Reference(TableRef::bracket(table, row, col)))
}

fn parse_row_spec(input: &mut &str, orig: &str) -> Result<RowSelector, ParseError> {
    skip_ws(input);
    let offset = orig.len() - input.len();

    // Special row: rows.<group>[.first|.last]
    if input.starts_with("rows.") {
        *input = &input[5..];
        let group_id = parse_identifier(input);
        if group_id.is_empty() {
            return Err(ParseError::UnexpectedToken {
                expected: "row group identifier after 'rows.'",
                found: input
                    .chars()
                    .next()
                    .map_or("".to_string(), |c| c.to_string()),
                offset,
            });
        }

        if input.starts_with(".first") && has_ident_boundary(input, 6) {
            *input = &input[6..];
            return Ok(RowSelector::Special(SpecialRow::GroupFirst(
                RowGroupId::new(group_id),
            )));
        } else if input.starts_with(".last") && has_ident_boundary(input, 5) {
            *input = &input[5..];
            return Ok(RowSelector::Special(SpecialRow::GroupLast(
                RowGroupId::new(group_id),
            )));
        } else {
            return Ok(RowSelector::Special(SpecialRow::Group(RowGroupId::new(
                group_id,
            ))));
        }
    }

    // Quoted string row key: "key" or "key_a".."key_b"
    if input.starts_with('"') {
        let key1 = parse_string_literal(input, orig)?;
        skip_ws(input);
        if input.starts_with("..") {
            *input = &input[2..];
            skip_ws(input);
            let key2 = parse_string_literal(input, orig)?;
            return Ok(RowSelector::Range(RowId::new(key1), RowId::new(key2)));
        }
        return Ok(RowSelector::Key(RowId::new(key1)));
    }

    // Bare identifier inside coordinates is prohibited
    if input.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
        let bare = parse_unquoted_coordinate_token(input);
        return Err(ParseError::UnquotedCoordinateKey {
            found: bare,
            offset,
        });
    }

    if input.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "row selector (quoted string key or 'rows.<group>')",
        });
    }

    Err(ParseError::UnexpectedToken {
        expected: "row selector (quoted string key or 'rows.<group>')",
        found: input
            .chars()
            .next()
            .map_or("".to_string(), |c| c.to_string()),
        offset,
    })
}

fn parse_col_spec(input: &mut &str, orig: &str) -> Result<ColSelector, ParseError> {
    skip_ws(input);
    let offset = orig.len() - input.len();

    // Active column: @
    if input.starts_with('@') {
        *input = &input[1..];
        return Ok(ColSelector::Active);
    }

    // Temporal predecessor: col.prev[(k)]
    if input.starts_with("col.prev") && has_ident_boundary(input, 8) {
        *input = &input[8..];
        skip_ws(input);
        if input.starts_with('(') {
            *input = &input[1..];
            skip_ws(input);
            let k = parse_u32(input, orig)?;
            skip_ws(input);
            if input.is_empty() {
                return Err(ParseError::UnexpectedEof {
                    expected: "')' after col.prev offset",
                });
            }
            if !input.starts_with(')') {
                let err_offset = orig.len() - input.len();
                return Err(ParseError::UnexpectedToken {
                    expected: "')' after col.prev offset",
                    found: input
                        .chars()
                        .next()
                        .map_or("".to_string(), |c| c.to_string()),
                    offset: err_offset,
                });
            }
            *input = &input[1..];
            return Ok(ColSelector::Special(SpecialCol::Prev(k)));
        }
        return Ok(ColSelector::Special(SpecialCol::Prev(1)));
    }

    // Temporal successor: col.next[(k)]
    if input.starts_with("col.next") && has_ident_boundary(input, 8) {
        *input = &input[8..];
        skip_ws(input);
        if input.starts_with('(') {
            *input = &input[1..];
            skip_ws(input);
            let k = parse_u32(input, orig)?;
            skip_ws(input);
            if input.is_empty() {
                return Err(ParseError::UnexpectedEof {
                    expected: "')' after col.next offset",
                });
            }
            if !input.starts_with(')') {
                let err_offset = orig.len() - input.len();
                return Err(ParseError::UnexpectedToken {
                    expected: "')' after col.next offset",
                    found: input
                        .chars()
                        .next()
                        .map_or("".to_string(), |c| c.to_string()),
                    offset: err_offset,
                });
            }
            *input = &input[1..];
            return Ok(ColSelector::Special(SpecialCol::Next(k)));
        }
        return Ok(ColSelector::Special(SpecialCol::Next(1)));
    }

    // Column group: cols.<group>[.first|.last]
    if input.starts_with("cols.") {
        *input = &input[5..];
        let group_id = parse_identifier(input);
        if group_id.is_empty() {
            return Err(ParseError::UnexpectedToken {
                expected: "column group identifier after 'cols.'",
                found: input
                    .chars()
                    .next()
                    .map_or("".to_string(), |c| c.to_string()),
                offset,
            });
        }

        if input.starts_with(".first") && has_ident_boundary(input, 6) {
            *input = &input[6..];
            return Ok(ColSelector::Special(SpecialCol::GroupFirst(
                ColGroupId::new(group_id),
            )));
        } else if input.starts_with(".last") && has_ident_boundary(input, 5) {
            *input = &input[5..];
            return Ok(ColSelector::Special(SpecialCol::GroupLast(
                ColGroupId::new(group_id),
            )));
        } else {
            return Ok(ColSelector::Special(SpecialCol::Group(ColGroupId::new(
                group_id,
            ))));
        }
    }

    // Quoted string column key: "key" or "key_a".."key_b"
    if input.starts_with('"') {
        let key1 = parse_string_literal(input, orig)?;
        skip_ws(input);
        if input.starts_with("..") {
            *input = &input[2..];
            skip_ws(input);
            let key2 = parse_string_literal(input, orig)?;
            return Ok(ColSelector::Range(ColId::new(key1), ColId::new(key2)));
        }
        return Ok(ColSelector::Key(ColId::new(key1)));
    }

    // Bare identifier inside coordinates is prohibited
    if input.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
        let bare = parse_unquoted_coordinate_token(input);
        return Err(ParseError::UnquotedCoordinateKey {
            found: bare,
            offset,
        });
    }

    if input.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "column selector ('@', quoted string key, 'col.prev', 'col.next', or 'cols.<group>')",
        });
    }

    Err(ParseError::UnexpectedToken {
        expected: "column selector ('@', quoted string key, 'col.prev', 'col.next', or 'cols.<group>')",
        found: input
            .chars()
            .next()
            .map_or("".to_string(), |c| c.to_string()),
        offset,
    })
}

fn parse_unquoted_coordinate_token(input: &mut &str) -> String {
    let res: Result<&str, ErrMode<ContextError>> = take_while(1.., |c: char| {
        !c.is_ascii_whitespace() && c != ']' && c != ','
    })
    .parse_next(input);
    match res {
        Ok(s) => s.to_string(),
        Err(_) => String::new(),
    }
}

fn parse_u32(input: &mut &str, orig: &str) -> Result<u32, ParseError> {
    let offset = orig.len() - input.len();
    let digits: Result<&str, ErrMode<ContextError>> = digit1.parse_next(input);
    let digits = digits.map_err(|_| ParseError::UnexpectedToken {
        expected: "integer offset",
        found: input
            .chars()
            .next()
            .map_or("".to_string(), |c| c.to_string()),
        offset,
    })?;

    digits
        .parse::<u32>()
        .map_err(|_| ParseError::InvalidNumber {
            raw: digits.to_string(),
            offset,
        })
}

fn peek_binary_op(input: &&str) -> Option<(BinaryOp, u8, bool)> {
    // Multi-char operators first
    if input.starts_with("<=") {
        return Some((BinaryOp::Lte, BinaryOp::Lte.precedence(), false));
    }
    if input.starts_with(">=") {
        return Some((BinaryOp::Gte, BinaryOp::Gte.precedence(), false));
    }
    if input.starts_with("<>") {
        return Some((BinaryOp::NotEq, BinaryOp::NotEq.precedence(), false));
    }

    // Single-char operators
    if input.starts_with('^') {
        return Some((BinaryOp::Pow, BinaryOp::Pow.precedence(), true));
    }
    if input.starts_with('*') {
        return Some((BinaryOp::Mul, BinaryOp::Mul.precedence(), false));
    }
    if input.starts_with('/') {
        return Some((BinaryOp::Div, BinaryOp::Div.precedence(), false));
    }
    if input.starts_with('+') {
        return Some((BinaryOp::Add, BinaryOp::Add.precedence(), false));
    }
    if input.starts_with('-') {
        return Some((BinaryOp::Sub, BinaryOp::Sub.precedence(), false));
    }
    if input.starts_with('=') {
        return Some((BinaryOp::Eq, BinaryOp::Eq.precedence(), false));
    }
    if input.starts_with('<') {
        return Some((BinaryOp::Lt, BinaryOp::Lt.precedence(), false));
    }
    if input.starts_with('>') {
        return Some((BinaryOp::Gt, BinaryOp::Gt.precedence(), false));
    }

    None
}

fn consume_binary_op(input: &mut &str, op: BinaryOp) {
    let len = op.as_str().len();
    *input = &input[len..];
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::id::{ColGroupId, ColId, RowGroupId, RowId, TableId};

    #[test]
    fn test_arithmetic_precedence_and_associativity() {
        let expr = parse_expr_str("1 + 2 * 3 ^ 2").unwrap();
        assert_eq!(
            expr,
            Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::number(1.0)),
                right: Box::new(Expr::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(Expr::number(2.0)),
                    right: Box::new(Expr::Binary {
                        op: BinaryOp::Pow,
                        left: Box::new(Expr::number(3.0)),
                        right: Box::new(Expr::number(2.0)),
                    }),
                }),
            }
        );
    }

    #[test]
    fn test_unary_operator_precedence() {
        let expr = parse_expr_str("-2 ^ 2").unwrap();
        assert_eq!(
            expr,
            Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(Expr::Binary {
                    op: BinaryOp::Pow,
                    left: Box::new(Expr::number(2.0)),
                    right: Box::new(Expr::number(2.0)),
                }),
            }
        );

        let double_neg = parse_expr_str("--5").unwrap();
        assert_eq!(
            double_neg,
            Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(Expr::number(5.0)),
                }),
            }
        );
    }

    #[test]
    fn test_exponentiation_right_associativity() {
        let expr = parse_expr_str("2 ^ 3 ^ 4").unwrap();
        assert_eq!(
            expr,
            Expr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(Expr::number(2.0)),
                right: Box::new(Expr::Binary {
                    op: BinaryOp::Pow,
                    left: Box::new(Expr::number(3.0)),
                    right: Box::new(Expr::number(4.0)),
                }),
            }
        );
    }

    #[test]
    fn test_subtraction_left_associativity() {
        let expr = parse_expr_str("10 - 4 - 2").unwrap();
        assert_eq!(
            expr,
            Expr::Binary {
                op: BinaryOp::Sub,
                left: Box::new(Expr::Binary {
                    op: BinaryOp::Sub,
                    left: Box::new(Expr::number(10.0)),
                    right: Box::new(Expr::number(4.0)),
                }),
                right: Box::new(Expr::number(2.0)),
            }
        );
    }

    #[test]
    fn test_parentheses_override_precedence() {
        let expr = parse_expr_str("(1 + 2) * 3").unwrap();
        assert_eq!(
            expr,
            Expr::Binary {
                op: BinaryOp::Mul,
                left: Box::new(Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::number(1.0)),
                    right: Box::new(Expr::number(2.0)),
                }),
                right: Box::new(Expr::number(3.0)),
            }
        );
    }

    #[test]
    fn test_relational_comparisons() {
        let expr = parse_expr_str("IncomeStatement.revenue[@] >= 100").unwrap();
        assert_eq!(
            expr,
            Expr::Binary {
                op: BinaryOp::Gte,
                left: Box::new(Expr::Reference(TableRef::dot(
                    "IncomeStatement",
                    "revenue",
                    ColSelector::Active,
                ))),
                right: Box::new(Expr::number(100.0)),
            }
        );

        let ops = [
            ("=", BinaryOp::Eq),
            ("<>", BinaryOp::NotEq),
            ("<", BinaryOp::Lt),
            ("<=", BinaryOp::Lte),
            (">", BinaryOp::Gt),
            (">=", BinaryOp::Gte),
        ];

        for (op_str, op) in ops {
            let formula = format!("10 {op_str} 20");
            let parsed = parse_expr_str(&formula).unwrap();
            assert_eq!(
                parsed,
                Expr::Binary {
                    op,
                    left: Box::new(Expr::number(10.0)),
                    right: Box::new(Expr::number(20.0)),
                }
            );
        }
    }

    #[test]
    fn test_function_calls() {
        let expr = parse_expr_str("IF(Assumptions.flag[\"active\"] = 1, 100, 0)").unwrap();
        match expr {
            Expr::FunctionCall { name, args } => {
                assert_eq!(name, "IF");
                assert_eq!(args.len(), 3);
                assert_eq!(
                    args[0],
                    Expr::Binary {
                        op: BinaryOp::Eq,
                        left: Box::new(Expr::Reference(TableRef::dot(
                            "Assumptions",
                            "flag",
                            ColSelector::Key(ColId::new("active")),
                        ))),
                        right: Box::new(Expr::number(1.0)),
                    }
                );
                assert_eq!(args[1], Expr::number(100.0));
                assert_eq!(args[2], Expr::number(0.0));
            }
            other => panic!("Expected FunctionCall, got {other:?}"),
        }

        let nullary = parse_expr_str("PI()").unwrap();
        assert_eq!(
            nullary,
            Expr::FunctionCall {
                name: "PI".to_string(),
                args: vec![],
            }
        );
    }

    #[test]
    fn test_dot_access_addressing_forms() {
        // Active column
        let active = parse_expr_str("IncomeStatement.revenue[@]").unwrap();
        assert_eq!(
            active,
            Expr::Reference(TableRef::dot(
                TableId::new("IncomeStatement"),
                RowId::new("revenue"),
                ColSelector::Active,
            ))
        );

        // Specific column key
        let specific = parse_expr_str("Assumptions.tax_rate[\"value\"]").unwrap();
        assert_eq!(
            specific,
            Expr::Reference(TableRef::dot(
                TableId::new("Assumptions"),
                RowId::new("tax_rate"),
                ColSelector::Key(ColId::new("value")),
            ))
        );

        // col.prev default (1)
        let prev1 = parse_expr_str("BalanceSheet.cash[col.prev]").unwrap();
        assert_eq!(
            prev1,
            Expr::Reference(TableRef::dot(
                TableId::new("BalanceSheet"),
                RowId::new("cash"),
                ColSelector::Special(SpecialCol::Prev(1)),
            ))
        );

        // col.prev(2)
        let prev2 = parse_expr_str("BalanceSheet.cash[col.prev(2)]").unwrap();
        assert_eq!(
            prev2,
            Expr::Reference(TableRef::dot(
                TableId::new("BalanceSheet"),
                RowId::new("cash"),
                ColSelector::Special(SpecialCol::Prev(2)),
            ))
        );

        // col.next(3)
        let next3 = parse_expr_str("BalanceSheet.cash[col.next(3)]").unwrap();
        assert_eq!(
            next3,
            Expr::Reference(TableRef::dot(
                TableId::new("BalanceSheet"),
                RowId::new("cash"),
                ColSelector::Special(SpecialCol::Next(3)),
            ))
        );

        // cols.historical.last
        let hist_last = parse_expr_str("DebtSchedule.balance[cols.historical.last]").unwrap();
        assert_eq!(
            hist_last,
            Expr::Reference(TableRef::dot(
                TableId::new("DebtSchedule"),
                RowId::new("balance"),
                ColSelector::Special(SpecialCol::GroupLast(ColGroupId::new("historical"))),
            ))
        );

        // cols.forecast.first
        let fc_first = parse_expr_str("DebtSchedule.balance[cols.forecast.first]").unwrap();
        assert_eq!(
            fc_first,
            Expr::Reference(TableRef::dot(
                TableId::new("DebtSchedule"),
                RowId::new("balance"),
                ColSelector::Special(SpecialCol::GroupFirst(ColGroupId::new("forecast"))),
            ))
        );

        // Horizontal range
        let h_range = parse_expr_str("SUM(IncomeStatement.revenue[\"2024E\"..\"2028E\"])").unwrap();
        assert_eq!(
            h_range,
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![Expr::Reference(TableRef::dot(
                    TableId::new("IncomeStatement"),
                    RowId::new("revenue"),
                    ColSelector::Range(ColId::new("2024E"), ColId::new("2028E")),
                ))],
            }
        );
    }

    #[test]
    fn test_bracket_access_addressing_forms() {
        // Vertical slice active column: Opex["r_and_d".."g_and_a", @]
        let v_slice = parse_expr_str("Opex[\"r_and_d\"..\"g_and_a\", @]").unwrap();
        assert_eq!(
            v_slice,
            Expr::Reference(TableRef::bracket(
                TableId::new("Opex"),
                RowSelector::Range(RowId::new("r_and_d"), RowId::new("g_and_a")),
                ColSelector::Active,
            ))
        );

        // Row group active column: BalanceSheet[rows.current_assets, @]
        let rg_active = parse_expr_str("BalanceSheet[rows.current_assets, @]").unwrap();
        assert_eq!(
            rg_active,
            Expr::Reference(TableRef::bracket(
                TableId::new("BalanceSheet"),
                RowSelector::Special(SpecialRow::Group(RowGroupId::new("current_assets"))),
                ColSelector::Active,
            ))
        );

        // Row group specific column: BalanceSheet[rows.current_assets, "2024E"]
        let rg_specific = parse_expr_str("BalanceSheet[rows.current_assets, \"2024E\"]").unwrap();
        assert_eq!(
            rg_specific,
            Expr::Reference(TableRef::bracket(
                TableId::new("BalanceSheet"),
                RowSelector::Special(SpecialRow::Group(RowGroupId::new("current_assets"))),
                ColSelector::Key(ColId::new("2024E")),
            ))
        );

        // Single row key specific column: BalanceSheet["cash", "2024E"]
        let single_row = parse_expr_str("BalanceSheet[\"cash\", \"2024E\"]").unwrap();
        assert_eq!(
            single_row,
            Expr::Reference(TableRef::bracket(
                TableId::new("BalanceSheet"),
                RowSelector::Key(RowId::new("cash")),
                ColSelector::Key(ColId::new("2024E")),
            ))
        );

        // 2D block range: Opex["r_and_d".."g_and_a", "2024E".."2028E"]
        let block2d =
            parse_expr_str("Opex[\"r_and_d\"..\"g_and_a\", \"2024E\"..\"2028E\"]").unwrap();
        assert_eq!(
            block2d,
            Expr::Reference(TableRef::bracket(
                TableId::new("Opex"),
                RowSelector::Range(RowId::new("r_and_d"), RowId::new("g_and_a")),
                ColSelector::Range(ColId::new("2024E"), ColId::new("2028E")),
            ))
        );

        // 2D groups: BalanceSheet[rows.current_assets, cols.forecast]
        let block_groups =
            parse_expr_str("BalanceSheet[rows.current_assets, cols.forecast]").unwrap();
        assert_eq!(
            block_groups,
            Expr::Reference(TableRef::bracket(
                TableId::new("BalanceSheet"),
                RowSelector::Special(SpecialRow::Group(RowGroupId::new("current_assets"))),
                ColSelector::Special(SpecialCol::Group(ColGroupId::new("forecast"))),
            ))
        );
    }

    #[test]
    fn test_reject_unquoted_coordinate_keys() {
        let err1 = parse_expr_str("IncomeStatement.revenue[2024E]").unwrap_err();
        match err1 {
            ParseError::UnquotedCoordinateKey { found, .. } => assert_eq!(found, "2024E"),
            other => panic!("Expected UnquotedCoordinateKey, got {other:?}"),
        }

        let err2 = parse_expr_str("Opex[r_and_d, @]").unwrap_err();
        match err2 {
            ParseError::UnquotedCoordinateKey { found, .. } => assert_eq!(found, "r_and_d"),
            other => panic!("Expected UnquotedCoordinateKey, got {other:?}"),
        }
    }

    #[test]
    fn test_reject_lowercase_function_name() {
        let err = parse_expr_str("sum(1, 2)").unwrap_err();
        match err {
            ParseError::InvalidFunctionName { name, .. } => assert_eq!(name, "sum"),
            other => panic!("Expected InvalidFunctionName, got {other:?}"),
        }
    }

    #[test]
    fn test_reject_bare_identifiers() {
        // Bare row access without column brackets at EOF
        let err_row = parse_expr_str("IncomeStatement.revenue").unwrap_err();
        assert_eq!(err_row, ParseError::UnexpectedEof { expected: "'['" });

        // Bare table identifier without access operator
        let err_table = parse_expr_str("IncomeStatement").unwrap_err();
        match err_table {
            ParseError::UnexpectedToken { found, .. } => assert_eq!(found, "IncomeStatement"),
            other => panic!("Expected UnexpectedToken, got {other:?}"),
        }
    }

    #[test]
    fn test_keyword_boundary_validation() {
        // col.prevent should not parse as col.prev
        let err = parse_expr_str("Table.row[col.prevent]").unwrap_err();
        match err {
            ParseError::UnquotedCoordinateKey { found, .. } => assert_eq!(found, "col.prevent"),
            ParseError::UnexpectedToken { .. } => {}
            other => panic!("Expected unquoted coordinate key error, got {other:?}"),
        }

        // rows.group.firstly should not parse as rows.group.first
        let err = parse_expr_str("Table[rows.group.firstly, @]").unwrap_err();
        match err {
            ParseError::UnexpectedToken { .. } => {}
            other => panic!("Expected unexpected token error, got {other:?}"),
        }
    }

    #[test]
    fn test_unexpected_eof_on_unclosed_delimiters() {
        assert_eq!(
            parse_expr_str("(1 + 2").unwrap_err(),
            ParseError::UnexpectedEof { expected: "')'" }
        );
        assert_eq!(
            parse_expr_str("Table.row[@").unwrap_err(),
            ParseError::UnexpectedEof { expected: "']'" }
        );
        assert_eq!(
            parse_expr_str("Table[rows.opex, @").unwrap_err(),
            ParseError::UnexpectedEof { expected: "']'" }
        );
        assert_eq!(
            parse_expr_str("SUM(1, 2").unwrap_err(),
            ParseError::UnexpectedEof {
                expected: "',' or ')'"
            }
        );
    }

    #[test]
    fn test_display_roundtrip() {
        let formulas = [
            "1 + 2 * 3 ^ 2",
            "-2 ^ 2",
            "10 - 4 - 2",
            "(1 + 2) * 3",
            "IF(Assumptions.flag[\"active\"] = 1, 100, 0)",
            "IncomeStatement.revenue[@] - IncomeStatement.cogs[@]",
            "SUM(IncomeStatement.revenue[\"2024E\"..\"2028E\"])",
            "Opex[\"r_and_d\"..\"g_and_a\", @]",
            "BalanceSheet[rows.current_assets, cols.forecast]",
            "BalanceSheet.cash[col.prev]",
            "BalanceSheet.cash[col.prev(2)]",
            "DebtSchedule.balance[cols.historical.last]",
            "(2 ^ 3) ^ 4",
            "2 ^ (3 ^ 4)",
            "BalanceSheet[\"cash\", @]",
        ];

        for formula in formulas {
            let parsed1 = parse_expr_str(formula).unwrap();
            let emitted = parsed1.to_string();
            let parsed2 = parse_expr_str(&emitted).unwrap();
            assert_eq!(
                parsed1, parsed2,
                "Roundtrip failed for formula: {formula} -> emitted: {emitted}"
            );
        }
    }
}
