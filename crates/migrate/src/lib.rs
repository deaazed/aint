//! Rewrites an AINT program to the latest idiomatic syntax (milestone
//! 45) — the deterministic half of `aint migrate`. No AI, no network
//! call, no dependency on any model's quota: every rewrite here is a
//! pure, provably behavior-preserving AST transform, verified again
//! (not just trusted) by re-typechecking and re-running the migrated
//! program's own tests before `aint migrate` ever writes a file back
//! to disk. See `docs/milestones/45-migrate/SPEC.md`.
//!
//! Two rewrites exist today, both chosen because they're mechanically
//! safe — no understanding of intent required, just a structural match:
//!
//! - **`if cond { return a } else { return b }` → `return if cond { a }
//!   else { b }`** (adopting milestone 37's if-expression syntax).
//!   Applied bottom-up, so an `else if` chain collapses into one flat
//!   `ExprKind::If` spine for free: a nested if-return pattern is
//!   already converted to `return <if-expr>` by the time the outer
//!   pattern is checked, and `return <if-expr>` matches the same
//!   single-`Return`-statement shape a plain `return b` would.
//! - **`x == true` / `x == false` / `x != true` / `x != false`
//!   simplified to `x` / `!x` / `!x` / `x`** (adopting milestone 38's
//!   `!` operator).
//!
//! Both directions of `==`/`!=` (literal on either side) are handled.
//! Nothing here attempts the harder, semantics-requiring migrations
//! (hand-built HTML strings becoming `Node` literals, a hand-rolled
//! `replace` helper becoming `string_replace`) — those need real
//! understanding of intent, which is exactly what `aint migrate`'s
//! AI-assisted, verified-before-accepted pass is for instead.

use aint_ast::{BinaryOp, Block, Expr, ExprKind, Param, Program, Span, Stmt, StmtKind, UnaryOp};

/// One applied rewrite, for `aint migrate`'s own narration — never
/// silent, so a user can see exactly what changed and why.
#[derive(Debug, Clone)]
pub struct Migration {
    pub description: String,
    pub span: Span,
}

/// Rewrites every statement/expression in `program` to the latest
/// idiomatic syntax, returning the new program and a log of what
/// changed. Idempotent: running this again on its own output finds
/// nothing left to rewrite (see `crates/migrate`'s own tests).
pub fn migrate_deterministic(program: Program) -> (Program, Vec<Migration>) {
    let mut log = Vec::new();
    let statements = program
        .statements
        .into_iter()
        .map(|stmt| transform_stmt(stmt, &mut log))
        .collect();
    (Program { statements }, log)
}

fn transform_block(block: Block, log: &mut Vec<Migration>) -> Block {
    let statements = block
        .statements
        .into_iter()
        .map(|stmt| transform_stmt(stmt, log))
        .collect();
    Block {
        statements,
        span: block.span,
    }
}

fn transform_params(params: Vec<Param>) -> Vec<Param> {
    // Parameter types carry no syntax this migration touches; kept as
    // its own pass-through function so a future rewrite that *does*
    // need to visit types has one obvious place to add it.
    params
}

fn transform_stmt(stmt: Stmt, log: &mut Vec<Migration>) -> Stmt {
    let span = stmt.span;
    let kind = match stmt.kind {
        StmtKind::Let { name, value } => StmtKind::Let {
            name,
            value: transform_expr(value, log),
        },
        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let condition = transform_expr(condition, log);
            let then_branch = transform_block(then_branch, log);
            let else_branch = else_branch.map(|b| transform_block(b, log));

            if let Some(else_branch) = else_branch {
                if let Some(if_expr) = if_return_as_expr(&condition, &then_branch, &else_branch) {
                    log.push(Migration {
                        description:
                            "`if { return a } else { return b }` rewritten to `return if { a } else { b }`"
                                .to_string(),
                        span,
                    });
                    return Stmt::new(StmtKind::Return(Expr::new(if_expr, span)), span);
                }
                StmtKind::If {
                    condition,
                    then_branch,
                    else_branch: Some(else_branch),
                }
            } else {
                StmtKind::If {
                    condition,
                    then_branch,
                    else_branch: None,
                }
            }
        }
        StmtKind::Expr(expr) => StmtKind::Expr(transform_expr(expr, log)),
        StmtKind::Fn {
            name,
            params,
            return_type,
            body,
            is_async,
            effects,
        } => StmtKind::Fn {
            name,
            params: transform_params(params),
            return_type,
            body: transform_block(body, log),
            is_async,
            effects,
        },
        StmtKind::Return(expr) => StmtKind::Return(transform_expr(expr, log)),
        StmtKind::Import(name) => StmtKind::Import(name),
        StmtKind::ImportFile { path, alias } => StmtKind::ImportFile { path, alias },
        StmtKind::Infer {
            name,
            params,
            return_type,
            permissions,
        } => StmtKind::Infer {
            name,
            params: transform_params(params),
            return_type,
            permissions,
        },
        StmtKind::Enum { name, variants } => StmtKind::Enum { name, variants },
        StmtKind::Tool {
            name,
            params,
            return_type,
            body,
        } => StmtKind::Tool {
            name,
            params: transform_params(params),
            return_type,
            body: body.map(|b| transform_block(b, log)),
        },
        StmtKind::Test { name, body } => StmtKind::Test {
            name,
            body: transform_block(body, log),
        },
        StmtKind::Mock { function, value } => StmtKind::Mock {
            function,
            value: transform_expr(value, log),
        },
        StmtKind::Assert { condition } => StmtKind::Assert {
            condition: transform_expr(condition, log),
        },
        StmtKind::Budget {
            max_tokens,
            max_model_calls,
            max_cost,
            timeout_ms,
        } => StmtKind::Budget {
            max_tokens,
            max_model_calls,
            max_cost,
            timeout_ms,
        },
    };
    Stmt::new(kind, span)
}

/// `then_branch`/`else_branch` are assumed already recursively
/// transformed — an `else if` chain collapses for free because a
/// nested `if`/`else` inside `else_branch` was already rewritten to a
/// single `return <if-expr>` statement by the time this runs on the
/// outer `if`, which matches the same "exactly one `return`" shape a
/// plain `return b` would.
fn if_return_as_expr(
    condition: &Expr,
    then_branch: &Block,
    else_branch: &Block,
) -> Option<ExprKind> {
    let then_value = single_return(then_branch)?;
    let else_value = single_return(else_branch)?;
    Some(ExprKind::If {
        condition: Box::new(condition.clone()),
        then_value: Box::new(then_value),
        else_value: Box::new(else_value),
    })
}

fn single_return(block: &Block) -> Option<Expr> {
    match block.statements.as_slice() {
        [Stmt {
            kind: StmtKind::Return(value),
            ..
        }] => Some(value.clone()),
        _ => None,
    }
}

fn transform_expr(expr: Expr, log: &mut Vec<Migration>) -> Expr {
    let span = expr.span;
    let kind = match expr.kind {
        ExprKind::Integer(n) => ExprKind::Integer(n),
        ExprKind::Float(n) => ExprKind::Float(n),
        ExprKind::String(s) => ExprKind::String(s),
        ExprKind::Bool(b) => ExprKind::Bool(b),
        ExprKind::Identifier(name) => ExprKind::Identifier(name),
        ExprKind::Unary { op, operand } => ExprKind::Unary {
            op,
            operand: Box::new(transform_expr(*operand, log)),
        },
        ExprKind::Binary { op, left, right } => {
            let left = transform_expr(*left, log);
            let right = transform_expr(*right, log);
            transform_binary(op, left, right, span, log)
        }
        ExprKind::Call { callee, args } => ExprKind::Call {
            callee: Box::new(transform_expr(*callee, log)),
            args: args.into_iter().map(|a| transform_expr(a, log)).collect(),
        },
        ExprKind::List(elements) => ExprKind::List(
            elements
                .into_iter()
                .map(|e| transform_expr(e, log))
                .collect(),
        ),
        ExprKind::Index { object, index } => ExprKind::Index {
            object: Box::new(transform_expr(*object, log)),
            index: Box::new(transform_expr(*index, log)),
        },
        ExprKind::Await(inner) => ExprKind::Await(Box::new(transform_expr(*inner, log))),
        ExprKind::Lambda {
            params,
            return_type,
            body,
        } => ExprKind::Lambda {
            params: transform_params(params),
            return_type,
            body: transform_block(body, log),
        },
        ExprKind::If {
            condition,
            then_value,
            else_value,
        } => ExprKind::If {
            condition: Box::new(transform_expr(*condition, log)),
            then_value: Box::new(transform_expr(*then_value, log)),
            else_value: Box::new(transform_expr(*else_value, log)),
        },
        ExprKind::NodeLiteral {
            role,
            props,
            children,
        } => ExprKind::NodeLiteral {
            role,
            props: props
                .into_iter()
                .map(|(name, value)| (name, transform_expr(value, log)))
                .collect(),
            children: children
                .into_iter()
                .map(|c| transform_expr(c, log))
                .collect(),
        },
    };
    Expr::new(kind, span)
}

/// `left`/`right` are assumed already recursively transformed.
fn transform_binary(
    op: BinaryOp,
    left: Expr,
    right: Expr,
    span: Span,
    log: &mut Vec<Migration>,
) -> ExprKind {
    let left_span = left.span;
    let right_span = right.span;
    match (op, left.kind, right.kind) {
        (BinaryOp::Eq, ExprKind::Bool(true), other)
        | (BinaryOp::Eq, other, ExprKind::Bool(true)) => {
            log.push(Migration {
                description: "`x == true` simplified to `x`".to_string(),
                span,
            });
            other
        }
        (BinaryOp::Eq, ExprKind::Bool(false), other) => {
            log.push(Migration {
                description: "`false == x` simplified to `!x`".to_string(),
                span,
            });
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand: Box::new(Expr::new(other, right_span)),
            }
        }
        (BinaryOp::Eq, other, ExprKind::Bool(false)) => {
            log.push(Migration {
                description: "`x == false` simplified to `!x`".to_string(),
                span,
            });
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand: Box::new(Expr::new(other, left_span)),
            }
        }
        (BinaryOp::NotEq, ExprKind::Bool(false), other)
        | (BinaryOp::NotEq, other, ExprKind::Bool(false)) => {
            log.push(Migration {
                description: "`x != false` simplified to `x`".to_string(),
                span,
            });
            other
        }
        (BinaryOp::NotEq, ExprKind::Bool(true), other) => {
            log.push(Migration {
                description: "`true != x` simplified to `!x`".to_string(),
                span,
            });
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand: Box::new(Expr::new(other, right_span)),
            }
        }
        (BinaryOp::NotEq, other, ExprKind::Bool(true)) => {
            log.push(Migration {
                description: "`x != true` simplified to `!x`".to_string(),
                span,
            });
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand: Box::new(Expr::new(other, left_span)),
            }
        }
        (op, left_kind, right_kind) => ExprKind::Binary {
            op,
            left: Box::new(Expr::new(left_kind, left_span)),
            right: Box::new(Expr::new(right_kind, right_span)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrate(src: &str) -> (String, Vec<Migration>) {
        let program = aint_parser::parse_source(src).expect("should parse");
        let (migrated, log) = migrate_deterministic(program);
        (aint_fmt::format_program(&migrated), log)
    }

    #[test]
    fn collapses_if_return_else_return_into_a_return_if_expression() {
        let (out, log) = migrate(
            "fn sign(n: Int) -> String {\n\
             if n < 0 {\n\
             return \"negative\"\n\
             } else {\n\
             return \"positive\"\n\
             }\n\
             }",
        );
        assert_eq!(
            out,
            "fn sign(n: Int) -> String {\n    return if n < 0 { \"negative\" } else { \"positive\" }\n}\n"
        );
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn collapses_an_else_if_chain_into_one_flat_if_expression() {
        let (out, log) = migrate(
            "fn grade(score: Int) -> String {\n\
             if score < 60 {\n\
             return \"F\"\n\
             } else if score < 80 {\n\
             return \"C\"\n\
             } else {\n\
             return \"A\"\n\
             }\n\
             }",
        );
        assert_eq!(
            out,
            "fn grade(score: Int) -> String {\n    return if score < 60 { \"F\" } else if score < 80 { \"C\" } else { \"A\" }\n}\n"
        );
        assert_eq!(
            log.len(),
            2,
            "one rewrite per collapsed if, inner then outer"
        );
    }

    #[test]
    fn leaves_an_if_with_extra_statements_in_a_branch_untouched() {
        let (out, log) = migrate(
            "fn f(n: Int) -> Int {\n\
             if n < 0 {\n\
             let doubled = n * 2\n\
             return doubled\n\
             } else {\n\
             return n\n\
             }\n\
             }",
        );
        assert!(out.contains("if n < 0 {"), "should stay a statement: {out}");
        assert!(log.is_empty());
    }

    #[test]
    fn leaves_an_if_without_else_untouched() {
        let (out, log) = migrate("fn f(n: Int) -> Unit {\nif n < 0 {\nprint(n)\n}\n}");
        assert!(out.contains("if n < 0 {"));
        assert!(log.is_empty());
    }

    #[test]
    fn simplifies_equals_true_either_side() {
        let (out, log) = migrate("print(x == true)\nprint(true == x)");
        assert_eq!(out, "print(x)\nprint(x)\n");
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn simplifies_equals_false_either_side_to_not() {
        let (out, log) = migrate("print(x == false)\nprint(false == x)");
        assert_eq!(out, "print(!x)\nprint(!x)\n");
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn simplifies_not_equals_true_and_false() {
        let (out, log) = migrate("print(x != true)\nprint(x != false)");
        assert_eq!(out, "print(!x)\nprint(x)\n");
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn leaves_a_plain_boolean_comparison_untouched() {
        let (out, log) = migrate("print(x == y)");
        assert_eq!(out, "print(x == y)\n");
        assert!(log.is_empty());
    }

    #[test]
    fn rewrites_recurse_into_lambda_bodies() {
        let (out, log) = migrate(
            "let f = fn(n: Int) -> String {\n\
             if n < 0 {\nreturn \"neg\"\n} else {\nreturn \"pos\"\n}\n\
             }",
        );
        assert!(out.contains("return if n < 0"), "got: {out}");
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn migrating_output_a_second_time_is_a_no_op() {
        let src = "fn sign(n: Int) -> String {\nif n < 0 {\nreturn \"negative\"\n} else {\nreturn \"positive\"\n}\n}";
        let (once, _) = migrate(src);
        let (twice, log_twice) = migrate(&once);
        assert_eq!(once, twice);
        assert!(log_twice.is_empty());
    }

    #[test]
    fn an_already_modern_program_is_reported_as_unchanged() {
        let (out, log) = migrate("fn sign(n: Int) -> String {\nreturn if n < 0 { \"negative\" } else { \"positive\" }\n}");
        assert_eq!(
            out,
            "fn sign(n: Int) -> String {\n    return if n < 0 { \"negative\" } else { \"positive\" }\n}\n"
        );
        assert!(log.is_empty());
    }
}
