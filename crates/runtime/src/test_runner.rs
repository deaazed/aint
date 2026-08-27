//! The `aint test` execution model: each `test` block gets its own
//! fresh `Interpreter`, configured from that block's own `mock`
//! statements. See
//! `docs/milestones/15-deterministic-ai-testing/SPEC.md`.

use std::collections::HashMap;
use std::io;

use aint_ast::{Block, Expr, ExprKind, Program, Stmt, StmtKind};

use crate::error::RuntimeError;
use crate::interpreter::Interpreter;
use crate::model::MockModel;
use crate::tool::MockTool;
use crate::value::Value;

/// One `test` block's result.
pub struct TestOutcome {
    pub name: String,
    pub result: Result<(), RuntimeError>,
}

/// Which backing store a `mock` statement's target belongs to,
/// determined once from the program's top-level `infer`/`tool`
/// declarations — the type checker already guarantees every `mock` in
/// a well-typed program targets one of these, so this is a lookup, not
/// a fresh validation.
enum MockTarget {
    Infer,
    Tool,
}

/// Runs every top-level `test` block in `program`, each in its own
/// fresh `Interpreter`. Every non-`Test` top-level statement
/// (`fn`/`infer`/`tool`/`enum`/...) is re-executed into each test's
/// interpreter before that test's own body runs — real, deliberate
/// redundancy in exchange for total isolation between tests; see
/// SPEC.md.
pub async fn run_tests(program: &Program) -> Vec<TestOutcome> {
    let enum_variants = collect_enum_variants(program);
    let mock_targets = collect_mock_targets(program);
    let non_test_statements: Vec<Stmt> = program
        .statements
        .iter()
        .filter(|stmt| !matches!(stmt.kind, StmtKind::Test { .. }))
        .cloned()
        .collect();

    let mut outcomes = Vec::new();
    for stmt in &program.statements {
        if let StmtKind::Test { name, body } = &stmt.kind {
            let result =
                run_one_test(&non_test_statements, body, &enum_variants, &mock_targets).await;
            outcomes.push(TestOutcome {
                name: name.clone(),
                result,
            });
        }
    }
    outcomes
}

async fn run_one_test(
    non_test_statements: &[Stmt],
    body: &Block,
    enum_variants: &HashMap<String, Value>,
    mock_targets: &HashMap<String, MockTarget>,
) -> Result<(), RuntimeError> {
    let mut model = MockModel::new();
    let mut tools = MockTool::new();
    for stmt in &body.statements {
        if let StmtKind::Mock { function, value } = &stmt.kind {
            let mock_value = eval_mock_value(value, enum_variants)?;
            match mock_targets.get(function) {
                Some(MockTarget::Infer) => model = model.mock(function.clone(), mock_value),
                Some(MockTarget::Tool) => tools = tools.mock(function.clone(), mock_value),
                // Already rejected by the type checker for any program
                // that went through it - a defensive fallback, not a
                // reachable path for `aint test`.
                None => {
                    return Err(RuntimeError::UnsupportedMockValue {
                        message: format!("`{function}` is not a declared `infer` or `tool`"),
                        span: stmt.span,
                    });
                }
            }
        }
    }

    let interpreter = Interpreter::with_output_model_and_tools(io::stdout(), model, tools);
    interpreter.run_statements(non_test_statements).await?;
    interpreter.run_statements(&body.statements).await
}

fn collect_enum_variants(program: &Program) -> HashMap<String, Value> {
    let mut variants = HashMap::new();
    for stmt in &program.statements {
        if let StmtKind::Enum {
            name,
            variants: names,
        } = &stmt.kind
        {
            for variant in names {
                variants.insert(
                    format!("{name}_{variant}"),
                    Value::Enum(name.clone(), variant.clone()),
                );
            }
        }
    }
    variants
}

fn collect_mock_targets(program: &Program) -> HashMap<String, MockTarget> {
    let mut targets = HashMap::new();
    for stmt in &program.statements {
        match &stmt.kind {
            StmtKind::Infer { name, .. } => {
                targets.insert(name.clone(), MockTarget::Infer);
            }
            StmtKind::Tool { name, .. } => {
                targets.insert(name.clone(), MockTarget::Tool);
            }
            _ => {}
        }
    }
    targets
}

/// Evaluates a `mock` statement's value with no running interpreter —
/// deliberately restricted to literals and `EnumName_Variant`
/// references. See SPEC.md for why this is a small standalone
/// evaluator instead of `Interpreter::eval_expr`.
fn eval_mock_value(
    expr: &Expr,
    enum_variants: &HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match &expr.kind {
        ExprKind::Integer(n) => Ok(Value::Int(*n)),
        ExprKind::Float(n) => Ok(Value::Float(*n)),
        ExprKind::String(s) => Ok(Value::String(s.clone())),
        ExprKind::Bool(b) => Ok(Value::Bool(*b)),
        ExprKind::Identifier(name) => {
            enum_variants
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError::UnsupportedMockValue {
                    message: format!("`{name}` is not a known enum variant"),
                    span: expr.span,
                })
        }
        _ => Err(RuntimeError::UnsupportedMockValue {
            message: "mock values must be a literal or an EnumName_Variant reference".to_string(),
            span: expr.span,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `run_tests` is async and touches `Interpreter` (not `Send`, see
    /// milestone 07/08's established pattern), so every test here runs
    /// on a dedicated big-stack thread with its own Tokio runtime, same
    /// as `interpreter.rs`'s own tests.
    fn run_on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("failed to spawn a big-stack thread")
            .join()
            .expect("the big-stack thread panicked")
    }

    fn run(src: &'static str) -> Vec<(String, bool)> {
        run_on_big_stack(move || {
            let program = aint_parser::parse_source(src).expect("should parse");
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a test runtime");
            runtime
                .block_on(run_tests(&program))
                .into_iter()
                .map(|outcome| (outcome.name, outcome.result.is_ok()))
                .collect()
        })
    }

    #[test]
    fn a_passing_test_mocking_an_infer_function() {
        let outcomes = run("enum Sentiment { Positive Neutral Negative }\n\
             infer classify(text: String) -> Sentiment\n\
             test \"positive review\" {\n\
                 mock classify -> Sentiment_Positive\n\
                 assert await classify(\"great\") == Sentiment_Positive\n\
             }");
        assert_eq!(outcomes, vec![("positive review".to_string(), true)]);
    }

    #[test]
    fn a_failing_assertion_fails_just_that_test() {
        let outcomes = run("enum Sentiment { Positive Neutral Negative }\n\
             infer classify(text: String) -> Sentiment\n\
             test \"wrong expectation\" {\n\
                 mock classify -> Sentiment_Positive\n\
                 assert await classify(\"great\") == Sentiment_Negative\n\
             }");
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].0, "wrong expectation");
        assert!(!outcomes[0].1);
    }

    #[test]
    fn mocking_a_tool() {
        let outcomes = run("tool database_get_email(id: String) -> String\n\
             test \"looks up email\" {\n\
                 mock database_get_email -> \"a@b.com\"\n\
                 assert await database_get_email(\"1\") == \"a@b.com\"\n\
             }");
        assert_eq!(outcomes, vec![("looks up email".to_string(), true)]);
    }

    #[test]
    fn tests_are_isolated_from_each_other() {
        // The second test never mocks `classify` - if state leaked
        // from the first test, this would return the first test's
        // answer instead of failing with a clear "unconfigured" error.
        let outcomes = run(
            "enum Sentiment { Positive Neutral Negative }\n\
             infer classify(text: String) -> Sentiment\n\
             test \"a\" { mock classify -> Sentiment_Positive\n assert await classify(\"x\") == Sentiment_Positive }\n\
             test \"b\" { assert await classify(\"x\") == Sentiment_Positive }",
        );
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0], ("a".to_string(), true));
        assert_eq!(outcomes[1].0, "b");
        assert!(!outcomes[1].1);
    }

    #[test]
    fn a_test_can_call_a_helper_fn_declared_elsewhere_in_the_file() {
        let outcomes = run("enum Sentiment { Positive Neutral Negative }\n\
             infer classify(text: String) -> Sentiment\n\
             fn is_positive(text: String) -> Bool {\n\
                 return await classify(text) == Sentiment_Positive\n\
             }\n\
             test \"uses a helper\" {\n\
                 mock classify -> Sentiment_Positive\n\
                 assert is_positive(\"great\")\n\
             }");
        assert_eq!(outcomes, vec![("uses a helper".to_string(), true)]);
    }

    #[test]
    fn multiple_tests_all_run_and_report_independently() {
        let outcomes = run("test \"one\" { assert true }\n\
             test \"two\" { assert false }\n\
             test \"three\" { assert 1 == 1 }");
        assert_eq!(
            outcomes,
            vec![
                ("one".to_string(), true),
                ("two".to_string(), false),
                ("three".to_string(), true),
            ]
        );
    }
}
