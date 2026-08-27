mod common;

const ENUMS_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/enums.an"
));

#[test]
fn runs_enums_an() {
    let output = common::run_capturing(ENUMS_AN);
    assert_eq!(
        output,
        "\
North
East
North
true
true
true
true
false
"
    );
}
