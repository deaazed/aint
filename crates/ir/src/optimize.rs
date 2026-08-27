//! Deduplicates repeated `infer`/`tool`/`Distribution<T>` operations
//! within a block — the one optimization this milestone actually
//! builds, out of the seven `ROADMAP.md` names. See
//! `docs/milestones/19-optimization/SPEC.md` for which four aren't,
//! and why each is blocked on a specific, named prerequisite rather
//! than just unstarted.

use std::collections::HashMap;

use crate::air::{AirBlock, AirExpr, AirProgram, AirStmt};

/// What `optimize` actually did, for anything that wants to report or
/// assert on it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizationStats {
    /// How many redundant calls were rewritten into a reference to an
    /// earlier identical one.
    pub eliminated: usize,
}

/// Rewrites `program` in place (functionally — returns a new
/// `AirProgram`), deduplicating repeated AI operations block by block.
/// See SPEC.md for the exact soundness argument and what counts as
/// "the identical call."
pub fn optimize(program: &AirProgram) -> (AirProgram, OptimizationStats) {
    let mut stats = OptimizationStats::default();
    let statements = optimize_statements(&program.statements, &mut stats);
    (AirProgram { statements }, stats)
}

fn optimize_block(block: &AirBlock, stats: &mut OptimizationStats) -> AirBlock {
    AirBlock {
        statements: optimize_statements(&block.statements, stats),
    }
}

/// Optimizes one block's statement list — the unit of caching. Every
/// call into this function starts a fresh cache: an `if`'s `then` and
/// `else` branches, a function body, a test body, and the top level
/// are all independent, since at most one of any two different blocks
/// ever actually runs together.
fn optimize_statements(statements: &[AirStmt], stats: &mut OptimizationStats) -> Vec<AirStmt> {
    let mut cache: HashMap<String, String> = HashMap::new();
    let mut next_temp = 0usize;
    let mut result = Vec::with_capacity(statements.len());

    for stmt in statements {
        let stmt = recurse_into_nested_blocks(stmt, stats);
        match stmt {
            AirStmt::Let { name, value } => {
                if let Some(key) = cache_key(&value) {
                    if let Some(existing) = cache.get(&key) {
                        stats.eliminated += 1;
                        result.push(AirStmt::Let {
                            name,
                            value: AirExpr::Identifier(existing.clone()),
                        });
                        continue;
                    }
                    cache.insert(key, name.clone());
                }
                result.push(AirStmt::Let { name, value });
            }
            AirStmt::Expr(value) => {
                if let Some(key) = cache_key(&value) {
                    if let Some(existing) = cache.get(&key) {
                        stats.eliminated += 1;
                        result.push(AirStmt::Expr(AirExpr::Identifier(existing.clone())));
                        continue;
                    }
                    // No name to reference yet - hoist into a
                    // synthesized `let` so a later duplicate has
                    // something to point at. The call still happens,
                    // in the same position; it's just named now.
                    let temp = format!("__air_cse_{next_temp}");
                    next_temp += 1;
                    cache.insert(key, temp.clone());
                    result.push(AirStmt::Let { name: temp, value });
                    continue;
                }
                result.push(AirStmt::Expr(value));
            }
            other => result.push(other),
        }
    }

    result
}

/// Applies `optimize_statements` to every nested block a statement may
/// carry, leaving everything else about the statement untouched.
fn recurse_into_nested_blocks(stmt: &AirStmt, stats: &mut OptimizationStats) -> AirStmt {
    match stmt {
        AirStmt::If {
            condition,
            then_branch,
            else_branch,
        } => AirStmt::If {
            condition: condition.clone(),
            then_branch: optimize_block(then_branch, stats),
            else_branch: else_branch
                .as_ref()
                .map(|block| optimize_block(block, stats)),
        },
        AirStmt::Fn {
            name,
            params,
            body,
            is_async,
        } => AirStmt::Fn {
            name: name.clone(),
            params: params.clone(),
            body: optimize_block(body, stats),
            is_async: *is_async,
        },
        AirStmt::Test { name, body } => AirStmt::Test {
            name: name.clone(),
            body: optimize_block(body, stats),
        },
        other => other.clone(),
    }
}

/// A cache key for `expr`, if it's a call this pass is willing to
/// treat as cacheable — `None` for anything else, conservatively.
/// Only `Infer`/`ToolCall`/`Distribution`/`Probability` (optionally
/// `Await`-wrapped) with arguments that are each a literal or a bare
/// identifier qualify; anything with a nested call as an argument is
/// never considered, since nothing here attempts to prove that nested
/// call has no side effects.
fn cache_key(expr: &AirExpr) -> Option<String> {
    match expr {
        AirExpr::Await(inner) => cache_key(inner).map(|key| format!("await({key})")),
        AirExpr::Infer { function, args } => {
            stable_args_key(args).map(|args| format!("infer:{function}({args})"))
        }
        AirExpr::ToolCall { tool, args } => {
            stable_args_key(args).map(|args| format!("tool:{tool}({args})"))
        }
        AirExpr::Distribution { op, args } => {
            stable_args_key(args).map(|args| format!("dist:{op:?}({args})"))
        }
        AirExpr::Probability {
            distribution,
            value,
        } => {
            let distribution = stable_scalar_key(distribution)?;
            let value = stable_scalar_key(value)?;
            Some(format!("prob({distribution},{value})"))
        }
        _ => None,
    }
}

fn stable_args_key(args: &[AirExpr]) -> Option<String> {
    args.iter()
        .map(stable_scalar_key)
        .collect::<Option<Vec<_>>>()
        .map(|keys| keys.join(","))
}

/// A key for one argument — only literals and bare identifiers are
/// considered stable enough to compare across two call sites within
/// the same block. Safe specifically because AINT has no mutation or
/// reassignment anywhere: a bound name can't have changed value
/// between two points in the same block. See SPEC.md.
fn stable_scalar_key(expr: &AirExpr) -> Option<String> {
    match expr {
        AirExpr::Integer(n) => Some(format!("i{n}")),
        AirExpr::Float(n) => Some(format!("f{n}")),
        AirExpr::String(s) => Some(format!("s{s:?}")),
        AirExpr::Bool(b) => Some(format!("b{b}")),
        AirExpr::Identifier(name) => Some(format!("id_{name}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower_and_optimize(src: &str) -> (AirProgram, OptimizationStats) {
        let program = aint_parser::parse_source(src).expect("should parse");
        aint_typechecker::check_program(&program).expect("should type-check");
        let air = crate::lower(&program).expect("should lower");
        optimize(&air)
    }

    #[test]
    fn duplicate_calls_with_identical_literal_args_are_deduplicated() {
        let (air, stats) = lower_and_optimize(
            "infer classify(text: String) -> Bool\n\
             fn f() -> Bool {\n\
                 let a = await classify(\"great\")\n\
                 let b = await classify(\"great\")\n\
                 return a == b\n\
             }",
        );
        assert_eq!(stats.eliminated, 1);
        let AirStmt::Fn { body, .. } = &air.statements[1] else {
            panic!("expected Fn");
        };
        match &body.statements[1] {
            AirStmt::Let { name, value } => {
                assert_eq!(name, "b");
                assert_eq!(*value, AirExpr::Identifier("a".to_string()));
            }
            other => panic!("expected Let, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_calls_with_identical_identifier_args_are_deduplicated() {
        // Sound specifically because AINT has no reassignment - `text`
        // cannot have changed value between the two calls.
        let (_, stats) = lower_and_optimize(
            "infer classify(text: String) -> Bool\n\
             fn f(text: String) -> Bool {\n\
                 let a = await classify(text)\n\
                 let b = await classify(text)\n\
                 return a == b\n\
             }",
        );
        assert_eq!(stats.eliminated, 1);
    }

    #[test]
    fn calls_with_different_arguments_both_survive() {
        let (air, stats) = lower_and_optimize(
            "infer classify(text: String) -> Bool\n\
             fn f() -> Bool {\n\
                 let a = await classify(\"great\")\n\
                 let b = await classify(\"terrible\")\n\
                 return a == b\n\
             }",
        );
        assert_eq!(stats.eliminated, 0);
        let AirStmt::Fn { body, .. } = &air.statements[1] else {
            panic!("expected Fn");
        };
        for stmt in &body.statements[..2] {
            match stmt {
                AirStmt::Let { value, .. } => {
                    assert!(matches!(value, AirExpr::Await(_)));
                }
                other => panic!("expected Let, got {other:?}"),
            }
        }
    }

    #[test]
    fn calls_in_different_if_branches_are_not_shared() {
        let (_, stats) = lower_and_optimize(
            "infer classify(text: String) -> Bool\n\
             fn f(flag: Bool) -> Bool {\n\
                 if flag {\n\
                     return await classify(\"great\")\n\
                 } else {\n\
                     return await classify(\"great\")\n\
                 }\n\
             }",
        );
        // Each branch is its own block with its own cache - neither
        // ever runs alongside the other, so nothing is eliminated.
        assert_eq!(stats.eliminated, 0);
    }

    #[test]
    fn calls_with_a_nested_call_as_an_argument_are_never_deduplicated() {
        let (_, stats) = lower_and_optimize(
            "infer classify(text: String) -> Bool\n\
             fn identity(text: String) -> String { return text }\n\
             fn f() -> Bool {\n\
                 let a = await classify(identity(\"great\"))\n\
                 let b = await classify(identity(\"great\"))\n\
                 return a == b\n\
             }",
        );
        assert_eq!(stats.eliminated, 0);
    }

    #[test]
    fn a_bare_expression_statement_is_hoisted_into_a_let_on_first_occurrence() {
        let (air, stats) = lower_and_optimize(
            "tool database_get_email(id: String) -> String\n\
             fn f() -> Unit {\n\
                 await database_get_email(\"1\")\n\
                 await database_get_email(\"1\")\n\
             }",
        );
        assert_eq!(stats.eliminated, 1);
        let AirStmt::Fn { body, .. } = &air.statements[1] else {
            panic!("expected Fn");
        };
        assert_eq!(body.statements.len(), 2);
        match &body.statements[0] {
            AirStmt::Let { name, value } => {
                assert_eq!(name, "__air_cse_0");
                assert!(matches!(value, AirExpr::Await(_)));
            }
            other => panic!("expected the first call hoisted into a Let, got {other:?}"),
        }
        match &body.statements[1] {
            AirStmt::Expr(AirExpr::Identifier(name)) => assert_eq!(name, "__air_cse_0"),
            other => panic!("expected Expr(Identifier), got {other:?}"),
        }
    }

    #[test]
    fn tool_calls_are_deduplicated_the_same_way() {
        let (_, stats) = lower_and_optimize(
            "tool database_get_email(id: String) -> String\n\
             fn f() -> Bool {\n\
                 let a = await database_get_email(\"1\")\n\
                 let b = await database_get_email(\"1\")\n\
                 return a == b\n\
             }",
        );
        assert_eq!(stats.eliminated, 1);
    }

    #[test]
    fn distribution_operations_are_deduplicated() {
        let (_, stats) = lower_and_optimize(
            "enum Sentiment { Positive Neutral Negative }\n\
             import distribution\n\
             fn f(d: Distribution<Sentiment>) -> Bool {\n\
                 let a = distribution_argmax(d)\n\
                 let b = distribution_argmax(d)\n\
                 return a == b\n\
             }",
        );
        assert_eq!(stats.eliminated, 1);
    }

    #[test]
    fn top_level_program_is_unaffected_when_there_is_nothing_to_deduplicate() {
        let (air, stats) = lower_and_optimize("print(1)\nprint(2)");
        assert_eq!(stats.eliminated, 0);
        assert_eq!(air.statements.len(), 2);
    }
}
