//! Shared by the whole-program integration tests: the full
//! `AST -> AIR -> Bytecode -> AINT VM` pipeline a real `aint run --vm`
//! would go through, end to end.
//!
//! Deliberately *not* run on a dedicated big-stack thread the way
//! `aint-runtime`'s equivalent helper is — the VM's whole point is
//! that its call frames live on the heap (`Vec<Frame>`, milestone 22),
//! not the Rust call stack, so deep AINT-level recursion (Collatz(27)
//! in `examples/showcase.an`, 111 levels) should run fine on an
//! ordinary thread. If it didn't, that would itself be a bug this
//! test setup is meant to catch, not paper over with a bigger stack.

pub fn run_capturing(source: &'static str) -> String {
    let program = aint_parser::parse_source(source).expect("should parse");
    aint_typechecker::check_program(&program).expect("should type-check");
    let air = aint_ir::lower(&program).expect("should lower to AIR");
    let compiled = aint_vm::compile(&air).expect("should compile to bytecode");
    let mut vm = aint_vm::Vm::new(Vec::new());
    vm.run(&compiled).expect("should run without error");
    String::from_utf8(vm.into_output()).expect("output should be valid utf8")
}
