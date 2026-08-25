//! Abstract syntax tree definitions for the AINT language.
//!
//! This crate has no compiler logic of its own. It exists so that the
//! lexer, parser, type checker, IR lowering, and runtime can all agree on
//! one representation of AINT programs without depending on each other.
//!
//! Populated starting at milestone 03 (parser + AST). See
//! `docs/ARCHITECTURE.md` for how this crate fits into the pipeline.
