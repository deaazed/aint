use std::fmt;

use aint_ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    UndefinedVariable {
        name: String,
        span: Span,
    },
    NotCallable {
        type_name: &'static str,
        span: Span,
    },
    ArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    TypeMismatch {
        message: String,
        span: Span,
    },
    DivisionByZero {
        span: Span,
    },
    Io {
        message: String,
        span: Span,
    },
    ReturnOutsideFunction {
        span: Span,
    },
    UnknownModule {
        name: String,
        span: Span,
    },
    IndexOutOfBounds {
        index: i64,
        len: usize,
        span: Span,
    },
    /// A [`crate::Model`] couldn't answer an `infer` call — for
    /// `MockModel`, this means nothing was configured for that function
    /// name. See `docs/milestones/08-first-ai-primitive/SPEC.md`.
    ModelError {
        message: String,
        span: Span,
    },
}

impl RuntimeError {
    /// The span to point a diagnostic at, regardless of which variant.
    pub fn span(&self) -> Span {
        match self {
            RuntimeError::UndefinedVariable { span, .. }
            | RuntimeError::NotCallable { span, .. }
            | RuntimeError::ArityMismatch { span, .. }
            | RuntimeError::TypeMismatch { span, .. }
            | RuntimeError::DivisionByZero { span }
            | RuntimeError::Io { span, .. }
            | RuntimeError::ReturnOutsideFunction { span }
            | RuntimeError::UnknownModule { span, .. }
            | RuntimeError::IndexOutOfBounds { span, .. }
            | RuntimeError::ModelError { span, .. } => *span,
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::UndefinedVariable { name, span } => {
                write!(f, "{}: undefined variable `{name}`", span.start)
            }
            RuntimeError::NotCallable { type_name, span } => {
                write!(f, "{}: {type_name} is not callable", span.start)
            }
            RuntimeError::ArityMismatch {
                name,
                expected,
                found,
                span,
            } => write!(
                f,
                "{}: `{name}` expects {expected} argument(s), found {found}",
                span.start
            ),
            RuntimeError::TypeMismatch { message, span } => write!(f, "{}: {message}", span.start),
            RuntimeError::DivisionByZero { span } => write!(f, "{}: division by zero", span.start),
            RuntimeError::Io { message, span } => write!(f, "{}: {message}", span.start),
            RuntimeError::ReturnOutsideFunction { span } => {
                write!(f, "{}: `return` outside a function", span.start)
            }
            RuntimeError::UnknownModule { name, span } => {
                write!(f, "{}: unknown module `{name}`", span.start)
            }
            RuntimeError::IndexOutOfBounds { index, len, span } => write!(
                f,
                "{}: index {index} out of bounds for a list of length {len}",
                span.start
            ),
            RuntimeError::ModelError { message, span } => {
                write!(f, "{}: model error: {message}", span.start)
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
