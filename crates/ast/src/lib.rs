//! Abstract syntax tree definitions for the AINT language.
//!
//! This crate has no compiler logic of its own. It exists so that the
//! lexer, parser, type checker, IR lowering, and runtime can all agree on
//! one representation of AINT programs without depending on each other.
//!
//! `Expr`/`Stmt` are deliberately `{ kind, span }` wrappers around a
//! `*Kind` enum (mirroring `aint_lexer::Token`), so later milestones can
//! add AI-specific variants (`infer`, `tool`, `Distribution<T>`, ...)
//! without restructuring what's already here. See
//! `docs/ARCHITECTURE.md` for how this crate fits into the pipeline.

mod expr;
mod span;
mod stmt;
mod ty;

pub use expr::{BinaryOp, Expr, ExprKind, UnaryOp};
pub use span::{Position, Span};
pub use stmt::{Block, Param, Program, Stmt, StmtKind};
pub use ty::Type;
