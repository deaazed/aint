//! Abstract syntax tree definitions for the AINT language.
//!
//! This crate has no compiler logic of its own. It exists so that the
//! lexer, parser, type checker, IR lowering, and runtime can all agree on
//! one representation of AINT programs without depending on each other.
//!
//! The AST node types themselves are populated starting at milestone 03
//! (parser + AST). `Position`/`Span` land here first (milestone 02) since
//! the lexer needs them too. See `docs/ARCHITECTURE.md` for how this
//! crate fits into the pipeline.

mod span;

pub use span::{Position, Span};
