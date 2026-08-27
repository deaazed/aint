use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write};
use std::rc::Rc;
use std::time::Duration;

use aint_ast::{BinaryOp, Block, Expr, ExprKind, Program, Span, Stmt, StmtKind, Type, UnaryOp};
use async_recursion::async_recursion;

use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::model::{InferenceRequest, MockModel, Model};
use crate::stdlib;
use crate::value::{Function, InferenceFn, NativeFunction, PendingInference, Task, Value};

/// Signals whether a `return` unwound out of the statement/block just
/// executed. Rust has no exceptions, so this is the tree-walk
/// interpreter's stand-in for one — `RuntimeError` stays reserved for
/// actual errors.
enum Flow {
    Normal,
    Return(Value),
}

/// Tree-walk interpreter over the AST from `aint-ast`.
///
/// Generic over the output writer so tests can capture what `print`
/// wrote (`Interpreter::with_output(Vec::new())` then `.into_output()`)
/// instead of asserting against real stdout. `Interpreter::new()` is
/// the `io::Stdout`-backed default the CLI uses.
///
/// Every evaluation method is `async fn`, driven by a Tokio runtime
/// (milestone 07) — `await` can appear anywhere an expression can, so
/// the whole call graph has to be async, not just the pieces that use
/// it. See `docs/milestones/07-async-concurrency/SPEC.md` for why this
/// is a single-threaded runtime with no `tokio::spawn`, and why the
/// mutually recursive methods below are `#[async_recursion(?Send)]`.
///
/// Generic over `M: Model` (milestone 08) so the interpreter can run
/// against a real model later without changing anything here — only
/// `MockModel` exists today, and it's the default, so every pre-08 call
/// site (`Interpreter::new()`, `Interpreter::with_output(...)`) keeps
/// compiling unchanged. See
/// `docs/milestones/08-first-ai-primitive/SPEC.md`.
pub struct Interpreter<W: Write = io::Stdout, M: Model = MockModel> {
    globals: Rc<RefCell<Environment>>,
    output: RefCell<W>,
    model: M,
    /// Every declared `enum`, by name, to its variant names — populated
    /// as `StmtKind::Enum` statements execute. Used to validate a
    /// model's response against the schema an `infer` call declared.
    /// See `docs/milestones/09-typed-structured-inference/SPEC.md`.
    enums: RefCell<HashMap<String, Vec<String>>>,
}

impl Interpreter<io::Stdout, MockModel> {
    pub fn new() -> Self {
        Self::with_output(io::stdout())
    }
}

impl Default for Interpreter<io::Stdout, MockModel> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Write> Interpreter<W, MockModel> {
    pub fn with_output(output: W) -> Self {
        Self::with_output_and_model(output, MockModel::new())
    }
}

impl<W: Write, M: Model> Interpreter<W, M> {
    pub fn with_output_and_model(output: W, model: M) -> Self {
        let globals = Environment::new();
        globals
            .borrow_mut()
            .define("print", Value::Native(NativeFunction::Print));
        Self {
            globals,
            output: RefCell::new(output),
            model,
            enums: RefCell::new(HashMap::new()),
        }
    }

    /// Consumes the interpreter, returning the underlying writer.
    pub fn into_output(self) -> W {
        self.output.into_inner()
    }

    pub async fn run(&self, program: &Program) -> Result<(), RuntimeError> {
        let env = Rc::clone(&self.globals);
        for stmt in &program.statements {
            match self.exec_stmt(stmt, &env).await? {
                Flow::Normal => {}
                Flow::Return(_) => {
                    return Err(RuntimeError::ReturnOutsideFunction { span: stmt.span });
                }
            }
        }
        Ok(())
    }

    #[async_recursion(?Send)]
    async fn exec_block(
        &self,
        block: &Block,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Flow, RuntimeError> {
        for stmt in &block.statements {
            match self.exec_stmt(stmt, env).await? {
                Flow::Normal => {}
                flow @ Flow::Return(_) => return Ok(flow),
            }
        }
        Ok(Flow::Normal)
    }

    #[async_recursion(?Send)]
    async fn exec_stmt(
        &self,
        stmt: &Stmt,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Flow, RuntimeError> {
        match &stmt.kind {
            StmtKind::Let { name, value } => {
                let v = self.eval_expr(value, env).await?;
                env.borrow_mut().define(name.clone(), v);
                Ok(Flow::Normal)
            }
            StmtKind::Fn {
                name,
                params,
                body,
                return_type: _,
                is_async,
            } => {
                // The type checker already validated this signature by
                // the time a real `aint run` gets here; the interpreter
                // only needs param names to bind argument values.
                let function = Value::Function(Rc::new(Function {
                    name: name.clone(),
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    body: body.clone(),
                    is_async: *is_async,
                }));
                env.borrow_mut().define(name.clone(), function);
                Ok(Flow::Normal)
            }
            StmtKind::Infer {
                name,
                params,
                return_type,
            } => {
                // Same reasoning as `StmtKind::Fn` above: the type
                // checker already validated this signature, so the
                // interpreter only needs param names — there's no body
                // at all, unlike `Fn`. `return_type` *is* needed now
                // (milestone 09), to validate the model's response
                // against it once awaited.
                let infer_fn = Value::InferenceFn(Rc::new(InferenceFn {
                    name: name.clone(),
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    return_type: return_type.clone(),
                }));
                env.borrow_mut().define(name.clone(), infer_fn);
                Ok(Flow::Normal)
            }
            StmtKind::Enum { name, variants } => {
                self.enums
                    .borrow_mut()
                    .insert(name.clone(), variants.clone());
                let mut env = env.borrow_mut();
                for variant in variants {
                    env.define(
                        format!("{name}_{variant}"),
                        Value::Enum(name.clone(), variant.clone()),
                    );
                }
                Ok(Flow::Normal)
            }
            StmtKind::Return(value) => {
                let v = self.eval_expr(value, env).await?;
                Ok(Flow::Return(v))
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.eval_expr(condition, env).await?;
                if expect_bool(&cond, condition.span)? {
                    let child = Environment::child(env);
                    self.exec_block(then_branch, &child).await
                } else if let Some(else_branch) = else_branch {
                    let child = Environment::child(env);
                    self.exec_block(else_branch, &child).await
                } else {
                    Ok(Flow::Normal)
                }
            }
            StmtKind::Expr(expr) => {
                self.eval_expr(expr, env).await?;
                Ok(Flow::Normal)
            }
            StmtKind::Import(module) => match stdlib::module_bindings(module) {
                Some(bindings) => {
                    let mut env = env.borrow_mut();
                    for (name, native) in bindings {
                        env.define(name, Value::Native(native));
                    }
                    Ok(Flow::Normal)
                }
                None => Err(RuntimeError::UnknownModule {
                    name: module.clone(),
                    span: stmt.span,
                }),
            },
        }
    }

    #[async_recursion(?Send)]
    async fn eval_expr(
        &self,
        expr: &Expr,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, RuntimeError> {
        match &expr.kind {
            ExprKind::Integer(n) => Ok(Value::Int(*n)),
            ExprKind::Float(n) => Ok(Value::Float(*n)),
            ExprKind::String(s) => Ok(Value::String(s.clone())),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::Identifier(name) => {
                env.borrow()
                    .get(name)
                    .ok_or_else(|| RuntimeError::UndefinedVariable {
                        name: name.clone(),
                        span: expr.span,
                    })
            }
            ExprKind::Unary { op, operand } => {
                let v = self.eval_expr(operand, env).await?;
                match op {
                    UnaryOp::Neg => match v {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        Value::Float(n) => Ok(Value::Float(-n)),
                        other => Err(RuntimeError::TypeMismatch {
                            message: format!("cannot negate a {}", other.type_name()),
                            span: expr.span,
                        }),
                    },
                }
            }
            ExprKind::Binary { op, left, right } => {
                let l = self.eval_expr(left, env).await?;
                let r = self.eval_expr(right, env).await?;
                eval_binary(*op, l, r, expr.span)
            }
            ExprKind::Call { callee, args } => {
                let callee_value = self.eval_expr(callee, env).await?;
                let mut arg_values = Vec::with_capacity(args.len());
                for arg in args {
                    arg_values.push(self.eval_expr(arg, env).await?);
                }
                self.call(callee_value, arg_values, expr.span).await
            }
            ExprKind::List(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(self.eval_expr(element, env).await?);
                }
                Ok(Value::List(values))
            }
            ExprKind::Index { object, index } => {
                let object_value = self.eval_expr(object, env).await?;
                let index_value = self.eval_expr(index, env).await?;

                let items = match object_value {
                    Value::List(items) => items,
                    other => {
                        return Err(RuntimeError::TypeMismatch {
                            message: format!("cannot index into a {}", other.type_name()),
                            span: expr.span,
                        });
                    }
                };
                let idx = match index_value {
                    Value::Int(n) => n,
                    other => {
                        return Err(RuntimeError::TypeMismatch {
                            message: format!(
                                "list index must be an Int, found {}",
                                other.type_name()
                            ),
                            span: expr.span,
                        });
                    }
                };

                if idx < 0 || idx as usize >= items.len() {
                    return Err(RuntimeError::IndexOutOfBounds {
                        index: idx,
                        len: items.len(),
                        span: expr.span,
                    });
                }
                Ok(items[idx as usize].clone())
            }
            ExprKind::Await(inner) => {
                let value = self.eval_expr(inner, env).await?;
                match value {
                    Value::Task(task) => self.eval_await(&task, expr.span).await,
                    Value::Inference(pending) => self.eval_inference(&pending, expr.span).await,
                    other => Err(RuntimeError::TypeMismatch {
                        message: format!("cannot await a {}", other.type_name()),
                        span: expr.span,
                    }),
                }
            }
        }
    }

    #[async_recursion(?Send)]
    async fn call(
        &self,
        callee: Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match callee {
            Value::Function(function) => {
                if function.params.len() != args.len() {
                    return Err(RuntimeError::ArityMismatch {
                        name: function.name.clone(),
                        expected: function.params.len(),
                        found: args.len(),
                        span,
                    });
                }
                if function.is_async {
                    // Deferred, not run: see Task's doc comment and
                    // SPEC.md. Nothing happens until this is awaited.
                    Ok(Value::Task(Rc::new(Task::Function { function, args })))
                } else {
                    self.run_function(&function, args).await
                }
            }
            Value::Native(NativeFunction::Print) => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        name: "print".to_string(),
                        expected: 1,
                        found: args.len(),
                        span,
                    });
                }
                writeln!(self.output.borrow_mut(), "{}", args[0]).map_err(|e| {
                    RuntimeError::Io {
                        message: e.to_string(),
                        span,
                    }
                })?;
                Ok(Value::Unit)
            }
            Value::Native(native) => {
                if native.is_async() {
                    Ok(Value::Task(Rc::new(Task::Native { native, args })))
                } else {
                    stdlib::call(native, args, span)
                }
            }
            Value::InferenceFn(infer_fn) => {
                if infer_fn.params.len() != args.len() {
                    return Err(RuntimeError::ArityMismatch {
                        name: infer_fn.name.clone(),
                        expected: infer_fn.params.len(),
                        found: args.len(),
                        span,
                    });
                }
                // Deferred, not run: exactly like `Value::Task` above —
                // nothing happens until this is awaited.
                Ok(Value::Inference(Rc::new(PendingInference {
                    function: infer_fn.name.clone(),
                    args,
                    return_type: infer_fn.return_type.clone(),
                })))
            }
            other => Err(RuntimeError::NotCallable {
                type_name: other.type_name(),
                span,
            }),
        }
    }

    /// Runs a function body to completion: parented to *globals*, not
    /// the caller's environment, since there's no real closure
    /// semantics yet (see SPEC.md). Shared by the sync-call path in
    /// [`Self::call`] and the await path in [`Self::eval_await`].
    #[async_recursion(?Send)]
    async fn run_function(
        &self,
        function: &Rc<Function>,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let call_env = Environment::child(&self.globals);
        for (param, value) in function.params.iter().zip(args) {
            call_env.borrow_mut().define(param.clone(), value);
        }
        match self.exec_block(&function.body, &call_env).await? {
            Flow::Return(v) => Ok(v),
            Flow::Normal => Ok(Value::Unit),
        }
    }

    /// Actually runs the deferred computation behind a `Value::Task` —
    /// the only place `await` has any effect.
    #[async_recursion(?Send)]
    async fn eval_await(&self, task: &Task, span: Span) -> Result<Value, RuntimeError> {
        match task {
            Task::Function { function, args } => self.run_function(function, args.clone()).await,
            Task::Native { native, args } => {
                self.run_async_native(*native, args.clone(), span).await
            }
        }
    }

    /// Actually runs the deferred computation behind a
    /// `Value::Inference` — sends it to this interpreter's `Model`. The
    /// only place `self.model` is ever touched.
    async fn eval_inference(
        &self,
        pending: &PendingInference,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let value = self
            .model
            .infer(InferenceRequest {
                function: pending.function.clone(),
                args: pending.args.clone(),
                return_type: pending.return_type.clone(),
                span,
            })
            .await?;
        self.validate_inference_result(&value, &pending.return_type, span)?;
        Ok(value)
    }

    /// Checks a model's response against an `infer` call's declared
    /// return type before it becomes a usable AINT value — the
    /// "validating the response against the schema" half of milestone
    /// 09. Only `Enum` return types are checked: a `Bool`/`Int`/etc.
    /// mismatch already gets caught wherever the value is next used
    /// (`if` requires a real `Bool`, and so on), but nothing else would
    /// ever catch a model returning an unlisted enum variant — it would
    /// just compare unequal to every real one, silently. See SPEC.md.
    fn validate_inference_result(
        &self,
        value: &Value,
        expected: &Type,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match expected {
            Type::Enum(expected_name) => self.validate_enum_result(value, expected_name, span),
            Type::Distribution(inner) => match inner.as_ref() {
                Type::Enum(expected_name) => {
                    self.validate_distribution_result(value, expected_name, span)
                }
                // The type checker restricts `Distribution<T>` to enum
                // `T` (see SPEC.md); a program that reaches here without
                // having gone through it (e.g. a runtime-only test)
                // gets a clear error instead of a panic.
                _ => Err(RuntimeError::SchemaViolation {
                    message: "Distribution<T> requires T to be an enum".to_string(),
                    span,
                }),
            },
            _ => Ok(()),
        }
    }

    /// The variants a declared enum actually has, or a `SchemaViolation`
    /// if nothing registered that name — reachable if an `infer`
    /// function names an enum that either doesn't exist or was never
    /// executed (both normally caught earlier by the type checker, but
    /// this interpreter doesn't assume it always ran first; see
    /// `docs/milestones/10-uncertainty/SPEC.md`).
    fn known_variants(&self, enum_name: &str, span: Span) -> Result<Vec<String>, RuntimeError> {
        self.enums
            .borrow()
            .get(enum_name)
            .cloned()
            .ok_or_else(|| RuntimeError::SchemaViolation {
                message: format!("no enum named `{enum_name}` is declared"),
                span,
            })
    }

    fn validate_enum_result(
        &self,
        value: &Value,
        expected_name: &str,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match value {
            Value::Enum(name, variant) if name == expected_name => {
                let variants = self.known_variants(expected_name, span)?;
                if variants.contains(variant) {
                    Ok(())
                } else {
                    Err(RuntimeError::SchemaViolation {
                        message: format!(
                            "model returned `{variant}`, which is not a variant of `{expected_name}`"
                        ),
                        span,
                    })
                }
            }
            Value::Enum(name, _) => Err(RuntimeError::SchemaViolation {
                message: format!(
                    "model returned a value of enum `{name}`, expected `{expected_name}`"
                ),
                span,
            }),
            other => Err(RuntimeError::SchemaViolation {
                message: format!(
                    "model returned a {}, expected the enum `{expected_name}`",
                    other.type_name()
                ),
                span,
            }),
        }
    }

    /// Checks a `Distribution<Enum>` response: right enum, every listed
    /// variant real, every probability in `[0.0, 1.0]`, and the whole
    /// distribution summing to `1.0` within `1e-6`. This is the
    /// entirety of what AINT validates about "probability" — see
    /// `docs/milestones/10-uncertainty/SPEC.md`'s explicit decision on
    /// what it deliberately does *not* claim to validate.
    fn validate_distribution_result(
        &self,
        value: &Value,
        expected_name: &str,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let (name, entries) = match value {
            Value::Distribution(name, entries) if name == expected_name => (name, entries),
            Value::Distribution(name, _) => {
                return Err(RuntimeError::SchemaViolation {
                    message: format!(
                        "model returned a distribution over `{name}`, expected `{expected_name}`"
                    ),
                    span,
                });
            }
            other => {
                return Err(RuntimeError::SchemaViolation {
                    message: format!(
                        "model returned a {}, expected a Distribution over `{expected_name}`",
                        other.type_name()
                    ),
                    span,
                });
            }
        };

        let variants = self.known_variants(name, span)?;
        for (variant, probability) in entries {
            if !variants.contains(variant) {
                return Err(RuntimeError::SchemaViolation {
                    message: format!(
                        "distribution lists `{variant}`, which is not a variant of `{expected_name}`"
                    ),
                    span,
                });
            }
            if !(0.0..=1.0).contains(probability) {
                return Err(RuntimeError::SchemaViolation {
                    message: format!(
                        "distribution assigns `{variant}` an invalid probability {probability}"
                    ),
                    span,
                });
            }
        }

        let total: f64 = entries.iter().map(|(_, p)| p).sum();
        if (total - 1.0).abs() > 1e-6 {
            return Err(RuntimeError::SchemaViolation {
                message: format!("distribution's probabilities sum to {total}, expected 1.0"),
                span,
            });
        }

        Ok(())
    }

    async fn run_async_native(
        &self,
        native: NativeFunction,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match native {
            NativeFunction::TimeSleepMs => {
                let [ms_value] = stdlib::one(native, args, span)?;
                let ms = stdlib::int(ms_value, span)?;
                tokio::time::sleep(Duration::from_millis(ms.max(0) as u64)).await;
                Ok(Value::Unit)
            }
            _ => unreachable!("only async natives should ever reach eval_await"),
        }
    }
}

fn expect_bool(value: &Value, span: Span) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(b) => Ok(*b),
        other => Err(RuntimeError::TypeMismatch {
            message: format!("expected Bool, found {}", other.type_name()),
            span,
        }),
    }
}

fn eval_binary(op: BinaryOp, left: Value, right: Value, span: Span) -> Result<Value, RuntimeError> {
    match op {
        BinaryOp::Add => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l + r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l + r)),
            (l, r) => Err(type_mismatch("+", &l, &r, span)),
        },
        BinaryOp::Sub => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l - r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l - r)),
            (l, r) => Err(type_mismatch("-", &l, &r, span)),
        },
        BinaryOp::Mul => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l * r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l * r)),
            (l, r) => Err(type_mismatch("*", &l, &r, span)),
        },
        BinaryOp::Div => match (left, right) {
            (Value::Int(_), Value::Int(0)) => Err(RuntimeError::DivisionByZero { span }),
            (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l / r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l / r)),
            (l, r) => Err(type_mismatch("/", &l, &r, span)),
        },
        BinaryOp::Less => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l < r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l < r)),
            (l, r) => Err(type_mismatch("<", &l, &r, span)),
        },
        BinaryOp::Greater => match (left, right) {
            (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l > r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l > r)),
            (l, r) => Err(type_mismatch(">", &l, &r, span)),
        },
        BinaryOp::Eq => Ok(Value::Bool(left == right)),
        BinaryOp::NotEq => Ok(Value::Bool(left != right)),
    }
}

fn type_mismatch(op: &str, left: &Value, right: &Value, span: Span) -> RuntimeError {
    RuntimeError::TypeMismatch {
        message: format!(
            "cannot apply `{op}` to {} and {}",
            left.type_name(),
            right.type_name()
        ),
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bridges the now-async `Interpreter::run` into these otherwise
    /// unchanged sync `#[test]` bodies, so the 24 tests below stay
    /// exactly as they were before milestone 07 — only this helper (and
    /// its sibling `run_expect_err`) had to change. See SPEC.md.
    /// Runs `src` on a dedicated thread with a large stack, not just a
    /// throwaway current-thread runtime on the test-harness thread.
    /// Deep AINT recursion (the language's only iteration mechanism —
    /// see SPEC.md) needs far more Rust stack once every eval step is
    /// async; the default thread stack overflows well before anything
    /// resembling a real program does, as `showcase.an`'s Collatz(27)
    /// (111 levels) found the hard way. `Interpreter` holds `Rc`, so it
    /// can't be built outside and moved in — everything happens inside
    /// the closure, which only captures `Send` data (the source text).
    fn run_on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("failed to spawn a big-stack thread")
            .join()
            .expect("the big-stack thread panicked")
    }

    fn run_capturing(src: &'static str) -> String {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(src).expect("should parse");
            let interpreter = Interpreter::with_output(Vec::new());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect("should run without error");
            String::from_utf8(interpreter.into_output()).expect("output should be valid utf8")
        })
    }

    fn run_expect_err(src: &'static str) -> RuntimeError {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(src).expect("should parse");
            let interpreter = Interpreter::with_output(Vec::new());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        })
    }

    #[test]
    fn let_and_arithmetic() {
        assert_eq!(run_capturing("let x = 1 + 2 * 3\nprint(x)"), "7\n");
    }

    #[test]
    fn if_without_else_when_false_does_nothing() {
        assert_eq!(run_capturing("if false { print(1) }"), "");
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
    fn function_call_and_recursion() {
        assert_eq!(
            run_capturing(
                "fn fibonacci(n: Int) -> Int {\n\
                     if n < 2 { return n }\n\
                     return fibonacci(n - 1) + fibonacci(n - 2)\n\
                 }\n\
                 print(fibonacci(10))"
            ),
            "55\n"
        );
    }

    #[test]
    fn function_with_no_return_yields_unit_and_does_not_print_it() {
        assert_eq!(run_capturing("fn noop() -> Unit { let x = 1 }\nnoop()"), "");
    }

    #[test]
    fn equality_across_different_types_is_false() {
        assert_eq!(run_capturing("print(1 == \"1\")"), "false\n");
    }

    #[test]
    fn errors_on_undefined_variable() {
        let err = run_expect_err("print(missing)");
        assert!(matches!(err, RuntimeError::UndefinedVariable { .. }));
    }

    #[test]
    fn errors_on_not_callable() {
        let err = run_expect_err("let x = 1\nx()");
        assert!(matches!(err, RuntimeError::NotCallable { .. }));
    }

    #[test]
    fn errors_on_arity_mismatch() {
        let err = run_expect_err("fn add(a: Int, b: Int) -> Int { return a + b }\nadd(1)");
        assert!(matches!(err, RuntimeError::ArityMismatch { .. }));
    }

    #[test]
    fn errors_on_type_mismatch() {
        let err = run_expect_err("print(1 + \"x\")");
        assert!(matches!(err, RuntimeError::TypeMismatch { .. }));
    }

    #[test]
    fn errors_on_integer_division_by_zero() {
        let err = run_expect_err("print(1 / 0)");
        assert!(matches!(err, RuntimeError::DivisionByZero { .. }));
    }

    #[test]
    fn errors_on_return_outside_function() {
        let err = run_expect_err("return 1");
        assert!(matches!(err, RuntimeError::ReturnOutsideFunction { .. }));
    }

    #[test]
    fn print_wrong_arity_is_arity_mismatch() {
        let err = run_expect_err("print(1, 2)");
        assert!(matches!(err, RuntimeError::ArityMismatch { .. }));
    }

    // --- lists and indexing --------------------------------------------

    #[test]
    fn list_literal_and_indexing() {
        assert_eq!(run_capturing("print([10, 20, 30][1])"), "20\n");
    }

    #[test]
    fn list_display_format() {
        assert_eq!(run_capturing("print([1, 2, 3])"), "[1, 2, 3]\n");
    }

    #[test]
    fn errors_on_negative_index() {
        let err = run_expect_err("print([1, 2, 3][-1])");
        assert!(matches!(
            err,
            RuntimeError::IndexOutOfBounds {
                index: -1,
                len: 3,
                ..
            }
        ));
    }

    #[test]
    fn errors_on_index_past_the_end() {
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
    fn recursive_list_processing_via_index_and_length() {
        assert_eq!(
            run_capturing(
                "import collections\n\
                 fn sum(xs: List<Float>, i: Int) -> Float {\n\
                     if i < collections_length(xs) {\n\
                         return xs[i] + sum(xs, i + 1)\n\
                     } else {\n\
                         return 0.0\n\
                     }\n\
                 }\n\
                 print(sum([1.0, 2.0, 3.0], 0))"
            ),
            "6\n"
        );
    }

    // --- import / stdlib ------------------------------------------------

    #[test]
    fn errors_on_unknown_module() {
        let err = run_expect_err("import frobnicate");
        assert!(matches!(err, RuntimeError::UnknownModule { .. }));
    }

    #[test]
    fn math_functions() {
        assert_eq!(
            run_capturing(
                "import math\n\
                 print(math_sqrt(9.0))\n\
                 print(math_pow(2.0, 10.0))\n\
                 print(math_floor(1.9))\n\
                 print(math_ceil(1.1))\n\
                 print(math_round(1.5))\n\
                 print(math_abs(-3.5))\n\
                 print(math_min(2.0, 5.0))\n\
                 print(math_max(2.0, 5.0))"
            ),
            "3\n1024\n1\n2\n2\n3.5\n2\n5\n"
        );
    }

    #[test]
    fn string_functions() {
        assert_eq!(
            run_capturing(
                "import string\n\
                 print(string_length(\"hello\"))\n\
                 print(string_to_upper(\"hello\"))\n\
                 print(string_to_lower(\"HELLO\"))\n\
                 print(string_trim(\"  hi  \"))\n\
                 print(string_contains(\"hello\", \"ell\"))\n\
                 print(string_concat(\"foo\", \"bar\"))"
            ),
            "5\nHELLO\nhello\nhi\ntrue\nfoobar\n"
        );
    }

    #[test]
    fn time_now_seconds_returns_a_plausible_timestamp() {
        assert_eq!(
            run_capturing("import time\nprint(time_now_seconds() > 1700000000)"),
            "true\n"
        );
    }

    #[test]
    fn collections_length_works_for_any_element_type() {
        assert_eq!(
            run_capturing(
                "import collections\n\
                 print(collections_length([1, 2, 3]))\n\
                 print(collections_length([\"a\", \"b\"]))"
            ),
            "3\n2\n"
        );
    }

    // --- async / await ---------------------------------------------------

    #[test]
    fn calling_an_async_fn_without_await_does_not_run_its_body() {
        // If this ever actually executed, dividing by zero would error
        // the whole program. It shouldn't - nothing awaits it.
        assert_eq!(
            run_capturing(
                "async fn boom() -> Int { return 1 / 0 }\n\
                 let _pending = boom()\n\
                 print(1)"
            ),
            "1\n"
        );
    }

    #[test]
    fn awaiting_an_async_fn_runs_its_body_and_returns_the_value() {
        assert_eq!(
            run_capturing(
                "async fn double(n: Int) -> Int { return n * 2 }\n\
                 print(await double(21))"
            ),
            "42\n"
        );
    }

    #[test]
    fn nested_async_calls_compose() {
        assert_eq!(
            run_capturing(
                "async fn inner(n: Int) -> Int { return n + 1 }\n\
                 async fn outer(n: Int) -> Int { return await inner(n) * 2 }\n\
                 print(await outer(5))"
            ),
            "12\n"
        );
    }

    #[test]
    fn sync_and_async_functions_interoperate() {
        assert_eq!(
            run_capturing(
                "fn double(n: Int) -> Int { return n * 2 }\n\
                 async fn triple(n: Int) -> Int { return n * 3 }\n\
                 print(double(5) + await triple(5))"
            ),
            "25\n"
        );
    }

    #[test]
    fn errors_on_awaiting_a_non_task() {
        let err = run_expect_err("await 1");
        assert!(matches!(err, RuntimeError::TypeMismatch { .. }));
    }

    #[test]
    fn time_sleep_ms_actually_suspends() {
        // The one test that proves this milestone did something real:
        // a genuine suspend/resume through Tokio, not synchronous code
        // dressed up in async syntax.
        let program = aint_parser::parse_source("import time\nawait time_sleep_ms(30)")
            .expect("should parse");
        let interpreter = Interpreter::with_output(Vec::new());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build a test runtime");

        let start = std::time::Instant::now();
        runtime
            .block_on(interpreter.run(&program))
            .expect("should run without error");
        let elapsed = start.elapsed();

        assert!(
            elapsed >= std::time::Duration::from_millis(25),
            "expected at least ~30ms to have actually elapsed from a real sleep, got {elapsed:?}"
        );
    }

    // --- infer / Model ----------------------------------------------

    /// Takes a *builder* for the model, not the model itself: a
    /// `MockModel` holding a mocked `Value` isn't `Send` any more than
    /// `Interpreter` is (`Value` holds `Rc` throughout), so it has to be
    /// constructed inside the big-stack closure too, not captured from
    /// outside it.
    fn run_capturing_with_model(
        src: &'static str,
        build_model: impl FnOnce() -> crate::model::MockModel + Send + 'static,
    ) -> String {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(src).expect("should parse");
            let interpreter = Interpreter::with_output_and_model(Vec::new(), build_model());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect("should run without error");
            String::from_utf8(interpreter.into_output()).expect("output should be valid utf8")
        })
    }

    #[test]
    fn calling_an_infer_fn_without_await_does_not_touch_the_model() {
        // An unconfigured MockModel would error if this ran. It
        // shouldn't - nothing awaits it, same as an unawaited Task.
        assert_eq!(
            run_capturing_with_model(
                "infer is_positive(text: String) -> Bool\n\
                 let _pending = is_positive(\"x\")\n\
                 print(1)",
                crate::model::MockModel::new,
            ),
            "1\n"
        );
    }

    #[test]
    fn awaiting_an_infer_call_returns_the_mocked_value() {
        assert_eq!(
            run_capturing_with_model(
                "infer is_positive(text: String) -> Bool\n\
                 print(await is_positive(\"great product\"))",
                || crate::model::MockModel::new().mock("is_positive", Value::Bool(true)),
            ),
            "true\n"
        );
    }

    #[test]
    fn awaiting_an_unconfigured_infer_call_is_a_clear_model_error() {
        // `Interpreter` holds `Rc` and isn't `Send` - built inside the
        // closure, same rule as every other big-stack test here.
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "infer is_positive(text: String) -> Bool\n\
                 await is_positive(\"x\")",
            )
            .expect("should parse");
            let interpreter =
                Interpreter::with_output_and_model(Vec::new(), crate::model::MockModel::new());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }

    #[test]
    fn default_interpreter_uses_an_empty_mock_model() {
        // `Interpreter::with_output` (the pre-08 constructor, still used
        // everywhere else in this file) keeps compiling and behaving
        // exactly as before - it just happens to default to a MockModel
        // with nothing configured.
        let err = run_expect_err(
            "infer greeting() -> String\n\
             await greeting()",
        );
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }

    // --- enum / schema validation ------------------------------------

    #[test]
    fn enum_variants_construct_and_compare() {
        assert_eq!(
            run_capturing(
                "enum Sentiment { Positive Neutral Negative }\n\
                 print(Sentiment_Positive == Sentiment_Positive)\n\
                 print(Sentiment_Positive == Sentiment_Negative)\n\
                 print(Sentiment_Positive)"
            ),
            "true\nfalse\nPositive\n"
        );
    }

    #[test]
    fn enum_value_flows_through_a_function() {
        assert_eq!(
            run_capturing(
                "enum Sentiment { Positive Neutral Negative }\n\
                 fn describe(s: Sentiment) -> Sentiment { return s }\n\
                 print(describe(Sentiment_Positive) == Sentiment_Positive)"
            ),
            "true\n"
        );
    }

    #[test]
    fn infer_returning_an_enum_with_a_valid_mocked_variant_succeeds() {
        assert_eq!(
            run_capturing_with_model(
                "enum Sentiment { Positive Neutral Negative }\n\
                 infer sentiment(text: String) -> Sentiment\n\
                 print(await sentiment(\"great\") == Sentiment_Positive)",
                || {
                    crate::model::MockModel::new().mock(
                        "sentiment",
                        Value::Enum("Sentiment".to_string(), "Positive".to_string()),
                    )
                },
            ),
            "true\n"
        );
    }

    #[test]
    fn infer_returning_a_hallucinated_variant_is_a_schema_violation() {
        // The model answered - it just invented a variant that was
        // never declared. This must not silently compare `false`
        // against every real variant; it has to be a loud, specific
        // error.
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "enum Sentiment { Positive Neutral Negative }\n\
                 infer sentiment(text: String) -> Sentiment\n\
                 print(await sentiment(\"great\"))",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().mock(
                "sentiment",
                Value::Enum("Sentiment".to_string(), "Ecstatic".to_string()),
            );
            let interpreter = Interpreter::with_output_and_model(Vec::new(), model);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::SchemaViolation { .. }));
    }

    #[test]
    fn infer_returning_the_wrong_enum_entirely_is_a_schema_violation() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "enum Sentiment { Positive Neutral Negative }\n\
                 enum Direction { North South }\n\
                 infer sentiment(text: String) -> Sentiment\n\
                 print(await sentiment(\"great\"))",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().mock(
                "sentiment",
                Value::Enum("Direction".to_string(), "North".to_string()),
            );
            let interpreter = Interpreter::with_output_and_model(Vec::new(), model);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::SchemaViolation { .. }));
    }

    // --- Distribution<T> / Option<T> -----------------------------------

    fn sentiment_distribution(entries: Vec<(&str, f64)>) -> Value {
        Value::Distribution(
            "Sentiment".to_string(),
            entries
                .into_iter()
                .map(|(v, p)| (v.to_string(), p))
                .collect(),
        )
    }

    #[test]
    fn distribution_argmax_probability_and_entropy() {
        assert_eq!(
            run_capturing_with_model(
                "enum Sentiment { Positive Neutral Negative }\n\
                 import distribution\n\
                 infer classify(text: String) -> Distribution<Sentiment>\n\
                 let d = await classify(\"great\")\n\
                 print(distribution_argmax(d) == Sentiment_Positive)\n\
                 print(distribution_probability(d, Sentiment_Positive))\n\
                 print(distribution_probability(d, Sentiment_Negative))\n\
                 print(distribution_entropy(d) > 0.0)",
                || {
                    crate::model::MockModel::new().mock(
                        "classify",
                        sentiment_distribution(vec![
                            ("Positive", 0.7),
                            ("Neutral", 0.2),
                            ("Negative", 0.1),
                        ]),
                    )
                },
            ),
            "true\n0.7\n0.1\ntrue\n"
        );
    }

    #[test]
    fn distribution_sample_is_deterministic_for_a_degenerate_distribution() {
        assert_eq!(
            run_capturing_with_model(
                "enum Sentiment { Positive Neutral Negative }\n\
                 import distribution\n\
                 infer classify(text: String) -> Distribution<Sentiment>\n\
                 let d = await classify(\"great\")\n\
                 print(distribution_sample(d) == Sentiment_Positive)",
                || {
                    crate::model::MockModel::new().mock(
                        "classify",
                        sentiment_distribution(vec![
                            ("Positive", 1.0),
                            ("Neutral", 0.0),
                            ("Negative", 0.0),
                        ]),
                    )
                },
            ),
            "true\n"
        );
    }

    #[test]
    fn require_confidence_above_threshold_is_some_of_the_argmax() {
        assert_eq!(
            run_capturing_with_model(
                "enum Sentiment { Positive Neutral Negative }\n\
                 import distribution\n\
                 import option\n\
                 infer classify(text: String) -> Distribution<Sentiment>\n\
                 let d = await classify(\"great\")\n\
                 let result = distribution_require_confidence(d, 0.5)\n\
                 print(option_is_some(result))\n\
                 print(option_unwrap(result) == Sentiment_Positive)",
                || {
                    crate::model::MockModel::new().mock(
                        "classify",
                        sentiment_distribution(vec![
                            ("Positive", 0.7),
                            ("Neutral", 0.2),
                            ("Negative", 0.1),
                        ]),
                    )
                },
            ),
            "true\ntrue\n"
        );
    }

    #[test]
    fn require_confidence_below_threshold_is_none() {
        assert_eq!(
            run_capturing_with_model(
                "enum Sentiment { Positive Neutral Negative }\n\
                 import distribution\n\
                 import option\n\
                 infer classify(text: String) -> Distribution<Sentiment>\n\
                 let d = await classify(\"meh\")\n\
                 print(option_is_some(distribution_require_confidence(d, 0.9)))",
                || {
                    crate::model::MockModel::new().mock(
                        "classify",
                        sentiment_distribution(vec![
                            ("Positive", 0.7),
                            ("Neutral", 0.2),
                            ("Negative", 0.1),
                        ]),
                    )
                },
            ),
            "false\n"
        );
    }

    #[test]
    fn option_unwrap_on_none_is_a_clear_error_not_a_panic() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "enum Sentiment { Positive Neutral Negative }\n\
                 import distribution\n\
                 import option\n\
                 infer classify(text: String) -> Distribution<Sentiment>\n\
                 let d = await classify(\"meh\")\n\
                 print(option_unwrap(distribution_require_confidence(d, 0.99)))",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().mock(
                "classify",
                sentiment_distribution(vec![
                    ("Positive", 0.7),
                    ("Neutral", 0.2),
                    ("Negative", 0.1),
                ]),
            );
            let interpreter = Interpreter::with_output_and_model(Vec::new(), model);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::TypeMismatch { .. }));
    }

    #[test]
    fn distribution_with_an_unlisted_variant_is_a_schema_violation() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "enum Sentiment { Positive Neutral Negative }\n\
                 infer classify(text: String) -> Distribution<Sentiment>\n\
                 await classify(\"x\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new()
                .mock("classify", sentiment_distribution(vec![("Ecstatic", 1.0)]));
            let interpreter = Interpreter::with_output_and_model(Vec::new(), model);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::SchemaViolation { .. }));
    }

    #[test]
    fn distribution_not_summing_to_one_is_a_schema_violation() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "enum Sentiment { Positive Neutral Negative }\n\
                 infer classify(text: String) -> Distribution<Sentiment>\n\
                 await classify(\"x\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().mock(
                "classify",
                sentiment_distribution(vec![("Positive", 0.5), ("Neutral", 0.2)]),
            );
            let interpreter = Interpreter::with_output_and_model(Vec::new(), model);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::SchemaViolation { .. }));
    }

    #[test]
    fn distribution_over_the_wrong_enum_is_a_schema_violation() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "enum Sentiment { Positive Neutral Negative }\n\
                 enum Direction { North South }\n\
                 infer classify(text: String) -> Distribution<Sentiment>\n\
                 await classify(\"x\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().mock(
                "classify",
                Value::Distribution("Direction".to_string(), vec![("North".to_string(), 1.0)]),
            );
            let interpreter = Interpreter::with_output_and_model(Vec::new(), model);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::SchemaViolation { .. }));
    }
}
