use std::fmt;

use crate::expr::ast::{
    BinaryOp, BlankCheck, CallArg, ColEndpoint, ColSelector, CriteriaComparator, Expr, Literal,
    RelativeOffset, RowEndpoint, RowSelector, TableRef, TableSelector, UnaryOp, fmt_number_literal,
    fmt_quoted_f8_str, format_number_literal,
};
use crate::expr::error::ParseError;
use crate::model::id::{ColId, RowId};

/// Lexical token recognized by the formula tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token<'a> {
    /// Floating-point or integer numeric literal.
    Number(f64),
    /// Quoted text string constant.
    String(String),
    /// Boolean constant (`TRUE` or `FALSE`).
    Boolean(bool),
    /// General identifier (table name, row/col identifier, function name, or coordinate word).
    Ident(&'a str),
    /// Explicit omitted argument placeholder (`_`).
    Underscore,

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
    /// Active evaluation table symbol `@table`.
    AtTable,

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
            Self::Number(_)
            | Self::String(_)
            | Self::Boolean(_)
            | Self::Ident(_)
            | Self::Underscore
            | Self::LParen
            | Self::RParen
            | Self::LBracket
            | Self::RBracket
            | Self::Comma
            | Self::Dot
            | Self::DotDot
            | Self::AtCol
            | Self::AtRow
            | Self::AtTable => None,
        }
    }

    /// Maps the token to a scoped criteria comparator if applicable.
    #[must_use]
    pub const fn as_criteria_comparator(&self) -> Option<CriteriaComparator> {
        match self {
            Self::NotEq => Some(CriteriaComparator::NotEq),
            Self::Lt => Some(CriteriaComparator::Lt),
            Self::Lte => Some(CriteriaComparator::Lte),
            Self::Gt => Some(CriteriaComparator::Gt),
            Self::Gte => Some(CriteriaComparator::Gte),
            Self::Number(_)
            | Self::String(_)
            | Self::Boolean(_)
            | Self::Ident(_)
            | Self::Underscore
            | Self::LParen
            | Self::RParen
            | Self::LBracket
            | Self::RBracket
            | Self::Comma
            | Self::Dot
            | Self::DotDot
            | Self::AtCol
            | Self::AtRow
            | Self::AtTable
            | Self::Plus
            | Self::Minus
            | Self::Star
            | Self::Slash
            | Self::Caret
            | Self::Eq => None,
        }
    }
}

impl<'a> fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => fmt_number_literal(*n, f),
            Self::String(s) => fmt_quoted_f8_str(s, f),
            Self::Boolean(true) => write!(f, "TRUE"),
            Self::Boolean(false) => write!(f, "FALSE"),
            Self::Ident(id) => write!(f, "{id}"),
            Self::Underscore => write!(f, "_"),
            Self::LParen => write!(f, "("),
            Self::RParen => write!(f, ")"),
            Self::LBracket => write!(f, "["),
            Self::RBracket => write!(f, "]"),
            Self::Comma => write!(f, ","),
            Self::Dot => write!(f, "."),
            Self::DotDot => write!(f, ".."),
            Self::AtCol => write!(f, "@col"),
            Self::AtRow => write!(f, "@row"),
            Self::AtTable => write!(f, "@table"),
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

    /// Peeks at the token `n` positions ahead of the cursor without advancing.
    pub fn peek_nth(&self, n: usize) -> Option<&SpannedToken<'a>> {
        self.tokens.get(self.pos + n)
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
                if cursor.starts_with("@table")
                    && cursor
                        .as_bytes()
                        .get(6)
                        .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_')
                {
                    tokens.push(SpannedToken {
                        token: Token::AtTable,
                        offset,
                        len: 6,
                    });
                    cursor = &cursor[6..];
                    continue;
                }
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

        // Digits (finite numeric literals, scientific notation, or alphanumeric coordinate tokens like 2024E)
        if first_char.is_ascii_digit() {
            let int_digits = cursor.bytes().take_while(u8::is_ascii_digit).count();
            let after_int = &cursor[int_digits..];
            let has_dot = after_int.starts_with('.') && !after_int.starts_with("..");

            if has_dot {
                let frac_digits = after_int[1..]
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .count();
                if frac_digits == 0 {
                    let trail = after_int[1..]
                        .bytes()
                        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                        .count();
                    let bad_len = int_digits + 1 + trail;
                    return Err(ParseError::InvalidNumber {
                        raw: cursor[..bad_len].to_string(),
                        offset,
                    });
                }

                let mut pos = int_digits + 1 + frac_digits;
                if matches!(cursor.as_bytes().get(pos), Some(b'e' | b'E')) {
                    pos += 1;
                    if matches!(cursor.as_bytes().get(pos), Some(b'+' | b'-')) {
                        pos += 1;
                    }
                    let exp_digits = cursor[pos..].bytes().take_while(u8::is_ascii_digit).count();
                    pos += exp_digits;
                    let trail = cursor[pos..]
                        .bytes()
                        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                        .count();
                    if exp_digits == 0 || trail > 0 {
                        return Err(ParseError::InvalidNumber {
                            raw: cursor[..pos + trail].to_string(),
                            offset,
                        });
                    }
                } else {
                    let trail = cursor[pos..]
                        .bytes()
                        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                        .count();
                    if trail > 0 {
                        return Err(ParseError::InvalidNumber {
                            raw: cursor[..pos + trail].to_string(),
                            offset,
                        });
                    }
                }

                let num_str = &cursor[..pos];
                let val = parse_finite_number_literal(num_str, offset)?;
                cursor = &cursor[pos..];
                tokens.push(SpannedToken {
                    token: Token::Number(val),
                    offset,
                    len: pos,
                });
                continue;
            }

            // Signed scientific prefix `[0-9]+[eE][+-]...`
            if matches!(after_int.as_bytes().first(), Some(b'e' | b'E'))
                && matches!(after_int.as_bytes().get(1), Some(b'+' | b'-'))
            {
                let exp_start = int_digits + 2;
                let exp_digits = cursor[exp_start..]
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .count();
                let end = exp_start + exp_digits;
                let trail = cursor[end..]
                    .bytes()
                    .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    .count();
                if exp_digits == 0 || trail > 0 {
                    return Err(ParseError::InvalidNumber {
                        raw: cursor[..end + trail].to_string(),
                        offset,
                    });
                }
                let num_str = &cursor[..end];
                let val = parse_finite_number_literal(num_str, offset)?;
                cursor = &cursor[end..];
                tokens.push(SpannedToken {
                    token: Token::Number(val),
                    offset,
                    len: end,
                });
                continue;
            }

            // Unsigned digit-leading word: pure integer, `[0-9]+[eE][0-9]+`, or `Token::Ident`
            let word_len = cursor
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            let word = &cursor[..word_len];
            let is_unsigned_sci = word_len > int_digits + 1
                && matches!(word.as_bytes().get(int_digits), Some(b'e' | b'E'))
                && word[int_digits + 1..].bytes().all(|b| b.is_ascii_digit());

            if word_len == int_digits || is_unsigned_sci {
                let val = parse_finite_number_literal(word, offset)?;
                cursor = &cursor[word_len..];
                tokens.push(SpannedToken {
                    token: Token::Number(val),
                    offset,
                    len: word_len,
                });
                continue;
            }

            cursor = &cursor[word_len..];
            tokens.push(SpannedToken {
                token: Token::Ident(word),
                offset,
                len: word_len,
            });
            continue;
        }

        // Identifier, boolean literal, or underscore
        if first_char.is_ascii_alphabetic() || first_char == '_' {
            let ident_len = cursor
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            let ident = &cursor[..ident_len];
            cursor = &cursor[ident_len..];

            match ident {
                "TRUE" => tokens.push(SpannedToken {
                    token: Token::Boolean(true),
                    offset,
                    len: ident_len,
                }),
                "FALSE" => tokens.push(SpannedToken {
                    token: Token::Boolean(false),
                    offset,
                    len: ident_len,
                }),
                "_" => tokens.push(SpannedToken {
                    token: Token::Underscore,
                    offset,
                    len: ident_len,
                }),
                _ => tokens.push(SpannedToken {
                    token: Token::Ident(ident),
                    offset,
                    len: ident_len,
                }),
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

fn parse_finite_number_literal(raw: &str, offset: usize) -> Result<f64, ParseError> {
    match raw.parse::<f64>() {
        Ok(val) if val.is_finite() => Ok(if val == 0.0 { 0.0 } else { val }),
        Ok(_) | Err(_) => Err(ParseError::InvalidNumber {
            raw: raw.to_string(),
            offset,
        }),
    }
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
    let mut saw_relational = false;

    while let Some(tok) = stream.peek() {
        let Some(op) = tok.token.as_binary_op() else {
            break;
        };

        let op_prec = op.precedence();
        if op_prec < min_bp {
            break;
        }

        if op.is_relational() {
            if saw_relational {
                return Err(ParseError::ChainedComparison {
                    operator: op.as_str(),
                    offset: tok.offset,
                });
            }
            saw_relational = true;
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
        Token::Boolean(b) => {
            let val = *b;
            stream.advance();
            Ok(Expr::Literal(Literal::Boolean(val)))
        }
        Token::Ident(ident) => {
            let name = *ident;
            let first_end = st.offset + st.len;
            stream.advance();

            let mut lookahead_idx = 0usize;
            let mut prev_end = first_end;
            while let (Some(dot_st), Some(seg_st)) = (
                stream.peek_nth(lookahead_idx),
                stream.peek_nth(lookahead_idx + 1),
            ) {
                if dot_st.token == Token::Dot
                    && dot_st.offset == prev_end
                    && let Token::Ident(_) = seg_st.token
                    && seg_st.offset == dot_st.offset + 1
                {
                    prev_end = seg_st.offset + seg_st.len;
                    lookahead_idx += 2;
                } else {
                    break;
                }
            }

            if lookahead_idx > 0
                && stream
                    .peek_nth(lookahead_idx)
                    .is_some_and(|after| after.token == Token::LParen)
            {
                let mut full_name = String::from(name);
                for _ in 0..(lookahead_idx / 2) {
                    stream.advance(); // '.'
                    if let Some(seg_st) = stream.consume()
                        && let Token::Ident(seg) = seg_st.token
                    {
                        full_name.push('.');
                        full_name.push_str(seg);
                    }
                }
                return parse_function_call(&full_name, offset, stream);
            }

            match stream.peek() {
                Some(st) if st.token == Token::LParen => parse_function_call(name, offset, stream),
                Some(st) if st.token == Token::LBracket => {
                    parse_bracket_access(TableSelector::from(name), stream)
                }
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
        Token::AtTable => {
            stream.advance();
            match stream.peek() {
                Some(st) if st.token == Token::LBracket => {
                    parse_bracket_access(TableSelector::Active, stream)
                }
                Some(st) => Err(ParseError::UnexpectedToken {
                    expected: "table reference '['",
                    found: st.token.to_string(),
                    offset: st.offset,
                }),
                None => Err(ParseError::UnexpectedToken {
                    expected: "table reference '['",
                    found: "@table".to_string(),
                    offset,
                }),
            }
        }
        other @ (Token::Underscore
        | Token::RParen
        | Token::LBracket
        | Token::RBracket
        | Token::Comma
        | Token::Dot
        | Token::DotDot
        | Token::AtCol
        | Token::AtRow
        | Token::Plus
        | Token::Star
        | Token::Slash
        | Token::Caret
        | Token::Eq
        | Token::NotEq
        | Token::Lt
        | Token::Lte
        | Token::Gt
        | Token::Gte) => {
            let found = other.to_string();
            Err(ParseError::UnexpectedToken {
                expected: "expression",
                found,
                offset,
            })
        }
    }
}

fn parse_argument_list(stream: &mut TokenStream<'_>) -> Result<Vec<CallArg>, ParseError> {
    if stream.peek_token() == Some(&Token::RParen) {
        return Ok(Vec::new());
    }

    let mut args = Vec::new();
    loop {
        if stream.is_empty() {
            return Err(ParseError::UnexpectedEof {
                expected: "argument expression, '_', or ')'",
            });
        }

        let arg = if let Some(st) = stream.peek()
            && st.token == Token::Underscore
        {
            stream.advance();
            CallArg::Omitted
        } else if let Some(st) = stream.peek()
            && st.token == Token::Eq
        {
            return Err(ParseError::RedundantCriteriaEquals { offset: st.offset });
        } else if let Some(op) = stream
            .peek()
            .and_then(|st| st.token.as_criteria_comparator())
        {
            stream.advance();
            let expr = parse_expression(stream, 2)?;
            if let Some(rel_tok) = stream.peek()
                && let Some(bin_op) = rel_tok.token.as_binary_op()
                && bin_op.is_relational()
            {
                return Err(ParseError::ChainedComparison {
                    operator: bin_op.as_str(),
                    offset: rel_tok.offset,
                });
            }
            CallArg::Comparator { op, expr }
        } else if let Some(st) = stream.peek()
            && matches!(st.token, Token::Ident("BLANK" | "NOT_BLANK"))
            && !stream.peek_nth(1).is_some_and(|next_st| {
                next_st.token == Token::LParen
                    || next_st.token == Token::LBracket
                    || (next_st.token == Token::Dot && next_st.offset == st.offset + st.len)
            })
        {
            let check = if st.token == Token::Ident("BLANK") {
                BlankCheck::Blank
            } else {
                BlankCheck::NotBlank
            };
            stream.advance();
            CallArg::BlankCheck(check)
        } else {
            CallArg::Expr(parse_expression(stream, 0)?)
        };
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
    // Validate uppercase function name across every '.'-separated segment
    let is_valid = !name.is_empty()
        && name.split('.').all(|seg| {
            !seg.is_empty()
                && seg.starts_with(|c: char| c.is_ascii_uppercase())
                && seg
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        });

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

fn parse_bracket_access(
    table: TableSelector,
    stream: &mut TokenStream<'_>,
) -> Result<Expr, ParseError> {
    // Consume '['
    let _ = stream.expect(&Token::LBracket, "'['")?;

    let row = parse_row_spec(stream)?;
    let _ = stream.expect(&Token::Comma, "',' separating row and column selectors")?;
    let col = parse_col_spec(stream)?;
    let _ = stream.expect(&Token::RBracket, "']'")?;

    Ok(Expr::Reference(TableRef::new(table, row, col)))
}

fn parse_relative_offset(
    stream: &mut TokenStream<'_>,
    is_row: bool,
) -> Result<RelativeOffset, ParseError> {
    let dot_expected = if is_row {
        "'.' after 'row'"
    } else {
        "'.' after 'col'"
    };
    let _ = stream.expect(&Token::Dot, dot_expected)?;

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

    let expected_rparen = match (is_row, is_prev) {
        (true, true) => "')' after row.prev offset",
        (true, false) => "')' after row.next offset",
        (false, true) => "')' after col.prev offset",
        (false, false) => "')' after col.next offset",
    };

    let k = if stream.peek_token() == Some(&Token::LParen) {
        stream.advance();
        let k = match stream.consume() {
            Some(num_st) => {
                if let Token::Number(n) = num_st.token
                    && n.is_finite()
                    && n.fract() == 0.0
                    && (1.0..=f64::from(i32::MAX)).contains(&n)
                {
                    n as u32
                } else {
                    return Err(ParseError::UnexpectedToken {
                        expected: "positive integer offset",
                        found: num_st.token.to_string(),
                        offset: num_st.offset,
                    });
                }
            }
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: expected_rparen,
                });
            }
        };
        let _ = stream.expect(&Token::RParen, expected_rparen)?;
        k
    } else {
        1
    };

    if is_prev {
        Ok(RelativeOffset::Prev(k))
    } else {
        Ok(RelativeOffset::Next(k))
    }
}

fn parse_row_endpoint(stream: &mut TokenStream<'_>) -> Result<RowEndpoint, ParseError> {
    let Some(st) = stream.peek() else {
        return Err(ParseError::UnexpectedEof {
            expected: "row selector ('@row', quoted string key, 'row.prev', or 'row.next')",
        });
    };

    let offset = st.offset;
    match &st.token {
        Token::AtRow => {
            stream.advance();
            Ok(RowEndpoint::Active)
        }
        Token::Ident("row") => {
            stream.advance();
            Ok(RowEndpoint::Offset(parse_relative_offset(stream, true)?))
        }
        Token::String(key) => {
            let key = key.clone();
            stream.advance();
            Ok(RowEndpoint::Key(RowId::new(key)))
        }
        Token::Ident(bare) => {
            let found = (*bare).to_string();
            stream.advance();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        Token::Number(num) => {
            let found = format_number_literal(*num);
            stream.advance();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        other @ (Token::Boolean(_)
        | Token::Underscore
        | Token::LParen
        | Token::RParen
        | Token::LBracket
        | Token::RBracket
        | Token::Comma
        | Token::Dot
        | Token::DotDot
        | Token::AtCol
        | Token::AtTable
        | Token::Plus
        | Token::Minus
        | Token::Star
        | Token::Slash
        | Token::Caret
        | Token::Eq
        | Token::NotEq
        | Token::Lt
        | Token::Lte
        | Token::Gt
        | Token::Gte) => {
            let found = other.to_string();
            Err(ParseError::UnexpectedToken {
                expected: "row selector ('@row', quoted string key, 'row.prev', or 'row.next')",
                found,
                offset,
            })
        }
    }
}

fn parse_row_spec(stream: &mut TokenStream<'_>) -> Result<RowSelector, ParseError> {
    let start = parse_row_endpoint(stream)?;
    if stream.peek_token() == Some(&Token::DotDot) {
        stream.advance();
        let end = parse_row_endpoint(stream)?;
        Ok(RowSelector::Range(start, end))
    } else {
        Ok(RowSelector::from(start))
    }
}

fn parse_col_endpoint(stream: &mut TokenStream<'_>) -> Result<ColEndpoint, ParseError> {
    let Some(st) = stream.peek() else {
        return Err(ParseError::UnexpectedEof {
            expected: "column selector ('@col', quoted string key, 'col.prev', or 'col.next')",
        });
    };

    let offset = st.offset;
    match &st.token {
        Token::AtCol => {
            stream.advance();
            Ok(ColEndpoint::Active)
        }
        Token::Ident("col") => {
            stream.advance();
            Ok(ColEndpoint::Offset(parse_relative_offset(stream, false)?))
        }
        Token::String(key) => {
            let key = key.clone();
            stream.advance();
            Ok(ColEndpoint::Key(ColId::new(key)))
        }
        Token::Ident(bare) => {
            let found = (*bare).to_string();
            stream.advance();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        Token::Number(num) => {
            let found = format_number_literal(*num);
            stream.advance();
            Err(ParseError::UnquotedCoordinateKey { found, offset })
        }
        other @ (Token::Boolean(_)
        | Token::Underscore
        | Token::LParen
        | Token::RParen
        | Token::LBracket
        | Token::RBracket
        | Token::Comma
        | Token::Dot
        | Token::DotDot
        | Token::AtRow
        | Token::AtTable
        | Token::Plus
        | Token::Minus
        | Token::Star
        | Token::Slash
        | Token::Caret
        | Token::Eq
        | Token::NotEq
        | Token::Lt
        | Token::Lte
        | Token::Gt
        | Token::Gte) => {
            let found = other.to_string();
            Err(ParseError::UnexpectedToken {
                expected: "column selector ('@col', quoted string key, 'col.prev', or 'col.next')",
                found,
                offset,
            })
        }
    }
}

fn parse_col_spec(stream: &mut TokenStream<'_>) -> Result<ColSelector, ParseError> {
    let start = parse_col_endpoint(stream)?;
    if stream.peek_token() == Some(&Token::DotDot) {
        stream.advance();
        let end = parse_col_endpoint(stream)?;
        Ok(ColSelector::Range(start, end))
    } else {
        Ok(ColSelector::from(start))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]
mod tests {
    use super::*;
    use crate::model::id::{ColId, RowId, TableId};

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
        let Expr::FunctionCall { name, args } = expr else {
            panic!("Expected FunctionCall, got {expr:?}");
        };
        assert_eq!(name, "IF");
        assert_eq!(args.len(), 3);
        assert_eq!(
            args[0],
            CallArg::Expr(Expr::Binary {
                op: BinaryOp::Eq,
                left: Box::new(Expr::Reference(TableRef::new(
                    "Assumptions",
                    RowSelector::Key(RowId::new("flag")),
                    ColSelector::Key(ColId::new("active")),
                ))),
                right: Box::new(Expr::number(1.0)),
            })
        );
        assert_eq!(args[1], CallArg::Expr(Expr::number(100.0)));
        assert_eq!(args[2], CallArg::Expr(Expr::number(0.0)));

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
                ColSelector::Offset(RelativeOffset::Prev(1)),
            ))
        );

        // col.prev(2)
        let prev2 = parse_expr_str("BalanceSheet[\"cash\", col.prev(2)]").unwrap();
        assert_eq!(
            prev2,
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Key(RowId::new("cash")),
                ColSelector::Offset(RelativeOffset::Prev(2)),
            ))
        );

        // col.next(3)
        let next3 = parse_expr_str("BalanceSheet[\"cash\", col.next(3)]").unwrap();
        assert_eq!(
            next3,
            Expr::Reference(TableRef::new(
                TableId::new("BalanceSheet"),
                RowSelector::Key(RowId::new("cash")),
                ColSelector::Offset(RelativeOffset::Next(3)),
            ))
        );

        // row.prev default (1)
        let row_prev1 = parse_expr_str("DebtSchedule[row.prev, \"balance\"]").unwrap();
        assert_eq!(
            row_prev1,
            Expr::Reference(TableRef::new(
                TableId::new("DebtSchedule"),
                RowSelector::Offset(RelativeOffset::Prev(1)),
                ColSelector::Key(ColId::new("balance")),
            ))
        );

        // row.next(2)
        let row_next2 = parse_expr_str("DebtSchedule[row.next(2), \"balance\"]").unwrap();
        assert_eq!(
            row_next2,
            Expr::Reference(TableRef::new(
                TableId::new("DebtSchedule"),
                RowSelector::Offset(RelativeOffset::Next(2)),
                ColSelector::Key(ColId::new("balance")),
            ))
        );

        // Horizontal range: SUM(Table["row", "2024E".."2028E"])
        let h_range =
            parse_expr_str("SUM(IncomeStatement[\"revenue\", \"2024E\"..\"2028E\"])").unwrap();
        assert_eq!(
            h_range,
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![CallArg::Expr(Expr::Reference(TableRef::new(
                    TableId::new("IncomeStatement"),
                    RowSelector::Key(RowId::new("revenue")),
                    ColSelector::Range("2024E".into(), "2028E".into()),
                )))],
            }
        );

        // Vertical slice active column: Opex["r_and_d".."g_and_a", @col]
        let v_slice = parse_expr_str("Opex[\"r_and_d\"..\"g_and_a\", @col]").unwrap();
        assert_eq!(
            v_slice,
            Expr::Reference(TableRef::new(
                TableId::new("Opex"),
                RowSelector::Range("r_and_d".into(), "g_and_a".into()),
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
                RowSelector::Range("r_and_d".into(), "g_and_a".into()),
                ColSelector::Range("2024E".into(), "2028E".into()),
            ))
        );
    }

    #[test]
    fn test_reject_dot_access() {
        // Dot access with active column
        let err_dot = parse_expr_str("IncomeStatement.revenue[@col]").unwrap_err();
        let ParseError::UnexpectedToken { found, offset, .. } = err_dot else {
            panic!("Expected UnexpectedToken for dot, got {err_dot:?}");
        };
        assert_eq!(found, ".");
        assert_eq!(offset, 15);

        // Dot access with row identifier only
        let err_dot_bare = parse_expr_str("IncomeStatement.revenue").unwrap_err();
        let ParseError::UnexpectedToken { found, offset, .. } = err_dot_bare else {
            panic!("Expected UnexpectedToken for dot, got {err_dot_bare:?}");
        };
        assert_eq!(found, ".");
        assert_eq!(offset, 15);
    }

    #[test]
    fn test_reject_unquoted_coordinate_keys() {
        let err1 = parse_expr_str("IncomeStatement[\"revenue\", 2024E]").unwrap_err();
        let ParseError::UnquotedCoordinateKey { found, .. } = err1 else {
            panic!("Expected UnquotedCoordinateKey, got {err1:?}");
        };
        assert_eq!(found, "2024E");

        let err2 = parse_expr_str("Opex[r_and_d, @col]").unwrap_err();
        let ParseError::UnquotedCoordinateKey { found, .. } = err2 else {
            panic!("Expected UnquotedCoordinateKey, got {err2:?}");
        };
        assert_eq!(found, "r_and_d");
    }

    #[test]
    fn test_reject_lowercase_function_name() {
        let err = parse_expr_str("sum(1, 2)").unwrap_err();
        let ParseError::InvalidFunctionName { name, .. } = err else {
            panic!("Expected InvalidFunctionName, got {err:?}");
        };
        assert_eq!(name, "sum");
    }

    #[test]
    fn test_reject_bare_identifiers() {
        // Bare table identifier without access operator
        let err_table = parse_expr_str("IncomeStatement").unwrap_err();
        let ParseError::UnexpectedToken { found, .. } = err_table else {
            panic!("Expected UnexpectedToken, got {err_table:?}");
        };
        assert_eq!(found, "IncomeStatement");
    }

    #[test]
    fn test_keyword_boundary_validation() {
        // col.prevent should not parse as col.prev
        let err = parse_expr_str("Table[\"row\", col.prevent]").unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));

        // row.prevent should not parse as row.prev
        let err_row = parse_expr_str("Table[row.prevent, \"col\"]").unwrap_err();
        assert!(matches!(err_row, ParseError::UnexpectedToken { .. }));

        // col.nextly should not parse as col.next
        let err = parse_expr_str("Table[\"row\", col.nextly]").unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));

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
            parse_expr_str("SUM(1, 2").unwrap_err(),
            ParseError::UnexpectedEof {
                expected: "',' or ')'"
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
            "TRUE",
            "FALSE",
            "PMT(0.05, 10, 1000, _, 1)",
            "IF(Assumptions[\"flag\", \"active\"] = 1, 100, 0)",
            "IF(Assumptions[\"flag\", \"active\"] = TRUE, 100, _)",
            "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
            "SUM(IncomeStatement[\"revenue\", \"2024E\"..\"2028E\"])",
            "Opex[\"r_and_d\"..\"g_and_a\", @col]",
            "BalanceSheet[\"cash\", col.prev]",
            "BalanceSheet[\"cash\", col.prev(2)]",
            "(2 ^ 3) ^ 4",
            "2 ^ (3 ^ 4)",
            "BalanceSheet[\"cash\", @col]",
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
    fn test_boolean_literals_parsing_and_roundtrip() {
        let true_expr = parse_expr_str("TRUE").unwrap();
        assert_eq!(true_expr, Expr::Literal(Literal::Boolean(true)));
        assert_eq!(true_expr.to_string(), "TRUE");

        let false_expr = parse_expr_str("FALSE").unwrap();
        assert_eq!(false_expr, Expr::Literal(Literal::Boolean(false)));
        assert_eq!(false_expr.to_string(), "FALSE");

        // Compound expression with booleans
        let if_bool = parse_expr_str("IF(TRUE, 1, 0)").unwrap();
        assert_eq!(if_bool.to_string(), "IF(TRUE, 1, 0)");
    }

    #[test]
    fn test_omitted_argument_in_function_call() {
        let parsed = parse_expr_str("PMT(0.05, 10, 1000, _, 1)").unwrap();
        if let Expr::FunctionCall { name, args } = &parsed {
            assert_eq!(name, "PMT");
            assert_eq!(args.len(), 5);
            assert_eq!(args[0], CallArg::Expr(Expr::number(0.05)));
            assert_eq!(args[1], CallArg::Expr(Expr::number(10.0)));
            assert_eq!(args[2], CallArg::Expr(Expr::number(1000.0)));
            assert_eq!(args[3], CallArg::Omitted);
            assert_eq!(args[4], CallArg::Expr(Expr::number(1.0)));
        } else {
            panic!("Expected FunctionCall");
        }
        assert_eq!(parsed.to_string(), "PMT(0.05, 10, 1000, _, 1)");
    }

    #[test]
    fn test_grammatical_confinement_of_underscore() {
        // Standalone underscore at root and in arithmetic / parenthesized / call sub-expressions
        assert!(matches!(
            parse_expr_str("_").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
        assert!(matches!(
            parse_expr_str("1 + _").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
        assert!(matches!(
            parse_expr_str("-_").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
        assert!(matches!(
            parse_expr_str("_ * 2").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
        assert!(matches!(
            parse_expr_str("_ + 1").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
        assert!(matches!(
            parse_expr_str("IF(TRUE, 10, _ + 1)").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
        assert!(matches!(
            parse_expr_str("IF(TRUE, 10, (_))").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));

        // In table selector coordinates
        assert!(matches!(
            parse_expr_str("Table[_, @col]").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
        assert!(matches!(
            parse_expr_str("Table[@row, _]").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));

        // As table name
        assert!(matches!(
            parse_expr_str("_[\"rev\", @col]").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
    }

    #[test]
    fn test_identifier_preservation_with_underscores() {
        // Embedded and leading underscores in identifiers
        let tokens = tokenize("tax_rate _revenue rev_2024").unwrap();
        assert_eq!(tokens[0].token, Token::Ident("tax_rate"));
        assert_eq!(tokens[1].token, Token::Ident("_revenue"));
        assert_eq!(tokens[2].token, Token::Ident("rev_2024"));

        // Standalone underscore tokenizes as Underscore
        let tokens2 = tokenize("FUNC(a, _, c)").unwrap();
        assert_eq!(tokens2[4].token, Token::Underscore);
    }

    #[test]
    fn test_rejection_of_null_in_formulas() {
        // Standalone null is rejected
        assert!(matches!(
            parse_expr_str("null").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));

        // null inside function calls is rejected as a bare identifier
        assert!(matches!(
            parse_expr_str("IF(A1, B1, null)").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));

        // null in arithmetic is rejected
        assert!(matches!(
            parse_expr_str("1 + null").unwrap_err(),
            ParseError::UnexpectedToken { .. }
        ));
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

    #[test]
    fn test_quoted_string_and_coordinate_key_display_escaping_roundtrip() {
        let samples = [
            "Domestic \"Main\"",
            "a\\b",
            "C:\\path\\",
            "\\\"",
            "\\\"mix\\\"\\",
            "line1\nline2",
            "España 東京 € 😀 \"quoted\" \\slash\\",
        ];

        // Empty string round-trip for Literal::String and Token::String
        let empty_lit = Expr::Literal(Literal::String(String::new()));
        assert_eq!(empty_lit.to_string(), "\"\"");
        assert_eq!(parse_expr_str(&empty_lit.to_string()).unwrap(), empty_lit);
        let empty_tok = Token::String(String::new()).to_string();
        assert_eq!(empty_tok, "\"\"");
        assert_eq!(
            tokenize(&empty_tok).unwrap()[0].token,
            Token::String(String::new())
        );

        // Direct escaping check on C:\path\ and \"
        assert_eq!(
            Literal::String("C:\\path\\".into()).to_string(),
            "\"C:\\\\path\\\\\""
        );
        assert_eq!(Literal::String("\\\"".into()).to_string(), "\"\\\\\\\"\"");

        for sample in samples {
            // 1. Token::String
            let tok_rendered = Token::String(sample.to_string()).to_string();
            let lexed = tokenize(&tok_rendered).unwrap_or_else(|e| {
                panic!("Token::String Display failed to re-tokenize for {sample:?}: {tok_rendered} ({e})")
            });
            assert_eq!(lexed.len(), 1);
            assert_eq!(lexed[0].token, Token::String(sample.to_string()));

            // 2. Literal::String
            let lit_expr = Expr::Literal(Literal::String(sample.to_string()));
            let lit_rendered = lit_expr.to_string();
            assert_eq!(lit_rendered, tok_rendered);
            assert_eq!(parse_expr_str(&lit_rendered).unwrap(), lit_expr);

            // 3. RowSelector::Key & 4. ColSelector::Key
            let key_ref = Expr::Reference(TableRef::new(
                "T",
                RowSelector::Key(RowId::new(sample)),
                ColSelector::Key(ColId::new(sample)),
            ));
            assert_eq!(
                RowSelector::Key(RowId::new(sample)).to_string(),
                tok_rendered
            );
            assert_eq!(
                ColSelector::Key(ColId::new(sample)).to_string(),
                tok_rendered
            );
            assert_eq!(parse_expr_str(&key_ref.to_string()).unwrap(), key_ref);

            // 5. RowEndpoint::Key & 6. ColEndpoint::Key
            let range_ref = Expr::FunctionCall {
                name: "SUM".into(),
                args: vec![CallArg::Expr(Expr::Reference(TableRef::new(
                    "T",
                    RowSelector::Range(RowEndpoint::Key(RowId::new(sample)), RowEndpoint::Active),
                    ColSelector::Range(ColEndpoint::Active, ColEndpoint::Key(ColId::new(sample))),
                )))],
            };
            assert_eq!(
                RowEndpoint::Key(RowId::new(sample)).to_string(),
                tok_rendered
            );
            assert_eq!(
                ColEndpoint::Key(ColId::new(sample)).to_string(),
                tok_rendered
            );
            assert_eq!(parse_expr_str(&range_ref.to_string()).unwrap(), range_ref);
        }
    }

    #[test]
    fn test_programmatic_negative_literal_binary_operand_parenthesization() {
        let neg_pow = Expr::Binary {
            op: BinaryOp::Pow,
            left: Box::new(Expr::number(-2.0)),
            right: Box::new(Expr::number(2.0)),
        };
        assert_eq!(neg_pow.to_string(), "(-2) ^ 2");
        assert_eq!(
            parse_expr_str(&neg_pow.to_string()).unwrap(),
            parse_expr_str("(-2) ^ 2").unwrap()
        );

        let sub_neg = Expr::Binary {
            op: BinaryOp::Sub,
            left: Box::new(Expr::number(1.0)),
            right: Box::new(Expr::number(-2.0)),
        };
        assert_eq!(sub_neg.to_string(), "1 - (-2)");
        assert_eq!(
            parse_expr_str(&sub_neg.to_string()).unwrap(),
            parse_expr_str("1 - (-2)").unwrap()
        );

        let add_neg_left = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::number(-2.0)),
            right: Box::new(Expr::number(1.0)),
        };
        assert_eq!(add_neg_left.to_string(), "-2 + 1");
        assert_eq!(
            parse_expr_str(&add_neg_left.to_string()).unwrap(),
            parse_expr_str("-2 + 1").unwrap()
        );

        let pow_neg_right = Expr::Binary {
            op: BinaryOp::Pow,
            left: Box::new(Expr::number(2.0)),
            right: Box::new(Expr::number(-2.0)),
        };
        assert_eq!(pow_neg_right.to_string(), "2 ^ (-2)");

        // Programmatic -0.0 normalizes to "0" without wrapping in parentheses
        assert_eq!(Expr::number(-0.0).to_string(), "0");
        assert_eq!(Token::Number(-0.0).to_string(), "0");
        assert_eq!(
            Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::number(1.0)),
                right: Box::new(Expr::number(-0.0)),
            }
            .to_string(),
            "1 + 0"
        );
        assert_eq!(
            Expr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(Expr::number(-0.0)),
                right: Box::new(Expr::number(2.0)),
            }
            .to_string(),
            "0 ^ 2"
        );
        assert_eq!(
            Expr::Binary {
                op: BinaryOp::Sub,
                left: Box::new(Expr::number(1.0)),
                right: Box::new(Expr::number(-0.0)),
            }
            .to_string(),
            "1 - 0"
        );
    }

    #[test]
    fn test_scientific_notation_lexing_and_coordinate_key_boundaries() {
        let pos_cases = [
            ("1e9", 1e9_f64),
            ("1E9", 1e9_f64),
            ("1e-9", 1e-9_f64),
            ("1E+9", 1e9_f64),
            ("1.5e-9", 1.5e-9_f64),
            ("0.25E+4", 2500.0_f64),
            ("0e-5", 0.0_f64),
        ];

        for (raw, expected) in pos_cases {
            let parsed = parse_expr_str(raw).unwrap_or_else(|e| panic!("failed on '{raw}': {e}"));
            let Expr::Literal(Literal::Number(actual)) = parsed else {
                panic!("expected Literal::Number for '{raw}', got {parsed:?}");
            };
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "bit mismatch on '{raw}': expected {expected:?}, got {actual:?}"
            );
        }

        let neg_sci = parse_expr_str("-1.25e-6").unwrap();
        assert_eq!(
            neg_sci,
            Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(Expr::number(1.25e-6)),
            }
        );

        let sub_sci = parse_expr_str("1e-9-2e-9").unwrap();
        assert_eq!(
            sub_sci,
            Expr::Binary {
                op: BinaryOp::Sub,
                left: Box::new(Expr::number(1e-9)),
                right: Box::new(Expr::number(2e-9)),
            }
        );

        // Unquoted alphanumeric coordinate keys remain Token::Ident and yield UnquotedCoordinateKey
        for key in ["2024E", "2024E1Q", "2023A"] {
            let toks = tokenize(key).unwrap();
            assert_eq!(toks.len(), 1);
            assert_eq!(toks[0].token, Token::Ident(key));

            let col_formula = format!("IncomeStatement[\"revenue\", {key}]");
            let expected_col_offset = col_formula.find(key).unwrap();
            assert_eq!(
                parse_expr_str(&col_formula).unwrap_err(),
                ParseError::UnquotedCoordinateKey {
                    found: key.to_string(),
                    offset: expected_col_offset,
                }
            );

            let row_formula = format!("IncomeStatement[{key}, @col]");
            let expected_row_offset = row_formula.find(key).unwrap();
            assert_eq!(
                parse_expr_str(&row_formula).unwrap_err(),
                ParseError::UnquotedCoordinateKey {
                    found: key.to_string(),
                    offset: expected_row_offset,
                }
            );
        }
    }

    #[test]
    fn test_overflow_and_malformed_numeric_literals_rejected() {
        let huge_int = "9".repeat(310);
        let bad_literals = [
            "1e500",
            "1.8e308",
            huge_int.as_str(),
            "1.5e",
            "1.5e-",
            "1.5e+",
            "1.5e9a",
            "1.5abc",
            "1e+",
            "1e-",
            "1e+9a",
            "1e-abc",
            "1.",
            "1.e5",
        ];

        for bad in bad_literals {
            assert_eq!(
                parse_expr_str(bad).unwrap_err(),
                ParseError::InvalidNumber {
                    raw: bad.to_string(),
                    offset: 0,
                },
                "expected InvalidNumber at offset 0 for '{bad}'"
            );

            let prefixed = format!("10 + {bad}");
            assert_eq!(
                parse_expr_str(&prefixed).unwrap_err(),
                ParseError::InvalidNumber {
                    raw: bad.to_string(),
                    offset: 5,
                },
                "expected InvalidNumber at offset 5 for '{prefixed}'"
            );
        }
    }

    #[test]
    fn test_number_literal_display_decimal_window_and_lossless_roundtrip() {
        let decimal_cases = [
            (0.0_f64, "0"),
            (-0.0_f64, "0"),
            (1e-6_f64, "0.000001"),
            (-1e-6_f64, "-0.000001"),
            (42.5_f64, "42.5"),
            (999_999_999_999_999.0_f64, "999999999999999"),
            (0.333_333_333_333_333_37_f64, "0.33333333333333337"),
        ];

        let sci_cases = [
            (1e-9_f64, "1E-9"),
            (9.999_999_999_999_997e-7_f64, "9.999999999999997E-7"),
            (1e15_f64, "1E15"),
            (1e16_f64, "1E16"),
            (1e300_f64, "1E300"),
            (1e-300_f64, "1E-300"),
            (f64::MAX, "1.7976931348623157E308"),
            (-f64::MAX, "-1.7976931348623157E308"),
            (f64::MIN_POSITIVE, "2.2250738585072014E-308"),
            (5e-324_f64, "5E-324"),
        ];

        for (val, expected_str) in decimal_cases.into_iter().chain(sci_cases) {
            let expr_str = Expr::number(val).to_string();
            let tok_str = Token::Number(val).to_string();
            assert_eq!(expr_str, expected_str, "Expr::Display mismatch for {val:?}");
            assert_eq!(tok_str, expected_str, "Token::Display mismatch for {val:?}");

            let reparsed = parse_expr_str(&expr_str)
                .unwrap_or_else(|e| panic!("failed to re-parse '{expr_str}' for {val:?}: {e}"));
            let expected_bits = if val == 0.0 {
                0.0_f64.to_bits()
            } else {
                val.to_bits()
            };
            let actual_bits = if let Expr::Literal(Literal::Number(n)) = &reparsed {
                n.to_bits()
            } else if let Expr::Unary {
                op: UnaryOp::Neg,
                expr,
            } = &reparsed
                && let Expr::Literal(Literal::Number(n)) = expr.as_ref()
            {
                (-n).to_bits()
            } else {
                panic!("unexpected reparsed expr {reparsed:?}");
            };
            assert_eq!(
                actual_bits, expected_bits,
                "bit-level roundtrip mismatch for {val:?} via '{expr_str}'"
            );
        }

        // Programmatic non-finite fallback in Display
        assert_eq!(Expr::number(f64::NAN).to_string(), "NaN");
        assert_eq!(Expr::number(f64::INFINITY).to_string(), "inf");
        assert_eq!(Expr::number(f64::NEG_INFINITY).to_string(), "-inf");
    }
}
