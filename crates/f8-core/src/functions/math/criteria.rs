//! Excel criteria parsing and matching engine.
//!
//! Provides single-pass compilation of criteria arguments for conditional aggregation
//! (`*IFS`) and pattern-matching lookup functions, conforming to Excel criteria parity rules.

use std::cmp::Ordering;

pub use crate::functions::common::WildcardToken;
use crate::functions::common::{matches_wildcard, parse_wildcard_tokens};
use crate::value::{
    CellValue, ExcelError, compare_ascii_case_insensitive, normalize_zero, parse_excel_number,
};

/// Relational comparison operators supported in criteria strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    /// Exact equality (`=`).
    Eq,
    /// Inequality (`<>`).
    NotEq,
    /// Strictly less than (`<`).
    Lt,
    /// Less than or equal to (`<=`).
    LtEq,
    /// Strictly greater than (`>`).
    Gt,
    /// Greater than or equal to (`>=`).
    GtEq,
}

impl ComparisonOp {
    /// Evaluates this comparison operator against an `Ordering`.
    #[must_use]
    pub fn matches_ordering(self, ordering: Ordering) -> bool {
        match self {
            Self::Eq => ordering.is_eq(),
            Self::NotEq => ordering.is_ne(),
            Self::Lt => ordering.is_lt(),
            Self::LtEq => ordering.is_le(),
            Self::Gt => ordering.is_gt(),
            Self::GtEq => ordering.is_ge(),
        }
    }
}

/// Target operand against which candidate cells are compared.
#[derive(Debug, Clone, PartialEq)]
pub enum CriteriaTarget {
    /// Matches blank cells (`CellValue::Empty` or `CellValue::Text("")`).
    Empty,
    /// Numeric target value.
    Number(f64),
    /// Boolean target value.
    Boolean(bool),
    /// Domain error target value.
    Error(ExcelError),
    /// Wildcard text pattern.
    Wildcard(Vec<WildcardToken>),
    /// Plain text target (case-insensitive ASCII matching).
    Text(String),
}

/// A compiled, executable Excel criterion.
#[derive(Debug, Clone, PartialEq)]
pub struct Criterion {
    /// Relational comparison operator.
    pub op: ComparisonOp,
    /// Target value or pattern.
    pub target: CriteriaTarget,
}

impl Criterion {
    /// Compiles a runtime `CellValue` into an executable `Criterion`.
    #[must_use]
    pub fn parse(val: &CellValue) -> Self {
        match val {
            CellValue::Number(n) => {
                if n.is_finite() {
                    Self {
                        op: ComparisonOp::Eq,
                        target: CriteriaTarget::Number(normalize_zero(*n)),
                    }
                } else {
                    Self {
                        op: ComparisonOp::Eq,
                        target: CriteriaTarget::Error(ExcelError::Num),
                    }
                }
            }
            CellValue::Boolean(b) => Self {
                op: ComparisonOp::Eq,
                target: CriteriaTarget::Boolean(*b),
            },
            CellValue::Error(e) => Self {
                op: ComparisonOp::Eq,
                target: CriteriaTarget::Error(*e),
            },
            CellValue::Empty => {
                // In Excel, an empty cell reference used as criterion evaluates to numeric 0.
                Self {
                    op: ComparisonOp::Eq,
                    target: CriteriaTarget::Number(0.0),
                }
            }
            CellValue::Text(s) => Self::parse_str(s),
        }
    }

    /// Parses a string expression into a `Criterion`.
    #[must_use]
    pub fn parse_str(s: &str) -> Self {
        let (op, rest, has_explicit_op) = if let Some(stripped) = s.strip_prefix(">=") {
            (ComparisonOp::GtEq, stripped, true)
        } else if let Some(stripped) = s.strip_prefix("<=") {
            (ComparisonOp::LtEq, stripped, true)
        } else if let Some(stripped) = s.strip_prefix("<>") {
            (ComparisonOp::NotEq, stripped, true)
        } else if let Some(stripped) = s.strip_prefix('>') {
            (ComparisonOp::Gt, stripped, true)
        } else if let Some(stripped) = s.strip_prefix('<') {
            (ComparisonOp::Lt, stripped, true)
        } else if let Some(stripped) = s.strip_prefix('=') {
            (ComparisonOp::Eq, stripped, true)
        } else {
            (ComparisonOp::Eq, s, false)
        };

        // Check for empty/blank sentinels: "" or "=" matches empty cells, "<>" matches non-blank
        if rest.is_empty() {
            return match op {
                ComparisonOp::Eq | ComparisonOp::NotEq => Self {
                    op,
                    target: CriteriaTarget::Empty,
                },
                ComparisonOp::Lt | ComparisonOp::LtEq | ComparisonOp::Gt | ComparisonOp::GtEq => {
                    Self {
                        op,
                        target: CriteriaTarget::Text(String::new()),
                    }
                }
            };
        }

        // For plain text without an explicit operator (e.g. " 10", "10 ", " TRUE"),
        // leading or trailing ASCII spaces force literal text matching in Excel.
        // After an explicit operator (e.g. "> 10", "> 10 ", "= TRUE "), ASCII spaces
        // around numeric, boolean, and error operands are trimmed.
        let can_parse_typed_operand =
            has_explicit_op || (!rest.starts_with(' ') && !rest.ends_with(' '));

        if can_parse_typed_operand {
            if let Some(num) = parse_excel_number(rest) {
                return Self {
                    op,
                    target: CriteriaTarget::Number(num),
                };
            }

            let trimmed_for_keywords = rest.trim_matches(' ');
            if trimmed_for_keywords.eq_ignore_ascii_case("true") {
                return Self {
                    op,
                    target: CriteriaTarget::Boolean(true),
                };
            }
            if trimmed_for_keywords.eq_ignore_ascii_case("false") {
                return Self {
                    op,
                    target: CriteriaTarget::Boolean(false),
                };
            }

            // Check if operand parses as an Excel error string
            if let Ok(err) = trimmed_for_keywords.parse::<ExcelError>() {
                return Self {
                    op,
                    target: CriteriaTarget::Error(err),
                };
            }
        }

        // Wildcard patterns and text unescaping for Eq and NotEq
        if matches!(op, ComparisonOp::Eq | ComparisonOp::NotEq) {
            let (tokens, has_wildcard) = parse_wildcard_tokens(rest);
            if has_wildcard {
                return Self {
                    op,
                    target: CriteriaTarget::Wildcard(tokens),
                };
            }
            let unescaped: String = tokens
                .into_iter()
                .filter_map(|t| match t {
                    WildcardToken::Char(c) => Some(c),
                    WildcardToken::AnySingle | WildcardToken::AnyZeroOrMore => None,
                })
                .collect();
            return Self {
                op,
                target: CriteriaTarget::Text(unescaped),
            };
        }

        Self {
            op,
            target: CriteriaTarget::Text(rest.to_string()),
        }
    }

    /// Evaluates whether a cell value satisfies this criterion.
    #[must_use]
    pub fn matches(&self, cell: &CellValue) -> bool {
        match &self.target {
            CriteriaTarget::Empty => {
                let is_empty = cell.is_empty() || cell.as_text() == Some("");
                match self.op {
                    ComparisonOp::Eq => is_empty,
                    ComparisonOp::NotEq => !is_empty,
                    ComparisonOp::Lt
                    | ComparisonOp::LtEq
                    | ComparisonOp::Gt
                    | ComparisonOp::GtEq => false,
                }
            }
            CriteriaTarget::Number(target_n) => match cell {
                CellValue::Number(cell_n) => {
                    if !cell_n.is_finite() || !target_n.is_finite() {
                        return self.op == ComparisonOp::NotEq;
                    }
                    let ord = normalize_zero(*cell_n).total_cmp(&normalize_zero(*target_n));
                    self.op.matches_ordering(ord)
                }
                CellValue::Text(s) => {
                    // In Excel *IFS, exact equality/inequality matches string cells that parse to a finite number
                    if let Some(num) = parse_excel_number(s) {
                        if !target_n.is_finite() {
                            return self.op == ComparisonOp::NotEq;
                        }
                        let ord = num.total_cmp(&normalize_zero(*target_n));
                        match self.op {
                            ComparisonOp::Eq => ord.is_eq(),
                            ComparisonOp::NotEq => ord.is_ne(),
                            ComparisonOp::Lt
                            | ComparisonOp::LtEq
                            | ComparisonOp::Gt
                            | ComparisonOp::GtEq => false,
                        }
                    } else {
                        // String that cannot be parsed as finite number is not equal to target number
                        self.op == ComparisonOp::NotEq
                    }
                }
                CellValue::Empty | CellValue::Boolean(_) | CellValue::Error(_) => {
                    // Empty, boolean, or error cell does not equal target number
                    self.op == ComparisonOp::NotEq
                }
            },
            CriteriaTarget::Boolean(target_b) => match cell {
                CellValue::Boolean(cell_b) => {
                    let ord = cell_b.cmp(target_b);
                    self.op.matches_ordering(ord)
                }
                CellValue::Text(s) => {
                    if s.eq_ignore_ascii_case("true") {
                        let ord = true.cmp(target_b);
                        self.op.matches_ordering(ord)
                    } else if s.eq_ignore_ascii_case("false") {
                        let ord = false.cmp(target_b);
                        self.op.matches_ordering(ord)
                    } else {
                        self.op == ComparisonOp::NotEq
                    }
                }
                CellValue::Number(_) | CellValue::Empty | CellValue::Error(_) => {
                    self.op == ComparisonOp::NotEq
                }
            },
            CriteriaTarget::Error(target_e) => match cell {
                CellValue::Error(cell_e) => match self.op {
                    ComparisonOp::Eq => cell_e == target_e,
                    ComparisonOp::NotEq => cell_e != target_e,
                    ComparisonOp::Lt
                    | ComparisonOp::LtEq
                    | ComparisonOp::Gt
                    | ComparisonOp::GtEq => false,
                },
                CellValue::Number(_)
                | CellValue::Text(_)
                | CellValue::Boolean(_)
                | CellValue::Empty => self.op == ComparisonOp::NotEq,
            },
            CriteriaTarget::Text(target_s) => match cell {
                CellValue::Text(cell_s) => {
                    let ord = compare_ascii_case_insensitive(cell_s, target_s);
                    self.op.matches_ordering(ord)
                }
                CellValue::Number(_)
                | CellValue::Boolean(_)
                | CellValue::Empty
                | CellValue::Error(_) => self.op == ComparisonOp::NotEq,
            },
            CriteriaTarget::Wildcard(tokens) => match cell {
                CellValue::Text(cell_s) => {
                    let matched = matches_wildcard(tokens, cell_s);
                    match self.op {
                        ComparisonOp::Eq => matched,
                        ComparisonOp::NotEq => !matched,
                        ComparisonOp::Lt
                        | ComparisonOp::LtEq
                        | ComparisonOp::Gt
                        | ComparisonOp::GtEq => false,
                    }
                }
                CellValue::Number(_)
                | CellValue::Boolean(_)
                | CellValue::Empty
                | CellValue::Error(_) => self.op == ComparisonOp::NotEq,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeric_comparisons() {
        let gt10 = Criterion::parse_str(">10");
        assert!(gt10.matches(&CellValue::Number(15.0)));
        assert!(!gt10.matches(&CellValue::Number(10.0)));
        assert!(!gt10.matches(&CellValue::Number(5.0)));
        assert!(!gt10.matches(&CellValue::Text("15".into()))); // relational doesn't match text
        assert!(!gt10.matches(&CellValue::Empty));

        let gte0 = Criterion::parse_str(">=0");
        assert!(gte0.matches(&CellValue::Number(0.0)));
        assert!(gte0.matches(&CellValue::Number(1.0)));
        assert!(!gte0.matches(&CellValue::Number(-1.0)));

        let ne5 = Criterion::parse_str("<>5");
        assert!(ne5.matches(&CellValue::Number(4.0)));
        assert!(!ne5.matches(&CellValue::Number(5.0)));
        assert!(ne5.matches(&CellValue::Text("hello".into())));
        assert!(ne5.matches(&CellValue::Empty));
    }

    #[test]
    fn test_text_exact_and_wildcard() {
        let us = Criterion::parse_str("US");
        assert!(us.matches(&CellValue::Text("us".into())));
        assert!(us.matches(&CellValue::Text("US".into())));
        assert!(!us.matches(&CellValue::Text("USA".into())));

        let star_st = Criterion::parse_str("*st");
        assert!(star_st.matches(&CellValue::Text("East".into())));
        assert!(star_st.matches(&CellValue::Text("WEST".into())));
        assert!(star_st.matches(&CellValue::Text("st".into())));
        assert!(!star_st.matches(&CellValue::Text("South".into())));

        let q_mark = Criterion::parse_str("?est");
        assert!(q_mark.matches(&CellValue::Text("West".into())));
        assert!(q_mark.matches(&CellValue::Text("Best".into())));
        assert!(!q_mark.matches(&CellValue::Text("East".into())));

        let escaped = Criterion::parse_str("a~*c");
        assert!(escaped.matches(&CellValue::Text("a*c".into())));
        assert!(!escaped.matches(&CellValue::Text("abc".into())));
    }

    #[test]
    fn test_blank_and_non_blank_sentinels() {
        let empty_str = Criterion::parse_str("");
        assert!(empty_str.matches(&CellValue::Empty));
        assert!(empty_str.matches(&CellValue::Text("".into())));
        assert!(!empty_str.matches(&CellValue::Number(0.0)));
        assert!(!empty_str.matches(&CellValue::Text("abc".into())));

        let eq_bare = Criterion::parse_str("=");
        assert!(eq_bare.matches(&CellValue::Empty));
        assert!(eq_bare.matches(&CellValue::Text("".into())));

        let non_blank = Criterion::parse_str("<>");
        assert!(!non_blank.matches(&CellValue::Empty));
        assert!(!non_blank.matches(&CellValue::Text("".into())));
        assert!(non_blank.matches(&CellValue::Number(0.0)));
        assert!(non_blank.matches(&CellValue::Text("abc".into())));
    }

    #[test]
    fn test_boolean_and_error_criteria() {
        let bool_crit = Criterion::parse_str("TRUE");
        assert!(bool_crit.matches(&CellValue::Boolean(true)));
        assert!(bool_crit.matches(&CellValue::Text("true".into())));
        assert!(!bool_crit.matches(&CellValue::Boolean(false)));

        let err_crit = Criterion::parse_str("#DIV/0!");
        assert!(err_crit.matches(&CellValue::Error(ExcelError::Div0)));
        assert!(!err_crit.matches(&CellValue::Error(ExcelError::Value)));
    }

    #[test]
    fn test_whitespace_preservation_and_operator_spaces() {
        // Plain string with leading whitespace preserves spaces and does not match trimmed text
        let leading_spaces = Criterion::parse_str("  DeptA");
        assert!(leading_spaces.matches(&CellValue::Text("  DeptA".into())));
        assert!(!leading_spaces.matches(&CellValue::Text("DeptA".into())));

        // String with only whitespace does not match empty cell
        let space_only = Criterion::parse_str(" ");
        assert!(space_only.matches(&CellValue::Text(" ".into())));
        assert!(!space_only.matches(&CellValue::Empty));
        assert!(!space_only.matches(&CellValue::Text("".into())));

        // Operator with space after operator correctly parses numeric operand
        let gt_with_space = Criterion::parse_str("> 10");
        assert!(gt_with_space.matches(&CellValue::Number(15.0)));
        assert!(!gt_with_space.matches(&CellValue::Number(10.0)));

        // Number with leading space is treated as text search in Excel
        let num_with_space = Criterion::parse_str(" 10");
        assert!(num_with_space.matches(&CellValue::Text(" 10".into())));
        assert!(!num_with_space.matches(&CellValue::Number(10.0)));

        // Not equal to space "<> " matches non-space cells (including Empty and numbers)
        // but does not match a cell containing a single space
        let ne_space = Criterion::parse_str("<> ");
        assert!(ne_space.matches(&CellValue::Empty));
        assert!(ne_space.matches(&CellValue::Number(10.0)));
        assert!(ne_space.matches(&CellValue::Text("abc".into())));
        assert!(!ne_space.matches(&CellValue::Text(" ".into())));

        // Equal to space "= " matches only a cell containing a single space
        let eq_space = Criterion::parse_str("= ");
        assert!(eq_space.matches(&CellValue::Text(" ".into())));
        assert!(!eq_space.matches(&CellValue::Empty));
        assert!(!eq_space.matches(&CellValue::Text("".into())));
        assert!(!eq_space.matches(&CellValue::Number(0.0)));

        // Operator with text operand preserves space after operator
        let gt_text_with_space = Criterion::parse_str("> text");
        assert!(gt_text_with_space.matches(&CellValue::Text(" textA".into())));
        assert!(!gt_text_with_space.matches(&CellValue::Text(" text".into())));
        assert!(!gt_text_with_space.matches(&CellValue::Text(" a".into())));

        // Explicit operators trim both leading and trailing ASCII spaces on typed operands
        let gt_trailing_space = Criterion::parse_str("> 10 ");
        assert_eq!(
            gt_trailing_space,
            Criterion {
                op: ComparisonOp::Gt,
                target: CriteriaTarget::Number(10.0),
            }
        );
        assert!(gt_trailing_space.matches(&CellValue::Number(15.0)));
        assert!(!gt_trailing_space.matches(&CellValue::Number(10.0)));

        let eq_bool_trailing = Criterion::parse_str("= TRUE ");
        assert_eq!(
            eq_bool_trailing,
            Criterion {
                op: ComparisonOp::Eq,
                target: CriteriaTarget::Boolean(true),
            }
        );
        assert!(eq_bool_trailing.matches(&CellValue::Boolean(true)));

        let eq_err_trailing = Criterion::parse_str("= #N/A ");
        assert_eq!(
            eq_err_trailing,
            Criterion {
                op: ComparisonOp::Eq,
                target: CriteriaTarget::Error(ExcelError::Na),
            }
        );
        assert!(eq_err_trailing.matches(&CellValue::Error(ExcelError::Na)));

        // Plain number with trailing space remains literal text search
        let num_trailing_space = Criterion::parse_str("10 ");
        assert!(num_trailing_space.matches(&CellValue::Text("10 ".into())));
        assert!(!num_trailing_space.matches(&CellValue::Number(10.0)));
    }

    #[test]
    fn test_non_finite_strings_and_floats_do_not_poison_criteria() {
        for token in [
            "NaN",
            "nan",
            "NAN",
            "-NaN",
            "inf",
            "-inf",
            "INF",
            "Infinity",
            "-Infinity",
            "1e500",
        ] {
            let crit = Criterion::parse_str(token);
            assert!(
                matches!(crit.target, CriteriaTarget::Text(_)),
                "expected Text target for {token:?}, got {crit:?}"
            );
            assert!(!crit.matches(&CellValue::Number(0.0)));
            assert!(!crit.matches(&CellValue::Number(42.0)));
            assert!(crit.matches(&CellValue::Text(token.to_ascii_lowercase())));
            assert!(crit.matches(&CellValue::Text(token.to_ascii_uppercase())));

            let eq_op_crit = Criterion::parse_str(&format!("={token}"));
            assert!(matches!(eq_op_crit.target, CriteriaTarget::Text(_)));
            assert!(!eq_op_crit.matches(&CellValue::Number(42.0)));

            let gt_op_crit = Criterion::parse_str(&format!(">{token}"));
            assert!(matches!(gt_op_crit.target, CriteriaTarget::Text(_)));
            assert!(!gt_op_crit.matches(&CellValue::Number(42.0)));
        }

        // Numeric criterion must not match "NaN", "inf", "Infinity", or overflow text cells,
        // while still matching padded finite numeric text cells (" 42 ") on Eq/NotEq.
        let eq42 = Criterion::parse(&CellValue::Number(42.0));
        let ne42 = Criterion::parse_str("<>42");
        let gt0 = Criterion::parse_str(">0");
        for non_finite_text in ["NaN", "NAN", "inf", "-inf", "Infinity", "1e500"] {
            let text_cell = CellValue::Text(non_finite_text.into());
            assert!(!eq42.matches(&text_cell));
            assert!(ne42.matches(&text_cell));
            assert!(!gt0.matches(&text_cell));
        }
        assert!(eq42.matches(&CellValue::Text(" 42 ".into())));
        assert!(!ne42.matches(&CellValue::Text(" 42 ".into())));

        // Defensive raw non-finite CellValue::Number handling
        let nan_crit = Criterion::parse(&CellValue::Number(f64::NAN));
        assert_eq!(
            nan_crit,
            Criterion {
                op: ComparisonOp::Eq,
                target: CriteriaTarget::Error(ExcelError::Num),
            }
        );
        assert!(!eq42.matches(&CellValue::Number(f64::NAN)));
        assert!(ne42.matches(&CellValue::Number(f64::NAN)));
        assert!(!gt0.matches(&CellValue::Number(f64::NAN)));
        assert!(!gt0.matches(&CellValue::Number(f64::INFINITY)));
    }
}
