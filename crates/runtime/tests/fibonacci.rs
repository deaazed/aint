mod common;

const FIBONACCI_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/fibonacci.an"
));

#[test]
fn runs_fibonacci_an() {
    assert_eq!(common::run_capturing(FIBONACCI_AN), "55\n");
}
