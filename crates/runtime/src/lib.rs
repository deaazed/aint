//! AINT's execution engine.
//!
//! A plain tree-walk interpreter (milestone 04): values, environments,
//! functions, call frames. `infer` calls (milestone 08) run against a
//! `Model` trait so real model calls, local models, and deterministic
//! mocks are all interchangeable; tool execution follows in milestones
//! 11-13. See `docs/RUNTIME.md` and
//! `docs/milestones/04-tree-walk-interpreter/SPEC.md`.

mod environment;
mod error;
mod interpreter;
mod model;
mod stdlib;
mod value;

pub use error::RuntimeError;
pub use interpreter::Interpreter;
pub use model::{InferenceRequest, MockModel, Model};
pub use value::{Function, InferenceFn, NativeFunction, PendingInference, Value};
