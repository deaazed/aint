//! `AirProgram -> CompiledProgram`: resolves every name (local slot,
//! global slot, enum-variant constant, function index, or native) to
//! a plain array index at compile time, so the VM never does a
//! name-keyed lookup at runtime. See
//! `docs/milestones/22-bytecode-vm/SPEC.md` for the scope this covers
//! and what it deliberately doesn't.

use std::collections::HashMap;
use std::fmt;

use aint_ir::{AirExpr, AirProgram, AirStmt};
use aint_runtime::{stdlib, NativeFunction, Value};

use crate::bytecode::{Chunk, Instruction};

#[derive(Debug, Clone, PartialEq)]
pub enum CompileError {
    /// An AIR node this VM's deterministic-core subset doesn't cover —
    /// `infer`/`tool`/`await`/`Distribution<T>` operations, all of
    /// which need an async, `Model`/tool-aware dispatch loop the VM
    /// doesn't have. See SPEC.md's "Explicitly out of scope."
    Unsupported(String),
    /// A name that resolved to nothing — locals, enum-variant
    /// constants, globals-so-far, and functions were all checked.
    /// Shouldn't happen against a program the type checker already
    /// accepted; kept as a real error rather than a panic in case it
    /// ever does.
    UndefinedName(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::Unsupported(what) => write!(f, "the bytecode VM doesn't support {what}"),
            CompileError::UndefinedName(name) => {
                write!(f, "no local, global, or function named `{name}`")
            }
        }
    }
}

impl std::error::Error for CompileError {}

/// Everything the VM needs to run a program: one chunk for top-level
/// code, one per top-level function (indexed by
/// [`Instruction::CallFn`]'s first field), a shared constant pool, and
/// how many global slots to allocate.
#[derive(Debug, Clone, Default)]
pub struct CompiledProgram {
    pub top_level: Chunk,
    pub functions: Vec<Chunk>,
    pub constants: Vec<Value>,
    pub global_count: usize,
}

/// Which kind of chunk is currently being compiled — the only thing
/// that changes how a *root-level* `let` (not one nested inside an
/// `if`) is compiled: a global slot at the top level, a local
/// anywhere else. See the `Let` arm of `compile_stmt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkKind {
    TopLevel,
    Function,
}

/// Local-variable resolution for one chunk currently being compiled.
/// Locals live directly on the VM's value stack; a name's "slot" is
/// just its position in this flat list, which is kept in exact
/// lockstep with how many values are actually sitting on the stack
/// for this frame. Nested blocks (`if`/`else` bodies) push a marker on
/// `block_starts` and, on exit, truncate back to it — emitting one
/// `Pop` per local that goes out of scope, so the stack shape always
/// matches what `GetLocal` indices assume.
#[derive(Debug, Default)]
struct Locals {
    names: Vec<String>,
    block_starts: Vec<usize>,
}

impl Locals {
    fn depth(&self) -> usize {
        self.block_starts.len()
    }

    fn declare(&mut self, name: &str) {
        self.names.push(name.to_string());
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        self.names.iter().rposition(|n| n == name)
    }

    fn enter_block(&mut self) {
        self.block_starts.push(self.names.len());
    }

    /// Returns how many locals this block introduced, so the caller
    /// can emit that many `Pop`s.
    fn exit_block(&mut self) -> usize {
        let start = self.block_starts.pop().expect("matching enter_block");
        let introduced = self.names.len() - start;
        self.names.truncate(start);
        introduced
    }
}

pub fn compile(program: &AirProgram) -> Result<CompiledProgram, CompileError> {
    let mut compiler = Compiler::new();
    compiler.compile_program(program)
}

struct Compiler {
    constants: Vec<Value>,
    /// `"EnumName_Variant" -> constant pool index`, populated upfront
    /// from every `AirStmt::Enum` — see `docs/milestones/09-typed-
    /// structured-inference/SPEC.md` for why the identifier itself
    /// *is* the value's spelling. A pre-pass, not incremental, mirror-
    /// ing the type checker's own "enums are hoisted first" ordering.
    enum_constants: HashMap<String, usize>,
    /// `function name -> index into functions` — a pre-pass over every
    /// top-level `AirStmt::Fn`, mirroring the type checker's "hoist
    /// every top-level fn/infer signature before checking any body"
    /// pass, which is what makes forward references and mutual
    /// recursion between top-level functions resolvable at all.
    function_index: HashMap<String, usize>,
    functions: Vec<Chunk>,
    /// `top-level let name -> global slot`, grown incrementally while
    /// walking top-level statements in source order — deliberately
    /// *not* hoisted, matching the type checker's own behavior: a
    /// function body compiled before a later top-level `let` cannot
    /// see it, exactly like it can't type-check against it either.
    globals: HashMap<String, usize>,
    /// `native call name -> NativeFunction`, grown incrementally as
    /// `import` statements are walked — same reasoning as `globals`,
    /// and the same reasoning `Interpreter` itself has for only
    /// binding a module's names once its `import` actually executes.
    natives: HashMap<String, NativeFunction>,
}

impl Compiler {
    fn new() -> Self {
        Self {
            constants: Vec::new(),
            enum_constants: HashMap::new(),
            function_index: HashMap::new(),
            functions: Vec::new(),
            globals: HashMap::new(),
            natives: HashMap::new(),
        }
    }

    fn push_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    fn compile_program(&mut self, program: &AirProgram) -> Result<CompiledProgram, CompileError> {
        // Pass 1: enum variants, exactly like the type checker's own
        // enum pre-pass.
        for stmt in &program.statements {
            if let AirStmt::Enum { name, variants } = stmt {
                for variant in variants {
                    let idx = self.push_constant(Value::Enum(name.clone(), variant.clone()));
                    self.enum_constants.insert(format!("{name}_{variant}"), idx);
                }
            }
        }

        // Pass 2: function signatures (name -> index), exactly like
        // the type checker's own fn/infer/tool hoisting pass. Only
        // top-level `fn` gets a chunk here — `infer`/`tool` are
        // declarations with no body to compile (see AIR's own doc
        // comment on `AirStmt::Infer`/`Tool`), inert unless actually
        // called, which is rejected where the call itself compiles.
        for stmt in &program.statements {
            if let AirStmt::Fn { name, .. } = stmt {
                self.function_index
                    .insert(name.clone(), self.functions.len());
                self.functions.push(Chunk::default());
            }
        }

        // Pass 3: one top-to-bottom walk. Top-level statements compile
        // directly into the top-level chunk; each `AirStmt::Fn`'s body
        // compiles into its pre-reserved slot from pass 2, using
        // `self.globals`/`self.natives` exactly as they stand *at this
        // point* in the walk - see the field docs above for why that's
        // load-bearing, not incidental.
        let mut top_level = Chunk::default();
        let mut locals = Locals::default();
        for stmt in &program.statements {
            self.compile_stmt(stmt, ChunkKind::TopLevel, &mut top_level, &mut locals)?;
        }

        Ok(CompiledProgram {
            top_level,
            functions: self.functions.clone(),
            constants: self.constants.clone(),
            global_count: self.globals.len(),
        })
    }

    fn compile_block(
        &mut self,
        statements: &[AirStmt],
        kind: ChunkKind,
        chunk: &mut Chunk,
        locals: &mut Locals,
    ) -> Result<(), CompileError> {
        locals.enter_block();
        for stmt in statements {
            self.compile_stmt(stmt, kind, chunk, locals)?;
        }
        let introduced = locals.exit_block();
        for _ in 0..introduced {
            chunk.push(Instruction::Pop);
        }
        Ok(())
    }

    fn compile_stmt(
        &mut self,
        stmt: &AirStmt,
        kind: ChunkKind,
        chunk: &mut Chunk,
        locals: &mut Locals,
    ) -> Result<(), CompileError> {
        match stmt {
            AirStmt::Let { name, value } => {
                self.compile_expr(value, chunk, locals)?;
                if kind == ChunkKind::TopLevel && locals.depth() == 0 {
                    let slot = self.globals.len();
                    self.globals.insert(name.clone(), slot);
                    chunk.push(Instruction::SetGlobal(slot));
                } else {
                    locals.declare(name);
                }
                Ok(())
            }
            AirStmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.compile_expr(condition, chunk, locals)?;
                let jump_if_false = chunk.push(Instruction::JumpIfFalse(usize::MAX));
                self.compile_block(&then_branch.statements, kind, chunk, locals)?;

                if let Some(else_branch) = else_branch {
                    let jump_over_else = chunk.push(Instruction::Jump(usize::MAX));
                    let else_start = chunk.instructions.len();
                    chunk.instructions[jump_if_false] = Instruction::JumpIfFalse(else_start);
                    self.compile_block(&else_branch.statements, kind, chunk, locals)?;
                    let end = chunk.instructions.len();
                    chunk.instructions[jump_over_else] = Instruction::Jump(end);
                } else {
                    let end = chunk.instructions.len();
                    chunk.instructions[jump_if_false] = Instruction::JumpIfFalse(end);
                }
                Ok(())
            }
            AirStmt::Expr(expr) => {
                self.compile_expr(expr, chunk, locals)?;
                chunk.push(Instruction::Pop);
                Ok(())
            }
            AirStmt::Return(expr) => {
                self.compile_expr(expr, chunk, locals)?;
                chunk.push(Instruction::Return);
                Ok(())
            }
            AirStmt::Fn {
                name,
                params,
                body,
                is_async,
            } => {
                if *is_async {
                    return Err(CompileError::Unsupported(format!(
                        "`async fn` (`{name}`) - needs an async, suspend-and-resume VM dispatch loop this milestone doesn't build; see SPEC.md"
                    )));
                }
                let mut fn_locals = Locals::default();
                for param in params {
                    fn_locals.declare(param);
                }
                let mut fn_chunk = Chunk::default();
                self.compile_block(
                    &body.statements,
                    ChunkKind::Function,
                    &mut fn_chunk,
                    &mut fn_locals,
                )?;
                // Implicit `Unit` return for a body that falls through
                // without an explicit `return` - only reachable when
                // the type checker already proved that's fine (a
                // `Unit`-returning function need not return on every
                // path). Harmless, dead bytecode after an unconditional
                // `return` earlier in the body.
                let unit = self.push_constant(Value::Unit);
                fn_chunk.push(Instruction::PushConst(unit));
                fn_chunk.push(Instruction::Return);

                let index = *self
                    .function_index
                    .get(name)
                    .expect("every top-level fn was pre-registered in pass 2");
                self.functions[index] = fn_chunk;
                Ok(())
            }
            AirStmt::Import(module) => match stdlib::module_bindings(module) {
                Some(bindings) => {
                    for (name, native) in bindings {
                        self.natives.insert(name.to_string(), native);
                    }
                    Ok(())
                }
                None => Err(CompileError::UndefinedName(format!(
                    "unknown module `{module}`"
                ))),
            },
            AirStmt::Assert { condition } => {
                self.compile_expr(condition, chunk, locals)?;
                chunk.push(Instruction::AssertTrue);
                Ok(())
            }
            // Declarations with no body to compile - see the pass-2
            // comment above. A call to either is rejected in
            // `compile_expr`, not here.
            AirStmt::Infer { .. } | AirStmt::Tool { .. } => Ok(()),
            // Already consumed in pass 1.
            AirStmt::Enum { .. } => Ok(()),
            // Inert during `aint run` regardless of engine - the tree-
            // walking `Interpreter` skips `test` bodies entirely
            // outside the milestone-15 test runner, and `budget` only
            // ever matters at a model call, which this VM can't reach.
            // Matching that, not rejecting it, keeps `--vm` behaving
            // like `aint run` for the AI-feature statements it can't
            // otherwise execute.
            AirStmt::Test { .. } | AirStmt::Budget { .. } => Ok(()),
            AirStmt::Mock { .. } => Ok(()),
        }
    }

    fn compile_expr(
        &mut self,
        expr: &AirExpr,
        chunk: &mut Chunk,
        locals: &mut Locals,
    ) -> Result<(), CompileError> {
        match expr {
            AirExpr::Integer(n) => {
                let idx = self.push_constant(Value::Int(*n));
                chunk.push(Instruction::PushConst(idx));
                Ok(())
            }
            AirExpr::Float(n) => {
                let idx = self.push_constant(Value::Float(*n));
                chunk.push(Instruction::PushConst(idx));
                Ok(())
            }
            AirExpr::String(s) => {
                let idx = self.push_constant(Value::String(s.clone()));
                chunk.push(Instruction::PushConst(idx));
                Ok(())
            }
            AirExpr::Bool(b) => {
                let idx = self.push_constant(Value::Bool(*b));
                chunk.push(Instruction::PushConst(idx));
                Ok(())
            }
            AirExpr::Identifier(name) => {
                if let Some(slot) = locals.resolve(name) {
                    chunk.push(Instruction::GetLocal(slot));
                    return Ok(());
                }
                if let Some(idx) = self.enum_constants.get(name) {
                    chunk.push(Instruction::PushConst(*idx));
                    return Ok(());
                }
                if let Some(slot) = self.globals.get(name) {
                    chunk.push(Instruction::GetGlobal(*slot));
                    return Ok(());
                }
                Err(CompileError::UndefinedName(name.clone()))
            }
            AirExpr::Unary { op, operand } => {
                self.compile_expr(operand, chunk, locals)?;
                chunk.push(Instruction::Unary(*op));
                Ok(())
            }
            AirExpr::Binary { op, left, right } => {
                self.compile_expr(left, chunk, locals)?;
                self.compile_expr(right, chunk, locals)?;
                chunk.push(Instruction::Binary(*op));
                Ok(())
            }
            AirExpr::List(elements) => {
                for element in elements {
                    self.compile_expr(element, chunk, locals)?;
                }
                chunk.push(Instruction::MakeList(elements.len()));
                Ok(())
            }
            AirExpr::Index { object, index } => {
                self.compile_expr(object, chunk, locals)?;
                self.compile_expr(index, chunk, locals)?;
                chunk.push(Instruction::Index);
                Ok(())
            }
            AirExpr::Call { callee, args } => {
                for arg in args {
                    self.compile_expr(arg, chunk, locals)?;
                }
                if callee == "print" {
                    chunk.push(Instruction::Print);
                    return Ok(());
                }
                if let Some(index) = self.function_index.get(callee) {
                    chunk.push(Instruction::CallFn(*index, args.len()));
                    return Ok(());
                }
                if let Some(native) = self.natives.get(callee) {
                    chunk.push(Instruction::CallNative(*native, args.len()));
                    return Ok(());
                }
                Err(CompileError::UndefinedName(callee.clone()))
            }
            AirExpr::Await(_) => Err(CompileError::Unsupported(
                "`await` - needs an async, suspend-and-resume VM dispatch loop; see SPEC.md"
                    .to_string(),
            )),
            AirExpr::Infer { function, .. } => Err(CompileError::Unsupported(format!(
                "calling `infer {function}` - needs a `Model`-aware dispatch loop; see SPEC.md"
            ))),
            AirExpr::ToolCall { tool, .. } => Err(CompileError::Unsupported(format!(
                "calling `tool {tool}` - needs a tool-aware dispatch loop; see SPEC.md"
            ))),
            AirExpr::Distribution { .. } => Err(CompileError::Unsupported(
                "Distribution<T> operations - only ever produced by a validated `infer` \
                 response, which this VM doesn't support calling; see SPEC.md"
                    .to_string(),
            )),
            AirExpr::Probability { .. } => Err(CompileError::Unsupported(
                "distribution_probability - same reasoning as Distribution<T>; see SPEC.md"
                    .to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_source(src: &str) -> Result<CompiledProgram, CompileError> {
        let program = aint_parser::parse_source(src).expect("should parse");
        aint_typechecker::check_program(&program).expect("should type-check");
        let air = aint_ir::lower(&program).expect("should lower to AIR");
        compile(&air)
    }

    #[test]
    fn compiles_arithmetic_and_recursion() {
        assert!(compile_source(
            "fn fibonacci(n: Int) -> Int {\n\
                 if n < 2 { return n }\n\
                 return fibonacci(n - 1) + fibonacci(n - 2)\n\
             }\n\
             print(fibonacci(10))"
        )
        .is_ok());
    }

    #[test]
    fn rejects_await() {
        let err = compile_source(
            "fn f() -> Unit {\n\
                 let t = async_helper()\n\
                 await t\n\
             }\n\
             async fn async_helper() -> Unit {}",
        )
        .unwrap_err();
        assert!(matches!(err, CompileError::Unsupported(_)));
    }

    #[test]
    fn rejects_async_fn() {
        let err = compile_source("async fn f() -> Unit {}").unwrap_err();
        assert!(matches!(err, CompileError::Unsupported(_)));
    }

    #[test]
    fn rejects_calling_infer() {
        let err = compile_source(
            "infer classify(text: String) -> Bool\n\
             print(await classify(\"x\"))",
        )
        .unwrap_err();
        assert!(matches!(err, CompileError::Unsupported(_)));
    }

    #[test]
    fn rejects_calling_tool() {
        let err = compile_source(
            "tool database_get_email(id: String) -> String\n\
             print(await database_get_email(\"1\"))",
        )
        .unwrap_err();
        assert!(matches!(err, CompileError::Unsupported(_)));
    }

    #[test]
    fn declaring_infer_or_tool_without_calling_them_is_fine() {
        // The declaration itself is inert - only an actual call site
        // needs the async, Model-aware dispatch loop this VM doesn't
        // have. See the `AirStmt::Infer | AirStmt::Tool` arm.
        assert!(compile_source(
            "infer classify(text: String) -> Bool\n\
             tool database_get_email(id: String) -> String\n\
             print(1)"
        )
        .is_ok());
    }

    #[test]
    fn rejects_distribution_operations() {
        let err = compile_source(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             fn f(d: Distribution<Sentiment>) -> Bool {\n\
                 return distribution_argmax(d) == Sentiment_Positive\n\
             }\n\
             print(1)",
        )
        .unwrap_err();
        assert!(matches!(err, CompileError::Unsupported(_)));
    }

    #[test]
    fn test_blocks_and_budget_are_inert_no_ops_matching_aint_run() {
        assert!(compile_source(
            "budget { max_model_calls = 5 }\n\
             test \"unreachable via --vm\" {\n\
                 assert 1 == 1\n\
             }\n\
             print(1)"
        )
        .is_ok());
    }

    #[test]
    fn a_function_can_forward_reference_a_later_function() {
        assert!(compile_source(
            "fn is_even(n: Int) -> Bool {\n\
                 if n == 0 { return true }\n\
                 return is_odd(n - 1)\n\
             }\n\
             fn is_odd(n: Int) -> Bool {\n\
                 if n == 0 { return false }\n\
                 return is_even(n - 1)\n\
             }\n\
             print(is_even(10))"
        )
        .is_ok());
    }
}
