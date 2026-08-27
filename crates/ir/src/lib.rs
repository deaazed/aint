//! AINT Intermediate Representation (AIR).
//!
//! Lowers a type-checked AST into an explicit form with first-class
//! `Infer`, `ToolCall`, `Distribution`, and `Probability` operations,
//! instead of treating them as ordinary function calls. This is the
//! prerequisite optimization (19) and the bytecode VM (22) both need —
//! it does not, itself, change how any AINT program executes today;
//! see `docs/milestones/18-compiler-ir/SPEC.md`.

mod air;
mod lower;

pub use air::{AirBlock, AirExpr, AirProgram, AirStmt, DistributionOp};
pub use lower::{lower, LowerError};
