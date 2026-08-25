# End-to-end tests

Per-crate unit and integration tests live inside each crate
(`crates/<name>/src`, `crates/<name>/tests`), next to the code they
cover.

This directory is for whole-program tests: running actual `.an` files
under `examples/` (and fixtures added here) through the real `aint`
binary and checking output, once there's an interpreter to run them
against (milestone 04 onward).
