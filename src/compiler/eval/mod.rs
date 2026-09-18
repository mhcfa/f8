//! Pass 4: In-memory topological evaluator and evaluated matrix.
//!
//! Executes formula expressions with Excel parity and topological execution
//! over the cell dependency DAG as documented in `docs/compiler/evaluator.md`
//! and `docs/compiler/parity.md`.

pub mod context;
pub mod error;
pub mod expr;

pub use context::CellEvalContext;
pub use error::EvalError;
pub use expr::eval_expr;
