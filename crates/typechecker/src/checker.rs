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
    /// `distribution_*`: five functions genuinely polymorphic over
    /// `Distribution<T>`'s `T`. Same shape as
    /// `PolymorphicListFunction`, one per `stdlib.rs` module, see
    /// `docs/milestones/10-uncertainty/SPEC.md`.
    PolymorphicDistributionFunction(DistributionOp),
    /// `option_*`: two functions genuinely polymorphic over
    /// `Option<T>`'s `T`.
    PolymorphicOptionFunction(OptionOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DistributionOp {
    Probability,
    Argmax,
    Entropy,
    Sample,
    RequireConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptionOp {
    IsSome,
    Unwrap,
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
    /// Every declared `enum`, by name, to its variant names — checked
    /// against by `validate_type` and populated before anything else,
    /// so `enum` declarations get the same forward-reference support
    /// `fn`/`infer` already have. See
    /// `docs/milestones/09-typed-structured-inference/SPEC.md`.
    enums: HashMap<String, Vec<String>>,
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
            enums: HashMap::new(),
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<(), TypeError> {
        // Enums are hoisted first, and separately, so a `fn`/`infer`
        // signature appearing earlier in the file can still reference
        // one declared later.
        for stmt in &program.statements {
            if let StmtKind::Enum { name, variants } = &stmt.kind {
                self.enums.insert(name.clone(), variants.clone());
                for variant in variants {
                    self.define(
                        format!("{name}_{variant}"),
                        Binding::Variable(Type::Enum(name.clone())),
                    );
                }
            }
        }

        // Hoist every top-level `fn`/`infer` signature before checking
        // any body, so forward references and mutual/self recursion
        // between top-level functions type-check regardless of source
        // order.
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

    /// Rejects a `Type::Enum` that doesn't name a declared enum,
    /// recursing into `List`/`Option`/`Task`/`Inference`'s inner type.
    /// Needed because `parse_type` can no longer reject an unknown name
    /// itself — see SPEC.md.
    fn validate_type(&self, ty: &Type, span: Span) -> Result<(), TypeError> {
        match ty {
            Type::Enum(name) => {
                if self.enums.contains_key(name) {
                    Ok(())
                } else {
                    Err(TypeError::UnknownType {
                        name: name.clone(),
                        span,
                    })
                }
            }
            Type::List(inner)
            | Type::Option(inner)
            | Type::Task(inner)
            | Type::Inference(inner) => self.validate_type(inner, span),
            Type::Distribution(inner) => match inner.as_ref() {
                Type::Enum(_) => self.validate_type(inner, span),
                other => Err(TypeError::Mismatch {
                    message: format!(
                        "Distribution<T> requires T to be an enum, found Distribution<{other}>"
                    ),
                    span,
                }),
            },
            Type::Int | Type::Float | Type::Bool | Type::String | Type::Unit => Ok(()),
        }
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
                for param in params {
                    self.validate_type(&param.ty, stmt.span)?;
                }
                self.validate_type(return_type, stmt.span)?;

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
                for param in params {
                    self.validate_type(&param.ty, stmt.span)?;
                }
                self.validate_type(return_type, stmt.span)?;
                Ok(())
            }
            StmtKind::Enum { name, variants } => {
                // Already registered by `check`'s enum pre-pass; this
                // only adds the one check that pass doesn't do.
                if variants.is_empty() {
                    return Err(TypeError::EmptyEnum {
                        name: name.clone(),
                        span: stmt.span,
                    });
                }
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
                if module == "distribution" {
                    for (name, op) in [
                        ("distribution_probability", DistributionOp::Probability),
                        ("distribution_argmax", DistributionOp::Argmax),
                        ("distribution_entropy", DistributionOp::Entropy),
                        ("distribution_sample", DistributionOp::Sample),
                        (
                            "distribution_require_confidence",
                            DistributionOp::RequireConfidence,
                        ),
                    ] {
                        self.define(
                            name.to_string(),
                            Binding::PolymorphicDistributionFunction(op),
                        );
                    }
                    return Ok(());
                }
                if module == "option" {
                    for (name, op) in [
                        ("option_is_some", OptionOp::IsSome),
                        ("option_unwrap", OptionOp::Unwrap),
                    ] {
                        self.define(name.to_string(), Binding::PolymorphicOptionFunction(op));
                    }
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
                Some(
                    Binding::Function(_)
                    | Binding::PolymorphicListFunction
                    | Binding::PolymorphicDistributionFunction(_)
                    | Binding::PolymorphicOptionFunction(_),
                ) => Err(TypeError::Mismatch {
                    message: format!("`{name}` is a function; call it with `{name}(...)`"),
                    span: expr.span,
                }),
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
            Some(Binding::PolymorphicDistributionFunction(op)) => {
                let op = *op;
                let name = name.clone();
                return self.check_distribution_call(&name, op, args, span);
            }
            Some(Binding::PolymorphicOptionFunction(op)) => {
                let op = *op;
                let name = name.clone();
                return self.check_option_call(&name, op, args, span);
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

    /// Type-checks a call to one of the five `distribution_*`
    /// functions — polymorphic over `Distribution<T>`'s `T`, same
    /// shape as `PolymorphicListFunction`'s handling in `check_call`.
    fn check_distribution_call(
        &mut self,
        name: &str,
        op: DistributionOp,
        args: &[Expr],
        span: Span,
    ) -> Result<Type, TypeError> {
        let expected_arity = match op {
            DistributionOp::Argmax | DistributionOp::Entropy | DistributionOp::Sample => 1,
            DistributionOp::Probability | DistributionOp::RequireConfidence => 2,
        };
        if args.len() != expected_arity {
            return Err(TypeError::ArityMismatch {
                name: name.to_string(),
                expected: expected_arity,
                found: args.len(),
                span,
            });
        }

        let dist_ty = self.check_expr(&args[0])?;
        let inner = match dist_ty {
            Type::Distribution(inner) => *inner,
            other => {
                return Err(TypeError::Mismatch {
                    message: format!("`{name}` expects a Distribution, found {other}"),
                    span: args[0].span,
                });
            }
        };

        match op {
            DistributionOp::Argmax | DistributionOp::Sample => Ok(inner),
            DistributionOp::Entropy => Ok(Type::Float),
            DistributionOp::Probability => {
                let value_ty = self.check_expr(&args[1])?;
                if value_ty != inner {
                    return Err(TypeError::Mismatch {
                        message: format!("`{name}` expects a {inner}, found {value_ty}"),
                        span: args[1].span,
                    });
                }
                Ok(Type::Float)
            }
            DistributionOp::RequireConfidence => {
                let threshold_ty = self.check_expr(&args[1])?;
                if threshold_ty != Type::Float {
                    return Err(TypeError::Mismatch {
                        message: format!(
                            "`{name}` expects a Float threshold, found {threshold_ty}"
                        ),
                        span: args[1].span,
                    });
                }
                Ok(Type::Option(Box::new(inner)))
            }
        }
    }

    /// Type-checks a call to `option_is_some`/`option_unwrap` —
    /// polymorphic over `Option<T>`'s `T`.
    fn check_option_call(
        &mut self,
        name: &str,
        op: OptionOp,
        args: &[Expr],
        span: Span,
    ) -> Result<Type, TypeError> {
        if args.len() != 1 {
            return Err(TypeError::ArityMismatch {
                name: name.to_string(),
                expected: 1,
                found: args.len(),
                span,
            });
        }
        let arg_ty = self.check_expr(&args[0])?;
        let inner = match arg_ty {
            Type::Option(inner) => *inner,
            other => {
                return Err(TypeError::Mismatch {
                    message: format!("`{name}` expects an Option, found {other}"),
                    span: args[0].span,
                });
            }
        };
        match op {
            OptionOp::IsSome => Ok(Type::Bool),
            OptionOp::Unwrap => Ok(inner),
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

    // --- enum ----------------------------------------------------------

    #[test]
    fn accepts_enum_declaration_and_variant_reference() {
        assert!(check(
            "enum Sentiment { Positive Neutral Negative }\n\
             let s = Sentiment_Positive\n\
             print(s == Sentiment_Positive)"
        )
        .is_ok());
    }

    #[test]
    fn enum_can_be_used_before_its_declaration() {
        assert!(check(
            "fn f() -> Sentiment { return Sentiment_Positive }\n\
             enum Sentiment { Positive Neutral Negative }"
        )
        .is_ok());
    }

    #[test]
    fn enum_is_usable_as_a_param_and_return_type() {
        assert!(check(
            "enum Sentiment { Positive Neutral Negative }\n\
             fn describe(s: Sentiment) -> Sentiment { return s }\n\
             print(describe(Sentiment_Positive) == Sentiment_Neutral)"
        )
        .is_ok());
    }

    #[test]
    fn infer_can_return_an_enum() {
        assert!(check(
            "enum Sentiment { Positive Neutral Negative }\n\
             infer sentiment(text: String) -> Sentiment\n\
             print(await sentiment(\"great\") == Sentiment_Positive)"
        )
        .is_ok());
    }

    #[test]
    fn rejects_unknown_type_name_as_a_return_type() {
        let err = check("fn f() -> Frobnicate { return 1 }").unwrap_err();
        assert!(matches!(err, TypeError::UnknownType { .. }));
    }

    #[test]
    fn rejects_unknown_type_name_as_a_param_type() {
        let err = check("fn f(x: Frobnicate) -> Int { return 1 }").unwrap_err();
        assert!(matches!(err, TypeError::UnknownType { .. }));
    }

    #[test]
    fn rejects_empty_enum() {
        let err = check("enum Empty { }").unwrap_err();
        assert!(matches!(err, TypeError::EmptyEnum { .. }));
    }

    #[test]
    fn rejects_comparing_two_different_enums() {
        let err = check(
            "enum Sentiment { Positive Negative }\n\
             enum Direction { North South }\n\
             print(Sentiment_Positive == Direction_North)",
        )
        .unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    // --- Distribution<T> / Option<T> ------------------------------------

    #[test]
    fn distribution_type_over_an_enum_is_accepted_as_a_return_type() {
        assert!(check(
            "enum Sentiment { Positive Neutral Negative }\n\
             infer classify(text: String) -> Distribution<Sentiment>"
        )
        .is_ok());
    }

    #[test]
    fn distribution_over_a_non_enum_is_a_type_error() {
        let err = check("fn f(d: Distribution<Int>) -> Int { return 1 }").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn distribution_argmax_and_sample_return_the_enum_type() {
        assert!(check(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             fn f(d: Distribution<Sentiment>) -> Sentiment {\n\
                 return distribution_argmax(d)\n\
             }\n\
             fn g(d: Distribution<Sentiment>) -> Sentiment {\n\
                 return distribution_sample(d)\n\
             }"
        )
        .is_ok());
    }

    #[test]
    fn distribution_entropy_returns_float_regardless_of_element_type() {
        assert!(check(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             fn f(d: Distribution<Sentiment>) -> Float { return distribution_entropy(d) }"
        )
        .is_ok());
    }

    #[test]
    fn distribution_probability_checks_the_value_type() {
        let err = check(
            "enum Sentiment { Positive Neutral Negative }\n\
             enum Direction { North South }\n\
             import distribution\n\
             fn f(d: Distribution<Sentiment>) -> Float {\n\
                 return distribution_probability(d, Direction_North)\n\
             }",
        )
        .unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn distribution_require_confidence_returns_option_of_the_enum() {
        assert!(check(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             import option\n\
             fn f(d: Distribution<Sentiment>) -> Bool {\n\
                 let result = distribution_require_confidence(d, 0.8)\n\
                 return option_is_some(result)\n\
             }"
        )
        .is_ok());
    }

    #[test]
    fn option_unwrap_returns_the_inner_type() {
        assert!(check(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             import option\n\
             fn f(d: Distribution<Sentiment>) -> Sentiment {\n\
                 return option_unwrap(distribution_require_confidence(d, 0.8))\n\
             }"
        )
        .is_ok());
    }

    #[test]
    fn option_unwrap_on_a_non_option_is_a_type_error() {
        let err = check("import option\nprint(option_unwrap(1))").unwrap_err();
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }

    #[test]
    fn distribution_functions_undefined_before_import() {
        let err = check(
            "enum Sentiment { Positive Neutral Negative }\n\
             fn f(d: Distribution<Sentiment>) -> Sentiment { return distribution_argmax(d) }",
        )
        .unwrap_err();
        assert!(matches!(err, TypeError::UndefinedFunction { .. }));
    }
}
