//! Pass 6b: OpenXML workbook and headless data emission.
//!
//! Writes native `.xlsx` files via `rust_xlsxwriter` and headless data exports
//! as documented in `docs/compiler/lowering-and-emission.md`.

pub mod error;
pub mod format;

pub use error::EmissionError;
pub use format::{CellFormatKey, FormatCache, parse_hex_color, translate_border_style};
