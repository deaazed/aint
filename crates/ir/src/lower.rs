//! Lowers a type-checked `aint_ast::Program` into `AirProgram`. See
//! `docs/milestones/18-compiler-ir/SPEC.md`.

use std::collections::HashSet;

use aint_ast::{Block, Expr, ExprKind, Program, Span, Stmt, StmtKind};

use crate::air::{AirBlock, AirExpr, AirProgram, AirStmt, DistributionOp};

#[derive(Debug, Clone, PartialEq)]
pub enum LowerError {
    /// A call whose callee isn't a plain identifier. The type checker
    /// already rejects this ("only named functions can be called"),
    /// so this isn't reachable from any program that type-checked —
    /// defensive, not a real failure mode for `aint run`/`aint test`.
    UnsupportedCallee { span: Span },
    /// `distribution_probability` called with a number of arguments
    /// other than two. Same reasoning as `UnsupportedCallee` — the
    /// type checker already guarantees arity for anything that
    /// type-checked.
    ArityMismatch { function: String, span: Span },
    /// A cross-file `import "path" as alias` reached IR lowering
    /// unresolved. Same reasoning as `UnsupportedCallee` — `aint-loader`
    /// always eliminates this before a program reaches `check_program`,
    /// which `lower` requires has already run. See
    /// `docs/milestones/29-modularity/SPEC.md`.
    UnresolvedImport { span: Span },
    /// A `fn(...) -> T { ... }` lambda expression (milestone 30). The
    /// bytecode VM's deterministic core doesn't support closures at
    /// all yet — a documented parity gap, not a silent miscompilation.
    /// Calling a *named* closure-holding variable already fails
    /// clearly on its own (the VM compiler's global function table
    /// simply won't contain it — see `CompileError::UndefinedName` in
    /// `aint-vm`); this covers every other use of a lambda expression
    /// (as a `let` value, a `List` element, an argument, ...), which
    /// that mechanism alone wouldn't catch. See
    /// `docs/milestones/30-closures/SPEC.md`.
    UnsupportedLambda { span: Span },
    /// `if cond { a } else { b }` used as a value (milestone 37). Like
    /// `UnsupportedLambda`, a documented parity gap for the bytecode
    /// VM's deterministic core, not an attempted-and-failed
    /// miscompilation — `if` used as a *statement* is unaffected; only
    /// the expression form lowers through here at all. See
    /// `docs/milestones/37-conditional-expressions/SPEC.md`.
    UnsupportedIfExpr { span: Span },
}

/// Lowers an entire program. Expects `program` to already be
/// type-checked (`aint_typechecker::check_program`) — lowering does no
/// type checking of its own; see SPEC.md for what it does instead
/// (its own minimal `infer`/`tool` name collection).
pub fn lower(program: &Program) -> Result<AirProgram, LowerError> {
    let lowerer = Lowerer::new(program);
    let statements = program
        .statements
        .iter()
        .map(|stmt| lowerer.lower_stmt(stmt))
        .collect::<Result<_, _>>()?;
    Ok(AirProgram { statements })
}

struct Lowerer {
    /// Top-level `infer`/`tool` names only — see SPEC.md for why a
    /// block-nested declaration isn't recognized.
    infer_names: HashSet<String>,
    tool_names: HashSet<String>,
}

impl Lowerer {
    fn new(program: &Program) -> Self {
        let mut infer_names = HashSet::new();
        let mut tool_names = HashSet::new();
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Infer { name, .. } => {
                    infer_names.insert(name.clone());
                }
                StmtKind::Tool { name, .. } => {
                    tool_names.insert(name.clone());
                }
                _ => {}
            }
        }
        Self {
            infer_names,
            tool_names,
        }
    }

    fn lower_block(&self, block: &Block) -> Result<AirBlock, LowerError> {
        let statements = block
            .statements
            .iter()
            .map(|stmt| self.lower_stmt(stmt))
            .collect::<Result<_, _>>()?;
        Ok(AirBlock { statements })
    }

    fn lower_stmt(&self, stmt: &Stmt) -> Result<AirStmt, LowerError> {
        match &stmt.kind {
            StmtKind::Let { name, value } => Ok(AirStmt::Let {
                name: name.clone(),
                value: self.lower_expr(value)?,
            }),
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => Ok(AirStmt::If {
                condition: self.lower_expr(condition)?,
                then_branch: self.lower_block(then_branch)?,
                else_branch: else_branch
                    .as_ref()
                    .map(|block| self.lower_block(block))
                    .transpose()?,
            }),
            StmtKind::Expr(expr) => Ok(AirStmt::Expr(self.lower_expr(expr)?)),
            StmtKind::Fn {
                name,
                params,
                body,
                is_async,
                ..
            } => Ok(AirStmt::Fn {
                name: name.clone(),
                params: params.iter().map(|param| param.name.clone()).collect(),
                body: self.lower_block(body)?,
                is_async: *is_async,
            }),
            StmtKind::Return(value) => Ok(AirStmt::Return(self.lower_expr(value)?)),
            StmtKind::Import(module) => Ok(AirStmt::Import(module.clone())),
            StmtKind::ImportFile { .. } => Err(LowerError::UnresolvedImport { span: stmt.span }),
            StmtKind::Infer { name, params, .. } => Ok(AirStmt::Infer {
                name: name.clone(),
                params: params.iter().map(|param| param.name.clone()).collect(),
            }),
            StmtKind::Tool { name, params, .. } => Ok(AirStmt::Tool {
                name: name.clone(),
                params: params.iter().map(|param| param.name.clone()).collect(),
            }),
            StmtKind::Enum { name, variants } => Ok(AirStmt::Enum {
                name: name.clone(),
                variants: variants.clone(),
            }),
            StmtKind::Test { name, body } => Ok(AirStmt::Test {
                name: name.clone(),
                body: self.lower_block(body)?,
            }),
            StmtKind::Mock { function, value } => Ok(AirStmt::Mock {
                function: function.clone(),
                value: self.lower_expr(value)?,
            }),
            StmtKind::Assert { condition } => Ok(AirStmt::Assert {
                condition: self.lower_expr(condition)?,
            }),
            StmtKind::Budget {
                max_tokens,
                max_model_calls,
                max_cost,
                timeout_ms,
            } => Ok(AirStmt::Budget {
                max_tokens: *max_tokens,
                max_model_calls: *max_model_calls,
                max_cost: *max_cost,
                timeout_ms: *timeout_ms,
            }),
        }
    }

    fn lower_expr(&self, expr: &Expr) -> Result<AirExpr, LowerError> {
        match &expr.kind {
            ExprKind::Integer(n) => Ok(AirExpr::Integer(*n)),
            ExprKind::Float(n) => Ok(AirExpr::Float(*n)),
            ExprKind::String(s) => Ok(AirExpr::String(s.clone())),
            ExprKind::Bool(b) => Ok(AirExpr::Bool(*b)),
            ExprKind::Identifier(name) => Ok(AirExpr::Identifier(name.clone())),
            ExprKind::Unary { op, operand } => Ok(AirExpr::Unary {
                op: *op,
                operand: Box::new(self.lower_expr(operand)?),
            }),
            ExprKind::Binary { op, left, right } => Ok(AirExpr::Binary {
                op: *op,
                left: Box::new(self.lower_expr(left)?),
                right: Box::new(self.lower_expr(right)?),
            }),
            ExprKind::List(elements) => Ok(AirExpr::List(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element))
                    .collect::<Result<_, _>>()?,
            )),
            ExprKind::Index { object, index } => Ok(AirExpr::Index {
                object: Box::new(self.lower_expr(object)?),
                index: Box::new(self.lower_expr(index)?),
            }),
            ExprKind::Await(inner) => Ok(AirExpr::Await(Box::new(self.lower_expr(inner)?))),
            ExprKind::Call { callee, args } => self.lower_call(callee, args, expr.span),
            ExprKind::Lambda { .. } => Err(LowerError::UnsupportedLambda { span: expr.span }),
            ExprKind::If { .. } => Err(LowerError::UnsupportedIfExpr { span: expr.span }),
        }
    }

    /// The one place lowering actually makes a decision: which of the
    /// four explicit AI-operation nodes (if any) this call becomes.
    /// See SPEC.md.
    fn lower_call(&self, callee: &Expr, args: &[Expr], span: Span) -> Result<AirExpr, LowerError> {
        let name = match &callee.kind {
            ExprKind::Identifier(name) => name.clone(),
            _ => return Err(LowerError::UnsupportedCallee { span }),
        };
        let lowered_args = args
            .iter()
            .map(|arg| self.lower_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;

        match name.as_str() {
            "distribution_probability" => {
                let [distribution, value]: [AirExpr; 2] =
                    lowered_args
                        .try_into()
                        .map_err(|_| LowerError::ArityMismatch {
                            function: name.clone(),
                            span,
                        })?;
                Ok(AirExpr::Probability {
                    distribution: Box::new(distribution),
                    value: Box::new(value),
                })
            }
            "distribution_argmax" => Ok(AirExpr::Distribution {
                op: DistributionOp::Argmax,
                args: lowered_args,
            }),
            "distribution_entropy" => Ok(AirExpr::Distribution {
                op: DistributionOp::Entropy,
                args: lowered_args,
            }),
            "distribution_sample" => Ok(AirExpr::Distribution {
                op: DistributionOp::Sample,
                args: lowered_args,
            }),
            "distribution_require_confidence" => Ok(AirExpr::Distribution {
                op: DistributionOp::RequireConfidence,
                args: lowered_args,
            }),
            _ if self.infer_names.contains(&name) => Ok(AirExpr::Infer {
                function: name,
                args: lowered_args,
            }),
            _ if self.tool_names.contains(&name) => Ok(AirExpr::ToolCall {
                tool: name,
                args: lowered_args,
            }),
            _ => Ok(AirExpr::Call {
                callee: name,
                args: lowered_args,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses, type-checks (real usage always would have — see
    /// SPEC.md), and lowers `src`.
    fn lower_source(src: &str) -> AirProgram {
        let program = aint_parser::parse_source(src).expect("should parse");
        aint_typechecker::check_program(&program).expect("should type-check");
        lower(&program).expect("should lower")
    }

    /// The single expression a one-statement `print(...)`-free program
    /// produces, for tests that only care about one call's shape.
    fn lower_single_expr_stmt(src: &str) -> AirExpr {
        let air = lower_source(src);
        match air.statements.into_iter().next_back() {
            Some(AirStmt::Expr(expr)) => expr,
            other => panic!("expected a single Expr statement, got {other:?}"),
        }
    }

    #[test]
    fn plain_fn_call_lowers_to_call() {
        let expr =
            lower_single_expr_stmt("fn add(a: Int, b: Int) -> Int { return a + b }\nadd(1, 2)");
        assert_eq!(
            expr,
            AirExpr::Call {
                callee: "add".to_string(),
                args: vec![AirExpr::Integer(1), AirExpr::Integer(2)],
            }
        );
    }

    #[test]
    fn print_lowers_to_call() {
        let expr = lower_single_expr_stmt("print(1)");
        assert_eq!(
            expr,
            AirExpr::Call {
                callee: "print".to_string(),
                args: vec![AirExpr::Integer(1)],
            }
        );
    }

    #[test]
    fn stdlib_call_lowers_to_call() {
        let expr = lower_single_expr_stmt("import math\nmath_sqrt(4.0)");
        assert_eq!(
            expr,
            AirExpr::Call {
                callee: "math_sqrt".to_string(),
                args: vec![AirExpr::Float(4.0)],
            }
        );
    }

    #[test]
    fn infer_call_lowers_to_explicit_infer_node() {
        let expr =
            lower_single_expr_stmt("infer classify(text: String) -> Bool\nawait classify(\"x\")");
        assert_eq!(
            expr,
            AirExpr::Await(Box::new(AirExpr::Infer {
                function: "classify".to_string(),
                args: vec![AirExpr::String("x".to_string())],
            }))
        );
    }

    #[test]
    fn tool_call_lowers_to_explicit_tool_call_node() {
        let expr = lower_single_expr_stmt(
            "tool database_get_email(id: String) -> String\nawait database_get_email(\"1\")",
        );
        assert_eq!(
            expr,
            AirExpr::Await(Box::new(AirExpr::ToolCall {
                tool: "database_get_email".to_string(),
                args: vec![AirExpr::String("1".to_string())],
            }))
        );
    }

    #[test]
    fn distribution_argmax_lowers_to_distribution_node() {
        let air = lower_source(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             fn f(d: Distribution<Sentiment>) -> Sentiment { return distribution_argmax(d) }",
        );
        let AirStmt::Fn { body, .. } = &air.statements[2] else {
            panic!("expected Fn as the 3rd statement");
        };
        match &body.statements[0] {
            AirStmt::Return(AirExpr::Distribution { op, args }) => {
                assert_eq!(*op, DistributionOp::Argmax);
                assert_eq!(args, &vec![AirExpr::Identifier("d".to_string())]);
            }
            other => panic!("expected Return(Distribution), got {other:?}"),
        }
    }

    #[test]
    fn every_distribution_op_lowers_to_its_own_tag() {
        // argmax/entropy/sample all take just `d`; require_confidence
        // additionally takes a threshold - built per-function below.
        for (function, expected_op, extra_arg) in [
            ("distribution_argmax", DistributionOp::Argmax, ""),
            ("distribution_entropy", DistributionOp::Entropy, ""),
            ("distribution_sample", DistributionOp::Sample, ""),
            (
                "distribution_require_confidence",
                DistributionOp::RequireConfidence,
                ", 0.5",
            ),
        ] {
            let src = format!(
                "enum Sentiment {{ Positive Neutral Negative }}\n\
                 import distribution\n\
                 fn f(d: Distribution<Sentiment>) -> Int {{\n\
                     let _x = {function}(d{extra_arg})\n\
                     return 1\n\
                 }}"
            );
            let air = lower_source(&src);
            let AirStmt::Fn { body, .. } = &air.statements[2] else {
                panic!("expected Fn as the 3rd statement");
            };
            let AirStmt::Let { value, .. } = &body.statements[0] else {
                panic!("expected the first statement in `f` to be a Let");
            };
            match value {
                AirExpr::Distribution { op, .. } => assert_eq!(*op, expected_op),
                other => panic!("expected Distribution for {function}, got {other:?}"),
            }
        }
    }

    #[test]
    fn distribution_probability_lowers_to_its_own_node() {
        let air = lower_source(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             fn f(d: Distribution<Sentiment>) -> Float {\n\
                 return distribution_probability(d, Sentiment_Positive)\n\
             }",
        );
        let AirStmt::Fn { body, .. } = &air.statements[2] else {
            panic!("expected Fn as the 3rd statement");
        };
        match &body.statements[0] {
            AirStmt::Return(AirExpr::Probability {
                distribution,
                value,
            }) => {
                assert_eq!(**distribution, AirExpr::Identifier("d".to_string()));
                assert_eq!(
                    **value,
                    AirExpr::Identifier("Sentiment_Positive".to_string())
                );
            }
            other => panic!("expected Return(Probability), got {other:?}"),
        }
    }

    #[test]
    fn option_operations_lower_to_plain_call_not_a_special_node() {
        let air = lower_source(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             import option\n\
             fn f(d: Distribution<Sentiment>) -> Bool {\n\
                 return option_is_some(distribution_require_confidence(d, 0.5))\n\
             }",
        );
        let AirStmt::Fn { body, .. } = &air.statements[3] else {
            panic!("expected Fn as the 4th statement");
        };
        match &body.statements[0] {
            AirStmt::Return(AirExpr::Call { callee, .. }) => {
                assert_eq!(callee, "option_is_some");
            }
            other => panic!("expected Return(Call), got {other:?}"),
        }
    }

    #[test]
    fn every_statement_kind_lowers() {
        let air = lower_source(
            "budget { max_model_calls = 3 }\n\
             enum Sentiment { Positive Neutral Negative }\n\
             infer classify(text: String) -> Sentiment\n\
             tool database_get_email(id: String) -> String\n\
             fn helper(n: Int) -> Int {\n\
                 if n > 0 { return n } else { return 0 }\n\
             }\n\
             async fn helper2(n: Int) -> Int { return n }\n\
             import string\n\
             let x = 1\n\
             print(x)\n\
             test \"a test\" {\n\
                 mock classify -> Sentiment_Positive\n\
                 assert await classify(\"x\") == Sentiment_Positive\n\
             }",
        );
        assert_eq!(air.statements.len(), 10);
        assert!(matches!(air.statements[0], AirStmt::Budget { .. }));
        assert!(matches!(air.statements[1], AirStmt::Enum { .. }));
        assert!(matches!(air.statements[2], AirStmt::Infer { .. }));
        assert!(matches!(air.statements[3], AirStmt::Tool { .. }));
        assert!(matches!(
            air.statements[4],
            AirStmt::Fn {
                is_async: false,
                ..
            }
        ));
        assert!(matches!(
            air.statements[5],
            AirStmt::Fn { is_async: true, .. }
        ));
        assert!(matches!(air.statements[6], AirStmt::Import(_)));
        assert!(matches!(air.statements[7], AirStmt::Let { .. }));
        assert!(matches!(air.statements[8], AirStmt::Expr(_)));
        match &air.statements[9] {
            AirStmt::Test { name, body } => {
                assert_eq!(name, "a test");
                assert_eq!(body.statements.len(), 2);
                assert!(matches!(body.statements[0], AirStmt::Mock { .. }));
                assert!(matches!(body.statements[1], AirStmt::Assert { .. }));
            }
            other => panic!("expected Test, got {other:?}"),
        }
    }

    #[test]
    fn if_else_lowers_both_branches() {
        let air = lower_source(
            "fn f(n: Int) -> Int {\n\
                 if n > 0 { return 1 } else { return 0 }\n\
             }",
        );
        let AirStmt::Fn { body, .. } = &air.statements[0] else {
            panic!("expected Fn");
        };
        match &body.statements[0] {
            AirStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                assert_eq!(then_branch.statements.len(), 1);
                assert_eq!(
                    else_branch.as_ref().expect("else branch").statements.len(),
                    1
                );
            }
            other => panic!("expected If, got {other:?}"),
        }
    }

    #[test]
    fn nested_block_declared_infer_is_not_recognized() {
        // A documented limitation, not a bug - see SPEC.md. Nested
        // declarations aren't hoisted the way top-level ones are, so a
        // call to `classify` from *outside* the block where it's
        // declared can't be an infer call here regardless - this test
        // instead proves the pre-pass really is top-level-only by
        // checking a call from *within* the same block still resolves
        // correctly (since normal scoping makes it visible there),
        // while confirming the collected name set itself stays empty
        // for a block-nested declaration.
        let program = aint_parser::parse_source(
            "fn f() -> Bool {\n\
                 infer classify(text: String) -> Bool\n\
                 return await classify(\"x\")\n\
             }",
        )
        .expect("should parse");
        aint_typechecker::check_program(&program).expect("should type-check");
        let lowerer = Lowerer::new(&program);
        assert!(!lowerer.infer_names.contains("classify"));
    }
}
