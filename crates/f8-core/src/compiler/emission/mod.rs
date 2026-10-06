//! Pass 6b: OpenXML workbook and headless data emission.
//!
//! Writes native `.xlsx` files via `rust_xlsxwriter` and headless data exports.

pub mod error;
pub mod format;
pub mod workbook;

pub use error::EmissionError;
pub use format::{
    CellFormatKey, FormatCache, parse_fill_color, parse_hex_color, translate_border_style,
};
pub use workbook::{EmissionOptions, WorkbookEmitter};
