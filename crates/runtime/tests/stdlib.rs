use aint_parser::parse_source;
use aint_runtime::Interpreter;

const STDLIB_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/stdlib.an"
));

#[test]
fn runs_stdlib_an() {
    let program = parse_source(STDLIB_AN).expect("examples/stdlib.an should parse without errors");
    let interpreter = Interpreter::with_output(Vec::new());
    interpreter
        .run(&program)
        .expect("examples/stdlib.an should run without errors");

    let output = String::from_utf8(interpreter.into_output()).expect("output should be utf8");

    let total: f64 = 4.0 + 8.0 + 15.0 + 16.0 + 23.0 + 42.0;
    let expected = format!("{total}\n{}\nHELLO, AINT\ntrue\n", total.sqrt());
    assert_eq!(output, expected);
}
