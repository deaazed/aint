//! AINT's execution engine.
//!
//! Starts as a plain tree-walk interpreter (milestone 04): values,
//! environments, functions, call frames. Inference and tool execution
//! are added later (milestones 08-13) behind a `Model` trait so real
//! model calls, local models, and deterministic mocks are all
//! interchangeable. See `docs/RUNTIME.md`.
//!
//! Not implemented yet.
