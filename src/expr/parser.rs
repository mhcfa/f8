use crate::expr::ast::{
    BinaryOp, ColSelector, Expr, Literal, RowSelector, SpecialCol, SpecialRow, TableRef, UnaryOp,
};
use crate::expr::error::ParseError;
use crate::model::id::{ColGroupId, ColId, RowGroupId, RowId};
use std::fmt;

/// Lexical token recognized by the formula tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token<'a> {
    /// Floating-point or integer numeric literal.
    Number(f64),
    /// Quoted text string constant.
    String(String),
    /// Explicit null literal (`null`).
    Null,
    /// General identifier (table name, row/col identifier, function name, or coordinate word).
    Ident(&'a str),

    // Delimiters & punctuation
    /// Left parenthesis `(`.
    LParen,
    /// Right parenthesis `)`.
    RParen,
    /// Left bracket `[`.
    LBracket,
    /// Right bracket `]`.
    RBracket,
    /// Comma `,`.
    Comma,
    /// Period `.`.
    Dot,
    /// Range slice delimiter `..`.
    DotDot,
    /// Active evaluation column symbol `@col`.
    AtCol,
    /// Active evaluation row symbol `@row`.
    AtRow,

    // Operators
    /// Plus operator `+`.
    Plus,
    /// Minus operator `-`.
    Minus,
    /// Multiplication operator `*`.
    Star,
    /// Division operator `/`.
    Slash,
    /// Exponentiation operator `^`.
    Caret,
    /// Equality comparison `=`.
    Eq,
    /// Inequality comparison `<>`.
    NotEq,
    /// Less-than comparison `<`.
    Lt,
    /// Less-than-or-equal comparison `<=`.
    Lte,
    /// Greater-than comparison `>`.
    Gt,
    /// Greater-than-or-equal comparison `>=`.
    Gte,
}

impl<'a> Token<'a> {
    /// Maps the token to a binary operator if applicable.
    pub fn as_binary_op(&self) -> Option<BinaryOp> {
        match self {
            Self::Plus => Some(BinaryOp::Add),
            Self::Minus => Some(BinaryOp::Sub),
            Self::Star => Some(BinaryOp::Mul),
            Self::Slash => Some(BinaryOp::Div),
            Self::Caret => Some(BinaryOp::Pow),
            Self::Eq => Some(BinaryOp::Eq),
            Self::NotEq => Some(BinaryOp::NotEq),
            Self::Lt => Some(BinaryOp::Lt),
            Self::Lte => Some(BinaryOp::Lte),
            Self::Gt => Some(BinaryOp::Gt),
            Self::Gte => Some(BinaryOp::Gte),
            _ => None,
        }
    }
}

impl<'a> fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Null => write!(f, "null"),
            Self::Ident(id) => write!(f, "{id}"),
            Self::LParen => write!(f, "("),
            Self::RParen => write!(f, ")"),
            Self::LBracket => write!(f, "["),
            Self::RBracket => write!(f, "]"),
            Self::Comma => write!(f, ","),
            Self::Dot => write!(f, "."),
            Self::DotDot => write!(f, ".."),
            Self::AtCol => write!(f, "@col"),
            Self::AtRow => write!(f, "@row"),
            Self::Plus => write!(f, "+"),
            Self::Minus => write!(f, "-"),
            Self::Star => write!(f, "*"),
            Self::Slash => write!(f, "/"),
            Self::Caret => write!(f, "^"),
            Self::Eq => write!(f, "="),
            Self::NotEq => write!(f, "<>"),
            Self::Lt => write!(f, "<"),
            Self::Lte => write!(f, "<="),
            Self::Gt => write!(f, ">"),
            Self::Gte => write!(f, ">="),
        }
    }
}

/// A token tagged with its source byte range.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpannedToken<'a> {
    /// The token kind and payload.
    pub token: Token<'a>,
    /// Byte offset where the token begins in the original input string.
    pub offset: usize,
    /// Length of the token in bytes.
    pub len: usize,
}

/// Cursor over a slice of spanned tokens providing lookahead and consumption utilities.
#[derive(Debug, Clone)]
pub(crate) struct TokenStream<'a> {
    tokens: Vec<SpannedToken<'a>>,
    pos: usize,
    source_len: usize,
}

impl<'a> TokenStream<'a> {
    /// Creates a new token stream cursor over the given tokens and input source byte length.
    pub fn new(tokens: Vec<SpannedToken<'a>>, source_len: usize) -> Self {
        Self {
            tokens,
            pos: 0,
            source_len,
        }
    }

    /// Peeks at the next spanned token without advancing the cursor.
    pub fn peek(&self) -> Option<&SpannedToken<'a>> {
        self.tokens.get(self.pos)
    }

    /// Peeks at the next token kind without advancing the cursor.
    pub fn peek_token(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.pos).map(|st| &st.token)
    }

    /// Advances the cursor by one token position without allocation or cloning.
    pub fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    /// Consumes and returns a reference to the next spanned token.
    pub fn consume(&mut self) -> Option<&SpannedToken<'a>> {
        if self.pos < self.tokens.len() {
            let idx = self.pos;
            self.pos += 1;
            Some(&self.tokens[idx])
        } else {
            None
        }
    }

    /// Returns the byte offset of the current cursor position.
    pub fn current_offset(&self) -> usize {
        self.peek().map_or(self.source_len, |st| st.offset)
    }

    /// Returns `true` if all tokens have been consumed.
    pub fn is_empty(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Asserts that the next token matches `expected_tok`, consuming it if so.
    pub fn expect(
        &mut self,
        expected_tok: &Token<'_>,
        expected_str: &'static str,
    ) -> Result<&SpannedToken<'a>, ParseError> {
        match self.peek() {
            Some(st) if &st.token == expected_tok => {
                let idx = self.pos;
                self.pos += 1;
                Ok(&self.tokens[idx])
            }
            Some(st) => Err(ParseError::UnexpectedToken {
                expected: expected_str,
                found: st.token.to_string(),
                offset: st.offset,
            }),
            None => Err(ParseError::UnexpectedEof {
                expected: expected_str,
            }),
        }
    }
}

/// Tokenizes a raw formula expression string into a sequence of [`SpannedToken`]s.
pub(crate) fn tokenize(input: &str) -> Result<Vec<SpannedToken<'_>>, ParseError> {
    let mut tokens = Vec::new();
    let mut cursor = input;

    while !cursor.is_empty() {
        // Strip leading whitespace
        let trimmed = cursor.trim_start();
        if trimmed.is_empty() {
            break;
        }
        cursor = trimmed;

        let offset = input.len() - cursor.len();

        // 2-character tokens
        if cursor.starts_with("..") {
            tokens.push(SpannedToken {
                token: Token::DotDot,
                offset,
                len: 2,
            });
            cursor = &cursor[2..];
            continue;
        }
        if cursor.starts_with("<=") {
            tokens.push(SpannedToken {
                token: Token::Lte,
                offset,
                len: 2,
            });
            cursor = &cursor[2..];
            continue;
        }
        if cursor.starts_with(">=") {
            tokens.push(SpannedToken {
                token: Token::Gte,
                offset,
                len: 2,
            });
            cursor = &cursor[2..];
            continue;
        }
        if cursor.starts_with("<>") {
            tokens.push(SpannedToken {
                token: Token::NotEq,
                offset,
                len: 2,
            });
            cursor = &cursor[2..];
            continue;
        }

        let first_char = match cursor.chars().next() {
            Some(c) => c,
            None => break,
        };

        // 1-character delimiters and operators
        match first_char {
            '(' => {
                tokens.push(SpannedToken {
                    token: Token::LParen,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            ')' => {
                tokens.push(SpannedToken {
                    token: Token::RParen,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '[' => {
                tokens.push(SpannedToken {
                    token: Token::LBracket,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            ']' => {
                tokens.push(SpannedToken {
                    token: Token::RBracket,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            ',' => {
                tokens.push(SpannedToken {
                    token: Token::Comma,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '.' => {
                tokens.push(SpannedToken {
                    token: Token::Dot,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '@' => {
                if cursor.starts_with("@col")
                    && cursor
                        .as_bytes()
                        .get(4)
                        .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_')
                {
                    tokens.push(SpannedToken {
                        token: Token::AtCol,
                        offset,
                        len: 4,
                    });
                    cursor = &cursor[4..];
                    continue;
                }
                if cursor.starts_with("@row")
                    && cursor
                        .as_bytes()
                        .get(4)
                        .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_')
                {
                    tokens.push(SpannedToken {
                        token: Token::AtRow,
                        offset,
                        len: 4,
                    });
                    cursor = &cursor[4..];
                    continue;
                }
                return Err(ParseError::RetiredBareAt { offset });
            }
            '+' => {
                tokens.push(SpannedToken {
                    token: Token::Plus,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '-' => {
                tokens.push(SpannedToken {
                    token: Token::Minus,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '*' => {
                tokens.push(SpannedToken {
                    token: Token::Star,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '/' => {
                tokens.push(SpannedToken {
                    token: Token::Slash,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '^' => {
                tokens.push(SpannedToken {
                    token: Token::Caret,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '=' => {
                tokens.push(SpannedToken {
                    token: Token::Eq,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '<' => {
                tokens.push(SpannedToken {
                    token: Token::Lt,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '>' => {
                tokens.push(SpannedToken {
                    token: Token::Gt,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
            }
            '"' => {
                cursor = &cursor[1..];
                let mut out = String::new();
                let mut chars = cursor.char_indices();
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
                    cursor = &cursor[idx + 1..];
                    let total_len = 1 + idx + 1;
                    tokens.push(SpannedToken {
                        token: Token::String(out),
                        offset,
                        len: total_len,
                    });
                    continue;
                }
                return Err(ParseError::UnexpectedEof {
                    expected: "closing quote '\"'",
                });
            }
            _ => {}
        }

        // Digits (numeric literals or alphanumeric coordinate tokens like 2024E)
        if first_char.is_ascii_digit() {
            let word_len = cursor
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            let word = &cursor[..word_len];

            // Check if token contains letters or underscores (e.g. 2024E)
            let has_letters = word.bytes().any(|b| b.is_ascii_alphabetic() || b == b'_');
            if has_letters {
                cursor = &cursor[word_len..];
                tokens.push(SpannedToken {
                    token: Token::Ident(word),
                    offset,
                    len: word_len,
                });
                continue;
            }

            // Word is purely digits. Check for decimal point:
            let mut num_len = word_len;
            let after_digits = &cursor[word_len..];
            if after_digits.starts_with('.') && !after_digits.starts_with("..") {
                let frac = &after_digits[1..];
                let frac_digits = frac.bytes().take_while(|b| b.is_ascii_digit()).count();
                if frac_digits > 0 {
                    num_len += 1 + frac_digits;
                }
            }

            let num_str = &cursor[..num_len];
            match num_str.parse::<f64>() {
                Ok(val) => {
                    cursor = &cursor[num_len..];
                    tokens.push(SpannedToken {
                        token: Token::Number(val),
                        offset,
                        len: num_len,
                    });
                    continue;
                }
                Err(_) => {
                    return Err(ParseError::InvalidNumber {
                        raw: num_str.to_string(),
                        offset,
                    });
                }
            }
        }

        // Identifier or keyword
        if first_char.is_ascii_alphabetic() || first_char == '_' {
            let ident_len = cursor
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            let ident = &cursor[..ident_len];
            cursor = &cursor[ident_len..];

            if ident == "null" {
                tokens.push(SpannedToken {
                    token: Token::Null,
                    offset,
                    len: ident_len,
                });
            } else {
                tokens.push(SpannedToken {
                    token: Token::Ident(ident),
                    offset,
                    len: ident_len,
                });
            }
            continue;
        }

        // Unknown character
        return Err(ParseError::UnexpectedToken {
            expected: "expression",
            found: first_char.to_string(),
            offset,
        });
    }

    Ok(tokens)
}

/// Parses a raw formula expression string into an [`Expr`] AST.
pub(crate) fn parse_expr_str(input: &str) -> Result<Expr, ParseError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "expression",
        });
    }

    let mut stream = TokenStream::new(tokens, input.len());
    let expr = parse_expression(&mut stream, 0)?;

    if !stream.is_empty() {
        let offset = stream.current_offset();
        let trailing = input[offset..].to_string();
        return Err(ParseError::TrailingInput { trailing, offset });
    }

    Ok(expr)
}

fn parse_expression(stream: &mut TokenStream<'_>, min_bp: u8) -> Result<Expr, ParseError> {
    let mut left = parse_prefix(stream)?;

    while let Some(tok) = stream.peek() {
        let Some(op) = tok.token.as_binary_op() else {
            break;
        };

        let op_prec = op.precedence();
        if op_prec < min_bp {
            break;
        }

        // Consume binary operator
        stream.advance();

        let next_bp = if op.is_right_associative() {
            op_prec
        } else {
            op_prec + 1
        };

        let right = parse_expression(stream, next_bp)?;
        left = Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        };
    }

    Ok(left)
}

fn parse_prefix(stream: &mut TokenStream<'_>) -> Result<Expr, ParseError> {
    let Some(st) = stream.peek() else {
        return Err(ParseError::UnexpectedEof {
            expected: "expression",
        });
    };

    let offset = st.offset;
    match &st.token {
        Token::Plus => {
            stream.advance();
            let expr = parse_expression(stream, UnaryOp::Pos.precedence())?;
            Ok(Expr::Unary {
                op: UnaryOp::Pos,
                expr: Box::new(expr),
            })
        }
        Token::Minus => {
            stream.advance();
            let expr = parse_expression(stream, UnaryOp::Neg.precedence())?;
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            })
        }
        Token::LParen => {
            stream.advance();
            let expr = parse_expression(stream, 0)?;
            let _ = stream.expect(&Token::RParen, "')'")?;
            Ok(expr)
        }
        Token::Number(n) => {
            let val = *n;
            stream.advance();
            Ok(Expr::Literal(Literal::Number(val)))
        }
        Token::String(s) => {
            let val = s.clone();
            stream.advance();
            Ok(Expr::Literal(Literal::String(val)))
        }
        Token::Null => {
            stream.advance();
            Ok(Expr::Literal(Literal::Null))
        }
        Token::Ident(ident) => {
            let name = *ident;
            stream.advance();

            match stream.peek() {
                Some(st) if st.token == Token::LParen => parse_function_call(name, offset, stream),
                Some(st) if st.token == Token::LBracket => parse_bracket_access(name, stream),
                Some(st) => Err(ParseError::UnexpectedToken {
                    expected: "function call '(' or table reference '['",
                    found: st.token.to_string(),
                    offset: st.offset,
                }),
                None => Err(ParseError::UnexpectedToken {
                    expected: "function call '(' or table reference '['",
                    found: name.to_string(),
                    offset,
                }),
            }
        }
        other => {
            let found = other.to_string();
            Err(ParseError::UnexpectedToken {
                expected: "expression",
                found,
                offset,
            })
        }
    }
}

fn parse_argument_list(stream: &mut TokenStream<'_>) -> Result<Vec<Expr>, ParseError> {
    if stream.peek_token() == Some(&Token::RParen) {
        return Ok(Vec::new());
    }

    let mut args = Vec::new();
    loop {
        if stream.is_empty() {
            return Err(ParseError::UnexpectedEof {
                expected: "expression or ')'",
            });
        }

        let arg = parse_expression(stream, 0)?;
        args.push(arg);

        if stream.peek_token() == Some(&Token::Comma) {
            stream.advance();
            if stream.peek_token() == Some(&Token::RParen) {
                let err_offset = stream.current_offset();
                return Err(ParseError::UnexpectedToken {
                    expected: "expression after ','",
                    found: ")".to_string(),
                    offset: err_offset,
                });
            }
            continue;
        }

        if stream.peek_token() == Some(&Token::RParen) {
            break;
        }

        if stream.is_empty() {
            return Err(ParseError::UnexpectedEof {
                expected: "',' or ')'",
            });
        }

        let err_offset = stream.current_offset();
        let found = stream
            .peek()
            .map_or_else(String::new, |st| st.token.to_string());
        return Err(ParseError::UnexpectedToken {
            expected: "',' or ')'",
            found,
            offset: err_offset,
        });
    }

    Ok(args)
}

fn parse_function_call(
    name: &str,
    offset: usize,
    stream: &mut TokenStream<'_>,
) -> Result<Expr, ParseError> {
    // Validate uppercase function name
    let is_valid = !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');

    if !is_valid {
        return Err(ParseError::InvalidFunctionName {
            name: name.to_string(),
            offset,
        });
    }

    // Consume '('
    let _ = stream.expect(&Token::LParen, "'('")?;
    let args = parse_argument_list(stream)?;
    // Consume ')'
    let _ = stream.expect(&Token::RParen, "')'")?;

    Ok(Expr::FunctionCall {
        name: name.to_string(),
        args,
    })
}

fn parse_bracket_access(table: &str, stream: &mut TokenStream<'_>) -> Result<Expr, ParseError> {
    // Consume '['
    let _ = stream.expect(&Token::LBracket, "'['")?;

    let row = parse_row_spec(stream)?;
    let _ = stream.expect(&Token::Comma, "',' separating row and column selectors")?;
    let col = parse_col_spec(stream)?;
    let _ = stream.expect(&Token::RBracket, "']'")?;

    Ok(Expr::Reference(TableRef::new(table, row, col)))
}

fn parse_row_spec(stream: &mut TokenStream<'_>) -> Result<RowSelector, ParseError> {
    let Some(st) = stream.peek() else {
        return Err(ParseError::UnexpectedEof {
            expected: "row selector ('@row', quoted string key, 'row.prev', 'row.next', or 'rows.<group>')",
        });
    };

    let offset = st.offset;
    match &st.token {
        Token::AtRow => {
            stream.advance();
            Ok(RowSelector::Active)
        }
        Token::Ident("row") => {
            stream.advance();
            // Expect '.'
            let _ = stream.expect(&Token::Dot, "'.' after 'row'")?;

            // Expect 'prev' or 'next'
            let is_prev = match stream.peek_token() {
                Some(Token::Ident("prev")) => true,
                Some(Token::Ident("next")) => false,
                Some(other) => {
                    let err_offset = stream.current_offset();
                    let found = other.to_string();
                    return Err(ParseError::UnexpectedToken {
                        expected: "'prev' or 'next'",
                        found,
                        offset: err_offset,
                    });
                }
                None => {
                    return Err(ParseError::UnexpectedEof {
                        expected: "'prev' or 'next'",
                    });
                }
            };
            stream.advance();

            // Check for optional (k)
            if stream.peek_token() == Some(&Token::LParen) {
                stream.advance();
                let k = match stream.consume() {
                    Some(num_st) => match num_st.token {
                        Token::Number(n) if n.fract() == 0.0 && n >= 0.0 => n as u32,
                        ref other => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "integer offset",
                                found: other.to_string(),
                                offset: num_st.offset,
                            });
                        }
                    },
                    None => {
                        let expected = if is_prev {
                            "')' after row.prev offset"
                        } else {
                            "')' after row.next offset"
                        };
                        return Err(ParseError::UnexpectedEof { expected });
                    }
                };

                let expected_rparen = if is_prev {
                    "')' after row.prev offset"
                } else {
                    "')' after row.next offset"
                };
                let _ = stream.expect(&Token::RParen, expected_rparen)?;

                if is_prev {
                    Ok(RowSelector::Special(SpecialRow::Prev(k)))
                } else {
                    Ok(RowSelector::Special(SpecialRow::Next(k)))
                }
            } else if is_prev {
                Ok(RowSelector::Special(SpecialRow::Prev(1)))
            } else {
                Ok(RowSelector::Special(SpecialRow::Next(1)))
            }
        }
        Token::Ident("rows") => {
            stream.advance();
            // Expect '.'
            let _ = stream.expect(&Token::Dot, "'.' after 'rows'")?;

            // Expect group identifier
            let group_id = match stream.consume() {
                Some(next_st) => match next_st.token {
                    Token::Ident(id) => id,
                    ref other => {
                        return Err(ParseError::UnexpectedToken {
                            expected: "row group identifier after 'rows.'",
                            found: other.to_string(),
                            offset: next_st.offset,
                        });
                    }
                },
                None => {
                    return Err(ParseError::UnexpectedEof {
                        expected: "row group identifier after 'rows.'",
                    });
                }
            };

            // Check for optional .first or .last
            if stream.peek_token() == Some(&Token::Dot) {
                stream.advance();
                match stream.consume() {
                    Some(milestone_st) => match milestone_st.token {
                        Token::Ident("first") => Ok(RowSelector::Special(SpecialRow::GroupFirst(
                            RowGroupId::new(group_id),
                        ))),
                        Token::Ident("last") => Ok(RowSelector::Special(SpecialRow::GroupLast(
                            RowGroupId::new(group_id),
                        ))),
                        ref other => Err(ParseError::UnexpectedToken {
                            expected: "'first' or 'last'",
                            found: other.to_string(),
                            offset: milestone_st.offset,
                        }),
                    },
                    None => Err(ParseError::UnexpectedEof {
                        expected: "'first' or 'last'",
                    }),
                }
            } else {
                Ok(RowSelector::Special(SpecialRow::Group(RowGroupId::new(
                    group_id,
                ))))
            }
        }
        Token::String(key1) => {
            let key1 = key1.clone();
            stream.advance();
            if stream.peek_token() == Some(&Token::DotDot) {
                stream.advance();
                match stream.consume() {
                    Some(key2_st) => match &key2_st.token {
                        Token::String(key2) => {
                            Ok(RowSelector::Range(RowId::new(key1), RowId::new(key2)))
                        }
                        other => Err(ParseError::UnexpectedToken {
                            expected: "closing row range key string",
                            found: other.to_string(),
                            offset: key2_st.offset,
                        }),
                    },
                    None => Err(ParseError::UnexpectedEof {
                        expected: "closing row range key string",
                    }),
                }
            } else {
                Ok(RowSelector::Key(RowId::new(key1)))
            }
        }
        Token::Ident(bare) => {
            let found = (*bare).to_string();
            stream.advance();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        Token::Number(num) => {
            let found = num.to_string();
            stream.advance();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        other => {
            let found = other.to_string();
            Err(ParseError::UnexpectedToken {
                expected: "row selector ('@row', quoted string key, 'row.prev', 'row.next', or 'rows.<group>')",
                found,
                offset,
            })
        }
    }
}

fn parse_col_spec(stream: &mut TokenStream<'_>) -> Result<ColSelector, ParseError> {
    let Some(st) = stream.peek() else {
        return Err(ParseError::UnexpectedEof {
            expected: "column selector ('@col', quoted string key, 'col.prev', 'col.next', or 'cols.<group>')",
        });
    };

    let offset = st.offset;
    match &st.token {
        Token::AtCol => {
            stream.advance();
            Ok(ColSelector::Active)
        }
        Token::Ident("col") => {
            stream.advance();
            // Expect '.'
            let _ = stream.expect(&Token::Dot, "'.' after 'col'")?;

            // Expect 'prev' or 'next'
            let is_prev = match stream.peek_token() {
                Some(Token::Ident("prev")) => true,
                Some(Token::Ident("next")) => false,
                Some(other) => {
                    let err_offset = stream.current_offset();
                    let found = other.to_string();
                    return Err(ParseError::UnexpectedToken {
                        expected: "'prev' or 'next'",
                        found,
                        offset: err_offset,
                    });
                }
                None => {
                    return Err(ParseError::UnexpectedEof {
                        expected: "'prev' or 'next'",
                    });
                }
            };
            stream.advance();

            // Check for optional (k)
            if stream.peek_token() == Some(&Token::LParen) {
                stream.advance();
                let k = match stream.consume() {
                    Some(num_st) => match num_st.token {
                        Token::Number(n) if n.fract() == 0.0 && n >= 0.0 => n as u32,
                        ref other => {
                            return Err(ParseError::UnexpectedToken {
                                expected: "integer offset",
                                found: other.to_string(),
                                offset: num_st.offset,
                            });
                        }
                    },
                    None => {
                        let expected = if is_prev {
                            "')' after col.prev offset"
                        } else {
                            "')' after col.next offset"
                        };
                        return Err(ParseError::UnexpectedEof { expected });
                    }
                };

                let expected_rparen = if is_prev {
                    "')' after col.prev offset"
                } else {
                    "')' after col.next offset"
                };
                let _ = stream.expect(&Token::RParen, expected_rparen)?;

                if is_prev {
                    Ok(ColSelector::Special(SpecialCol::Prev(k)))
                } else {
                    Ok(ColSelector::Special(SpecialCol::Next(k)))
                }
            } else if is_prev {
                Ok(ColSelector::Special(SpecialCol::Prev(1)))
            } else {
                Ok(ColSelector::Special(SpecialCol::Next(1)))
            }
        }
        Token::Ident("cols") => {
            stream.advance();
            // Expect '.'
            let _ = stream.expect(&Token::Dot, "'.' after 'cols'")?;

            // Expect group identifier
            let group_id = match stream.consume() {
                Some(next_st) => match next_st.token {
                    Token::Ident(id) => id,
                    ref other => {
                        return Err(ParseError::UnexpectedToken {
                            expected: "column group identifier after 'cols.'",
                            found: other.to_string(),
                            offset: next_st.offset,
                        });
                    }
                },
                None => {
                    return Err(ParseError::UnexpectedEof {
                        expected: "column group identifier after 'cols.'",
                    });
                }
            };

            // Check for optional .first or .last
            if stream.peek_token() == Some(&Token::Dot) {
                stream.advance();
                match stream.consume() {
                    Some(milestone_st) => match milestone_st.token {
                        Token::Ident("first") => Ok(ColSelector::Special(SpecialCol::GroupFirst(
                            ColGroupId::new(group_id),
                        ))),
                        Token::Ident("last") => Ok(ColSelector::Special(SpecialCol::GroupLast(
                            ColGroupId::new(group_id),
                        ))),
                        ref other => Err(ParseError::UnexpectedToken {
                            expected: "'first' or 'last'",
                            found: other.to_string(),
                            offset: milestone_st.offset,
                        }),
                    },
                    None => Err(ParseError::UnexpectedEof {
                        expected: "'first' or 'last'",
                    }),
                }
            } else {
                Ok(ColSelector::Special(SpecialCol::Group(ColGroupId::new(
                    group_id,
                ))))
            }
        }
        Token::String(key1) => {
            let key1 = key1.clone();
            stream.advance();
            if stream.peek_token() == Some(&Token::DotDot) {
                stream.advance();
                match stream.consume() {
                    Some(key2_st) => match &key2_st.token {
                        Token::String(key2) => {
                            Ok(ColSelector::Range(ColId::new(key1), ColId::new(key2)))
                        }
                        other => Err(ParseError::UnexpectedToken {
                            expected: "closing column range key string",
                            found: other.to_string(),
                            offset: key2_st.offset,
                        }),
                    },
                    None => Err(ParseError::UnexpectedEof {
                        expected: "closing column range key string",
                    }),
                }
            } else {
                Ok(ColSelector::Key(ColId::new(key1)))
            }
        }
        Token::Ident(bare) => {
            let found = (*bare).to_string();
            stream.advance();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        Token::Number(num) => {
            let found = num.to_string();
            stream.advance();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        other => {
            let found = other.to_string();
            Err(ParseError::UnexpectedToken {
                expected: "column selector ('@col', quoted string key, 'col.prev', 'col.next', or 'cols.<group>')",
                found,
                offset,
            })
        }
    }
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
        let expr = parse_expr_str("IncomeStatement[\"revenue\", @col] >= 100").unwrap();
        assert_eq!(
            expr,
            Expr::Binary {
                op: BinaryOp::Gte,
                left: Box::new(Expr::Reference(TableRef::new(
                    "IncomeStatement",
                    RowSelector::Key(RowId::new("revenue")),
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
        let expr = parse_expr_str("IF(Assumptions[\"flag\", \"active\"] = 1, 100, 0)").unwrap();
        match expr {
            Expr::FunctionCall { name, args } => {
                assert_eq!(name, "IF");
                assert_eq!(args.len(), 3);
                assert_eq!(
                    args[0],
                    Expr::Binary {
                        op: BinaryOp::Eq,
                        left: Box::new(Expr::Reference(TableRef::new(
                            "Assumptions",
                            RowSelector::Key(RowId::new("flag")),
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
    fn test_orthogonal_addressing_forms() {
        // Active column: Table["row", @col]
        let active = parse_expr_str("IncomeStatement[\"revenue\", @col]").unwrap();
        assert_eq!(
            active,
            Expr::Reference(TableRef::new(
                TableId::new("IncomeStatement"),
                RowSelector::Key(RowId::new("revenue")),
                ColSelector::Active,
            ))
        );

        // Active row: Table[@row, "col"]
        let active_row = parse_expr_str("CapTable[@row, \"shares\"]").unwrap();
        assert_eq!(
            active_row,
            Expr::Reference(TableRef::new(
                TableId::new("CapTable"),
                RowSelector::Active,
                ColSelector::Key(ColId::new("shares")),
            ))
        );

        // Specific column key: Table["row", "col"]
        let specific = parse_expr_str("Assumptions[\"tax_rate\", \"value\"]").unwrap();
        assert_eq!(
            specific,
            Expr::Reference(TableRef::new(
                TableId::new("Assumptions"),
                RowSelector::Key(RowId::new("tax_rate")),
                ColSelector::Key(ColId::new("value")),
            ))
        );

        // col.prev default (1)
        let prev1 = parse_expr_str("BalanceSheet[\"cash\", col.prev]").unwrap();
        assert_eq!(
            prev1,
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Key(RowId::new("cash")),
                ColSelector::Special(SpecialCol::Prev(1)),
            ))
        );

        // col.prev(2)
        let prev2 = parse_expr_str("BalanceSheet[\"cash\", col.prev(2)]").unwrap();
        assert_eq!(
            prev2,
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Key(RowId::new("cash")),
                ColSelector::Special(SpecialCol::Prev(2)),
            ))
        );

        // col.next(3)
        let next3 = parse_expr_str("BalanceSheet[\"cash\", col.next(3)]").unwrap();
        assert_eq!(
            next3,
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Key(RowId::new("cash")),
                ColSelector::Special(SpecialCol::Next(3)),
            ))
        );

        // row.prev default (1)
        let row_prev1 = parse_expr_str("DebtSchedule[row.prev, \"balance\"]").unwrap();
        assert_eq!(
            row_prev1,
            Expr::Reference(TableRef::new(
                TableId::new("DebtSchedule"),
                RowSelector::Special(SpecialRow::Prev(1)),
                ColSelector::Key(ColId::new("balance")),
            ))
        );

        // row.next(2)
        let row_next2 = parse_expr_str("DebtSchedule[row.next(2), \"balance\"]").unwrap();
        assert_eq!(
            row_next2,
            Expr::Reference(TableRef::new(
                TableId::new("DebtSchedule"),
                RowSelector::Special(SpecialRow::Next(2)),
                ColSelector::Key(ColId::new("balance")),
            ))
        );

        // cols.historical.last
        let hist_last = parse_expr_str("DebtSchedule[\"balance\", cols.historical.last]").unwrap();
        assert_eq!(
            hist_last,
            Expr::Reference(TableRef::new(
                TableId::new("DebtSchedule"),
                RowSelector::Key(RowId::new("balance")),
                ColSelector::Special(SpecialCol::GroupLast(ColGroupId::new("historical"))),
            ))
        );

        // cols.forecast.first
        let fc_first = parse_expr_str("DebtSchedule[\"balance\", cols.forecast.first]").unwrap();
        assert_eq!(
            fc_first,
            Expr::Reference(TableRef::new(
                TableId::new("DebtSchedule"),
                RowSelector::Key(RowId::new("balance")),
                ColSelector::Special(SpecialCol::GroupFirst(ColGroupId::new("forecast"))),
            ))
        );

        // Horizontal range: SUM(Table["row", "2024E".."2028E"])
        let h_range =
            parse_expr_str("SUM(IncomeStatement[\"revenue\", \"2024E\"..\"2028E\"])").unwrap();
        assert_eq!(
            h_range,
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![Expr::Reference(TableRef::new(
                    TableId::new("IncomeStatement"),
                    RowSelector::Key(RowId::new("revenue")),
                    ColSelector::Range(ColId::new("2024E"), ColId::new("2028E")),
                ))],
            }
        );

        // Vertical slice active column: Opex["r_and_d".."g_and_a", @col]
        let v_slice = parse_expr_str("Opex[\"r_and_d\"..\"g_and_a\", @col]").unwrap();
        assert_eq!(
            v_slice,
            Expr::Reference(TableRef::new(
                TableId::new("Opex"),
                RowSelector::Range(RowId::new("r_and_d"), RowId::new("g_and_a")),
                ColSelector::Active,
            ))
        );

        // Row group active column: BalanceSheet[rows.current_assets, @col]
        let rg_active = parse_expr_str("BalanceSheet[rows.current_assets, @col]").unwrap();
        assert_eq!(
            rg_active,
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Special(SpecialRow::Group(RowGroupId::new("current_assets"))),
                ColSelector::Active,
            ))
        );

        // Row group specific column: BalanceSheet[rows.current_assets, "2024E"]
        let rg_specific = parse_expr_str("BalanceSheet[rows.current_assets, \"2024E\"]").unwrap();
        assert_eq!(
            rg_specific,
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Special(SpecialRow::Group(RowGroupId::new("current_assets"))),
                ColSelector::Key(ColId::new("2024E")),
            ))
        );

        // Row group milestone: BalanceSheet[rows.current_assets.first, @col]
        let rg_first = parse_expr_str("BalanceSheet[rows.current_assets.first, @col]").unwrap();
        assert_eq!(
            rg_first,
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Special(SpecialRow::GroupFirst(RowGroupId::new("current_assets"))),
                ColSelector::Active,
            ))
        );

        // Single row key specific column: BalanceSheet["cash", "2024E"]
        let single_row = parse_expr_str("BalanceSheet[\"cash\", \"2024E\"]").unwrap();
        assert_eq!(
            single_row,
            Expr::Reference(TableRef::new(
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
            Expr::Reference(TableRef::new(
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
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Special(SpecialRow::Group(RowGroupId::new("current_assets"))),
                ColSelector::Special(SpecialCol::Group(ColGroupId::new("forecast"))),
            ))
        );
    }

    #[test]
    fn test_reject_dot_access() {
        // Dot access with active column
        let err_dot = parse_expr_str("IncomeStatement.revenue[@col]").unwrap_err();
        match err_dot {
            ParseError::UnexpectedToken { found, offset, .. } => {
                assert_eq!(found, ".");
                assert_eq!(offset, 15);
            }
            other => panic!("Expected UnexpectedToken for dot, got {other:?}"),
        }

        // Dot access with row identifier only
        let err_dot_bare = parse_expr_str("IncomeStatement.revenue").unwrap_err();
        match err_dot_bare {
            ParseError::UnexpectedToken { found, offset, .. } => {
                assert_eq!(found, ".");
                assert_eq!(offset, 15);
            }
            other => panic!("Expected UnexpectedToken for dot, got {other:?}"),
        }
    }

    #[test]
    fn test_reject_unquoted_coordinate_keys() {
        let err1 = parse_expr_str("IncomeStatement[\"revenue\", 2024E]").unwrap_err();
        match err1 {
            ParseError::UnquotedCoordinateKey { found, .. } => assert_eq!(found, "2024E"),
            other => panic!("Expected UnquotedCoordinateKey, got {other:?}"),
        }

        let err2 = parse_expr_str("Opex[r_and_d, @col]").unwrap_err();
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
        let err = parse_expr_str("Table[\"row\", col.prevent]").unwrap_err();
        match err {
            ParseError::UnexpectedToken { .. } => {}
            other => panic!("Expected unexpected token error, got {other:?}"),
        }

        // row.prevent should not parse as row.prev
        let err_row = parse_expr_str("Table[row.prevent, \"col\"]").unwrap_err();
        match err_row {
            ParseError::UnexpectedToken { .. } => {}
            other => panic!("Expected unexpected token error, got {other:?}"),
        }

        // rows.group.firstly should not parse as rows.group.first
        let err = parse_expr_str("Table[rows.group.firstly, @col]").unwrap_err();
        match err {
            ParseError::UnexpectedToken { .. } => {}
            other => panic!("Expected unexpected token error, got {other:?}"),
        }

        // Bare @ is retired
        let err_at = parse_expr_str("Table[\"row\", @]").unwrap_err();
        assert_eq!(err_at, ParseError::RetiredBareAt { offset: 13 });

        // @column is retired/invalid
        let err_at_col = parse_expr_str("Table[\"row\", @column]").unwrap_err();
        assert_eq!(err_at_col, ParseError::RetiredBareAt { offset: 13 });
    }

    #[test]
    fn test_unexpected_eof_on_unclosed_delimiters() {
        assert_eq!(
            parse_expr_str("(1 + 2").unwrap_err(),
            ParseError::UnexpectedEof { expected: "')'" }
        );
        assert_eq!(
            parse_expr_str("Table[\"row\", @col").unwrap_err(),
            ParseError::UnexpectedEof { expected: "']'" }
        );
        assert_eq!(
            parse_expr_str("Table[rows.opex, @col").unwrap_err(),
            ParseError::UnexpectedEof { expected: "']'" }
        );
        assert_eq!(
            parse_expr_str("SUM(1, 2").unwrap_err(),
            ParseError::UnexpectedEof {
                expected: "',' or ')'"
            }
        );
        assert_eq!(
            parse_expr_str("Table[rows").unwrap_err(),
            ParseError::UnexpectedEof {
                expected: "'.' after 'rows'"
            }
        );
        assert_eq!(
            parse_expr_str("Table[\"row\", cols").unwrap_err(),
            ParseError::UnexpectedEof {
                expected: "'.' after 'cols'"
            }
        );
        assert_eq!(
            parse_expr_str("Table[\"row\", col").unwrap_err(),
            ParseError::UnexpectedEof {
                expected: "'.' after 'col'"
            }
        );
        assert_eq!(
            parse_expr_str("Table[row").unwrap_err(),
            ParseError::UnexpectedEof {
                expected: "'.' after 'row'"
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
            "IF(Assumptions[\"flag\", \"active\"] = 1, 100, 0)",
            "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
            "SUM(IncomeStatement[\"revenue\", \"2024E\"..\"2028E\"])",
            "Opex[\"r_and_d\"..\"g_and_a\", @col]",
            "BalanceSheet[rows.current_assets, cols.forecast]",
            "BalanceSheet[\"cash\", col.prev]",
            "BalanceSheet[\"cash\", col.prev(2)]",
            "DebtSchedule[\"balance\", cols.historical.last]",
            "(2 ^ 3) ^ 4",
            "2 ^ (3 ^ 4)",
            "BalanceSheet[\"cash\", @col]",
            "BalanceSheet[rows.current_assets.first, @col]",
            "CapTable[@row, \"shares\"]",
            "DebtSchedule[row.prev, \"balance\"]",
            "DebtSchedule[row.next(2), \"balance\"]",
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

    #[test]
    fn test_tokenizer_direct() {
        let tokens = tokenize("IncomeStatement[\"revenue\", @col] + 42").unwrap();
        assert_eq!(tokens.len(), 8);
        assert_eq!(tokens[0].token, Token::Ident("IncomeStatement"));
        assert_eq!(tokens[1].token, Token::LBracket);
        assert_eq!(tokens[2].token, Token::String("revenue".to_string()));
        assert_eq!(tokens[3].token, Token::Comma);
        assert_eq!(tokens[4].token, Token::AtCol);
        assert_eq!(tokens[5].token, Token::RBracket);
        assert_eq!(tokens[6].token, Token::Plus);
        assert_eq!(tokens[7].token, Token::Number(42.0));
    }
}
