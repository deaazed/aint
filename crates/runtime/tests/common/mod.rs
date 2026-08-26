//! Shared by the whole-program integration tests. `mod.rs` specifically
//! (not `common.rs`) so Cargo treats this as a helper module rather
//! than its own test binary.
//!
//! Deep AINT-level recursion (there are no loops in the language, so
//! recursion is the only iteration mechanism) needs a much bigger Rust
//! stack than the default once every eval step is async — five-ish
//! mutually recursive `async fn`s nest per level of AINT recursion, and
//! the default thread stack (especially small on Windows) overflows
//! well before anything resembling a real program does. `Interpreter`
//! holds `Rc`, so it can't be built outside and moved into a spawned
//! thread — everything from parsing to running has to happen inside
//! the closure, moving in only `Send` data (the source text).

use aint_runtime::Interpreter;

const STACK_SIZE: usize = 64 * 1024 * 1024;

/// Parses and runs `source` on a dedicated thread with a large stack,
/// returning whatever `print` wrote.
pub fn run_capturing(source: &'static str) -> String {
    run_on_big_stack(move || {
        let program = aint_parser::parse_source(source).expect("should parse");
        let interpreter = Interpreter::with_output(Vec::new());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build a runtime");
        runtime
            .block_on(interpreter.run(&program))
            .expect("should run without error");
        String::from_utf8(interpreter.into_output()).expect("output should be valid utf8")
    })
}

fn run_on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(f)
        .expect("failed to spawn a big-stack thread")
        .join()
        .expect("the big-stack thread panicked")
}
