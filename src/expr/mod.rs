//! Formula expression AST, lexer, and parser.
//!
//! Implements the formula grammar and operator precedence defined in
//! `docs/dsl/grammar.md` and addressing rules in `docs/dsl/addressing.md`.

pub mod ast;
pub mod error;
pub mod parser;

pub use ast::{
    BinaryOp, ColSelector, Expr, Literal, RefAccess, RowSelector, SpecialCol, SpecialRow, TableRef,
    UnaryOp,
};
pub use error::ParseError;
pub use parser::{SpannedToken, Token, tokenize};

impl Expr {
    /// Parses a formula expression string into an [`Expr`] AST.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parser::parse_expr_str(input)
    }
}
