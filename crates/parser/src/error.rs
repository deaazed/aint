use std::fmt;

use aint_ast::Span;
use aint_lexer::{LexError, Token};

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    Lex(LexError),
    Unexpected { expected: String, found: Token },
}

impl ParseError {
    /// The span to point a diagnostic at, regardless of which variant.
    pub fn span(&self) -> Span {
        match self {
            ParseError::Lex(err) => err.span,
            ParseError::Unexpected { found, .. } => found.span,
        }
    }
}

impl From<LexError> for ParseError {
    fn from(err: LexError) -> Self {
        ParseError::Lex(err)
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Lex(err) => write!(f, "{err}"),
            ParseError::Unexpected { expected, found } => {
                write!(
                    f,
                    "{}: expected {expected}, found {}",
                    found.span.start, found.kind
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}
