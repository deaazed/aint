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
    /// A [`crate::Model`]'s response didn't match an `infer` call's
    /// declared return type — e.g. a variant name that isn't one of
    /// the target `enum`'s. Distinct from `ModelError`: the model
    /// *did* answer, the answer just doesn't conform. See
    /// `docs/milestones/09-typed-structured-inference/SPEC.md`.
    SchemaViolation {
        message: String,
        span: Span,
    },
    /// A `MockTool` (or, later, a real tool backend) couldn't answer a
    /// `tool` call — for `MockTool`, nothing was configured for that
    /// tool name. Kept distinct from `ModelError`: a different external
    /// system, named honestly. See
    /// `docs/milestones/11-typed-tools/SPEC.md`.
    ToolError {
        message: String,
        span: Span,
    },
    /// A failed `assert`. See
    /// `docs/milestones/15-deterministic-ai-testing/SPEC.md`.
    AssertionFailed {
        span: Span,
    },
    /// A `mock` value the standalone mock-value evaluator (milestone
    /// 15's `test_runner` module) doesn't know how to handle — see
    /// `docs/milestones/15-deterministic-ai-testing/SPEC.md` for
    /// exactly what's supported (literals and `EnumName_Variant`
    /// references only).
    UnsupportedMockValue {
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
            | RuntimeError::ModelError { span, .. }
            | RuntimeError::SchemaViolation { span, .. }
            | RuntimeError::ToolError { span, .. }
            | RuntimeError::AssertionFailed { span }
            | RuntimeError::UnsupportedMockValue { span, .. } => *span,
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
            RuntimeError::SchemaViolation { message, span } => {
                write!(f, "{}: schema violation: {message}", span.start)
            }
            RuntimeError::ToolError { message, span } => {
                write!(f, "{}: tool error: {message}", span.start)
            }
            RuntimeError::AssertionFailed { span } => {
                write!(f, "{}: assertion failed", span.start)
            }
            RuntimeError::UnsupportedMockValue { message, span } => {
                write!(f, "{}: unsupported mock value: {message}", span.start)
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
