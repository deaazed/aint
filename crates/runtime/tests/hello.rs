use aint_parser::parse_source;
use aint_runtime::Interpreter;

const HELLO_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/hello.an"
));

#[test]
fn runs_hello_an() {
    let program = parse_source(HELLO_AN).expect("examples/hello.an should parse without errors");
    let interpreter = Interpreter::with_output(Vec::new());
    interpreter
        .run(&program)
        .expect("examples/hello.an should run without errors");

    let output = String::from_utf8(interpreter.into_output()).expect("output should be utf8");
    assert_eq!(output, "Hello, AINT!\n");
}
