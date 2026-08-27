//! `AST -> AIR -> Bytecode -> AINT VM` (milestone 22): a stack-based
//! bytecode compiler and executor for AINT's deterministic core -
//! arithmetic, `let`, `if`/`else`, function calls and recursion,
//! lists, indexing, enums, `assert`, and stdlib natives. Consumes
//! `aint_ir::AirProgram` (milestone 18), which nothing executed
//! before this. `infer`/`tool`/`await`/`Distribution<T>` are
//! explicitly out of scope - see
//! `docs/milestones/22-bytecode-vm/SPEC.md` for exactly why and what
//! would change that.
//!
//! This is a second, opt-in execution engine, not a replacement for
//! `aint_runtime::Interpreter` - `aint run` still uses the tree-walker
//! by default; `aint run --vm` uses this instead, and fails clearly at
//! compile time (not silently or by panicking) on anything outside
//! its scope.

mod bytecode;
mod compiler;
mod vm;

pub use bytecode::{Chunk, Instruction};
pub use compiler::{compile, CompileError, CompiledProgram};
pub use vm::Vm;
