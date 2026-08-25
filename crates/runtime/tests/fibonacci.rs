use aint_parser::parse_source;
use aint_runtime::Interpreter;

const FIBONACCI_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/fibonacci.an"
));

#[test]
fn runs_fibonacci_an() {
    let program =
        parse_source(FIBONACCI_AN).expect("examples/fibonacci.an should parse without errors");
    let interpreter = Interpreter::with_output(Vec::new());
    interpreter
        .run(&program)
        .expect("examples/fibonacci.an should run without errors");

    let output = String::from_utf8(interpreter.into_output()).expect("output should be utf8");
    assert_eq!(output, "55\n");
}
