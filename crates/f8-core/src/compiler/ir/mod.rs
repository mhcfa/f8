//! Pass 2: Grid expansion and cell-level intermediate representation.
//!
//! Defines `CellId` and expands high-level table rows into discrete cell nodes.

pub mod cell;
pub mod error;
pub mod expand;
pub mod id;
pub mod symbol;

pub use cell::{CellGrid, CellNode, CellPayload};
pub use error::IrError;
pub use expand::{expand_grid, expand_table, expand_workbook};
pub use id::CellId;
pub use symbol::{SymbolTable, TableMetadata};
