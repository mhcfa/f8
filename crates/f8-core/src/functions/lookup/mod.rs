//! Lookup and reference functions suite (`XLOOKUP`, `XMATCH`, `INDEX`).

pub mod index;
pub mod search;
pub mod xlookup;
pub mod xmatch;

pub use index::IndexFunction;
pub use search::{MatchMode, SearchMode, find_match_index};
pub use xlookup::XlookupFunction;
pub use xmatch::XmatchFunction;

use crate::functions::registry::FunctionRegistry;
use std::sync::Arc;

/// Registers all standard lookup and reference functions into the registry.
pub fn register(registry: &mut FunctionRegistry) {
    registry.register(Arc::new(XlookupFunction));
    registry.register(Arc::new(XmatchFunction));
    registry.register(Arc::new(IndexFunction));
}
