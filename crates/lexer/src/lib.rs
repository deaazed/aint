//! Tokenizer for AINT source files (`.an`).
//!
//! Turns raw UTF-8 source text into a token stream for the parser. See
//! `docs/milestones/02-lexer/SPEC.md` for exact scope, and
//! `docs/ARCHITECTURE.md` for how this crate fits into the pipeline.

mod error;
mod lexer;
mod token;

pub use error::{LexError, LexErrorKind};
pub use lexer::{tokenize, Lexer};
pub use token::{Token, TokenKind};
