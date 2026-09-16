//! Compilation engine and sequential pass orchestration.
//!
//! Orchestrates the multi-pass compilation pipeline transforming high-level
//! model specifications into evaluated models and compiled `.xlsx` workbooks.

pub mod emission;
pub mod eval;
pub mod graph;
pub mod ir;
pub mod layout;
pub mod lowering;
