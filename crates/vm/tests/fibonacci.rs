mod common;

const FIBONACCI_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/fibonacci.an"
));

/// Milestone 04's original acceptance bar - "fibonacci-level programs
/// running end to end" - now through the VM instead of the tree
/// walker, and on an ordinary thread stack rather than a dedicated
/// big-stack one (see `tests/common/mod.rs`).
#[test]
fn runs_fibonacci_an() {
    assert_eq!(common::run_capturing(FIBONACCI_AN), "55\n");
}
