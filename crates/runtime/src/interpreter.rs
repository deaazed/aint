use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

use aint_ast::{BinaryOp, Block, Expr, ExprKind, Program, Span, Stmt, StmtKind, UnaryOp};

use crate::environment::Environment;
use crate::error::RuntimeError;
use crate::value::{Function, NativeFunction, Value};

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
pub struct Interpreter<W: Write = io::Stdout> {
    globals: Rc<RefCell<Environment>>,
    output: RefCell<W>,
}

impl Interpreter<io::Stdout> {
    pub fn new() -> Self {
        Self::with_output(io::stdout())
    }
}

impl Default for Interpreter<io::Stdout> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Write> Interpreter<W> {
    pub fn with_output(output: W) -> Self {
        let globals = Environment::new();
        globals
            .borrow_mut()
            .define("print", Value::Native(NativeFunction::Print));
        Self {
            globals,
            output: RefCell::new(output),
        }
    }

    /// Consumes the interpreter, returning the underlying writer.
    pub fn into_output(self) -> W {
        self.output.into_inner()
    }

    pub fn run(&self, program: &Program) -> Result<(), RuntimeError> {
        let env = Rc::clone(&self.globals);
        for stmt in &program.statements {
            match self.exec_stmt(stmt, &env)? {
                Flow::Normal => {}
                Flow::Return(_) => {
                    return Err(RuntimeError::ReturnOutsideFunction { span: stmt.span });
                }
            }
        }
        Ok(())
    }

    fn exec_block(
        &self,
        block: &Block,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Flow, RuntimeError> {
        for stmt in &block.statements {
            match self.exec_stmt(stmt, env)? {
                Flow::Normal => {}
                flow @ Flow::Return(_) => return Ok(flow),
            }
        }
        Ok(Flow::Normal)
    }

    fn exec_stmt(&self, stmt: &Stmt, env: &Rc<RefCell<Environment>>) -> Result<Flow, RuntimeError> {
        match &stmt.kind {
            StmtKind::Let { name, value } => {
                let v = self.eval_expr(value, env)?;
                env.borrow_mut().define(name.clone(), v);
                Ok(Flow::Normal)
            }
            StmtKind::Fn { name, params, body } => {
                let function = Value::Function(Rc::new(Function {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                }));
                env.borrow_mut().define(name.clone(), function);
                Ok(Flow::Normal)
            }
            StmtKind::Return(value) => {
                let v = self.eval_expr(value, env)?;
                Ok(Flow::Return(v))
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.eval_expr(condition, env)?;
                if expect_bool(&cond, condition.span)? {
                    let child = Environment::child(env);
                    self.exec_block(then_branch, &child)
                } else if let Some(else_branch) = else_branch {
                    let child = Environment::child(env);
                    self.exec_block(else_branch, &child)
                } else {
                    Ok(Flow::Normal)
                }
            }
            StmtKind::Expr(expr) => {
                self.eval_expr(expr, env)?;
                Ok(Flow::Normal)
            }
        }
    }

    fn eval_expr(
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
                let v = self.eval_expr(operand, env)?;
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
                let l = self.eval_expr(left, env)?;
                let r = self.eval_expr(right, env)?;
                eval_binary(*op, l, r, expr.span)
            }
            ExprKind::Call { callee, args } => {
                let callee_value = self.eval_expr(callee, env)?;
                let mut arg_values = Vec::with_capacity(args.len());
                for arg in args {
                    arg_values.push(self.eval_expr(arg, env)?);
                }
                self.call(callee_value, arg_values, expr.span)
            }
        }
    }

    fn call(&self, callee: Value, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
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
                // Parented to *globals*, not the caller's environment:
                // there's no real closure semantics yet (see SPEC.md).
                let call_env = Environment::child(&self.globals);
                for (param, value) in function.params.iter().zip(args) {
                    call_env.borrow_mut().define(param.clone(), value);
                }
                match self.exec_block(&function.body, &call_env)? {
                    Flow::Return(v) => Ok(v),
                    Flow::Normal => Ok(Value::Unit),
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
            other => Err(RuntimeError::NotCallable {
                type_name: other.type_name(),
                span,
            }),
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

    fn run_capturing(src: &str) -> String {
        let program = aint_parser::parse_source(src).expect("should parse");
        let interpreter = Interpreter::with_output(Vec::new());
        interpreter.run(&program).expect("should run without error");
        String::from_utf8(interpreter.into_output()).expect("output should be valid utf8")
    }

    fn run_expect_err(src: &str) -> RuntimeError {
        let program = aint_parser::parse_source(src).expect("should parse");
        let interpreter = Interpreter::with_output(Vec::new());
        interpreter
            .run(&program)
            .expect_err("should produce a runtime error")
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
        // `x` from the if-block shouldn't shadow/persist for the outer
        // `x` once the block ends.
        let output = run_capturing("let x = 1\nif true { let x = 2 print(x) }\nprint(x)");
        assert_eq!(output, "2\n1\n");
    }

    #[test]
    fn function_call_and_recursion() {
        assert_eq!(
            run_capturing(
                "fn fibonacci(n) {\n\
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
        // Just confirms falling off the end of a function body doesn't
        // panic or error; nothing here actually prints the Unit result.
        assert_eq!(run_capturing("fn noop() { let x = 1 }\nnoop()"), "");
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
        let err = run_expect_err("fn add(a, b) { return a + b }\nadd(1)");
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
}
