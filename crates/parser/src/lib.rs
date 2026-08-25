//! Recursive-descent parser that turns a token stream from `aint-lexer`
//! into the AST defined in `aint-ast`.
//!
//! See `docs/milestones/03-parser-ast/SPEC.md` for exact scope, and
//! `docs/ARCHITECTURE.md` for how this crate fits into the pipeline.

mod error;
mod parser;

pub use error::ParseError;
pub use parser::{parse_source, Parser};
