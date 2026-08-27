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
    /// A type name that isn't a built-in and doesn't name any declared
    /// `enum` — see
    /// `docs/milestones/09-typed-structured-inference/SPEC.md` for why
    /// this is caught here instead of at parse time.
    UnknownType {
        name: String,
        span: Span,
    },
    /// `enum Name { }` — a variant list with nothing in it, so no value
    /// of this type could ever exist.
    EmptyEnum {
        name: String,
        span: Span,
    },
    /// A call from inside a function with a declared `effects` clause,
    /// to a callee whose own effects aren't provably a subset of the
    /// caller's — including a callee with no declared effects at all
    /// (untracked isn't the same as harmless). See
    /// `docs/milestones/13-effects/SPEC.md`.
    EffectMismatch {
        name: String,
        span: Span,
    },
    /// A second `budget` block in the same program. See
    /// `docs/milestones/17-ai-resource-management/SPEC.md`.
    DuplicateBudget {
        span: Span,
    },
    /// A name inside an `infer`'s `permissions [...]` clause that
    /// doesn't refer to a declared `tool` — a typo, an `infer`, a
    /// plain `fn`, or nothing at all. See
    /// `docs/milestones/20-security-model/SPEC.md`.
    UnknownTool {
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
            | TypeError::UnknownModule { span, .. }
            | TypeError::UnknownType { span, .. }
            | TypeError::EmptyEnum { span, .. }
            | TypeError::EffectMismatch { span, .. }
            | TypeError::DuplicateBudget { span }
            | TypeError::UnknownTool { span, .. } => *span,
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
            TypeError::UnknownType { name, span } => {
                write!(f, "{}: unknown type `{name}`", span.start)
            }
            TypeError::EmptyEnum { name, span } => {
                write!(f, "{}: `enum {name}` has no variants", span.start)
            }
            TypeError::EffectMismatch { name, span } => write!(
                f,
                "{}: `{name}` cannot be called here; its effects aren't declared compatible with the caller's `effects` clause",
                span.start
            ),
            TypeError::DuplicateBudget { span } => {
                write!(f, "{}: a program can only have one `budget` block", span.start)
            }
            TypeError::UnknownTool { name, span } => {
                write!(
                    f,
                    "{}: `permissions` names `{name}`, which isn't a declared `tool`",
                    span.start
                )
            }
        }
    }
}

impl std::error::Error for TypeError {}
