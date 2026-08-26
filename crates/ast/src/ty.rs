use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Unit,
    List(Box<Type>),
    Option(Box<Type>),
    /// The type of an unawaited call to an `async fn`. Never written as
    /// source syntax — `parse_type` doesn't recognize it — only ever
    /// computed by the type checker at the call site of an async
    /// function. See `docs/milestones/07-async-concurrency/SPEC.md`.
    Task(Box<Type>),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::String => write!(f, "String"),
            Type::Unit => write!(f, "Unit"),
            Type::List(inner) => write!(f, "List<{inner}>"),
            Type::Option(inner) => write!(f, "Option<{inner}>"),
            Type::Task(inner) => write!(f, "Task<{inner}>"),
        }
    }
}
