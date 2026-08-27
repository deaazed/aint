mod common;

const SHOWCASE_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/showcase.an"
));

/// The exact same program and expected output as
/// `aint-runtime/tests/showcase.rs` (milestone 04's acceptance
/// example) - proof the VM produces identical results to the
/// tree-walking interpreter on the same non-AI program: recursion
/// (trial-division primality, a 111-step Collatz sequence), both
/// math/string stdlib modules, and list indexing.
#[test]
fn runs_showcase_an_identically_to_the_tree_walker() {
    let output = common::run_capturing(SHOWCASE_AN);

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
