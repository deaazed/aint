//! The formatter's two real correctness properties, checked against
//! every shipped example file, not just hand-picked snippets:
//!
//! - **Idempotent**: `format(format(src)) == format(src)`.
//! - **AST-preserving**: re-parsing the formatted output produces the
//!   same AST (ignoring spans, which necessarily change) as parsing
//!   the original did.
//!
//! `examples/async.an` is the one shipped file with a real `//`
//! comment — it's used instead to verify `format` refuses cleanly
//! rather than silently deleting it.

use aint_ast::{BinaryOp, Block, Expr, ExprKind, Param, Program, Stmt, StmtKind};

macro_rules! example {
    ($name:expr) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/",
            $name
        ))
    };
}

const EXAMPLES_WITHOUT_COMMENTS: &[(&str, &str)] = &[
    ("enums.an", example!("enums.an")),
    ("fibonacci.an", example!("fibonacci.an")),
    ("hello.an", example!("hello.an")),
    ("security.an", example!("security.an")),
    ("showcase.an", example!("showcase.an")),
    ("stdlib.an", example!("stdlib.an")),
    ("testing.an", example!("testing.an")),
];

#[test]
fn every_example_without_a_comment_formats_idempotently_and_preserves_its_ast() {
    for (name, source) in EXAMPLES_WITHOUT_COMMENTS {
        let original = aint_parser::parse_source(source)
            .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

        let formatted_once =
            aint_fmt::format(source).unwrap_or_else(|err| panic!("{name} should format: {err}"));
        let formatted_twice = aint_fmt::format(&formatted_once)
            .unwrap_or_else(|err| panic!("re-formatting {name}'s output should succeed: {err}"));
        assert_eq!(
            formatted_once, formatted_twice,
            "{name}: formatting isn't idempotent"
        );

        let reformatted = aint_parser::parse_source(&formatted_once)
            .unwrap_or_else(|err| panic!("{name}'s formatted output should parse: {err}"));
        assert!(
            programs_eq(&original, &reformatted),
            "{name}: formatting changed the AST"
        );
    }
}

#[test]
fn async_an_is_refused_because_it_has_a_real_comment() {
    let source = example!("async.an");
    let err = aint_fmt::format(source).expect_err("should refuse a file with a comment");
    assert_eq!(err, aint_fmt::FormatError::ContainsComments);
}

// --- span-insensitive AST equality -------------------------------------
//
// `Stmt`/`Expr`'s derived `PartialEq` includes `span`, which always
// differs after reformatting (positions genuinely move) even when the
// program is otherwise identical. These walk both trees in lockstep,
// comparing everything *except* spans.

fn programs_eq(a: &Program, b: &Program) -> bool {
    a.statements.len() == b.statements.len()
        && a.statements
            .iter()
            .zip(&b.statements)
            .all(|(a, b)| stmt_eq(a, b))
}

fn block_eq(a: &Block, b: &Block) -> bool {
    a.statements.len() == b.statements.len()
        && a.statements
            .iter()
            .zip(&b.statements)
            .all(|(a, b)| stmt_eq(a, b))
}

fn params_eq(a: &[Param], b: &[Param]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| a.name == b.name && a.ty == b.ty)
}

fn stmt_eq(a: &Stmt, b: &Stmt) -> bool {
    match (&a.kind, &b.kind) {
        (
            StmtKind::Let {
                name: n1,
                value: v1,
            },
            StmtKind::Let {
                name: n2,
                value: v2,
            },
        ) => n1 == n2 && expr_eq(v1, v2),
        (
            StmtKind::If {
                condition: c1,
                then_branch: t1,
                else_branch: e1,
            },
            StmtKind::If {
                condition: c2,
                then_branch: t2,
                else_branch: e2,
            },
        ) => {
            expr_eq(c1, c2)
                && block_eq(t1, t2)
                && match (e1, e2) {
                    (Some(e1), Some(e2)) => block_eq(e1, e2),
                    (None, None) => true,
                    _ => false,
                }
        }
        (StmtKind::Expr(e1), StmtKind::Expr(e2)) => expr_eq(e1, e2),
        (
            StmtKind::Fn {
                name: n1,
                params: p1,
                return_type: r1,
                body: b1,
                is_async: a1,
                effects: e1,
            },
            StmtKind::Fn {
                name: n2,
                params: p2,
                return_type: r2,
                body: b2,
                is_async: a2,
                effects: e2,
            },
        ) => n1 == n2 && params_eq(p1, p2) && r1 == r2 && block_eq(b1, b2) && a1 == a2 && e1 == e2,
        (StmtKind::Return(e1), StmtKind::Return(e2)) => expr_eq(e1, e2),
        (StmtKind::Import(m1), StmtKind::Import(m2)) => m1 == m2,
        (
            StmtKind::ImportFile {
                path: p1,
                alias: a1,
            },
            StmtKind::ImportFile {
                path: p2,
                alias: a2,
            },
        ) => p1 == p2 && a1 == a2,
        (
            StmtKind::Infer {
                name: n1,
                params: p1,
                return_type: r1,
                permissions: perm1,
            },
            StmtKind::Infer {
                name: n2,
                params: p2,
                return_type: r2,
                permissions: perm2,
            },
        ) => n1 == n2 && params_eq(p1, p2) && r1 == r2 && perm1 == perm2,
        (
            StmtKind::Enum {
                name: n1,
                variants: v1,
            },
            StmtKind::Enum {
                name: n2,
                variants: v2,
            },
        ) => n1 == n2 && v1 == v2,
        (
            StmtKind::Tool {
                name: n1,
                params: p1,
                return_type: r1,
            },
            StmtKind::Tool {
                name: n2,
                params: p2,
                return_type: r2,
            },
        ) => n1 == n2 && params_eq(p1, p2) && r1 == r2,
        (StmtKind::Test { name: n1, body: b1 }, StmtKind::Test { name: n2, body: b2 }) => {
            n1 == n2 && block_eq(b1, b2)
        }
        (
            StmtKind::Mock {
                function: f1,
                value: v1,
            },
            StmtKind::Mock {
                function: f2,
                value: v2,
            },
        ) => f1 == f2 && expr_eq(v1, v2),
        (StmtKind::Assert { condition: c1 }, StmtKind::Assert { condition: c2 }) => expr_eq(c1, c2),
        (
            StmtKind::Budget {
                max_tokens: mt1,
                max_model_calls: mc1,
                max_cost: mcost1,
                timeout_ms: t1,
            },
            StmtKind::Budget {
                max_tokens: mt2,
                max_model_calls: mc2,
                max_cost: mcost2,
                timeout_ms: t2,
            },
        ) => mt1 == mt2 && mc1 == mc2 && mcost1 == mcost2 && t1 == t2,
        _ => false,
    }
}

fn expr_eq(a: &Expr, b: &Expr) -> bool {
    match (&a.kind, &b.kind) {
        (ExprKind::Integer(a), ExprKind::Integer(b)) => a == b,
        (ExprKind::Float(a), ExprKind::Float(b)) => a == b,
        (ExprKind::String(a), ExprKind::String(b)) => a == b,
        (ExprKind::Bool(a), ExprKind::Bool(b)) => a == b,
        (ExprKind::Identifier(a), ExprKind::Identifier(b)) => a == b,
        (
            ExprKind::Unary {
                op: op1,
                operand: o1,
            },
            ExprKind::Unary {
                op: op2,
                operand: o2,
            },
        ) => op1 == op2 && expr_eq(o1, o2),
        (
            ExprKind::Binary {
                op: op1,
                left: l1,
                right: r1,
            },
            ExprKind::Binary {
                op: op2,
                left: l2,
                right: r2,
            },
        ) => binary_op_eq(*op1, *op2) && expr_eq(l1, l2) && expr_eq(r1, r2),
        (
            ExprKind::Call {
                callee: c1,
                args: a1,
            },
            ExprKind::Call {
                callee: c2,
                args: a2,
            },
        ) => {
            expr_eq(c1, c2) && a1.len() == a2.len() && a1.iter().zip(a2).all(|(a, b)| expr_eq(a, b))
        }
        (ExprKind::List(a), ExprKind::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| expr_eq(a, b))
        }
        (
            ExprKind::Index {
                object: o1,
                index: i1,
            },
            ExprKind::Index {
                object: o2,
                index: i2,
            },
        ) => expr_eq(o1, o2) && expr_eq(i1, i2),
        (ExprKind::Await(a), ExprKind::Await(b)) => expr_eq(a, b),
        (
            ExprKind::Lambda {
                params: p1,
                return_type: r1,
                body: b1,
            },
            ExprKind::Lambda {
                params: p2,
                return_type: r2,
                body: b2,
            },
        ) => params_eq(p1, p2) && r1 == r2 && block_eq(b1, b2),
        _ => false,
    }
}

fn binary_op_eq(a: BinaryOp, b: BinaryOp) -> bool {
    a == b
}
