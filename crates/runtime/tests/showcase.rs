use aint_parser::parse_source;
use aint_runtime::Interpreter;

const SHOWCASE_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/showcase.an"
));

/// examples/showcase.an combines recursion (primality trial-division,
/// Collatz sequence, list traversal), both math/string stdlib modules,
/// and list indexing in one program. Every value here was independently
/// cross-checked outside the interpreter before being hardcoded (prime
/// count among the candidates, Collatz(27)'s well-known 111-step total,
/// and the trimmed string's length), not just copied from a first run.
#[test]
fn runs_showcase_an() {
    let program =
        parse_source(SHOWCASE_AN).expect("examples/showcase.an should parse without errors");
    let interpreter = Interpreter::with_output(Vec::new());
    interpreter
        .run(&program)
        .expect("examples/showcase.an should run without errors");

    let output = String::from_utf8(interpreter.into_output()).expect("output should be utf8");

    let expected = "\
11
111
false
3
4
4
2.4
3.7
-2.4
256
16
AINT can already do quite a lot
31
AINT CAN ALREADY DO QUITE A LOT
true
";
    assert_eq!(output, expected);
}
