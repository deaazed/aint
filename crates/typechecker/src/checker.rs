use std::collections::HashMap;

use aint_ast::{BinaryOp, Block, Expr, ExprKind, Program, Span, Stmt, StmtKind, Type, UnaryOp};

use crate::error::TypeError;
use crate::stdlib;

#[derive(Debug, Clone)]
enum Binding {
    Variable(Type),
    Function(FunctionSignature),
    /// `collections_length`: the one stdlib function genuinely
    /// polymorphic over `List<T>`. See `stdlib.rs`'s doc comment.
    PolymorphicListFunction,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    params: Vec<Type>,
    return_type: Type,
    mode: CallMode,
}

/// What a call-expression's type is, on top of the signature's declared
/// `return_type`: itself for a plain `fn`, `Task<return_type>` for an
/// `async fn`, `Inference<return_type>` for an `infer` declaration.
/// Replaces a plain `is_async: bool` now that there are three modes,
/// not two — see `docs/milestones/08-first-ai-primitive/SPEC.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallMode {
    Sync,
    Async,
    Infer,
}

/// Single-pass static type checker over the AST from `aint-ast`.
///
/// Scopes are a plain `Vec<HashMap<String, Binding>>` rather than the
/// interpreter's `Rc<RefCell<Environment>>`: this is a one-shot tree
/// walk, nothing here needs to outlive the call that created it. See
/// `docs/milestones/05-core-type-system/SPEC.md`.
pub struct TypeChecker {
    scopes: Vec<HashMap<String, Binding>>,
    current_return_type: Option<Type>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            current_return_type: None,
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<(), TypeError> {
        // Hoist every top-level `fn` signature before checking any
        // body, so forward references and mutual/self recursion between
        // top-level functions type-check regardless of source order.
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Fn {
                    name,
                    params,
                    return_type,
                    is_async,
                    ..
                } => {
                    let mode = if *is_async {
                        CallMode::Async
                    } else {
                        CallMode::Sync
                    };
                    self.define(
                        name.clone(),
                        Binding::Function(signature(params, return_type, mode)),
                    );
                }
                StmtKind::Infer {
                    name,
                    params,
                    return_type,
                } => {
                    self.define(
                        name.clone(),
                        Binding::Function(signature(params, return_type, CallMode::Infer)),
                    );
                }
                _ => {}
            }
        }

        for stmt in &program.statements {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String, binding: Binding) {
        self.scopes
            .last_mut()
            .expect("at least one scope")
            .insert(name, binding);
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), TypeError> {
        match &stmt.kind {
            StmtKind::Let { name, value } => {
                let ty = self.check_expr(value)?;
                self.define(name.clone(), Binding::Variable(ty));
                Ok(())
            }
            StmtKind::Fn {
                name,
                params,
                return_type,
                body,
                is_async,
            } => {
                // Redundant for a hoisted top-level function (already
                // bound above); the only place a block-nested `fn` gets
                // bound at all, so it's callable within its own block
                // the same way a `let` would be.
                let mode = if *is_async {
                    CallMode::Async
                } else {
                    CallMode::Sync
                };
                self.define(
                    name.clone(),
                    Binding::Function(signature(params, return_type, mode)),
                );

                self.push_scope();
                for param in params {
                    self.define(param.name.clone(), Binding::Variable(param.ty.clone()));
                }
                let previous_return_type = self.current_return_type.replace(return_type.clone());
                self.check_block(body)?;
                self.current_return_type = previous_return_type;
                self.pop_scope();

                if *return_type != Type::Unit && !definitely_returns(&body.statements) {
                    return Err(TypeError::MissingReturn {
                        name: name.clone(),
                        expected: return_type.clone(),
                        span: body.span,
                    });
                }
                Ok(())
            }
            StmtKind::Infer {
                name,
                params,
                return_type,
            } => {
                // Redundant for a hoisted top-level `infer` (already
                // bound above), same reasoning as `Fn`'s redundant
                // define — the only place a block-nested `infer` gets
                // bound at all. No body to check, no scope, no
                // missing-return analysis: there's nothing to walk.
                self.define(
                    name.clone(),
                    Binding::Function(signature(params, return_type, CallMode::Infer)),
                );
                Ok(())
            }
            StmtKind::Return(value) => {
                let ty = self.check_expr(value)?;
                match self.current_return_type.clone() {
                    Some(expected) if expected == ty => Ok(()),
                    Some(expected) => Err(TypeError::ReturnTypeMismatch {
                        expected,
                        found: ty,
                        span: stmt.span,
                    }),
                    None => Err(TypeError::ReturnOutsideFunction { span: stmt.span }),
                }
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Bool {
                    return Err(TypeError::Mismatch {
                        message: format!("expected Bool, found {cond_ty}"),
                        span: condition.span,
                    });
                }
                self.push_scope();
                self.check_block(then_branch)?;
                self.pop_scope();
                if let Some(else_branch) = else_branch {
                    self.push_scope();
                    self.check_block(else_branch)?;
                    self.pop_scope();
                }
                Ok(())
            }
            StmtKind::Expr(expr) => {
                self.check_expr(expr)?;
                Ok(())
            }
            StmtKind::Import(module) => {
                if module == "collections" {
                    self.define(
                        "collections_length".to_string(),
                        Binding::PolymorphicListFunction,
                    );
                    return Ok(());
                }
                match stdlib::module_functions(module) {
                    Some(functions) => {
                        for (name, sig) in functions {
                            let mode = if sig.is_async {
                                CallMode::Async
                            } else {
                                CallMode::Sync
                            };
                            self.define(
                                name.to_string(),
                                Binding::Function(FunctionSignature {
                                    params: sig.params,
                                    return_type: sig.return_type,
                                    mode,
                                }),
                            );
                        }
                        Ok(())
                    }
                    None => Err(TypeError::UnknownModule {
                        name: module.clone(),
                        span: stmt.span,
                    }),
                }
            }
        }
    }

    fn check_block(&mut self, block: &Block) -> Result<(), TypeError> {
        for stmt in &block.statements {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type, TypeError> {
        match &expr.kind {
            ExprKind::Integer(_) => Ok(Type::Int),
            ExprKind::Float(_) => Ok(Type::Float),
            ExprKind::String(_) => Ok(Type::String),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::Identifier(name) => match self.lookup(name) {
                Some(Binding::Variable(ty)) => Ok(ty.clone()),
                Some(Binding::Function(_) | Binding::PolymorphicListFunction) => {
                    Err(TypeError::Mismatch {
                        message: format!("`{name}` is a function; call it with `{name}(...)`"),
                        span: expr.span,
                    })
                }
                None => Err(TypeError::UndefinedVariable {
                    name: name.clone(),
                    span: expr.span,
                }),
            },
            ExprKind::Unary { op, operand } => {
                let ty = self.check_expr(operand)?;
                match op {
                    UnaryOp::Neg => match ty {
                        Type::Int | Type::Float => Ok(ty),
                        other => Err(TypeError::Mismatch {
                            message: format!("cannot negate a {other}"),
                            span: expr.span,
                        }),
                    },
                }
            }
            ExprKind::Binary { op, left, right } => {
                let l = self.check_expr(left)?;
                let r = self.check_expr(right)?;
                check_binary(*op, &l, &r, expr.span)
            }
            ExprKind::Call { callee, args } => self.check_call(callee, args, expr.span),
            ExprKind::List(elements) => {
                if elements.is_empty() {
                    return Err(TypeError::Mismatch {
                        message: "cannot infer the type of an empty list literal".to_string(),
                        span: expr.span,
                    });
                }
                let first_ty = self.check_expr(&elements[0])?;
                for element in &elements[1..] {
                    let ty = self.check_expr(element)?;
                    if ty != first_ty {
                        return Err(TypeError::Mismatch {
                            message: format!(
                                "list elements must all have the same type; found {first_ty} and {ty}"
                            ),
                            span: element.span,
                        });
                    }
                }
                Ok(Type::List(Box::new(first_ty)))
            }
            ExprKind::Index { object, index } => {
                let object_ty = self.check_expr(object)?;
                let index_ty = self.check_expr(index)?;
                if index_ty != Type::Int {
                    return Err(TypeError::Mismatch {
                        message: format!("list index must be Int, found {index_ty}"),
                        span: index.span,
                    });
                }
                match object_ty {
                    Type::List(elem_ty) => Ok(*elem_ty),
                    other => Err(TypeError::Mismatch {
                        message: format!("cannot index into {other}"),
                        span: object.span,
                    }),
                }
            }
            ExprKind::Await(inner) => {
                let ty = self.check_expr(inner)?;
                match ty {
                    Type::Task(inner_ty) => Ok(*inner_ty),
                    Type::Inference(inner_ty) => Ok(*inner_ty),
                    other => Err(TypeError::Mismatch {
                        message: format!("cannot await {other}; expected a Task or an Inference"),
                        span: inner.span,
                    }),
                }
            }
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr], span: Span) -> Result<Type, TypeError> {
        let name = match &callee.kind {
            ExprKind::Identifier(name) => name,
            _ => {
                return Err(TypeError::Mismatch {
                    message: "only named functions can be called".to_string(),
                    span: callee.span,
                });
            }
        };

        // `print` is a runtime builtin, not a user `fn` with a declared
        // signature - special-cased here the same way it's
        // special-cased in the interpreter (see SPEC.md).
        if name == "print" {
            if args.len() != 1 {
                return Err(TypeError::ArityMismatch {
                    name: "print".to_string(),
                    expected: 1,
                    found: args.len(),
                    span,
                });
            }
            self.check_expr(&args[0])?;
            return Ok(Type::Unit);
        }

        let sig = match self.lookup(name) {
            Some(Binding::Function(sig)) => sig.clone(),
            Some(Binding::PolymorphicListFunction) => {
                // `collections_length`, currently the only member.
                if args.len() != 1 {
                    return Err(TypeError::ArityMismatch {
                        name: name.clone(),
                        expected: 1,
                        found: args.len(),
                        span,
                    });
                }
                let arg_ty = self.check_expr(&args[0])?;
                return match arg_ty {
                    Type::List(_) => Ok(Type::Int),
                    other => Err(TypeError::Mismatch {
                        message: format!("`{name}` expects a List, found {other}"),
                        span: args[0].span,
                    }),
                };
            }
            Some(Binding::Variable(_)) => {
                return Err(TypeError::NotAFunction {
                    name: name.clone(),
                    span: callee.span,
                });
            }
            None => {
                return Err(TypeError::UndefinedFunction {
                    name: name.clone(),
                    span: callee.span,
                });
            }
        };

        if sig.params.len() != args.len() {
            return Err(TypeError::ArityMismatch {
                name: name.clone(),
                expected: sig.params.len(),
                found: args.len(),
                span,
            });
        }

        for (index, (arg, expected)) in args.iter().zip(&sig.params).enumerate() {
            let found = self.check_expr(arg)?;
            if found != *expected {
                return Err(TypeError::ArgumentTypeMismatch {
                    name: name.clone(),
                    index,
                    expected: expected.clone(),
                    found,
                    span: arg.span,
                });
            }
        }

        match sig.mode {
            CallMode::Sync => Ok(sig.return_type),
            CallMode::Async => Ok(Type::Task(Box::new(sig.return_type))),
            CallMode::Infer => Ok(Type::Inference(Box::new(sig.return_type))),
        }
    }
}

fn signature(params: &[aint_ast::Param], return_type: &Type, mode: CallMode) -> FunctionSignature {
    FunctionSignature {
        params: params.iter().map(|p| p.ty.clone()).collect(),
        return_type: return_type.clone(),
        mode,
    }
}

/// Whether every path through `statements` is guaranteed to hit a
/// `return`. An `if` only counts when it has an `else` and both
/// branches definitely return - the false path falls through otherwise.
/// No loops to consider yet (the language doesn't have any).
fn definitely_returns(statements: &[Stmt]) -> bool {
    statements.iter().any(|stmt| match &stmt.kind {
        StmtKind::Return(_) => true,
        StmtKind::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => {
            definitely_returns(&then_branch.statements)
                && definitely_returns(&else_branch.statements)
        }
        _ => false,
    })
}

fn check_binary(op: BinaryOp, left: &Type, right: &Type, span: Span) -> Result<Type, TypeError> {
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Float, Type::Float) => Ok(Type::Float),
            _ => Err(mismatch(op, left, right, span)),
        },
        BinaryOp::Less | BinaryOp::Greater => match (left, right) {
            (Type::Int, Type::Int) | (Type::Float, Type::Float) => Ok(Type::Bool),
            _ => Err(mismatch(op, left, right, span)),
        },
        BinaryOp::Eq | BinaryOp::NotEq => {
            if left == right {
                Ok(Type::Bool)
            } else {
                Err(mismatch(op, left, right, span))
            }
        }
    }
}

fn mismatch(op: BinaryOp, left: &Type, right: &Type, span: Span) -> TypeError {
    TypeError::Mismatch {
        message: format!("cannot apply `{}` to {left} and {right}", op_symbol(op)),
        span,
    }
}

fn op_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Less => "<",
        BinaryOp::Greater => ">",
    }
}

#[cfg(test)]
mod tests {
    use aint_parser::parse_source;

    use super::*;
    use crate::check_program;

    fn check(src: &str) -> Result<(), TypeError> {
        let program = parse_source(src).expect("should parse");
        check_program(&program)
    }

    #[test]
    fn accepts_a_well_typed_function_and_call() {
        assert!(check("fn add(a: Int, b: Int) -> Int { return a + b }\nprint(add(1, 2))").is_ok());
    }

    #[test]
    fn rejects_call_with_wrong_argument_types() {
        let err = check("fn add(a: Int, b: Int) -> Int { return a + b }\nadd(\"hello\", true)")
            .unwrap_err();
        assert!(matches!(
            err,
            TypeError::ArgumentTypeMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn rejects_call_with_wrong_argument_count() {
        let err = check("fn add(a: Int, b: Int) -> Int { return a + b }\nadd(1)").unwrap_err();
        assert!(matches!(err, TypeError::ArityMismatch { .. }));
    }

    #[test]
    fn rejects_return_type_mismatch() {
        let err = check("fn f() -> Int { return \"x\" }").unwrap_err();
        assert!(matches!(err, TypeError::ReturnTypeMismatch { .. }));
    }

    #[test]
    fn rejects_missing_return_on_a_non_unit_function() {
        let err = check("fn f() -> Int { let x = 1 }").unwrap_err();
        assert!(matches!(err, TypeError::MissingReturn { .. }));
    }

    #[test]
    fn unit_function_does_not_need_a_return() {
        assert!(check("fn f() -> Unit { let x = 1 }").is_ok());
    }

    #[test]
    fn if_else_where_both_branches_return_counts_as_returning() {
        assert!(check("fn f(n: Int) -> Int { if n > 0 { return 1 } else { return 0 } }").is_ok());
    }

    #[test]
    fn if_without_else_does_not_count_as_returning() {
        let err = check("fn f(n: Int) -> Int { if n > 0 { return 1 } }").unwrap_err();
        assert!(matches!(err, TypeError::MissingReturn { .. }));
    }

    #[test]
    fn rejects_return_outside_function() {
        let err = check("return 1").unwrap_err();
        assert!(matches!(err, TypeError::ReturnOutsideFunction { .. }));
    }

    #[test]
    fn rejects_non_bool_if_condition() {
        let err = check("if 1 { print(1) }").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn rejects_undefined_variable() {
        let err = check("print(missing)").unwrap_err();
        assert!(matches!(err, TypeError::UndefinedVariable { .. }));
    }

    #[test]
    fn rejects_undefined_function() {
        let err = check("missing_fn()").unwrap_err();
        assert!(matches!(err, TypeError::UndefinedFunction { .. }));
    }

    #[test]
    fn rejects_calling_a_non_function() {
        let err = check("let x = 1\nx()").unwrap_err();
        assert!(matches!(err, TypeError::NotAFunction { .. }));
    }

    #[test]
    fn rejects_equality_across_mismatched_types() {
        // Stricter than the interpreter's own runtime behavior on
        // purpose - see SPEC.md.
        let err = check("print(1 == \"x\")").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn accepts_self_recursion() {
        assert!(check(
            "fn fibonacci(n: Int) -> Int {\n\
                 if n < 2 { return n }\n\
                 return fibonacci(n - 1) + fibonacci(n - 2)\n\
             }\n\
             print(fibonacci(10))"
        )
        .is_ok());
    }

    #[test]
    fn accepts_forward_reference_between_top_level_functions() {
        // `a` calls `b`, but `b` is declared later in the file.
        assert!(
            check("fn a() -> Int { return b() }\nfn b() -> Int { return 1 }\nprint(a())").is_ok()
        );
    }

    #[test]
    fn accepts_list_and_option_type_annotations() {
        // Nothing can construct a List/Option value yet (see SPEC.md),
        // but the type syntax itself should check fine.
        assert!(check("fn f(x: List<Int>) -> Int { return 1 }").is_ok());
        assert!(check("fn g(x: Option<String>) -> Int { return 1 }").is_ok());
    }

    #[test]
    fn let_inside_if_scope_does_not_leak() {
        // Mirrors the interpreter's own block-scoping test.
        let err = check("if true { let x = 1 }\nprint(x)").unwrap_err();
        assert!(matches!(err, TypeError::UndefinedVariable { .. }));
    }

    // --- lists and indexing ------------------------------------------

    #[test]
    fn list_literal_infers_element_type() {
        assert!(check("let xs = [1, 2, 3]\nprint(xs[0])").is_ok());
    }

    #[test]
    fn empty_list_literal_is_a_type_error() {
        let err = check("let xs = []").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn mismatched_list_element_types_is_a_type_error() {
        let err = check("let xs = [1, \"x\"]").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn indexing_a_non_list_is_a_type_error() {
        let err = check("let x = 1\nprint(x[0])").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn indexing_with_a_non_int_is_a_type_error() {
        let err = check("let xs = [1, 2, 3]\nprint(xs[\"a\"])").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    // --- import / stdlib -----------------------------------------------

    #[test]
    fn stdlib_function_undefined_before_import() {
        let err = check("print(math_sqrt(4.0))").unwrap_err();
        assert!(matches!(err, TypeError::UndefinedFunction { .. }));
    }

    #[test]
    fn math_sqrt_type_checks_after_import() {
        assert!(check("import math\nprint(math_sqrt(4.0))").is_ok());
    }

    #[test]
    fn stdlib_function_still_checks_argument_types() {
        let err = check("import math\nmath_sqrt(\"x\")").unwrap_err();
        assert!(matches!(err, TypeError::ArgumentTypeMismatch { .. }));
    }

    #[test]
    fn collections_length_is_polymorphic_over_list_element_type() {
        assert!(
            check("import collections\nlet xs = [1, 2, 3]\nprint(collections_length(xs))").is_ok()
        );
        assert!(check(
            "import collections\nlet xs = [\"a\", \"b\"]\nprint(collections_length(xs))"
        )
        .is_ok());
    }

    #[test]
    fn unknown_module_is_a_positioned_error() {
        let err = check("import frobnicate").unwrap_err();
        assert!(matches!(err, TypeError::UnknownModule { .. }));
    }

    // --- async / await ---------------------------------------------------

    #[test]
    fn calling_an_async_fn_without_await_yields_a_task() {
        // If the call-expression's type were Int instead of Task<Int>,
        // this addition would type-check; it must not.
        let err = check(
            "async fn f() -> Int { return 1 }\n\
             print(f() + 1)",
        )
        .unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn awaiting_a_task_yields_the_inner_type() {
        assert!(check(
            "async fn f() -> Int { return 1 }\n\
             let x = await f()\n\
             print(x + 1)"
        )
        .is_ok());
    }

    #[test]
    fn awaiting_a_non_task_is_a_type_error() {
        let err = check("await 1").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn async_native_type_checks_as_a_task_after_import() {
        assert!(check("import time\nawait time_sleep_ms(10)").is_ok());
    }

    #[test]
    fn async_fn_still_needs_a_matching_return_type() {
        let err = check("async fn f() -> Int { return \"x\" }").unwrap_err();
        assert!(matches!(err, TypeError::ReturnTypeMismatch { .. }));
    }

    #[test]
    fn async_fn_still_needs_to_return_on_every_path() {
        let err = check("async fn f() -> Int { let x = 1 }").unwrap_err();
        assert!(matches!(err, TypeError::MissingReturn { .. }));
    }

    // --- infer -------------------------------------------------------

    #[test]
    fn calling_an_infer_fn_without_await_yields_an_inference() {
        // If the call-expression's type were Bool instead of
        // Inference<Bool>, this `if` would type-check; it must not.
        let err = check(
            "infer is_positive(text: String) -> Bool\n\
             if is_positive(\"great\") { print(1) }",
        )
        .unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn awaiting_an_infer_call_yields_the_declared_return_type() {
        assert!(check(
            "infer is_positive(text: String) -> Bool\n\
             if await is_positive(\"great\") { print(1) }"
        )
        .is_ok());
    }

    #[test]
    fn infer_call_still_checks_argument_types_and_arity() {
        let err = check(
            "infer is_positive(text: String) -> Bool\n\
             await is_positive(1)",
        )
        .unwrap_err();
        assert!(matches!(err, TypeError::ArgumentTypeMismatch { .. }));

        let err = check(
            "infer is_positive(text: String) -> Bool\n\
             await is_positive(\"a\", \"b\")",
        )
        .unwrap_err();
        assert!(matches!(err, TypeError::ArityMismatch { .. }));
    }

    #[test]
    fn infer_fn_can_be_called_before_its_declaration() {
        // Same forward-reference hoisting as top-level `fn`.
        assert!(check(
            "fn f() -> Bool { return await is_positive(\"x\") }\n\
             infer is_positive(text: String) -> Bool"
        )
        .is_ok());
    }
}
