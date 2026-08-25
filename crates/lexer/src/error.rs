use std::fmt;

use aint_ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum LexErrorKind {
    UnterminatedString,
    UnknownCharacter(char),
    MalformedNumber(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

impl LexError {
    pub fn new(kind: LexErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            LexErrorKind::UnterminatedString => {
                write!(f, "{}: unterminated string literal", self.span.start)
            }
            LexErrorKind::UnknownCharacter(c) => {
                write!(f, "{}: unexpected character '{c}'", self.span.start)
            }
            LexErrorKind::MalformedNumber(text) => {
                write!(f, "{}: malformed number literal `{text}`", self.span.start)
            }
        }
    }
}

impl std::error::Error for LexError {}
