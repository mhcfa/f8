use serde::{Deserialize, Serialize};
use std::fmt;

/// Standard semantic formatting presets supported by f8 models.
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
    Integer,
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
            Self::Integer => "integer",
        }
    }

    /// Returns true if this format preset supports fractional decimal precision (`decimals`).
    pub const fn supports_decimals(&self) -> bool {
        match self {
            Self::Currency
            | Self::Number
            | Self::Percentage
            | Self::Multiple
            | Self::BasisPoints => true,
            Self::Integer | Self::Date | Self::Text => false,
        }
    }
}

impl fmt::Display for FormatPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Structural presentation styles supported by f8 models.
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

/// Semantic background shade presets supported by f8 models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Shade {
    #[default]
    None,
    Muted,
    Accent,
    Alt,
}

impl Shade {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Muted => "muted",
            Self::Accent => "accent",
            Self::Alt => "alt",
        }
    }

    /// Returns true if this shade is `None`.
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl fmt::Display for Shade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Canonical 3-letter lowercase ISO 4217 currency codes supported by f8 models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CurrencyCode {
    #[default]
    Usd,
    Eur,
    Gbp,
    Jpy,
    Chf,
    Cad,
    Aud,
    Cny,
    Hkd,
    Sgd,
    Sek,
    Nok,
    Nzd,
    Inr,
    Brl,
}

impl CurrencyCode {
    /// Returns the canonical 3-letter lowercase code string.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Usd => "usd",
            Self::Eur => "eur",
            Self::Gbp => "gbp",
            Self::Jpy => "jpy",
            Self::Chf => "chf",
            Self::Cad => "cad",
            Self::Aud => "aud",
            Self::Cny => "cny",
            Self::Hkd => "hkd",
            Self::Sgd => "sgd",
            Self::Sek => "sek",
            Self::Nok => "nok",
            Self::Nzd => "nzd",
            Self::Inr => "inr",
            Self::Brl => "brl",
        }
    }

    /// Returns the standard currency symbol.
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Usd => "$",
            Self::Eur => "€",
            Self::Gbp => "£",
            Self::Jpy | Self::Cny => "¥",
            Self::Chf => "CHF",
            Self::Cad => "C$",
            Self::Aud => "A$",
            Self::Hkd => "HK$",
            Self::Sgd => "S$",
            Self::Sek | Self::Nok => "kr",
            Self::Nzd => "NZ$",
            Self::Inr => "₹",
            Self::Brl => "R$",
        }
    }

    /// Returns the Excel-safe mask symbol, quoting multi-letter and non-ASCII codes to prevent syntax collisions.
    pub const fn excel_symbol(&self) -> &'static str {
        match self {
            Self::Usd => "$",
            Self::Eur => "€",
            Self::Gbp => "£",
            Self::Jpy | Self::Cny => "¥",
            Self::Chf => "\"CHF\"",
            Self::Cad => "\"C$\"",
            Self::Aud => "\"A$\"",
            Self::Hkd => "\"HK$\"",
            Self::Sgd => "\"S$\"",
            Self::Sek | Self::Nok => "\"kr\"",
            Self::Nzd => "\"NZ$\"",
            Self::Inr => "\"₹\"",
            Self::Brl => "\"R$\"",
        }
    }
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
