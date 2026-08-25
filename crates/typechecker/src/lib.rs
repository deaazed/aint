//! Static type checker for AINT.
//!
//! Responsible for ordinary type checking (milestone 05) and, later, for
//! enforcing the uncertainty boundary: a `Distribution<T>` can never
//! silently collapse into a `T` (milestone 09-10). See
//! `docs/LANGUAGE_DESIGN.md` for why that boundary is load-bearing, and
//! `docs/milestones/05-core-type-system/SPEC.md` for this milestone's
//! exact scope.

mod checker;
mod error;
mod stdlib;

use aint_ast::Program;

pub use checker::TypeChecker;
pub use error::TypeError;

/// Type-checks `program` in full, stopping at the first error.
pub fn check_program(program: &Program) -> Result<(), TypeError> {
    TypeChecker::new().check(program)
}
