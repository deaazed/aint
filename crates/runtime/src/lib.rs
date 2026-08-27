//! AINT's execution engine.
//!
//! A plain tree-walk interpreter (milestone 04): values, environments,
//! functions, call frames. `infer` calls (milestone 08) run against a
//! `Model` trait so real model calls, local models, and deterministic
//! mocks are all interchangeable; `tool` calls (milestone 11) run
//! against `MockTool` the same way. See `docs/RUNTIME.md` and
//! `docs/milestones/04-tree-walk-interpreter/SPEC.md`.

mod environment;
mod error;
mod interpreter;
mod model;
mod stdlib;
mod tool;
mod value;

pub use error::RuntimeError;
pub use interpreter::Interpreter;
pub use model::{InferenceRequest, MockModel, Model};
pub use tool::{MockTool, ToolRequest};
pub use value::{
    Function, InferenceFn, NativeFunction, PendingInference, PendingToolCall, ToolFn, Value,
};
