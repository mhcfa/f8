use serde::{Deserialize, Serialize};
use std::fmt;

/// Standard semantic formatting presets supported by Finx models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatPreset {
    Currency,
    CurrencyTotal,
    Percentage,
    Multiple,
    Integer,
    Decimal,
    BasisPoints,
    Date,
    Text,
}

impl FormatPreset {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Currency => "currency",
            Self::CurrencyTotal => "currency_total",
            Self::Percentage => "percentage",
            Self::Multiple => "multiple",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::BasisPoints => "basis_points",
            Self::Date => "date",
            Self::Text => "text",
        }
    }
}

impl fmt::Display for FormatPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
