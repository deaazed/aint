//! AINT's execution engine.
//!
//! A plain tree-walk interpreter (milestone 04): values, environments,
//! functions, call frames. Inference and tool execution are added later
//! (milestones 08-13) behind a `Model` trait so real model calls, local
//! models, and deterministic mocks are all interchangeable. See
//! `docs/RUNTIME.md` and `docs/milestones/04-tree-walk-interpreter/SPEC.md`.

mod environment;
mod error;
mod interpreter;
mod value;

pub use error::RuntimeError;
pub use interpreter::Interpreter;
pub use value::{Function, NativeFunction, Value};
