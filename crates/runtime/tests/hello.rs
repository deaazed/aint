mod common;

const HELLO_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/hello.an"
));

#[test]
fn runs_hello_an() {
    assert_eq!(common::run_capturing(HELLO_AN), "Hello, AINT!\n");
}
