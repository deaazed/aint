mod common;

const STDLIB_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/stdlib.an"
));

#[test]
fn runs_stdlib_an() {
    let output = common::run_capturing(STDLIB_AN);

    let total: f64 = 4.0 + 8.0 + 15.0 + 16.0 + 23.0 + 42.0;
    let expected = format!("{total}\n{}\nHELLO, AINT\ntrue\n", total.sqrt());
    assert_eq!(output, expected);
}
