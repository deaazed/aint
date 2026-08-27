//! The instruction set `compiler.rs` targets and `vm.rs` executes: a
//! flat, stack-based bytecode, one `Chunk` per function plus one for
//! top-level code. See `docs/milestones/22-bytecode-vm/SPEC.md`.

use aint_ast::{BinaryOp, UnaryOp};
use aint_runtime::NativeFunction;

/// One bytecode instruction. Indices (`usize`) are resolved at compile
/// time — constant-pool slots, local-variable slots, global slots,
/// function indices, and jump targets are all plain array offsets,
/// not names looked up at runtime. That's the actual "execution
/// speed" payoff over the tree-walker's `HashMap`-based
/// `Environment::get`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Instruction {
    /// Pushes `constants[idx]`.
    PushConst(usize),
    /// Pushes `stack[frame_base + idx]` — reads a local without
    /// removing it, since a local may be read more than once.
    GetLocal(usize),
    /// Pushes `globals[idx]`.
    GetGlobal(usize),
    /// Pops the top of the stack into `globals[idx]` — only ever
    /// emitted for a top-level `let`, never a function-body one
    /// (those just leave their value in place; see `compiler.rs`).
    SetGlobal(usize),
    /// Discards the top of the stack — a statement-position
    /// expression's unused result, or a block-scoped local going out
    /// of scope.
    Pop,
    Unary(UnaryOp),
    Binary(BinaryOp),
    /// Pops `count` values and pushes a single `Value::List` of them,
    /// in original left-to-right order.
    MakeList(usize),
    /// Pops an index then a list, pushes the element — bounds-checked
    /// exactly like the tree-walker's own indexing.
    Index,
    /// `print`'s one argument is already on the stack; writes it (plus
    /// a newline) to the VM's output and pushes `Value::Unit`. Kept as
    /// its own instruction rather than routed through `CallNative`
    /// since it needs the VM's writer, not just its arguments — the
    /// same reason `Interpreter::call` special-cases it.
    Print,
    /// Unconditional jump to an absolute instruction index within the
    /// current chunk.
    Jump(usize),
    /// Pops the condition; jumps to an absolute instruction index if
    /// it's `false`.
    JumpIfFalse(usize),
    /// Calls a user-defined function by its resolved index into
    /// `CompiledProgram::functions`, consuming `argc` values already
    /// pushed (which become the callee's initial locals).
    CallFn(usize, usize),
    /// Calls a stdlib native by the exact [`NativeFunction`] the
    /// tree-walking `Interpreter` would have resolved `import` to,
    /// consuming `argc` already-pushed arguments.
    CallNative(NativeFunction, usize),
    /// Pops the condition and checks it's `Value::Bool(true)`;
    /// `RuntimeError::AssertionFailed` otherwise. The type checker
    /// already guarantees the operand is `Bool`, exactly like the
    /// tree-walker's own `assert` handling.
    AssertTrue,
    /// Pops the return value, unwinds the current call frame back to
    /// its caller, and pushes the value there.
    Return,
}

/// One function's (or the top level's) compiled instructions —
/// nothing but a flat `Vec<Instruction>`. No separate constant pool
/// per chunk: constants are pooled once, program-wide, in
/// [`crate::compiler::CompiledProgram`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chunk {
    pub instructions: Vec<Instruction>,
}

impl Chunk {
    pub fn push(&mut self, instruction: Instruction) -> usize {
        self.instructions.push(instruction);
        self.instructions.len() - 1
    }
}
