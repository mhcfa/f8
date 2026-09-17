use crate::model::id::{ColGroupId, ColId, RowGroupId, RowId, TableId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// An abstract syntax tree representation of a Finx formula expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Atomic constant literal (number, string, null).
    Literal(Literal),
    /// Coordinate or slice reference to a table.
    Reference(TableRef),
    /// Unary prefix operation (e.g. `-x`, `+x`).
    Unary { op: UnaryOp, expr: Box<Expr> },
    /// Binary infix operation (e.g. `a + b`, `x * y`).
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// Uppercase built-in function invocation (e.g. `SUM(...)`, `IF(...)`).
    FunctionCall { name: String, args: Vec<Expr> },
}

impl Expr {
    /// Helper to construct a number literal expression.
    pub fn number(val: f64) -> Self {
        Self::Literal(Literal::Number(val))
    }

    /// Helper to construct a null literal expression.
    pub fn null() -> Self {
        Self::Literal(Literal::Null)
    }

    /// Returns the binding power / precedence of this expression if it is a binary operation,
    /// or `None` if it is an atomic expression or function call.
    pub fn precedence(&self) -> Option<u8> {
        match self {
            Self::Binary { op, .. } => Some(op.precedence()),
            Self::Unary { op, .. } => Some(op.precedence()),
            _ => None,
        }
    }
}

/// Constant literal value in a formula expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    /// Floating-point or integer numeric constant.
    Number(f64),
    /// Quoted text string constant.
    String(String),
    /// Explicit null literal.
    Null,
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Null => write!(f, "null"),
        }
    }
}

/// Binary infix operators supported by the Finx formula DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinaryOp {
    /// Exponentiation (`^`).
    Pow,
    /// Multiplication (`*`).
    Mul,
    /// Division (`/`).
    Div,
    /// Addition (`+`).
    Add,
    /// Subtraction (`-`).
    Sub,
    /// Equality comparison (`=`).
    Eq,
    /// Inequality comparison (`<>`).
    NotEq,
    /// Less than (`<`).
    Lt,
    /// Less than or equal (`<=`).
    Lte,
    /// Greater than (`>`).
    Gt,
    /// Greater than or equal (`>=`).
    Gte,
}

impl BinaryOp {
    /// Returns the precedence level of the binary operator (1 = lowest, 5 = highest).
    pub fn precedence(self) -> u8 {
        match self {
            Self::Pow => 5,
            Self::Mul | Self::Div => 3,
            Self::Add | Self::Sub => 2,
            Self::Eq | Self::NotEq | Self::Lt | Self::Lte | Self::Gt | Self::Gte => 1,
        }
    }

    /// Returns `true` if the operator is right-associative (e.g. `^`).
    pub fn is_right_associative(self) -> bool {
        matches!(self, Self::Pow)
    }

    /// Returns the canonical textual representation of the operator.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pow => "^",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Eq => "=",
            Self::NotEq => "<>",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Gt => ">",
            Self::Gte => ">=",
        }
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Unary prefix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOp {
    /// Unary positive (`+`).
    Pos,
    /// Unary negation (`-`).
    Neg,
}

impl UnaryOp {
    /// Returns the precedence level of the unary operator.
    pub fn precedence(self) -> u8 {
        4
    }

    /// Returns the canonical textual representation of the unary operator.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pos => "+",
            Self::Neg => "-",
        }
    }
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Reference addressing target to a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRef {
    /// Target table identifier.
    pub table: TableId,
    /// Row coordinate selector.
    pub row: RowSelector,
    /// Column coordinate selector.
    pub col: ColSelector,
}

impl TableRef {
    /// Constructs a table reference: `Table[row, col]`.
    pub fn new(table: impl Into<TableId>, row: RowSelector, col: ColSelector) -> Self {
        Self {
            table: table.into(),
            row,
            col,
        }
    }
}

impl fmt::Display for TableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}, {}]", self.table, self.row, self.col)
    }
}

/// Row coordinate selector within a table reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowSelector {
    /// Explicit single row key: `["revenue"]`.
    Key(RowId),
    /// Explicit row range slice: `["r_and_d".."g_and_a"]`.
    Range(RowId, RowId),
    /// Semantic row group milestone or hierarchy: `rows.current_assets`.
    Special(SpecialRow),
}

impl fmt::Display for RowSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(k) => write!(f, "\"{k}\""),
            Self::Range(a, b) => write!(f, "\"{a}\"..\"{b}\""),
            Self::Special(s) => write!(f, "{s}"),
        }
    }
}

/// Column coordinate selector within a table reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColSelector {
    /// Active evaluation column: `[@]`.
    Active,
    /// Explicit column key: `["value"]`, `["2024E"]`.
    Key(ColId),
    /// Explicit column range slice: `["2024E".."2028E"]`.
    Range(ColId, ColId),
    /// Semantic column offset or group: `col.prev`, `cols.forecast`.
    Special(SpecialCol),
}

impl fmt::Display for ColSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "@"),
            Self::Key(k) => write!(f, "\"{k}\""),
            Self::Range(a, b) => write!(f, "\"{a}\"..\"{b}\""),
            Self::Special(s) => write!(f, "{s}"),
        }
    }
}

/// Semantic row milestone or group selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialRow {
    /// All descendant rows in a group: `rows.<group>`.
    Group(RowGroupId),
    /// First row in a group: `rows.<group>.first`.
    GroupFirst(RowGroupId),
    /// Last row in a group: `rows.<group>.last`.
    GroupLast(RowGroupId),
}

impl fmt::Display for SpecialRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Group(g) => write!(f, "rows.{g}"),
            Self::GroupFirst(g) => write!(f, "rows.{g}.first"),
            Self::GroupLast(g) => write!(f, "rows.{g}.last"),
        }
    }
}

/// Semantic column milestone, group, or temporal offset selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialCol {
    /// Temporal predecessor offset: `col.prev` (offset 1) or `col.prev(k)`.
    Prev(u32),
    /// Temporal successor offset: `col.next` (offset 1) or `col.next(k)`.
    Next(u32),
    /// All columns in a group: `cols.<group>`.
    Group(ColGroupId),
    /// First column in a group: `cols.<group>.first`.
    GroupFirst(ColGroupId),
    /// Last column in a group: `cols.<group>.last`.
    GroupLast(ColGroupId),
}

impl fmt::Display for SpecialCol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prev(1) => write!(f, "col.prev"),
            Self::Prev(k) => write!(f, "col.prev({k})"),
            Self::Next(1) => write!(f, "col.next"),
            Self::Next(k) => write!(f, "col.next({k})"),
            Self::Group(g) => write!(f, "cols.{g}"),
            Self::GroupFirst(g) => write!(f, "cols.{g}.first"),
            Self::GroupLast(g) => write!(f, "cols.{g}.last"),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(lit) => write!(f, "{lit}"),
            Self::Reference(r) => write!(f, "{r}"),
            Self::Unary { op, expr } => {
                let need_paren = expr.precedence().is_some_and(|p| p < op.precedence());
                if need_paren {
                    write!(f, "{op}({expr})")
                } else {
                    write!(f, "{op}{expr}")
                }
            }
            Self::Binary { op, left, right } => {
                let left_paren = left.precedence().is_some_and(|p| {
                    if op.is_right_associative() {
                        p <= op.precedence()
                    } else {
                        p < op.precedence()
                    }
                });
                let right_paren = right.precedence().is_some_and(|p| {
                    if op.is_right_associative() {
                        p < op.precedence()
                    } else {
                        p <= op.precedence()
                    }
                });

                if left_paren {
                    write!(f, "({left})")?;
                } else {
                    write!(f, "{left}")?;
                }

                write!(f, " {op} ")?;

                if right_paren {
                    write!(f, "({right})")
                } else {
                    write!(f, "{right}")
                }
            }
            Self::FunctionCall { name, args } => {
                write!(f, "{name}(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
        }
    }
}
