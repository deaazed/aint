use aint_ast::{ExprKind, StmtKind};
use aint_parser::parse_source;

const HELLO_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/hello.an"
));

#[test]
fn parses_hello_an() {
    let program = parse_source(HELLO_AN).expect("examples/hello.an should parse without errors");

    assert_eq!(program.statements.len(), 2, "expected two statements");

    match &program.statements[0].kind {
        StmtKind::Let { name, value } => {
            assert_eq!(name, "message");
            assert_eq!(value.kind, ExprKind::String("Hello, AINT!".into()));
        }
        other => panic!("expected the first statement to be a let, got {other:?}"),
    }

    match &program.statements[1].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Call { callee, args } => {
                assert_eq!(callee.kind, ExprKind::Identifier("print".into()));
                assert_eq!(args.len(), 1);
                assert_eq!(args[0].kind, ExprKind::Identifier("message".into()));
            }
            other => panic!("expected the second statement to be a call, got {other:?}"),
        },
        other => panic!("expected the second statement to be an expr-statement, got {other:?}"),
    }
}
