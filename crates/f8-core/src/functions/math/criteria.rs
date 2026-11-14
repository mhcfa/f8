//! Typed conditional aggregation (`*IFS`) criteria matching engine.
//!
//! Enforces unrepresentable invalid states across blank checks, IEEE-754 exact numeric matches,
//! 15-significant-digit numeric comparators, and homogeneous ASCII case-insensitive text and boolean equality.

use crate::expr::ast::{BlankCheck, CriteriaComparator};
use crate::value::{CellValue, compare_ascii_case_insensitive, normalize_zero};

/// Equality polarity for `Text` and `Boolean` criteria (`=` exact vs `<>` inequality).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqualityOp {
    /// Exact equality (`CallArg::Expr`).
    Exact,
    /// Inequality (`CriteriaComparator::NotEq`).
    NotEq,
}

/// Validated, strongly typed executable criterion for `*IFS` conditional aggregation.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedCriterion {
    /// Empty-cell predicate (`BLANK` or `NOT_BLANK`).
    Blank(BlankCheck),
    /// Exact IEEE-754 numeric match (`CallArg::Expr` on `ValueType::Number`).
    NumberExact(f64),
    /// Numeric relational comparison normalized to Excel's 15-significant-digit `&` threshold.
    NumberCmp {
        /// Relational comparator (`<>`, `<`, `<=`, `>`, `>=`).
        op: CriteriaComparator,
        /// Threshold normalized to 15 significant decimal digits (half-away-from-zero).
        threshold_15sig: f64,
    },
    /// Homogeneous `Text` exact or inequality match (ASCII case-insensitive).
    Text {
        /// Equality polarity (`Exact` or `NotEq`).
        op: EqualityOp,
        /// Validated text target.
        value: String,
    },
    /// Homogeneous `Boolean` exact or inequality match.
    Boolean {
        /// Equality polarity (`Exact` or `NotEq`).
        op: EqualityOp,
        /// Boolean target.
        value: bool,
    },
}

impl TypedCriterion {
    /// Evaluates whether `cell` satisfies this typed criterion with 1-to-1 Excel `*IFS` parity.
    #[must_use]
    pub fn matches(&self, cell: &CellValue) -> bool {
        match self {
            Self::Blank(BlankCheck::Blank) => cell.is_empty(),
            Self::Blank(BlankCheck::NotBlank) => !cell.is_empty(),
            Self::NumberExact(target) => match cell {
                CellValue::Number(n) => {
                    n.is_finite()
                        && target.is_finite()
                        && normalize_zero(*n) == normalize_zero(*target)
                }
                CellValue::Text(_)
                | CellValue::Boolean(_)
                | CellValue::Empty
                | CellValue::Error(_) => false,
            },
            Self::NumberCmp {
                op,
                threshold_15sig,
            } => match cell {
                CellValue::Number(n) => {
                    if !n.is_finite() || !threshold_15sig.is_finite() {
                        return *op == CriteriaComparator::NotEq;
                    }
                    let ord = normalize_zero(*n).total_cmp(&normalize_zero(*threshold_15sig));
                    op.matches_ordering(ord)
                }
                CellValue::Text(_)
                | CellValue::Boolean(_)
                | CellValue::Empty
                | CellValue::Error(_) => match op {
                    CriteriaComparator::NotEq => true,
                    CriteriaComparator::Lt
                    | CriteriaComparator::Lte
                    | CriteriaComparator::Gt
                    | CriteriaComparator::Gte => false,
                },
            },
            Self::Text { op, value } => match cell {
                CellValue::Text(cell_s) => {
                    let is_eq =
                        compare_ascii_case_insensitive(cell_s.as_str(), value.as_str()).is_eq();
                    match op {
                        EqualityOp::Exact => is_eq,
                        EqualityOp::NotEq => !is_eq,
                    }
                }
                CellValue::Number(_)
                | CellValue::Boolean(_)
                | CellValue::Empty
                | CellValue::Error(_) => match op {
                    EqualityOp::Exact => false,
                    EqualityOp::NotEq => true,
                },
            },
            Self::Boolean { op, value } => match cell {
                CellValue::Boolean(cell_b) => match op {
                    EqualityOp::Exact => cell_b == value,
                    EqualityOp::NotEq => cell_b != value,
                },
                CellValue::Number(_)
                | CellValue::Text(_)
                | CellValue::Empty
                | CellValue::Error(_) => match op {
                    EqualityOp::Exact => false,
                    EqualityOp::NotEq => true,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{ExcelError, normalize_criteria_number};

    #[test]
    fn test_blank_and_not_blank_matching() {
        let blank = TypedCriterion::Blank(BlankCheck::Blank);
        assert!(blank.matches(&CellValue::Empty));
        assert!(!blank.matches(&CellValue::Text(String::new())));
        assert!(!blank.matches(&CellValue::Number(0.0)));
        assert!(!blank.matches(&CellValue::Boolean(false)));
        assert!(!blank.matches(&CellValue::Error(ExcelError::Div0)));

        let not_blank = TypedCriterion::Blank(BlankCheck::NotBlank);
        assert!(!not_blank.matches(&CellValue::Empty));
        assert!(not_blank.matches(&CellValue::Text(String::new())));
        assert!(not_blank.matches(&CellValue::Number(0.0)));
        assert!(not_blank.matches(&CellValue::Boolean(false)));
        assert!(not_blank.matches(&CellValue::Error(ExcelError::Na)));
    }

    #[test]
    fn test_number_exact_vs_15sig_comparator() {
        let one_third = 1.0_f64 / 3.0_f64;
        let exact = TypedCriterion::NumberExact(one_third);
        assert!(exact.matches(&CellValue::Number(one_third)));
        assert!(!exact.matches(&CellValue::Number(0.333333333333333)));
        assert!(!exact.matches(&CellValue::Empty));
        assert!(!exact.matches(&CellValue::Error(ExcelError::Div0)));

        let lte_cmp = TypedCriterion::NumberCmp {
            op: CriteriaComparator::Lte,
            threshold_15sig: normalize_criteria_number(one_third),
        };
        assert!(!lte_cmp.matches(&CellValue::Number(one_third)));
        assert!(lte_cmp.matches(&CellValue::Number(0.333333333333333)));
        assert!(!lte_cmp.matches(&CellValue::Empty));
        assert!(!lte_cmp.matches(&CellValue::Error(ExcelError::Div0)));

        let ne_cmp = TypedCriterion::NumberCmp {
            op: CriteriaComparator::NotEq,
            threshold_15sig: 20.0,
        };
        assert!(!ne_cmp.matches(&CellValue::Number(20.0)));
        assert!(ne_cmp.matches(&CellValue::Number(0.0)));
        assert!(ne_cmp.matches(&CellValue::Empty));
        assert!(ne_cmp.matches(&CellValue::Error(ExcelError::Div0)));
    }

    #[test]
    fn test_text_and_boolean_equality_and_inequality() {
        let txt_eq = TypedCriterion::Text {
            op: EqualityOp::Exact,
            value: "Alpha".into(),
        };
        assert!(txt_eq.matches(&CellValue::Text("alpha".into())));
        assert!(!txt_eq.matches(&CellValue::Text("Beta".into())));
        assert!(!txt_eq.matches(&CellValue::Empty));
        assert!(!txt_eq.matches(&CellValue::Error(ExcelError::Na)));

        let txt_ne = TypedCriterion::Text {
            op: EqualityOp::NotEq,
            value: "Alpha".into(),
        };
        assert!(!txt_ne.matches(&CellValue::Text("ALPHA".into())));
        assert!(txt_ne.matches(&CellValue::Text("Beta".into())));
        assert!(txt_ne.matches(&CellValue::Empty));
        assert!(txt_ne.matches(&CellValue::Error(ExcelError::Na)));

        let bool_ne = TypedCriterion::Boolean {
            op: EqualityOp::NotEq,
            value: true,
        };
        assert!(!bool_ne.matches(&CellValue::Boolean(true)));
        assert!(bool_ne.matches(&CellValue::Boolean(false)));
        assert!(bool_ne.matches(&CellValue::Empty));
        assert!(bool_ne.matches(&CellValue::Error(ExcelError::Value)));
    }
}
