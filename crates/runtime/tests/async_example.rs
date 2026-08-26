mod common;

const ASYNC_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/async.an"
));

/// Confirms correctness. `time_sleep_ms_actually_suspends` (in
/// `interpreter.rs`'s own tests) is what proves this is a genuine
/// suspend/resume rather than synchronous code in async syntax; this
/// test isn't timing-sensitive, so it can run in the normal suite.
#[test]
fn runs_async_an() {
    assert_eq!(common::run_capturing(ASYNC_AN), "42\n36\ndone\n");
}
