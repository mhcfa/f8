//! Pass 2: Grid expansion and cell-level intermediate representation.
//!
//! Defines `CellId` and expands high-level table rows into discrete cell nodes
//! as documented in `docs/compiler/ir.md`.

pub mod cell;
pub mod error;
pub mod id;
pub mod symbol;

pub use cell::{CellGrid, CellNode, CellPayload};
pub use error::IrError;
pub use id::CellId;
pub use symbol::{SymbolTable, TableMetadata};
