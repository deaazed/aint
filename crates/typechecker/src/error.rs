use std::fmt;

use aint_ast::{Span, Type};

#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    UndefinedVariable {
        name: String,
        span: Span,
    },
    UndefinedFunction {
        name: String,
        span: Span,
    },
    NotAFunction {
        name: String,
        span: Span,
    },
    ArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    ArgumentTypeMismatch {
        name: String,
        index: usize,
        expected: Type,
        found: Type,
        span: Span,
    },
    Mismatch {
        message: String,
        span: Span,
    },
    ReturnTypeMismatch {
        expected: Type,
        found: Type,
        span: Span,
    },
    ReturnOutsideFunction {
        span: Span,
    },
    MissingReturn {
        name: String,
        expected: Type,
        span: Span,
    },
    UnknownModule {
        name: String,
        span: Span,
    },
}

impl TypeError {
    /// The span to point a diagnostic at, regardless of which variant.
    pub fn span(&self) -> Span {
        match self {
            TypeError::UndefinedVariable { span, .. }
            | TypeError::UndefinedFunction { span, .. }
            | TypeError::NotAFunction { span, .. }
            | TypeError::ArityMismatch { span, .. }
            | TypeError::ArgumentTypeMismatch { span, .. }
            | TypeError::Mismatch { span, .. }
            | TypeError::ReturnTypeMismatch { span, .. }
            | TypeError::ReturnOutsideFunction { span }
            | TypeError::MissingReturn { span, .. }
            | TypeError::UnknownModule { span, .. } => *span,
        }
    }
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::UndefinedVariable { name, span } => {
                write!(f, "{}: undefined variable `{name}`", span.start)
            }
            TypeError::UndefinedFunction { name, span } => {
                write!(f, "{}: undefined function `{name}`", span.start)
            }
            TypeError::NotAFunction { name, span } => {
                write!(f, "{}: `{name}` is not a function", span.start)
            }
            TypeError::ArityMismatch {
                name,
                expected,
                found,
                span,
            } => write!(
                f,
                "{}: `{name}` expects {expected} argument(s), found {found}",
                span.start
            ),
            TypeError::ArgumentTypeMismatch {
                name,
                index,
                expected,
                found,
                span,
            } => write!(
                f,
                "{}: `{name}` argument {} expects {expected}, found {found}",
                span.start,
                index + 1
            ),
            TypeError::Mismatch { message, span } => write!(f, "{}: {message}", span.start),
            TypeError::ReturnTypeMismatch {
                expected,
                found,
                span,
            } => write!(
                f,
                "{}: expected return type {expected}, found {found}",
                span.start
            ),
            TypeError::ReturnOutsideFunction { span } => {
                write!(f, "{}: `return` outside a function", span.start)
            }
            TypeError::MissingReturn {
                name,
                expected,
                span,
            } => write!(
                f,
                "{}: `{name}` is declared to return {expected} but doesn't return on every path",
                span.start
            ),
            TypeError::UnknownModule { name, span } => {
                write!(f, "{}: unknown module `{name}`", span.start)
            }
        }
    }
}

impl std::error::Error for TypeError {}
