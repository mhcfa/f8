use crate::model::id::{ColId, RowId, TableId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// An abstract syntax tree representation of an f8 formula expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Atomic constant literal (number, string, boolean).
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
    /// Explicitly omitted positional argument in a function call (`_`).
    Omitted,
}

impl Expr {
    /// Helper to construct a number literal expression.
    pub fn number(val: f64) -> Self {
        Self::Literal(Literal::Number(val))
    }

    /// Helper to construct a boolean literal expression.
    pub fn boolean(val: bool) -> Self {
        Self::Literal(Literal::Boolean(val))
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
    /// Boolean constant (`TRUE` or `FALSE`).
    Boolean(bool),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Boolean(true) => write!(f, "TRUE"),
            Self::Boolean(false) => write!(f, "FALSE"),
        }
    }
}

/// Binary infix operators supported by the f8 formula DSL.
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

/// Relative temporal offset on an axis: predecessor (`prev`) or successor (`next`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelativeOffset {
    /// Temporal predecessor offset: `prev` (1) or `prev(k)`.
    Prev(u32),
    /// Temporal successor offset: `next` (1) or `next(k)`.
    Next(u32),
}

impl RelativeOffset {
    /// Formats the relative offset prefixed with the given axis identifier (`"row"` or `"col"`).
    pub(crate) fn fmt_with_axis(self, axis: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prev(1) => write!(f, "{axis}.prev"),
            Self::Prev(k) => write!(f, "{axis}.prev({k})"),
            Self::Next(1) => write!(f, "{axis}.next"),
            Self::Next(k) => write!(f, "{axis}.next({k})"),
        }
    }
}

/// Single-coordinate endpoint on the row axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowEndpoint {
    /// Active evaluation row: `@row`.
    Active,
    /// Explicit row key: `"revenue"`.
    Key(RowId),
    /// Relative row offset: `row.prev`, `row.next(2)`.
    Offset(RelativeOffset),
}

impl RowEndpoint {
    /// Returns `true` if this endpoint tracks the active evaluation row or a relative offset.
    #[must_use]
    pub const fn is_dynamic(&self) -> bool {
        matches!(self, Self::Active | Self::Offset(_))
    }
}

impl From<RowId> for RowEndpoint {
    fn from(id: RowId) -> Self {
        Self::Key(id)
    }
}

impl From<&str> for RowEndpoint {
    fn from(s: &str) -> Self {
        Self::Key(RowId::from(s))
    }
}

impl From<RelativeOffset> for RowEndpoint {
    fn from(offset: RelativeOffset) -> Self {
        Self::Offset(offset)
    }
}

impl fmt::Display for RowEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "@row"),
            Self::Key(k) => write!(f, "\"{k}\""),
            Self::Offset(offset) => offset.fmt_with_axis("row", f),
        }
    }
}

/// Single-coordinate endpoint on the column axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColEndpoint {
    /// Active evaluation column: `@col`.
    Active,
    /// Explicit column key: `"2024E"`.
    Key(ColId),
    /// Relative column offset: `col.prev`, `col.next(2)`.
    Offset(RelativeOffset),
}

impl ColEndpoint {
    /// Returns `true` if this endpoint tracks the active evaluation column or a relative offset.
    #[must_use]
    pub const fn is_dynamic(&self) -> bool {
        matches!(self, Self::Active | Self::Offset(_))
    }
}

impl From<ColId> for ColEndpoint {
    fn from(id: ColId) -> Self {
        Self::Key(id)
    }
}

impl From<&str> for ColEndpoint {
    fn from(s: &str) -> Self {
        Self::Key(ColId::from(s))
    }
}

impl From<RelativeOffset> for ColEndpoint {
    fn from(offset: RelativeOffset) -> Self {
        Self::Offset(offset)
    }
}

impl fmt::Display for ColEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "@col"),
            Self::Key(k) => write!(f, "\"{k}\""),
            Self::Offset(offset) => offset.fmt_with_axis("col", f),
        }
    }
}

/// Row coordinate selector within a table reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowSelector {
    /// Active evaluation row: `[@row]`.
    Active,
    /// Explicit single row key: `["revenue"]`.
    Key(RowId),
    /// Row range slice: `["r_and_d".."g_and_a"]`, `["c1"..@row]`.
    Range(RowEndpoint, RowEndpoint),
    /// Relative row offset: `row.prev`, `row.next(2)`.
    Offset(RelativeOffset),
}

impl RowSelector {
    /// Returns `true` if this selector is a range slice (`start..end`).
    #[must_use]
    pub const fn is_range(&self) -> bool {
        matches!(self, Self::Range(_, _))
    }

    /// Returns `(start_dynamic, end_dynamic)` indicating whether each endpoint is dynamic.
    #[must_use]
    pub const fn dynamic_bounds(&self) -> (bool, bool) {
        match self {
            Self::Active | Self::Offset(_) => (true, true),
            Self::Key(_) => (false, false),
            Self::Range(start, end) => (start.is_dynamic(), end.is_dynamic()),
        }
    }
}

impl From<RowEndpoint> for RowSelector {
    fn from(ep: RowEndpoint) -> Self {
        match ep {
            RowEndpoint::Active => Self::Active,
            RowEndpoint::Key(k) => Self::Key(k),
            RowEndpoint::Offset(o) => Self::Offset(o),
        }
    }
}

impl fmt::Display for RowSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "@row"),
            Self::Key(k) => write!(f, "\"{k}\""),
            Self::Range(a, b) => write!(f, "{a}..{b}"),
            Self::Offset(offset) => offset.fmt_with_axis("row", f),
        }
    }
}

/// Column coordinate selector within a table reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColSelector {
    /// Active evaluation column: `[@col]`.
    Active,
    /// Explicit column key: `["value"]`, `["2024E"]`.
    Key(ColId),
    /// Column range slice: `["2024E".."2028E"]`, `["2024E"..@col]`.
    Range(ColEndpoint, ColEndpoint),
    /// Relative column offset: `col.prev`, `col.next(2)`.
    Offset(RelativeOffset),
}

impl ColSelector {
    /// Returns `true` if this selector is a range slice (`start..end`).
    #[must_use]
    pub const fn is_range(&self) -> bool {
        matches!(self, Self::Range(_, _))
    }

    /// Returns `(start_dynamic, end_dynamic)` indicating whether each endpoint is dynamic.
    #[must_use]
    pub const fn dynamic_bounds(&self) -> (bool, bool) {
        match self {
            Self::Active | Self::Offset(_) => (true, true),
            Self::Key(_) => (false, false),
            Self::Range(start, end) => (start.is_dynamic(), end.is_dynamic()),
        }
    }
}

impl From<ColEndpoint> for ColSelector {
    fn from(ep: ColEndpoint) -> Self {
        match ep {
            ColEndpoint::Active => Self::Active,
            ColEndpoint::Key(k) => Self::Key(k),
            ColEndpoint::Offset(o) => Self::Offset(o),
        }
    }
}

impl fmt::Display for ColSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "@col"),
            Self::Key(k) => write!(f, "\"{k}\""),
            Self::Range(a, b) => write!(f, "{a}..{b}"),
            Self::Offset(offset) => offset.fmt_with_axis("col", f),
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
            Self::Omitted => write!(f, "_"),
        }
    }
}
