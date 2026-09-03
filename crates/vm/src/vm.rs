//! The bytecode executor: an explicit, heap-allocated call-frame stack
//! and a flat value stack, dispatching one [`Instruction`] at a time.
//! Deliberately iterative, not a recursive Rust function per AINT
//! call — that's what lets AINT-level recursion depth scale with heap
//! memory instead of the tree-walking `Interpreter`'s OS thread stack
//! (see `docs/milestones/07-async-concurrency/SPEC.md`'s 64 MiB
//! dedicated-thread workaround, which this sidesteps by construction
//! rather than needing a bigger version of the same trick). See
//! `docs/milestones/22-bytecode-vm/SPEC.md`.

use std::io::Write;

use aint_ast::{BinaryOp, Position, Span, UnaryOp};
use aint_runtime::{stdlib, RuntimeError, Value};

use crate::bytecode::{Chunk, Instruction};
use crate::compiler::CompiledProgram;

/// AIR carries no source positions (nothing needed them before this
/// milestone gave AIR its first real executor) — every VM-produced
/// `RuntimeError` uses this in place of a real one. A stated, honest
/// gap, not a bug: see SPEC.md's "Known limitation."
fn placeholder_span() -> Span {
    Span::new(Position::start(), Position::start())
}

struct Frame<'a> {
    chunk: &'a Chunk,
    ip: usize,
    base: usize,
}

/// Executes a [`CompiledProgram`] to completion, writing anything
/// `print`ed to `output` - the same generic-writer shape
/// `aint_runtime::Interpreter` uses, for the same reason (tests can
/// capture it instead of going through real stdout).
pub struct Vm<W: Write> {
    output: W,
}

impl<W: Write> Vm<W> {
    pub fn new(output: W) -> Self {
        Self { output }
    }

    pub fn into_output(self) -> W {
        self.output
    }

    pub fn run(&mut self, program: &CompiledProgram) -> Result<(), RuntimeError> {
        let mut globals: Vec<Value> = vec![Value::Unit; program.global_count];
        let mut stack: Vec<Value> = Vec::new();
        let mut frames: Vec<Frame> = vec![Frame {
            chunk: &program.top_level,
            ip: 0,
            base: 0,
        }];

        loop {
            let instr = {
                let frame = frames.last_mut().expect("at least one frame");
                if frame.ip >= frame.chunk.instructions.len() {
                    // Only the top-level chunk is allowed to run off
                    // its own end - every function chunk always ends
                    // in `Return` (the compiler appends one
                    // unconditionally). Reaching this with more than
                    // one frame would mean a compiler bug, not a
                    // program error.
                    debug_assert_eq!(
                        frames.len(),
                        1,
                        "a function chunk fell off its end without returning"
                    );
                    return Ok(());
                }
                let instr = frame.chunk.instructions[frame.ip];
                frame.ip += 1;
                instr
            };

            match instr {
                Instruction::PushConst(idx) => stack.push(program.constants[idx].clone()),
                Instruction::GetLocal(slot) => {
                    let base = frames.last().expect("at least one frame").base;
                    stack.push(stack[base + slot].clone());
                }
                Instruction::GetGlobal(slot) => stack.push(globals[slot].clone()),
                Instruction::SetGlobal(slot) => {
                    let value = stack.pop().expect("value for SetGlobal");
                    globals[slot] = value;
                }
                Instruction::Pop => {
                    stack.pop().expect("value to pop");
                }
                Instruction::Unary(op) => {
                    let operand = stack.pop().expect("operand");
                    stack.push(eval_unary(op, operand)?);
                }
                Instruction::Binary(op) => {
                    let right = stack.pop().expect("right operand");
                    let left = stack.pop().expect("left operand");
                    stack.push(eval_binary(op, left, right)?);
                }
                Instruction::MakeList(count) => {
                    let start = stack.len() - count;
                    let items: Vec<Value> = stack.drain(start..).collect();
                    stack.push(Value::List(items));
                }
                Instruction::Index => {
                    let index = stack.pop().expect("index");
                    let object = stack.pop().expect("indexed object");
                    stack.push(eval_index(object, index)?);
                }
                Instruction::Print => {
                    let value = stack.pop().expect("print's argument");
                    writeln!(self.output, "{value}").map_err(|e| RuntimeError::Io {
                        message: e.to_string(),
                        span: placeholder_span(),
                    })?;
                    stack.push(Value::Unit);
                }
                Instruction::Jump(target) => {
                    frames.last_mut().expect("at least one frame").ip = target;
                }
                Instruction::JumpIfFalse(target) => {
                    let condition = stack.pop().expect("condition");
                    let take = match condition {
                        Value::Bool(b) => !b,
                        other => {
                            return Err(RuntimeError::TypeMismatch {
                                message: format!("expected Bool, found {}", other.type_name()),
                                span: placeholder_span(),
                            })
                        }
                    };
                    if take {
                        frames.last_mut().expect("at least one frame").ip = target;
                    }
                }
                Instruction::CallFn(index, argc) => {
                    let base = stack.len() - argc;
                    frames.push(Frame {
                        chunk: &program.functions[index],
                        ip: 0,
                        base,
                    });
                }
                Instruction::CallNative(native, argc) => {
                    let start = stack.len() - argc;
                    let args: Vec<Value> = stack.drain(start..).collect();
                    let result = stdlib::call(native, args, placeholder_span())?;
                    stack.push(result);
                }
                Instruction::AssertTrue => {
                    let condition = stack.pop().expect("assert's condition");
                    match condition {
                        Value::Bool(true) => {}
                        Value::Bool(false) => {
                            return Err(RuntimeError::AssertionFailed {
                                span: placeholder_span(),
                            })
                        }
                        other => {
                            return Err(RuntimeError::TypeMismatch {
                                message: format!("expected Bool, found {}", other.type_name()),
                                span: placeholder_span(),
                            })
                        }
                    }
                }
                Instruction::Return => {
                    let value = stack.pop().expect("return value");
                    let finished = frames.pop().expect("a frame to return from");
                    stack.truncate(finished.base);
                    stack.push(value);
                    if frames.is_empty() {
                        return Ok(());
                    }
                }
            }
        }
    }
}

fn eval_unary(op: UnaryOp, operand: Value) -> Result<Value, RuntimeError> {
    match op {
        UnaryOp::Neg => match operand {
            Value::Int(n) => Ok(Value::Int(-n)),
            Value::Float(n) => Ok(Value::Float(-n)),
            other => Err(RuntimeError::TypeMismatch {
                message: format!("cannot negate a {}", other.type_name()),
                span: placeholder_span(),
            }),
        },
        UnaryOp::Not => match operand {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            other => Err(RuntimeError::TypeMismatch {
                message: format!("cannot apply `!` to a {}", other.type_name()),
                span: placeholder_span(),
            }),
        },
    }
}

/// Mirrors `aint_runtime::Interpreter`'s own (private) `eval_binary`
/// exactly - same operator set, same per-operator type rules, same
/// error shape - for every operator that actually reaches here.
/// `&&`/`||` don't: `aint-ir` rejects them at lowering
/// (`LowerError::UnsupportedShortCircuit`, milestone 38) rather than
/// compiling them as an eager, non-short-circuiting evaluation, which
/// would silently disagree with `aint run`'s real semantics. Duplicated
/// rather than shared across the crate boundary: a handful of match
/// arms over a small, fixed operator set, the same scale of
/// duplication `aint-ir`'s `lower.rs` already accepted against the
/// type checker's internals rather than exposing them. See SPEC.md.
fn eval_binary(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    let span = placeholder_span();
    match op {
        BinaryOp::Add => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l + r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l + r)),
            (l, r) => Err(binary_type_mismatch("+", &l, &r)),
        },
        BinaryOp::Sub => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l - r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l - r)),
            (l, r) => Err(binary_type_mismatch("-", &l, &r)),
        },
        BinaryOp::Mul => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l * r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l * r)),
            (l, r) => Err(binary_type_mismatch("*", &l, &r)),
        },
        BinaryOp::Div => match (left, right) {
            (Value::Int(_), Value::Int(0)) => Err(RuntimeError::DivisionByZero { span }),
            (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l / r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l / r)),
            (l, r) => Err(binary_type_mismatch("/", &l, &r)),
        },
        BinaryOp::Less => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l < r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l < r)),
            (l, r) => Err(binary_type_mismatch("<", &l, &r)),
        },
        BinaryOp::Greater => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l > r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l > r)),
            (l, r) => Err(binary_type_mismatch(">", &l, &r)),
        },
        BinaryOp::LessEq => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l <= r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l <= r)),
            (l, r) => Err(binary_type_mismatch("<=", &l, &r)),
        },
        BinaryOp::GreaterEq => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l >= r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l >= r)),
            (l, r) => Err(binary_type_mismatch(">=", &l, &r)),
        },
        BinaryOp::Eq => Ok(Value::Bool(left == right)),
        BinaryOp::NotEq => Ok(Value::Bool(left != right)),
        BinaryOp::And | BinaryOp::Or => unreachable!(
            "aint-ir rejects And/Or at lowering (UnsupportedShortCircuit) - never reaches AIR"
        ),
    }
}

fn binary_type_mismatch(op: &str, left: &Value, right: &Value) -> RuntimeError {
    RuntimeError::TypeMismatch {
        message: format!(
            "cannot apply `{op}` to {} and {}",
            left.type_name(),
            right.type_name()
        ),
        span: placeholder_span(),
    }
}

fn eval_index(object: Value, index: Value) -> Result<Value, RuntimeError> {
    let items = match object {
        Value::List(items) => items,
        other => {
            return Err(RuntimeError::TypeMismatch {
                message: format!("cannot index a {}", other.type_name()),
                span: placeholder_span(),
            })
        }
    };
    let idx = match index {
        Value::Int(n) => n,
        other => {
            return Err(RuntimeError::TypeMismatch {
                message: format!("cannot index with a {}", other.type_name()),
                span: placeholder_span(),
            })
        }
    };
    if idx < 0 || idx as usize >= items.len() {
        return Err(RuntimeError::IndexOutOfBounds {
            index: idx,
            len: items.len(),
            span: placeholder_span(),
        });
    }
    Ok(items[idx as usize].clone())
}

#[cfg(test)]
mod tests {
    use crate::compiler::compile;

    fn run_capturing(src: &str) -> String {
        let program = aint_parser::parse_source(src).expect("should parse");
        aint_typechecker::check_program(&program).expect("should type-check");
        let air = aint_ir::lower(&program).expect("should lower to AIR");
        let compiled = compile(&air).expect("should compile");
        let mut vm = super::Vm::new(Vec::new());
        vm.run(&compiled).expect("should run without error");
        String::from_utf8(vm.into_output()).expect("valid utf8")
    }

    fn run_expect_err(src: &str) -> RuntimeError {
        let program = aint_parser::parse_source(src).expect("should parse");
        aint_typechecker::check_program(&program).expect("should type-check");
        let air = aint_ir::lower(&program).expect("should lower to AIR");
        let compiled = compile(&air).expect("should compile");
        let mut vm = super::Vm::new(Vec::new());
        vm.run(&compiled)
            .expect_err("should produce a runtime error")
    }

    use super::*;

    #[test]
    fn let_and_arithmetic() {
        assert_eq!(run_capturing("let x = 1 + 2 * 3\nprint(x)"), "7\n");
    }

    #[test]
    fn if_else_picks_the_right_branch() {
        assert_eq!(
            run_capturing("if 1 > 2 { print(\"a\") } else { print(\"b\") }"),
            "b\n"
        );
    }

    #[test]
    fn let_inside_if_does_not_leak_out() {
        let output = run_capturing("let x = 1\nif true { let x = 2 print(x) }\nprint(x)");
        assert_eq!(output, "2\n1\n");
    }

    #[test]
    fn list_literal_and_indexing() {
        assert_eq!(run_capturing("print([10, 20, 30][1])"), "20\n");
    }

    #[test]
    fn recursive_function_with_many_locals_and_a_global() {
        assert_eq!(
            run_capturing(
                "let base = 100\n\
                 fn add_base(n: Int) -> Int {\n\
                     let doubled = n * 2\n\
                     return doubled + base\n\
                 }\n\
                 print(add_base(5))"
            ),
            "110\n"
        );
    }

    #[test]
    fn errors_on_integer_division_by_zero() {
        let err = run_expect_err("print(1 / 0)");
        assert!(matches!(err, RuntimeError::DivisionByZero { .. }));
    }

    #[test]
    fn errors_on_index_out_of_bounds() {
        let err = run_expect_err("print([1, 2, 3][3])");
        assert!(matches!(
            err,
            RuntimeError::IndexOutOfBounds {
                index: 3,
                len: 3,
                ..
            }
        ));
    }

    #[test]
    fn errors_on_type_mismatch() {
        // Deliberately skips type-checking: a real `aint run --vm`
        // always type-checks first (matching `aint run`'s existing
        // gate), so `Int + String` is normally rejected long before
        // reaching the VM. This proves `eval_binary`'s own defensive
        // check is correct in case the VM is ever driven directly
        // (as this test does) - the same reason
        // `aint-runtime::Interpreter`'s equivalent test bypasses the
        // type checker too.
        let program = aint_parser::parse_source("print(1 + \"x\")").expect("should parse");
        let air = aint_ir::lower(&program).expect("should lower to AIR");
        let compiled = compile(&air).expect("should compile");
        let mut vm = super::Vm::new(Vec::new());
        let err = vm
            .run(&compiled)
            .expect_err("should produce a runtime error");
        assert!(matches!(err, RuntimeError::TypeMismatch { .. }));
    }

    #[test]
    fn a_failing_top_level_assert_is_an_assertion_error() {
        let err = run_expect_err("assert 1 == 2");
        assert!(matches!(err, RuntimeError::AssertionFailed { .. }));
    }

    #[test]
    fn a_passing_top_level_assert_is_silent() {
        assert_eq!(run_capturing("assert 1 == 1\nprint(\"ok\")"), "ok\n");
    }

    #[test]
    fn deep_recursion_does_not_need_a_bigger_rust_stack() {
        // 5000 levels of AINT recursion would overflow the default
        // Rust thread stack in the tree-walking interpreter (which is
        // why `aint-runtime`'s own tests run on a dedicated 64 MiB
        // thread). The VM's frames live on the heap, so this runs on
        // whatever stack `cargo test` gives this thread, unmodified.
        assert_eq!(
            run_capturing(
                "fn count_down(n: Int) -> Int {\n\
                     if n == 0 { return 0 }\n\
                     return count_down(n - 1)\n\
                 }\n\
                 print(count_down(5000))"
            ),
            "0\n"
        );
    }
}
