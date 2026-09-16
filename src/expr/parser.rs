use crate::expr::ast::{
    BinaryOp, ColSelector, Expr, Literal, RowSelector, SpecialCol, SpecialRow, TableRef, UnaryOp,
};
use crate::expr::error::ParseError;
use crate::model::id::{ColGroupId, ColId, RowGroupId, RowId};
use std::fmt;
use winnow::ascii::multispace0;
use winnow::error::ContextError;
use winnow::prelude::*;

/// Lexical token recognized by the formula tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Floating-point or integer numeric literal.
    Number(f64),
    /// Quoted text string constant.
    String(String),
    /// Explicit null literal (`null`).
    Null,
    /// General identifier (table name, row/col identifier, function name, or coordinate word).
    Ident(String),

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
    /// Active evaluation column symbol `@`.
    At,

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

impl Token {
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

impl fmt::Display for Token {
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
            Self::At => write!(f, "@"),
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
pub struct SpannedToken {
    /// The token kind and payload.
    pub token: Token,
    /// Byte offset where the token begins in the original input string.
    pub offset: usize,
    /// Length of the token in bytes.
    pub len: usize,
}

/// Cursor over a slice of spanned tokens providing lookahead and consumption utilities.
#[derive(Debug, Clone)]
pub struct TokenStream<'a> {
    tokens: Vec<SpannedToken>,
    pos: usize,
    orig: &'a str,
}

impl<'a> TokenStream<'a> {
    /// Creates a new token stream cursor over the given tokens and original source string.
    pub fn new(tokens: Vec<SpannedToken>, orig: &'a str) -> Self {
        Self {
            tokens,
            pos: 0,
            orig,
        }
    }

    /// Peeks at the next spanned token without advancing the cursor.
    pub fn peek(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.pos)
    }

    /// Peeks at the next token kind without advancing the cursor.
    pub fn peek_token(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|st| &st.token)
    }

    /// Returns the byte offset of the current cursor position.
    pub fn current_offset(&self) -> usize {
        self.peek().map_or(self.orig.len(), |st| st.offset)
    }

    /// Returns `true` if all tokens have been consumed.
    pub fn is_empty(&self) -> bool {
        self.pos >= self.tokens.len()
    }
}

impl<'a> Iterator for TokenStream<'a> {
    type Item = SpannedToken;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }
}

impl<'a> TokenStream<'a> {
    /// Asserts that the next token matches `expected_tok`, consuming it if so.
    pub fn expect(
        &mut self,
        expected_tok: &Token,
        expected_str: &'static str,
    ) -> Result<SpannedToken, ParseError> {
        match self.peek() {
            Some(st) if &st.token == expected_tok => {
                let tok = st.clone();
                self.pos += 1;
                Ok(tok)
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
pub fn tokenize(input: &str) -> Result<Vec<SpannedToken>, ParseError> {
    let mut tokens = Vec::new();
    let mut cursor = input;

    while !cursor.is_empty() {
        // Strip leading whitespace
        let _ = multispace0::<_, ContextError>.parse_next(&mut cursor);
        if cursor.is_empty() {
            break;
        }

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
                tokens.push(SpannedToken {
                    token: Token::At,
                    offset,
                    len: 1,
                });
                cursor = &cursor[1..];
                continue;
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
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .count();
            let word = &cursor[..word_len];

            // Check if token contains letters or underscores (e.g. 2024E)
            let has_letters = word.chars().any(|c| c.is_ascii_alphabetic() || c == '_');
            if has_letters {
                cursor = &cursor[word_len..];
                tokens.push(SpannedToken {
                    token: Token::Ident(word.to_string()),
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
                let frac_digits = frac.chars().take_while(|c| c.is_ascii_digit()).count();
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
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
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
                    token: Token::Ident(ident.to_string()),
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
pub fn parse_expr_str(input: &str) -> Result<Expr, ParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "expression",
        });
    }

    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(ParseError::UnexpectedEof {
            expected: "expression",
        });
    }

    let mut stream = TokenStream::new(tokens, input);
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
        stream.next();

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
            stream.next();
            let expr = parse_expression(stream, UnaryOp::Pos.precedence())?;
            Ok(Expr::Unary {
                op: UnaryOp::Pos,
                expr: Box::new(expr),
            })
        }
        Token::Minus => {
            stream.next();
            let expr = parse_expression(stream, UnaryOp::Neg.precedence())?;
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            })
        }
        Token::LParen => {
            stream.next();
            let expr = parse_expression(stream, 0)?;
            let _ = stream.expect(&Token::RParen, "')'")?;
            Ok(expr)
        }
        Token::Number(n) => {
            let val = *n;
            stream.next();
            Ok(Expr::Literal(Literal::Number(val)))
        }
        Token::String(s) => {
            let val = s.clone();
            stream.next();
            Ok(Expr::Literal(Literal::String(val)))
        }
        Token::Null => {
            stream.next();
            Ok(Expr::Literal(Literal::Null))
        }
        Token::Ident(ident) => {
            let name = ident.clone();
            stream.next();

            match stream.peek_token() {
                Some(Token::LParen) => parse_function_call(name, offset, stream),
                Some(Token::Dot) => parse_dot_access(name, stream),
                Some(Token::LBracket) => parse_bracket_access(name, stream),
                _ => Err(ParseError::UnexpectedToken {
                    expected: "function call '(', dot access '.', or bracket access '['",
                    found: name,
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
            stream.next();
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
    name: String,
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
        return Err(ParseError::InvalidFunctionName { name, offset });
    }

    // Consume '('
    let _ = stream.expect(&Token::LParen, "'('")?;
    let args = parse_argument_list(stream)?;
    // Consume ')'
    let _ = stream.expect(&Token::RParen, "')'")?;

    Ok(Expr::FunctionCall { name, args })
}

fn parse_dot_access(table: String, stream: &mut TokenStream<'_>) -> Result<Expr, ParseError> {
    // Consume '.'
    let _ = stream.expect(&Token::Dot, "'.'")?;

    let row_name = match stream.next() {
        Some(st) => match st.token {
            Token::Ident(id) => id,
            other => {
                return Err(ParseError::UnexpectedToken {
                    expected: "row identifier",
                    found: other.to_string(),
                    offset: st.offset,
                });
            }
        },
        None => {
            return Err(ParseError::UnexpectedEof {
                expected: "row identifier",
            });
        }
    };

    let _ = stream.expect(&Token::LBracket, "'['")?;
    let col = parse_col_spec(stream)?;
    let _ = stream.expect(&Token::RBracket, "']'")?;

    Ok(Expr::Reference(TableRef::dot(table, row_name, col)))
}

fn parse_bracket_access(table: String, stream: &mut TokenStream<'_>) -> Result<Expr, ParseError> {
    // Consume '['
    let _ = stream.expect(&Token::LBracket, "'['")?;

    let row = parse_row_spec(stream)?;
    let _ = stream.expect(&Token::Comma, "',' separating row and column selectors")?;
    let col = parse_col_spec(stream)?;
    let _ = stream.expect(&Token::RBracket, "']'")?;

    Ok(Expr::Reference(TableRef::bracket(table, row, col)))
}

fn parse_row_spec(stream: &mut TokenStream<'_>) -> Result<RowSelector, ParseError> {
    let Some(st) = stream.peek() else {
        return Err(ParseError::UnexpectedEof {
            expected: "row selector (quoted string key or 'rows.<group>')",
        });
    };

    let offset = st.offset;
    match &st.token {
        Token::Ident(name) if name == "rows" => {
            stream.next();
            // Expect '.'
            match stream.peek_token() {
                Some(Token::Dot) => {
                    stream.next();
                }
                _ => {
                    let err_offset = stream.current_offset();
                    let found = stream
                        .peek()
                        .map_or_else(String::new, |t| t.token.to_string());
                    return Err(ParseError::UnexpectedToken {
                        expected: "row group identifier after 'rows.'",
                        found,
                        offset: err_offset,
                    });
                }
            }

            // Expect group identifier
            let group_id = match stream.next() {
                Some(next_st) => match next_st.token {
                    Token::Ident(id) => id,
                    other => {
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
                stream.next();
                match stream.next() {
                    Some(milestone_st) => match &milestone_st.token {
                        Token::Ident(m) if m == "first" => Ok(RowSelector::Special(
                            SpecialRow::GroupFirst(RowGroupId::new(group_id)),
                        )),
                        Token::Ident(m) if m == "last" => Ok(RowSelector::Special(
                            SpecialRow::GroupLast(RowGroupId::new(group_id)),
                        )),
                        other => Err(ParseError::UnexpectedToken {
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
            stream.next();
            if stream.peek_token() == Some(&Token::DotDot) {
                stream.next();
                match stream.next() {
                    Some(key2_st) => match key2_st.token {
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
            let found = bare.clone();
            stream.next();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        Token::Number(num) => {
            let found = num.to_string();
            stream.next();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        other => {
            let found = other.to_string();
            Err(ParseError::UnexpectedToken {
                expected: "row selector (quoted string key or 'rows.<group>')",
                found,
                offset,
            })
        }
    }
}

fn parse_col_spec(stream: &mut TokenStream<'_>) -> Result<ColSelector, ParseError> {
    let Some(st) = stream.peek() else {
        return Err(ParseError::UnexpectedEof {
            expected: "column selector ('@', quoted string key, 'col.prev', 'col.next', or 'cols.<group>')",
        });
    };

    let offset = st.offset;
    match &st.token {
        Token::At => {
            stream.next();
            Ok(ColSelector::Active)
        }
        Token::Ident(name) if name == "col" => {
            stream.next();
            // Expect '.'
            let _ = stream.expect(&Token::Dot, "'.' after 'col'")?;

            // Expect 'prev' or 'next'
            let is_prev = match stream.peek_token() {
                Some(Token::Ident(dir)) if dir == "prev" => true,
                Some(Token::Ident(dir)) if dir == "next" => false,
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
            stream.next();

            // Check for optional (k)
            if stream.peek_token() == Some(&Token::LParen) {
                stream.next();
                let k = match stream.next() {
                    Some(num_st) => match num_st.token {
                        Token::Number(n) if n.fract() == 0.0 && n >= 0.0 => n as u32,
                        other => {
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
        Token::Ident(name) if name == "cols" => {
            stream.next();
            // Expect '.'
            match stream.peek_token() {
                Some(Token::Dot) => {
                    stream.next();
                }
                _ => {
                    let err_offset = stream.current_offset();
                    let found = stream
                        .peek()
                        .map_or_else(String::new, |t| t.token.to_string());
                    return Err(ParseError::UnexpectedToken {
                        expected: "column group identifier after 'cols.'",
                        found,
                        offset: err_offset,
                    });
                }
            }

            // Expect group identifier
            let group_id = match stream.next() {
                Some(next_st) => match next_st.token {
                    Token::Ident(id) => id,
                    other => {
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
                stream.next();
                match stream.next() {
                    Some(milestone_st) => match &milestone_st.token {
                        Token::Ident(m) if m == "first" => Ok(ColSelector::Special(
                            SpecialCol::GroupFirst(ColGroupId::new(group_id)),
                        )),
                        Token::Ident(m) if m == "last" => Ok(ColSelector::Special(
                            SpecialCol::GroupLast(ColGroupId::new(group_id)),
                        )),
                        other => Err(ParseError::UnexpectedToken {
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
            stream.next();
            if stream.peek_token() == Some(&Token::DotDot) {
                stream.next();
                match stream.next() {
                    Some(key2_st) => match key2_st.token {
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
            let found = bare.clone();
            stream.next();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        Token::Number(num) => {
            let found = num.to_string();
            stream.next();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        other => {
            let found = other.to_string();
            Err(ParseError::UnexpectedToken {
                expected: "column selector ('@', quoted string key, 'col.prev', 'col.next', or 'cols.<group>')",
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

    #[test]
    fn test_tokenizer_direct() {
        let tokens = tokenize("IncomeStatement.revenue[@] + 42").unwrap();
        assert_eq!(tokens.len(), 8);
        assert_eq!(tokens[0].token, Token::Ident("IncomeStatement".to_string()));
        assert_eq!(tokens[1].token, Token::Dot);
        assert_eq!(tokens[2].token, Token::Ident("revenue".to_string()));
        assert_eq!(tokens[3].token, Token::LBracket);
        assert_eq!(tokens[4].token, Token::At);
        assert_eq!(tokens[5].token, Token::RBracket);
        assert_eq!(tokens[6].token, Token::Plus);
        assert_eq!(tokens[7].token, Token::Number(42.0));
    }
}
