use serde::{Deserialize, Serialize};
use std::fmt;

/// Standard semantic formatting presets supported by Finx models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatPreset {
    Currency,
    Number,
    Percentage,
    Multiple,
    BasisPoints,
    Date,
    Text,
}

impl FormatPreset {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Currency => "currency",
            Self::Number => "number",
            Self::Percentage => "percentage",
            Self::Multiple => "multiple",
            Self::BasisPoints => "basis_points",
            Self::Date => "date",
            Self::Text => "text",
        }
    }

    /// Returns true if this format preset represents numeric data.
    pub const fn is_numeric(&self) -> bool {
        match self {
            Self::Currency
            | Self::Number
            | Self::Percentage
            | Self::Multiple
            | Self::BasisPoints => true,
            Self::Date | Self::Text => false,
        }
    }
}

impl fmt::Display for FormatPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Structural presentation styles supported by Finx models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStyle {
    #[default]
    Data,
    Subtotal,
    Total,
}

impl SemanticStyle {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Data => "data",
            Self::Subtotal => "subtotal",
            Self::Total => "total",
        }
    }

    /// Returns true if this is standard data style.
    pub fn is_data(&self) -> bool {
        matches!(self, Self::Data)
    }

    /// Returns true if this is subtotal summary style.
    pub fn is_subtotal(&self) -> bool {
        matches!(self, Self::Subtotal)
    }

    /// Returns true if this is total summary style.
    pub fn is_total(&self) -> bool {
        matches!(self, Self::Total)
    }
}

impl fmt::Display for SemanticStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
