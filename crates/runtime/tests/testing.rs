//! `examples/testing.an` (milestone 15) exercised through
//! `aint_runtime::run_tests` directly — the library-level counterpart
//! to the CLI's `aint test` integration test. `common::run_capturing`
//! doesn't fit here: the example's interesting behavior is entirely
//! inside `test` blocks, which a plain `Interpreter::run` skips.

use aint_runtime::run_tests;

const TESTING_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/testing.an"
));

const STACK_SIZE: usize = 64 * 1024 * 1024;

#[test]
fn all_test_blocks_in_testing_an_pass() {
    let outcomes = std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || {
            let program = aint_parser::parse_source(TESTING_AN).expect("should parse");
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build a runtime");
            runtime.block_on(run_tests(&program))
        })
        .expect("failed to spawn a big-stack thread")
        .join()
        .expect("the big-stack thread panicked");

    assert_eq!(outcomes.len(), 3);
    for outcome in &outcomes {
        assert!(
            outcome.result.is_ok(),
            "test {:?} failed: {:?}",
            outcome.name,
            outcome.result
        );
    }
}
