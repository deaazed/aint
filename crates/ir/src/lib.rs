//! AINT Intermediate Representation (AIR).
//!
//! Lowers the typed AST into an explicit form with first-class `Infer`,
//! `ToolCall`, `Distribution`, and `Probability` operations, instead of
//! treating them as ordinary function calls. This is what lets the
//! runtime cache, parallelize, and route inference deliberately instead
//! of by accident. Introduced starting at milestone 18; not needed
//! before the tree-walk interpreter proves the language semantics.
//!
//! Not implemented yet.
