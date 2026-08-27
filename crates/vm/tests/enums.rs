mod common;

const ENUMS_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/enums.an"
));

/// `examples/enums.an` (milestone 09) never touches `infer`/`tool` -
/// see its own top comment - so it's fully in scope for the VM:
/// enum-variant constants, equality across a user-declared enum, and
/// recursion (`turn_around`) all exercised together.
#[test]
fn runs_enums_an_identically_to_the_tree_walker() {
    let output = common::run_capturing(ENUMS_AN);
    assert_eq!(
        output,
        "North\nEast\nNorth\ntrue\ntrue\ntrue\ntrue\nfalse\n"
    );
}
