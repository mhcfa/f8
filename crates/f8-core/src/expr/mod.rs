//! Formula expression AST, lexer, and parser.
//!
//! Implements formula expression parsing, operator precedence, and 2D matrix coordinate addressing.

pub mod ast;
pub mod error;
pub mod parser;

pub use ast::{
    BinaryOp, ColEndpoint, ColSelector, Expr, Literal, RelativeOffset, RowEndpoint, RowSelector,
    TableRef, UnaryOp,
};
pub use error::ParseError;

impl Expr {
    /// Parses a formula expression string into an [`Expr`] AST.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parser::parse_expr_str(input)
    }
}
