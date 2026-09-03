use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::{self, Write};
use std::rc::Rc;
use std::time::{Duration, Instant};

use aint_ast::{BinaryOp, Block, Expr, ExprKind, Program, Span, Stmt, StmtKind, Type, UnaryOp};
use async_recursion::async_recursion;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::model::{InferenceOutcome, InferenceRequest, MockModel, Model};
use crate::stdlib;
use crate::tool::{MockTool, ToolExchange, ToolRequest, ToolSignature};
use crate::trace::{InferenceTraceOutcome, TokenUsage, TraceRecord};
use crate::value::{
    Function, InferenceFn, NativeFunction, PendingInference, PendingToolCall, Task, ToolBody,
    ToolFn, Value,
};

/// Signals whether a `return` unwound out of the statement/block just
/// executed. Rust has no exceptions, so this is the tree-walk
/// interpreter's stand-in for one — `RuntimeError` stays reserved for
/// actual errors.
enum Flow {
    Normal,
    Return(Value),
}

/// A program's resource ceiling, from at most one `budget` block. Every
/// field is optional — `None` means unlimited on that axis. See
/// `docs/milestones/17-ai-resource-management/SPEC.md` for which of
/// these are honestly enforceable today (`max_model_calls`,
/// `timeout_ms`) versus tracked-but-currently-vacuous
/// (`max_tokens`/`max_cost`, since every call reports zero tokens).
#[derive(Debug, Clone, Copy, Default)]
struct Budget {
    max_tokens: Option<i64>,
    max_model_calls: Option<i64>,
    max_cost: Option<f64>,
    timeout_ms: Option<i64>,
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
    /// Backs every `tool` call — concrete, not generic, unlike `model`;
    /// see `docs/milestones/11-typed-tools/SPEC.md` for why tools don't
    /// need the same swappable-implementation treatment `Model` does.
    tools: MockTool,
    /// Every declared `enum`, by name, to its variant names — populated
    /// as `StmtKind::Enum` statements execute. Used to validate a
    /// model's response against the schema an `infer` call declared.
    /// See `docs/milestones/09-typed-structured-inference/SPEC.md`.
    enums: RefCell<HashMap<String, Vec<String>>>,
    /// Every declared `tool`, by name, to its signature — populated as
    /// `StmtKind::Tool` statements execute. Lets a model-requested tool
    /// call be validated and looked up by an arbitrary runtime string,
    /// same shape as `enums`. See
    /// `docs/milestones/12-ai-tool-calling/SPEC.md`.
    tools_registry: RefCell<HashMap<String, Rc<ToolFn>>>,
    /// Every `infer`/`tool` call captured so far — unconditional, no
    /// opt-in. See `docs/milestones/14-ai-execution-tracing/SPEC.md`.
    traces: RefCell<Vec<TraceRecord>>,
    next_inference_id: Cell<u64>,
    next_tool_call_id: Cell<u64>,
    /// Set once, when a `budget` statement executes. `None` (the
    /// default) means no enforcement at all — opt-in, the same shape
    /// as `effects` (milestone 13). See
    /// `docs/milestones/17-ai-resource-management/SPEC.md`.
    budget: Cell<Option<Budget>>,
    total_model_calls: Cell<i64>,
    total_tokens: Cell<i64>,
    total_cost: Cell<f64>,
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
        Self::with_output_model_and_tools(output, model, MockTool::new())
    }

    pub fn with_output_model_and_tools(output: W, model: M, tools: MockTool) -> Self {
        let globals = Environment::new();
        globals
            .borrow_mut()
            .define("print", Value::Native(NativeFunction::Print));
        Self {
            globals,
            output: RefCell::new(output),
            model,
            tools,
            enums: RefCell::new(HashMap::new()),
            tools_registry: RefCell::new(HashMap::new()),
            traces: RefCell::new(Vec::new()),
            next_inference_id: Cell::new(1),
            next_tool_call_id: Cell::new(1),
            budget: Cell::new(None),
            total_model_calls: Cell::new(0),
            total_tokens: Cell::new(0),
            total_cost: Cell::new(0.0),
        }
    }

    /// Consumes the interpreter, returning the underlying writer.
    pub fn into_output(self) -> W {
        self.output.into_inner()
    }

    /// Every `Inference #N` / `Tool Call #N` record captured so far,
    /// in the order the calls actually happened. See
    /// `docs/milestones/14-ai-execution-tracing/SPEC.md`.
    pub fn traces(&self) -> Vec<TraceRecord> {
        self.traces.borrow().clone()
    }

    pub async fn run(&self, program: &Program) -> Result<(), RuntimeError> {
        self.run_statements(&program.statements).await
    }

    /// Runs a sequence of top-level statements against this
    /// interpreter's globals — what `run` does for a whole `Program`,
    /// generalized to a slice so the milestone-15 test runner can run
    /// "every declaration, then just this one test block's body" as
    /// two separate calls against the same interpreter.
    pub async fn run_statements(&self, statements: &[Stmt]) -> Result<(), RuntimeError> {
        let env = Rc::clone(&self.globals);
        for stmt in statements {
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
                effects: _,
            } => {
                // The type checker already validated this signature by
                // the time a real `aint run` gets here; the interpreter
                // only needs param names to bind argument values.
                // `effects` (milestone 13) is purely a type-checking
                // concept - erased after checking, never read here.
                let function = Value::Function(Rc::new(Function {
                    name: name.clone(),
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    body: body.clone(),
                    is_async: *is_async,
                    // For a top-level `fn`, `env` here is always
                    // `globals` (unchanged from before milestone 30) —
                    // this only differs for a block-nested `fn`, which
                    // now closes over its enclosing scope too.
                    captured_env: Rc::clone(env),
                }));
                env.borrow_mut().define(name.clone(), function);
                Ok(Flow::Normal)
            }
            StmtKind::Infer {
                name,
                params,
                return_type,
                permissions,
            } => {
                // Same reasoning as `StmtKind::Fn` above: the type
                // checker already validated this signature, so the
                // interpreter only needs param names — there's no body
                // at all, unlike `Fn`. `return_type` *is* needed now
                // (milestone 09), to validate the model's response
                // against it once awaited. `permissions` (milestone 20)
                // is carried through unchanged — the type checker
                // already validated every name in it refers to a
                // declared `tool`.
                let infer_fn = Value::InferenceFn(Rc::new(InferenceFn {
                    name: name.clone(),
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    return_type: return_type.clone(),
                    permissions: permissions.clone(),
                }));
                env.borrow_mut().define(name.clone(), infer_fn);
                Ok(Flow::Normal)
            }
            StmtKind::Tool {
                name,
                params,
                return_type,
                body,
            } => {
                // Same reasoning as `StmtKind::Infer` just above, plus
                // `self.tools_registry` (milestone 12): a model-driven
                // tool call is a runtime string, not statically-checked
                // AINT source, so the interpreter needs to look up a
                // tool's signature by name independent of any lexical
                // environment.
                let tool_fn = Rc::new(ToolFn {
                    name: name.clone(),
                    params: params.iter().map(|p| p.ty.clone()).collect(),
                    return_type: return_type.clone(),
                    // A real implementation (milestone 34) captures
                    // `env` exactly the way a top-level `fn` does —
                    // see `Function::captured_env`'s doc comment.
                    body: body.as_ref().map(|block| ToolBody {
                        param_names: params.iter().map(|p| p.name.clone()).collect(),
                        block: block.clone(),
                        captured_env: Rc::clone(env),
                    }),
                });
                self.tools_registry
                    .borrow_mut()
                    .insert(name.clone(), Rc::clone(&tool_fn));
                env.borrow_mut()
                    .define(name.clone(), Value::ToolFn(tool_fn));
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
            StmtKind::Test { .. } => {
                // Inert during `aint run` - a `test` block only
                // executes via the milestone-15 test runner, which
                // calls `run_statements` on its body directly rather
                // than reaching it through this arm at all.
                Ok(Flow::Normal)
            }
            StmtKind::Mock { .. } => {
                // A no-op when actually executed: its effect (telling
                // this interpreter's `MockModel`/`MockTool` what to
                // return) already happened before the interpreter was
                // constructed - see `test_runner.rs`.
                Ok(Flow::Normal)
            }
            StmtKind::Assert { condition } => {
                let value = self.eval_expr(condition, env).await?;
                if expect_bool(&value, condition.span)? {
                    Ok(Flow::Normal)
                } else {
                    Err(RuntimeError::AssertionFailed {
                        span: condition.span,
                    })
                }
            }
            StmtKind::Budget {
                max_tokens,
                max_model_calls,
                max_cost,
                timeout_ms,
            } => {
                // The type checker already rejected a second `budget`
                // block, so this always overwrites `None` with the
                // program's one declaration.
                self.budget.set(Some(Budget {
                    max_tokens: *max_tokens,
                    max_model_calls: *max_model_calls,
                    max_cost: *max_cost,
                    timeout_ms: *timeout_ms,
                }));
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
            // See the matching arm in `aint-typechecker`'s
            // `checker.rs` — always eliminated by `aint-loader` first.
            StmtKind::ImportFile { .. } => Err(RuntimeError::TypeMismatch {
                message: "cross-file imports must be resolved by aint-loader before running"
                    .to_string(),
                span: stmt.span,
            }),
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
                    Value::ToolCall(pending) => self.eval_tool_call(&pending, expr.span).await,
                    other => Err(RuntimeError::TypeMismatch {
                        message: format!("cannot await a {}", other.type_name()),
                        span: expr.span,
                    }),
                }
            }
            ExprKind::If {
                condition,
                then_value,
                else_value,
            } => {
                let cond = self.eval_expr(condition, env).await?;
                if expect_bool(&cond, condition.span)? {
                    self.eval_expr(then_value, env).await
                } else {
                    self.eval_expr(else_value, env).await
                }
            }
            ExprKind::Lambda { params, body, .. } => Ok(Value::Function(Rc::new(Function {
                name: "<lambda>".to_string(),
                params: params.iter().map(|p| p.name.clone()).collect(),
                body: body.clone(),
                is_async: false,
                // The actual capture: `env` here is whatever scope is
                // active where this lambda expression is evaluated, not
                // always `globals` — see `Function::captured_env`'s doc
                // comment.
                captured_env: Rc::clone(env),
            }))),
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
                    permissions: infer_fn.permissions.clone(),
                })))
            }
            Value::ToolFn(tool_fn) => {
                if tool_fn.params.len() != args.len() {
                    return Err(RuntimeError::ArityMismatch {
                        name: tool_fn.name.clone(),
                        expected: tool_fn.params.len(),
                        found: args.len(),
                        span,
                    });
                }
                // Deferred, not run: same reasoning as `InferenceFn`
                // just above.
                Ok(Value::ToolCall(Rc::new(PendingToolCall {
                    tool: tool_fn.name.clone(),
                    args,
                    return_type: tool_fn.return_type.clone(),
                })))
            }
            other => Err(RuntimeError::NotCallable {
                type_name: other.type_name(),
                span,
            }),
        }
    }

    /// Runs a function body to completion: parented to the function's
    /// *captured* environment (milestone 30) — `globals` for every
    /// top-level `fn`, exactly as before this milestone; the enclosing
    /// scope for a lambda, which is the actual closure semantics. Not
    /// the *caller's* environment either way — see
    /// `Function::captured_env`'s doc comment for why capturing by
    /// reference is sound. Shared by the sync-call path in
    /// [`Self::call`] and the await path in [`Self::eval_await`].
    #[async_recursion(?Send)]
    async fn run_function(
        &self,
        function: &Rc<Function>,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let call_env = Environment::child(&function.captured_env);
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

    /// Sends a `Value::Inference` to this interpreter's `Model`, and
    /// keeps going as long as the model keeps asking for tool calls
    /// instead of answering. The only place `self.model` is ever
    /// touched. See `docs/milestones/12-ai-tool-calling/SPEC.md`.
    ///
    /// Wraps `eval_inference_loop` in `tokio::time::timeout` when a
    /// `budget`'s `timeout_ms` is set — the actual loop, and the
    /// `max_model_calls`/`max_tokens`/`max_cost` checks, live there.
    /// See `docs/milestones/17-ai-resource-management/SPEC.md`.
    async fn eval_inference(
        &self,
        pending: &PendingInference,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let timeout_ms = self.budget.get().and_then(|budget| budget.timeout_ms);
        match timeout_ms {
            Some(timeout_ms) => {
                match tokio::time::timeout(
                    Duration::from_millis(timeout_ms.max(0) as u64),
                    self.eval_inference_loop(pending, span),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => Err(RuntimeError::BudgetExceeded {
                        message: format!(
                            "`{}` exceeded the budget's timeout_ms ({timeout_ms})",
                            pending.function
                        ),
                        span,
                    }),
                }
            }
            None => self.eval_inference_loop(pending, span).await,
        }
    }

    async fn eval_inference_loop(
        &self,
        pending: &PendingInference,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let mut history: Vec<ToolExchange> = Vec::new();
        loop {
            self.check_model_call_budget(&pending.function, span)?;

            let return_type_variants = match &pending.return_type {
                Type::Enum(name) => self.known_variants(name, span).ok(),
                _ => None,
            };

            let start = Instant::now();
            let result = self
                .model
                .infer(InferenceRequest {
                    function: pending.function.clone(),
                    args: pending.args.clone(),
                    return_type: pending.return_type.clone(),
                    return_type_variants,
                    available_tools: self.available_tools_for(&pending.permissions),
                    history: history.clone(),
                    span,
                })
                .await;
            let latency = start.elapsed();
            let tokens = TokenUsage::default();

            let id = self.next_inference_id.get();
            self.next_inference_id.set(id + 1);
            let trace_outcome = match &result {
                Ok(InferenceOutcome::Answer(value)) => InferenceTraceOutcome::Answer(value.clone()),
                Ok(InferenceOutcome::CallTool { tool, args }) => InferenceTraceOutcome::CallTool {
                    tool: tool.clone(),
                    args: args.clone(),
                },
                Err(err) => InferenceTraceOutcome::Error(err.to_string()),
            };
            self.traces.borrow_mut().push(TraceRecord::Inference {
                id,
                function: pending.function.clone(),
                model: "mock".to_string(),
                tokens,
                latency,
                outcome: trace_outcome,
            });
            self.record_model_call(tokens, &pending.function, span)?;

            match result? {
                InferenceOutcome::Answer(value) => {
                    self.validate_inference_result(&value, &pending.return_type, span)?;
                    return Ok(value);
                }
                InferenceOutcome::CallTool { tool, args } => {
                    self.check_tool_permission(
                        &pending.function,
                        &pending.permissions,
                        &tool,
                        span,
                    )?;
                    let result = self.call_requested_tool(&tool, args.clone(), span).await?;
                    history.push(ToolExchange { tool, args, result });
                }
            }
        }
    }

    /// Rejects a model call the budget's `max_model_calls` wouldn't
    /// allow, *before* it happens — the direct, honest payoff of
    /// milestone 12's documented gap ("no cap ... by design ...
    /// milestone 17 is explicitly where budget belongs").
    fn check_model_call_budget(&self, function: &str, span: Span) -> Result<(), RuntimeError> {
        if let Some(max_model_calls) = self.budget.get().and_then(|b| b.max_model_calls) {
            if self.total_model_calls.get() >= max_model_calls {
                return Err(RuntimeError::BudgetExceeded {
                    message: format!(
                        "calling `{function}` would exceed the budget's max_model_calls ({max_model_calls})"
                    ),
                    span,
                });
            }
        }
        Ok(())
    }

    /// Records a completed model call against the running totals, and
    /// checks `max_tokens`/`max_cost`. Real, tested comparison logic —
    /// but since every call reports `TokenUsage::default()` today (see
    /// `docs/milestones/14-ai-execution-tracing/SPEC.md`), and nothing
    /// computes a real cost anywhere in this codebase yet, neither
    /// check can actually fire through a live call path today. Stated
    /// in `docs/milestones/17-ai-resource-management/SPEC.md`, not
    /// hidden.
    fn record_model_call(
        &self,
        tokens: TokenUsage,
        function: &str,
        span: Span,
    ) -> Result<(), RuntimeError> {
        self.total_model_calls.set(self.total_model_calls.get() + 1);
        self.total_tokens
            .set(self.total_tokens.get() + tokens.prompt as i64 + tokens.completion as i64);

        let Some(budget) = self.budget.get() else {
            return Ok(());
        };
        if let Some(max_tokens) = budget.max_tokens {
            if self.total_tokens.get() > max_tokens {
                return Err(RuntimeError::BudgetExceeded {
                    message: format!(
                        "`{function}` exceeded the budget's max_tokens ({max_tokens})"
                    ),
                    span,
                });
            }
        }
        if let Some(max_cost) = budget.max_cost {
            if self.total_cost.get() > max_cost {
                return Err(RuntimeError::BudgetExceeded {
                    message: format!("`{function}` exceeded the budget's max_cost ({max_cost})"),
                    span,
                });
            }
        }
        Ok(())
    }

    /// The signature of every declared `tool` this particular
    /// inference is allowed to see, as a `Model` sees it — what
    /// `InferenceRequest::available_tools` is built from.
    /// `permissions: None` (no clause on the `infer` declaration) means
    /// every declared tool, unrestricted — the behavior every program
    /// had before milestone 20. `Some(names)` filters down to exactly
    /// those. This is the "what's offered" half of tool authorization;
    /// `check_tool_permission` is the "what's allowed to execute" half,
    /// enforced independently. See
    /// `docs/milestones/20-security-model/SPEC.md`.
    fn available_tools_for(&self, permissions: &Option<Vec<String>>) -> Vec<ToolSignature> {
        self.tools_registry
            .borrow()
            .values()
            .filter(|tool_fn| match permissions {
                Some(names) => names.iter().any(|name| name == &tool_fn.name),
                None => true,
            })
            .map(|tool_fn| ToolSignature {
                name: tool_fn.name.clone(),
                params: tool_fn.params.clone(),
                return_type: tool_fn.return_type.clone(),
            })
            .collect()
    }

    /// Rejects a model-requested tool call outside the requesting
    /// `infer`'s `permissions`, *before* it runs — independent of
    /// whether the tool was ever offered via `available_tools_for`. A
    /// model implementation that ignores what it was told is available
    /// and asks for something else anyway does not get a free pass;
    /// the enforcement point is here, at the actual call, not just the
    /// request construction. See
    /// `docs/milestones/20-security-model/SPEC.md`.
    fn check_tool_permission(
        &self,
        function: &str,
        permissions: &Option<Vec<String>>,
        tool: &str,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if let Some(names) = permissions {
            if !names.iter().any(|name| name == tool) {
                return Err(RuntimeError::PermissionDenied {
                    tool: tool.to_string(),
                    function: function.to_string(),
                    span,
                });
            }
        }
        Ok(())
    }

    /// Runs a tool call *requested by a model*, not written in AINT
    /// source — the "runtime validates arguments before execution"
    /// guarantee milestone 11 deferred to here, because this is the
    /// first place arguments can arrive without the static type
    /// checker ever having seen them. Looks the tool up by name
    /// (`RuntimeError::ToolError` if it isn't declared — "a model
    /// cannot invoke a tool that doesn't exist," now in its dynamic
    /// form), checks argument count and type against its declared
    /// signature, executes it, and validates the result exactly like
    /// `eval_tool_call` does for a directly-called tool.
    async fn call_requested_tool(
        &self,
        tool: &str,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let tool_fn = self
            .tools_registry
            .borrow()
            .get(tool)
            .cloned()
            .ok_or_else(|| RuntimeError::ToolError {
                message: format!("no tool named `{tool}` is declared"),
                span,
            })?;

        if tool_fn.params.len() != args.len() {
            return Err(RuntimeError::ArityMismatch {
                name: tool.to_string(),
                expected: tool_fn.params.len(),
                found: args.len(),
                span,
            });
        }
        for (arg, expected_ty) in args.iter().zip(&tool_fn.params) {
            validate_value_matches_type(arg, expected_ty, span)?;
        }

        let result = self.call_tool_traced(tool, args, span).await?;
        self.validate_inference_result(&result, &tool_fn.return_type, span)?;
        Ok(result)
    }

    /// Actually runs the deferred computation behind a
    /// `Value::ToolCall` — the tool-calling counterpart of
    /// `eval_inference`. Reuses `validate_inference_result` as-is: its
    /// logic (does this `Value` actually match this declared `Type`)
    /// has nothing model-specific about it — a `MockTool` can be
    /// misconfigured with an invalid enum variant exactly as easily as
    /// `MockModel` can. See
    /// `docs/milestones/11-typed-tools/SPEC.md`.
    async fn eval_tool_call(
        &self,
        pending: &PendingToolCall,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let value = self
            .call_tool_traced(&pending.tool, pending.args.clone(), span)
            .await?;
        self.validate_inference_result(&value, &pending.return_type, span)?;
        Ok(value)
    }

    /// The one place a tool call is actually answered — by a
    /// directly-called tool (`eval_tool_call`) or a model-requested one
    /// (`call_requested_tool`). Precedence (milestone 34): an explicit
    /// `mock` always wins, even over a tool with a real implementation —
    /// a test that mocks a tool is stating it doesn't want that tool's
    /// real body to run, not asking permission for it to run anyway. A
    /// real body is the fallback when nothing's mocked; `self.tools`
    /// (`MockTool`)'s own "no mock configured" error is the last resort,
    /// exactly as every tool without a body behaved before this
    /// milestone. Captures a `Tool Call #N` trace record regardless of
    /// which path answered it, or whether it failed. See
    /// `docs/milestones/14-ai-execution-tracing/SPEC.md`.
    async fn call_tool_traced(
        &self,
        tool: &str,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let start = Instant::now();
        let result = match self.tools.get(tool) {
            Some(value) => Ok(value),
            None => {
                let tool_fn = self.tools_registry.borrow().get(tool).cloned();
                match tool_fn.as_ref().and_then(|f| f.body.as_ref()) {
                    Some(body) => self.run_tool_body(body, args.clone()).await,
                    None => {
                        self.tools
                            .call(ToolRequest {
                                tool: tool.to_string(),
                                args: args.clone(),
                                span,
                            })
                            .await
                    }
                }
            }
        };
        let latency = start.elapsed();

        let id = self.next_tool_call_id.get();
        self.next_tool_call_id.set(id + 1);
        let trace_outcome = result.clone().map_err(|err| err.to_string());
        self.traces.borrow_mut().push(TraceRecord::ToolCall {
            id,
            tool: tool.to_string(),
            args,
            latency,
            outcome: trace_outcome,
        });

        result
    }

    /// Runs a tool's real implementation (milestone 34) to completion —
    /// the tool-calling counterpart of `run_function`, same reasoning
    /// throughout (parented to the tool's own captured environment, not
    /// the caller's).
    async fn run_tool_body(
        &self,
        body: &ToolBody,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let call_env = Environment::child(&body.captured_env);
        for (name, value) in body.param_names.iter().zip(args) {
            call_env.borrow_mut().define(name.clone(), value);
        }
        match self.exec_block(&body.block, &call_env).await? {
            Flow::Return(v) => Ok(v),
            Flow::Normal => Ok(Value::Unit),
        }
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
            NativeFunction::HttpServe => {
                let [port_value] = stdlib::one(native, args, span)?;
                let port = stdlib::int(port_value, span)?;
                self.http_serve(port, span).await
            }
            _ => unreachable!("only async natives should ever reach eval_await"),
        }
    }

    /// `http_serve(port)` (milestone 25): binds `127.0.0.1:port` and
    /// serves real HTTP/1.1 forever, one connection at a time,
    /// dispatching every request to a `handle_request(method: String,
    /// path: String, body: String) -> String` the AINT program must
    /// declare. See `docs/milestones/25-real-application/SPEC.md` for
    /// why this is hand-rolled over a raw `TcpListener` rather than
    /// `hyper`/`axum` (both want `Send` handler futures; `Value`
    /// never is), and why there's no router (AINT has no string-
    /// splitting/regex to build one out of) — routing is just
    /// `if`/`else` inside `handle_request` itself.
    async fn http_serve(&self, port: i64, span: Span) -> Result<Value, RuntimeError> {
        let handler = self.globals.borrow().get("handle_request").ok_or_else(|| {
            RuntimeError::UndefinedVariable {
                name: "handle_request".to_string(),
                span,
            }
        })?;

        let listener = TcpListener::bind(("127.0.0.1", port.clamp(0, u16::MAX as i64) as u16))
            .await
            .map_err(|err| RuntimeError::Io {
                message: format!("could not bind to port {port}: {err}"),
                span,
            })?;

        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };

            let (method, path, body) = match read_http_request(&mut stream).await {
                Ok(parsed) => parsed,
                Err(_) => {
                    let _ = write_http_response(&mut stream, 400, "", "").await;
                    continue;
                }
            };
            let response_path = path.clone();

            let args = vec![
                Value::String(method),
                Value::String(path),
                Value::String(body),
            ];
            let result = self.call(handler.clone(), args, span).await;
            let result = match result {
                Ok(Value::Task(task)) => self.eval_await(&task, span).await,
                other => other,
            };

            match result {
                Ok(Value::String(response_body)) => {
                    let _ =
                        write_http_response(&mut stream, 200, &response_path, &response_body).await;
                }
                Ok(other) => {
                    let _ = write_http_response(
                        &mut stream,
                        500,
                        &response_path,
                        &format!(
                            "handle_request must return a String, returned a {}",
                            other.type_name()
                        ),
                    )
                    .await;
                }
                Err(err) => {
                    // The full error (which may echo back request
                    // content, internal state, or a file path - see
                    // `RuntimeError`'s `Display` impls) goes to the
                    // server's own log, not the client. Sending it
                    // straight into the response body was a real
                    // information-disclosure gap, found and fixed in
                    // this milestone's security pass - the same
                    // conservative default FastAPI/Starlette already
                    // apply to unhandled exceptions. See
                    // `docs/milestones/28-production-language/SPEC.md`.
                    eprintln!("[http_serve] request failed: {err}");
                    let _ = write_http_response(
                        &mut stream,
                        500,
                        &response_path,
                        "internal server error",
                    )
                    .await;
                }
            }
        }
    }
}

/// Reads one HTTP/1.1 request off `stream`: the request line (method,
/// path), and — if a `Content-Length` header is present — exactly
/// that many bytes of body. No keep-alive, no chunked encoding, no
/// header value beyond `Content-Length` is interpreted at all; enough
/// to be a real, `curl`-able server and no more. See `http_serve`'s
/// own doc comment for why this is hand-rolled.
async fn read_http_request(stream: &mut TcpStream) -> Result<(String, String, String), String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("connection closed before headers were complete".to_string());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > 1_000_000 {
            return Err("request headers too large".to_string());
        }
    };

    let header_text = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().ok_or("empty request")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing method")?.to_string();
    let path = parts.next().ok_or("missing path")?.to_string();

    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }

    let body_start = header_end + 4;
    let mut body = buf[body_start..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    Ok((method, path, String::from_utf8_lossy(&body).into_owned()))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

async fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    path: &str,
    body: &str,
) -> Result<(), String> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Internal Server Error",
    };
    let content_type = content_type_for(path, body);
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

/// Picks a response `Content-Type` — first by `path`'s file extension
/// (the request path a static-asset route like `/style.css` or
/// `/app.js` would naturally use), then, for anything unrecognized
/// (an extensionless route like `/` or `/about`, exactly what a page
/// route looks like), by sniffing whether `body` looks like markup.
/// Defaults to `application/json` — the one and only content type
/// this ever sent before this check existed, so every existing
/// JSON-API-shaped `handle_request` (`examples/customer_support/`'s
/// among them) keeps exactly the response it always got. Added
/// because `http_serve` couldn't actually serve a webpage before this:
/// a browser won't render a `text/html` document that arrives labeled
/// `application/json`, which was the only label this ever sent.
fn content_type_for(path: &str, body: &str) -> &'static str {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let extension = file_name.rsplit_once('.').map(|(_, ext)| ext);
    match extension {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("txt") => "text/plain; charset=utf-8",
        _ if body.trim_start().starts_with('<') => "text/html; charset=utf-8",
        _ => "application/json",
    }
}

#[cfg(test)]
mod content_type_tests {
    use super::content_type_for;

    #[test]
    fn a_json_api_route_with_no_extension_and_an_object_body_stays_json() {
        assert_eq!(
            content_type_for("/register", "{\"user_id\":\"1\"}"),
            "application/json"
        );
    }

    #[test]
    fn an_extensionless_route_returning_markup_is_sniffed_as_html() {
        assert_eq!(
            content_type_for("/", "<!doctype html><html></html>"),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            content_type_for("/about", "  <html></html>"),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn known_static_asset_extensions_are_recognized_by_path_alone() {
        assert_eq!(
            content_type_for("/style.css", "body { color: red; }"),
            "text/css; charset=utf-8"
        );
        assert_eq!(
            content_type_for("/app.js", "console.log(1)"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type_for("/icon.svg", "<svg></svg>"),
            "image/svg+xml"
        );
    }

    #[test]
    fn an_unknown_extension_falls_back_to_body_sniffing() {
        assert_eq!(
            content_type_for("/data.xyz", "{\"a\":1}"),
            "application/json"
        );
        assert_eq!(
            content_type_for("/data.xyz", "<xyz/>"),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn a_malformed_request_with_an_empty_path_and_body_defaults_to_json() {
        assert_eq!(content_type_for("", ""), "application/json");
    }
}

/// Checks a runtime `Value` against a declared `Type` — the first
/// place AINT needs this check at all, since every prior call site was
/// AINT source the static type checker already validated. A
/// model-requested tool call's arguments are a runtime string and a
/// `Vec<Value>` the checker never saw. See
/// `docs/milestones/12-ai-tool-calling/SPEC.md`.
fn validate_value_matches_type(value: &Value, ty: &Type, span: Span) -> Result<(), RuntimeError> {
    let ok = match (value, ty) {
        (Value::Int(_), Type::Int) => true,
        (Value::Float(_), Type::Float) => true,
        (Value::Bool(_), Type::Bool) => true,
        (Value::String(_), Type::String) => true,
        (Value::Unit, Type::Unit) => true,
        (Value::Enum(name, _), Type::Enum(expected)) => name == expected,
        (Value::List(items), Type::List(inner)) => {
            return items
                .iter()
                .try_for_each(|item| validate_value_matches_type(item, inner, span));
        }
        (Value::Option(None), Type::Option(_)) => true,
        (Value::Option(Some(inner_value)), Type::Option(inner_ty)) => {
            return validate_value_matches_type(inner_value, inner_ty, span);
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(RuntimeError::SchemaViolation {
            message: format!("expected {ty}, found a {}", value.type_name()),
            span,
        })
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
    use aint_ast::Position;

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
    fn a_lambda_captures_its_defining_scope_not_globals() {
        // The real proof this is a closure and not still "parent to
        // globals" (milestone 30): `n` is `make_adder`'s own local
        // parameter, long out of scope by the time `add5` is actually
        // called — only correct if `fn(x: Int) -> Int { ... }` captured
        // the environment live where it was defined, not `globals`.
        assert_eq!(
            run_capturing(concat!(
                "fn make_adder(n: Int) -> fn(Int) -> Int {\n",
                "    return fn(x: Int) -> Int {\n",
                "        return x + n\n",
                "    }\n",
                "}\n",
                "let add5 = make_adder(5)\n",
                "let add10 = make_adder(10)\n",
                "print(add5(1))\n",
                "print(add10(1))\n"
            )),
            "6\n11\n"
        );
    }

    #[test]
    fn closures_stored_in_a_list_are_called_correctly_by_index() {
        assert_eq!(
            run_capturing(concat!(
                "let handlers = [fn(x: Int) -> Int {\n",
                "    return x + 1\n",
                "}, fn(x: Int) -> Int {\n",
                "    return x * 2\n",
                "}]\n",
                "print(handlers[0](5))\n",
                "print(handlers[1](5))\n"
            )),
            "6\n10\n"
        );
    }

    #[test]
    fn an_immediately_invoked_lambda_runs_directly() {
        assert_eq!(
            run_capturing("print((fn(x: Int) -> Int {\n    return x * x\n})(4))"),
            "16\n"
        );
    }

    #[test]
    fn an_if_expression_evaluates_the_taken_branch() {
        assert_eq!(
            run_capturing("let x = if true { 1 } else { 2 }\nprint(x)"),
            "1\n"
        );
        assert_eq!(
            run_capturing("let x = if false { 1 } else { 2 }\nprint(x)"),
            "2\n"
        );
    }

    #[test]
    fn an_else_if_expression_chain_evaluates_the_first_matching_branch() {
        assert_eq!(
            run_capturing(concat!(
                "fn sign(n: Int) -> String {\n",
                "    return if n < 0 { \"negative\" } else if n == 0 { \"zero\" } else { \"positive\" }\n",
                "}\n",
                "print(sign(-1))\n",
                "print(sign(0))\n",
                "print(sign(1))\n"
            )),
            "negative\nzero\npositive\n"
        );
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
    fn string_split_splits_on_every_occurrence() {
        assert_eq!(
            run_capturing(
                "import string\n\
                 let parts = string_split(\"a=1&b=2&c=3\", \"&\")\n\
                 print(parts[0])\n\
                 print(parts[1])\n\
                 print(parts[2])"
            ),
            "a=1\nb=2\nc=3\n"
        );
    }

    #[test]
    fn string_split_on_an_absent_separator_yields_one_element() {
        assert_eq!(
            run_capturing("import string\nprint(string_split(\"hello\", \",\")[0])"),
            "hello\n"
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

    // --- json/auth/log (milestone 25) ------------------------------------
    // `db`'s natives write real files, so they're covered at the
    // `db.rs` module level (with a properly isolated scratch
    // directory per test) instead of here, to avoid every parallel
    // `cargo test` thread racing over the same relative `.aintdb`.

    #[test]
    fn json_get_finds_a_flat_string_field() {
        assert_eq!(
            run_capturing(
                "import json\n\
                 import option\n\
                 let found = json_get(\"{\\\"subject\\\": \\\"help\\\"}\", \"subject\")\n\
                 print(option_unwrap(found))"
            ),
            "help\n"
        );
    }

    #[test]
    fn json_get_of_a_missing_key_is_none() {
        assert_eq!(
            run_capturing(
                "import json\n\
                 import option\n\
                 print(option_is_some(json_get(\"{\\\"a\\\": \\\"b\\\"}\", \"missing\")))"
            ),
            "false\n"
        );
    }

    #[test]
    fn json_object_builds_a_flat_object_json_get_can_read_back() {
        assert_eq!(
            run_capturing(
                "import json\n\
                 import option\n\
                 let built = json_object([\"id\", \"subject\"], [\"1\", \"help\"])\n\
                 print(option_unwrap(json_get(built, \"id\")))\n\
                 print(option_unwrap(json_get(built, \"subject\")))"
            ),
            "1\nhelp\n"
        );
    }

    #[test]
    fn auth_hash_and_verify_password_round_trip() {
        assert_eq!(
            run_capturing(
                "import auth\n\
                 let hash = auth_hash_password(\"correct horse\")\n\
                 print(auth_verify_password(\"correct horse\", hash))\n\
                 print(auth_verify_password(\"wrong password\", hash))"
            ),
            "true\nfalse\n"
        );
    }

    #[test]
    fn auth_generate_token_produces_distinct_nonempty_tokens() {
        assert_eq!(
            run_capturing(
                "import auth\n\
                 let a = auth_generate_token()\n\
                 let b = auth_generate_token()\n\
                 print(a == b)\n\
                 print(a != \"\")"
            ),
            "false\ntrue\n"
        );
    }

    #[test]
    fn log_functions_run_without_error() {
        assert_eq!(
            run_capturing(
                "import log\n\
                 log_info(\"server started\")\n\
                 log_error(\"something went wrong\")\n\
                 print(\"done\")"
            ),
            "done\n"
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

    // --- tool --------------------------------------------------------

    /// Same reasoning as `run_capturing_with_model`: `MockTool` holding
    /// a mocked `Value` isn't `Send`, so it's built from a `Send`
    /// builder closure inside the big-stack thread, not captured from
    /// outside it.
    fn run_capturing_with_tools(
        src: &'static str,
        build_tools: impl FnOnce() -> crate::tool::MockTool + Send + 'static,
    ) -> String {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(src).expect("should parse");
            let interpreter = Interpreter::with_output_model_and_tools(
                Vec::new(),
                MockModel::new(),
                build_tools(),
            );
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
    fn calling_a_tool_without_await_does_not_touch_the_mock() {
        // An unconfigured MockTool would error if this ran. It
        // shouldn't - nothing awaits it, same as an unawaited infer.
        assert_eq!(
            run_capturing_with_tools(
                "tool database_get_email(id: String) -> String\n\
                 let _pending = database_get_email(\"1\")\n\
                 print(1)",
                crate::tool::MockTool::new,
            ),
            "1\n"
        );
    }

    #[test]
    fn awaiting_a_tool_call_returns_the_mocked_value() {
        assert_eq!(
            run_capturing_with_tools(
                "tool database_get_email(id: String) -> String\n\
                 print(await database_get_email(\"1\"))",
                || crate::tool::MockTool::new()
                    .mock("database_get_email", Value::String("a@b.com".to_string())),
            ),
            "a@b.com\n"
        );
    }

    #[test]
    fn a_tool_with_a_real_body_runs_it_directly_with_no_mock_configured() {
        assert_eq!(
            run_capturing(
                "tool double(x: Int) -> Int {\n    return x * 2\n}\nprint(await double(21))"
            ),
            "42\n"
        );
    }

    #[test]
    fn a_tool_with_a_real_body_calling_stdlib_functions_runs_for_real() {
        assert_eq!(
            run_capturing(
                "import string\n\
                 tool greet(name: String) -> String {\n    return string_concat(\"Hi \", name)\n}\n\
                 print(await greet(\"Ada\"))"
            ),
            "Hi Ada\n"
        );
    }

    #[test]
    fn an_explicit_mock_wins_over_a_tools_real_body() {
        // A mock is a statement of intent — "don't run the real
        // implementation for this test" — not a fallback only used
        // when no real body exists. See `call_tool_traced`'s doc
        // comment (milestone 34).
        assert_eq!(
            run_capturing_with_tools(
                "tool double(x: Int) -> Int {\n    return x * 2\n}\nprint(await double(21))",
                || crate::tool::MockTool::new().mock("double", Value::Int(999)),
            ),
            "999\n"
        );
    }

    #[test]
    fn a_model_requested_tool_call_runs_a_real_body_too() {
        assert_eq!(
            run_capturing_with_model_and_tools(
                "infer agent(x: Int) -> Int\n\
                 tool double(y: Int) -> Int {\n    return y * 2\n}\n\
                 print(await agent(5))",
                || {
                    crate::model::MockModel::new().script(
                        "agent",
                        vec![
                            crate::model::InferenceOutcome::CallTool {
                                tool: "double".to_string(),
                                args: vec![Value::Int(5)],
                            },
                            crate::model::InferenceOutcome::Answer(Value::Int(10)),
                        ],
                    )
                },
                crate::tool::MockTool::new,
            ),
            "10\n"
        );
    }

    #[test]
    fn tool_returning_an_enum_is_schema_validated() {
        assert_eq!(
            run_capturing_with_tools(
                "enum Sentiment { Positive Neutral Negative }\n\
                 tool classify_cached(text: String) -> Sentiment\n\
                 print(await classify_cached(\"x\") == Sentiment_Positive)",
                || {
                    crate::tool::MockTool::new().mock(
                        "classify_cached",
                        Value::Enum("Sentiment".to_string(), "Positive".to_string()),
                    )
                },
            ),
            "true\n"
        );
    }

    #[test]
    fn tool_returning_a_hallucinated_variant_is_a_schema_violation() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "enum Sentiment { Positive Neutral Negative }\n\
                 tool classify_cached(text: String) -> Sentiment\n\
                 print(await classify_cached(\"x\"))",
            )
            .expect("should parse");
            let tools = crate::tool::MockTool::new().mock(
                "classify_cached",
                Value::Enum("Sentiment".to_string(), "Ecstatic".to_string()),
            );
            let interpreter =
                Interpreter::with_output_model_and_tools(Vec::new(), MockModel::new(), tools);
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
    fn awaiting_an_unconfigured_tool_call_is_a_clear_tool_error() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "tool database_get_email(id: String) -> String\n\
                 await database_get_email(\"1\")",
            )
            .expect("should parse");
            let interpreter = Interpreter::with_output(Vec::new());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::ToolError { .. }));
    }

    #[test]
    fn tool_and_infer_are_independent_mocks() {
        // A tool and an infer function can share a name-space-adjacent
        // identity without either mock leaking into the other's table.
        assert_eq!(
            run_capturing_with_tools(
                "infer classify(text: String) -> Bool\n\
                 tool classify_cached(text: String) -> Bool\n\
                 print(await classify_cached(\"x\"))",
                || crate::tool::MockTool::new().mock("classify_cached", Value::Bool(true)),
            ),
            "true\n"
        );
    }

    // --- AI tool calling (milestone 12) -------------------------------

    fn run_capturing_with_model_and_tools(
        src: &'static str,
        build_model: impl FnOnce() -> crate::model::MockModel + Send + 'static,
        build_tools: impl FnOnce() -> crate::tool::MockTool + Send + 'static,
    ) -> String {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(src).expect("should parse");
            let interpreter =
                Interpreter::with_output_model_and_tools(Vec::new(), build_model(), build_tools());
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
    fn model_requests_a_tool_call_then_answers() {
        // The actual payoff of this milestone: the model doesn't
        // answer directly - it asks for a tool call, gets the result,
        // and *then* answers. Two round trips to the model, one to the
        // tool, all driven entirely by `eval_inference`'s loop.
        assert_eq!(
            run_capturing_with_model_and_tools(
                "tool database_get_email(id: String) -> String\n\
                 infer greet_customer(id: String) -> String\n\
                 print(await greet_customer(\"42\"))",
                || {
                    crate::model::MockModel::new().script(
                        "greet_customer",
                        vec![
                            crate::model::InferenceOutcome::CallTool {
                                tool: "database_get_email".to_string(),
                                args: vec![Value::String("42".to_string())],
                            },
                            crate::model::InferenceOutcome::Answer(Value::String(
                                "Hello, a@b.com".to_string(),
                            )),
                        ],
                    )
                },
                || {
                    crate::tool::MockTool::new()
                        .mock("database_get_email", Value::String("a@b.com".to_string()))
                },
            ),
            "Hello, a@b.com\n"
        );
    }

    #[test]
    fn model_requests_two_tool_calls_in_sequence_before_answering() {
        assert_eq!(
            run_capturing_with_model_and_tools(
                "tool database_get_email(id: String) -> String\n\
                 tool database_get_name(id: String) -> String\n\
                 infer greet_customer(id: String) -> String\n\
                 print(await greet_customer(\"42\"))",
                || {
                    crate::model::MockModel::new().script(
                        "greet_customer",
                        vec![
                            crate::model::InferenceOutcome::CallTool {
                                tool: "database_get_name".to_string(),
                                args: vec![Value::String("42".to_string())],
                            },
                            crate::model::InferenceOutcome::CallTool {
                                tool: "database_get_email".to_string(),
                                args: vec![Value::String("42".to_string())],
                            },
                            crate::model::InferenceOutcome::Answer(Value::String(
                                "Hello Ada, a@b.com".to_string(),
                            )),
                        ],
                    )
                },
                || {
                    crate::tool::MockTool::new()
                        .mock("database_get_name", Value::String("Ada".to_string()))
                        .mock("database_get_email", Value::String("a@b.com".to_string()))
                },
            ),
            "Hello Ada, a@b.com\n"
        );
    }

    #[test]
    fn model_requesting_an_undeclared_tool_is_a_clear_tool_error() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "infer greet_customer(id: String) -> String\n\
                 await greet_customer(\"42\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().script(
                "greet_customer",
                vec![crate::model::InferenceOutcome::CallTool {
                    tool: "nonexistent_tool".to_string(),
                    args: vec![],
                }],
            );
            let interpreter =
                Interpreter::with_output_model_and_tools(Vec::new(), model, MockTool::new());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::ToolError { .. }));
    }

    #[test]
    fn a_permitted_tool_call_still_succeeds() {
        // `permissions` naming exactly the tool the model requests:
        // the conversation proceeds exactly as it would with no
        // `permissions` clause at all.
        assert_eq!(
            run_capturing_with_model_and_tools(
                "tool database_get_email(id: String) -> String\n\
                 tool send_email(to: String, body: String) -> Bool\n\
                 infer greet_customer(id: String) -> String permissions [database_get_email]\n\
                 print(await greet_customer(\"42\"))",
                || {
                    crate::model::MockModel::new().script(
                        "greet_customer",
                        vec![
                            crate::model::InferenceOutcome::CallTool {
                                tool: "database_get_email".to_string(),
                                args: vec![Value::String("42".to_string())],
                            },
                            crate::model::InferenceOutcome::Answer(Value::String(
                                "Hello, a@b.com".to_string(),
                            )),
                        ],
                    )
                },
                || {
                    crate::tool::MockTool::new()
                        .mock("database_get_email", Value::String("a@b.com".to_string()))
                },
            ),
            "Hello, a@b.com\n"
        );
    }

    #[test]
    fn a_tool_call_outside_permissions_is_rejected_even_though_the_tool_is_declared() {
        // `send_email` is a real, declared tool elsewhere in the same
        // program - just not one `greet_customer` is permitted to
        // request. This is the enforcement half, independent of
        // whether the model was ever offered `send_email` in
        // `available_tools` (it wasn't, but the check doesn't rely on
        // the model having behaved).
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "tool database_get_email(id: String) -> String\n\
                 tool send_email(to: String, body: String) -> Bool\n\
                 infer greet_customer(id: String) -> String permissions [database_get_email]\n\
                 await greet_customer(\"42\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().script(
                "greet_customer",
                vec![crate::model::InferenceOutcome::CallTool {
                    tool: "send_email".to_string(),
                    args: vec![
                        Value::String("a@b.com".to_string()),
                        Value::String("hi".to_string()),
                    ],
                }],
            );
            let interpreter = Interpreter::with_output_model_and_tools(
                Vec::new(),
                model,
                crate::tool::MockTool::new().mock("send_email", Value::Bool(true)),
            );
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        match err {
            RuntimeError::PermissionDenied { tool, function, .. } => {
                assert_eq!(tool, "send_email");
                assert_eq!(function, "greet_customer");
            }
            other => panic!("expected PermissionDenied, got {other:?}"),
        }
    }

    #[test]
    fn an_infer_with_no_permissions_clause_can_call_any_declared_tool() {
        // The default-unrestricted behavior every program from
        // milestone 12 onward already relies on - unaffected by this
        // milestone unless a program opts into `permissions`.
        assert_eq!(
            run_capturing_with_model_and_tools(
                "tool database_get_email(id: String) -> String\n\
                 tool send_email(to: String, body: String) -> Bool\n\
                 infer greet_customer(id: String) -> String\n\
                 print(await greet_customer(\"42\"))",
                || {
                    crate::model::MockModel::new().script(
                        "greet_customer",
                        vec![
                            crate::model::InferenceOutcome::CallTool {
                                tool: "send_email".to_string(),
                                args: vec![
                                    Value::String("a@b.com".to_string()),
                                    Value::String("hi".to_string()),
                                ],
                            },
                            crate::model::InferenceOutcome::Answer(Value::String(
                                "done".to_string(),
                            )),
                        ],
                    )
                },
                || crate::tool::MockTool::new().mock("send_email", Value::Bool(true)),
            ),
            "done\n"
        );
    }

    #[test]
    fn model_requesting_a_tool_call_with_wrong_argument_type_is_rejected() {
        // The runtime, not the type checker, catches this - the model
        // supplied the argument, so nothing statically checked it.
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "tool database_get_email(id: String) -> String\n\
                 infer greet_customer(id: String) -> String\n\
                 await greet_customer(\"42\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().script(
                "greet_customer",
                vec![crate::model::InferenceOutcome::CallTool {
                    tool: "database_get_email".to_string(),
                    args: vec![Value::Int(42)],
                }],
            );
            let interpreter =
                Interpreter::with_output_model_and_tools(Vec::new(), model, MockTool::new());
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
    fn model_requesting_a_tool_call_with_wrong_argument_count_is_rejected() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "tool database_get_email(id: String) -> String\n\
                 infer greet_customer(id: String) -> String\n\
                 await greet_customer(\"42\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().script(
                "greet_customer",
                vec![crate::model::InferenceOutcome::CallTool {
                    tool: "database_get_email".to_string(),
                    args: vec![],
                }],
            );
            let interpreter =
                Interpreter::with_output_model_and_tools(Vec::new(), model, MockTool::new());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::ArityMismatch { .. }));
    }

    // --- AI execution tracing (milestone 14) --------------------------

    // `TraceRecord` embeds `Value`, which holds `Rc` - not `Send`, same
    // as `Interpreter` itself. So every tracing test below does its
    // assertions *inside* the big-stack closure, rather than returning
    // `Vec<TraceRecord>` out of it the way `run_capturing` returns a
    // plain `String`.

    #[test]
    fn successful_infer_call_is_traced() {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "infer is_positive(text: String) -> Bool\n\
                 await is_positive(\"great\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().mock("is_positive", Value::Bool(true));
            let interpreter = Interpreter::with_output_and_model(Vec::new(), model);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect("should run without error");
            let traces = interpreter.traces();

            assert_eq!(traces.len(), 1);
            match &traces[0] {
                TraceRecord::Inference {
                    id,
                    function,
                    model,
                    outcome,
                    ..
                } => {
                    assert_eq!(*id, 1);
                    assert_eq!(function, "is_positive");
                    assert_eq!(model, "mock");
                    assert_eq!(*outcome, InferenceTraceOutcome::Answer(Value::Bool(true)));
                }
                other => panic!("expected an Inference trace, got {other:?}"),
            }
            assert_eq!(traces[0].label(), "Inference #1");
        });
    }

    #[test]
    fn failed_infer_call_is_still_traced() {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "infer is_positive(text: String) -> Bool\n\
                 await is_positive(\"great\")",
            )
            .expect("should parse");
            let interpreter = Interpreter::with_output(Vec::new());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            let _ = runtime.block_on(interpreter.run(&program));
            let traces = interpreter.traces();

            assert_eq!(traces.len(), 1);
            match &traces[0] {
                TraceRecord::Inference { outcome, .. } => {
                    assert!(matches!(outcome, InferenceTraceOutcome::Error(_)));
                }
                other => panic!("expected an Inference trace, got {other:?}"),
            }
        });
    }

    #[test]
    fn tool_call_is_traced() {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "tool database_get_email(id: String) -> String\n\
                 await database_get_email(\"1\")",
            )
            .expect("should parse");
            let tools = crate::tool::MockTool::new()
                .mock("database_get_email", Value::String("a@b.com".to_string()));
            let interpreter =
                Interpreter::with_output_model_and_tools(Vec::new(), MockModel::new(), tools);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect("should run without error");
            let traces = interpreter.traces();

            assert_eq!(traces.len(), 1);
            match &traces[0] {
                TraceRecord::ToolCall {
                    id, tool, outcome, ..
                } => {
                    assert_eq!(*id, 1);
                    assert_eq!(tool, "database_get_email");
                    assert_eq!(*outcome, Ok(Value::String("a@b.com".to_string())));
                }
                other => panic!("expected a ToolCall trace, got {other:?}"),
            }
            assert_eq!(traces[0].label(), "Tool Call #1");
        });
    }

    #[test]
    fn failed_tool_call_is_still_traced() {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "tool database_get_email(id: String) -> String\n\
                 await database_get_email(\"1\")",
            )
            .expect("should parse");
            let interpreter = Interpreter::with_output(Vec::new());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            let _ = runtime.block_on(interpreter.run(&program));
            let traces = interpreter.traces();

            assert_eq!(traces.len(), 1);
            match &traces[0] {
                TraceRecord::ToolCall { outcome, .. } => assert!(outcome.is_err()),
                other => panic!("expected a ToolCall trace, got {other:?}"),
            }
        });
    }

    #[test]
    fn multi_step_tool_calling_conversation_produces_the_right_trace_sequence() {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "tool database_get_name(id: String) -> String\n\
                 tool database_get_email(id: String) -> String\n\
                 infer greet_customer(id: String) -> String\n\
                 await greet_customer(\"42\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().script(
                "greet_customer",
                vec![
                    crate::model::InferenceOutcome::CallTool {
                        tool: "database_get_name".to_string(),
                        args: vec![Value::String("42".to_string())],
                    },
                    crate::model::InferenceOutcome::CallTool {
                        tool: "database_get_email".to_string(),
                        args: vec![Value::String("42".to_string())],
                    },
                    crate::model::InferenceOutcome::Answer(Value::String(
                        "Hello Ada, a@b.com".to_string(),
                    )),
                ],
            );
            let tools = crate::tool::MockTool::new()
                .mock("database_get_name", Value::String("Ada".to_string()))
                .mock("database_get_email", Value::String("a@b.com".to_string()));
            let interpreter = Interpreter::with_output_model_and_tools(Vec::new(), model, tools);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect("should run without error");
            let traces = interpreter.traces();

            let labels: Vec<String> = traces.iter().map(TraceRecord::label).collect();
            assert_eq!(
                labels,
                vec![
                    "Inference #1",
                    "Tool Call #1",
                    "Inference #2",
                    "Tool Call #2",
                    "Inference #3",
                ]
            );
        });
    }

    #[test]
    fn traces_capture_genuinely_measured_latency_and_placeholder_tokens() {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "infer is_positive(text: String) -> Bool\n\
                 await is_positive(\"great\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().mock("is_positive", Value::Bool(true));
            let interpreter = Interpreter::with_output_and_model(Vec::new(), model);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect("should run without error");
            let traces = interpreter.traces();

            match &traces[0] {
                TraceRecord::Inference {
                    latency, tokens, ..
                } => {
                    // Not asserting a nonzero duration - the mock is
                    // fast enough that it can legitimately round to
                    // zero on some platforms. The property under test
                    // is that this is a real, measured `Duration`,
                    // not that it's large - see SPEC.md.
                    assert!(latency.as_secs() < 1, "expected a fast, real duration");
                    assert_eq!(*tokens, TokenUsage::default());
                }
                other => panic!("expected an Inference trace, got {other:?}"),
            }
        });
    }

    // --- AI resource management (milestone 17) ------------------------

    /// A deliberately slow `Model` — nothing else in this test module
    /// takes real time, so a genuine `tokio::time::sleep` is the only
    /// way to prove `timeout_ms` fires against real elapsed time rather
    /// than some proxy for it.
    struct SlowModel {
        delay_ms: u64,
    }

    impl Model for SlowModel {
        async fn infer(
            &self,
            _request: InferenceRequest,
        ) -> Result<InferenceOutcome, RuntimeError> {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            Ok(InferenceOutcome::Answer(Value::Bool(true)))
        }
    }

    #[test]
    fn timeout_ms_fires_against_a_genuinely_slow_model() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "budget { timeout_ms = 20 }\n\
                 infer is_positive(text: String) -> Bool\n\
                 await is_positive(\"x\")",
            )
            .expect("should parse");
            let interpreter =
                Interpreter::with_output_and_model(Vec::new(), SlowModel { delay_ms: 300 });
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::BudgetExceeded { .. }));
    }

    #[test]
    fn max_model_calls_stops_a_tool_calling_conversation_before_it_answers() {
        let err = run_on_big_stack(move || {
            let program = aint_parser::parse_source(
                "budget { max_model_calls = 2 }\n\
                 tool database_get_name(id: String) -> String\n\
                 infer greet_customer(id: String) -> String\n\
                 await greet_customer(\"42\")",
            )
            .expect("should parse");
            let model = crate::model::MockModel::new().script(
                "greet_customer",
                vec![
                    crate::model::InferenceOutcome::CallTool {
                        tool: "database_get_name".to_string(),
                        args: vec![Value::String("42".to_string())],
                    },
                    crate::model::InferenceOutcome::CallTool {
                        tool: "database_get_name".to_string(),
                        args: vec![Value::String("42".to_string())],
                    },
                    crate::model::InferenceOutcome::Answer(Value::String("done".to_string())),
                ],
            );
            let tools = crate::tool::MockTool::new()
                .mock("database_get_name", Value::String("Ada".to_string()));
            let interpreter = Interpreter::with_output_model_and_tools(Vec::new(), model, tools);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(interpreter.run(&program))
                .expect_err("should produce a runtime error")
        });
        assert!(matches!(err, RuntimeError::BudgetExceeded { .. }));
    }

    #[test]
    fn a_budget_covering_only_other_fields_does_not_restrict_model_calls() {
        assert_eq!(
            run_capturing_with_model(
                "budget { max_tokens = 1000000 }\n\
                 infer is_positive(text: String) -> Bool\n\
                 print(await is_positive(\"x\"))",
                || crate::model::MockModel::new().mock("is_positive", Value::Bool(true)),
            ),
            "true\n"
        );
    }

    #[test]
    fn programs_without_a_budget_block_are_completely_unaffected() {
        // No budget block anywhere - every pre-17 test in this file
        // already proves this, but it's worth one direct case too.
        assert_eq!(
            run_capturing_with_model(
                "infer is_positive(text: String) -> Bool\n\
                 print(await is_positive(\"x\"))",
                || crate::model::MockModel::new().mock("is_positive", Value::Bool(true)),
            ),
            "true\n"
        );
    }

    #[test]
    fn record_model_call_checks_max_tokens_and_max_cost_directly() {
        // Real, tested comparison logic - but every call reports
        // `TokenUsage::default()` today (see
        // docs/milestones/14-ai-execution-tracing/SPEC.md), and
        // nothing computes a real cost anywhere in this codebase yet,
        // so neither check can actually fire through a live call path
        // yet. This pokes the accumulator fields directly (both are
        // `pub(crate)`-visible from this same-file test module) to
        // prove the check itself is correct, honestly documented as
        // not reachable end to end today - see
        // docs/milestones/17-ai-resource-management/SPEC.md.
        run_on_big_stack(move || {
            let interpreter = Interpreter::with_output(Vec::new());
            let span = Span::new(Position::new(1, 1), Position::new(1, 1));

            interpreter.budget.set(Some(Budget {
                max_tokens: Some(10),
                max_model_calls: None,
                max_cost: None,
                timeout_ms: None,
            }));
            interpreter.total_tokens.set(20);
            let err = interpreter
                .record_model_call(TokenUsage::default(), "f", span)
                .expect_err("should exceed max_tokens");
            assert!(matches!(err, RuntimeError::BudgetExceeded { .. }));

            interpreter.budget.set(Some(Budget {
                max_tokens: None,
                max_model_calls: None,
                max_cost: Some(0.01),
                timeout_ms: None,
            }));
            interpreter.total_cost.set(0.02);
            let err = interpreter
                .record_model_call(TokenUsage::default(), "f", span)
                .expect_err("should exceed max_cost");
            assert!(matches!(err, RuntimeError::BudgetExceeded { .. }));
        });
    }
}
