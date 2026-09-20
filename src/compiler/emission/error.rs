//! Diagnostic errors occurring during OpenXML emission and format synthesis.

use thiserror::Error;

use crate::model::error::ModelError;

/// Diagnostic errors occurring during OpenXML styling, format synthesis, and workbook emission.
#[derive(Debug, Error)]
pub enum EmissionError {
    /// Failure during xlsx generation or format configuration.
    #[error("Excel emission error: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),

    /// Invalid hex color string encountered during theme translation.
    #[error(
        "Invalid hex color for '{field}': '{color}'. Expected format '#RGB', '#RRGGBB', or '#RRGGBBAA'."
    )]
    InvalidColor {
        /// Name of the theme palette field.
        field: String,
        /// Invalid color string value.
        color: String,
    },

    /// Theme or number format resolution error.
    #[error("Theme resolution error: {0}")]
    Model(#[from] ModelError),
}
