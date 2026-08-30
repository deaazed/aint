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
    /// The type of an unawaited call to an `infer`-declared function.
    /// Never written as source syntax, same as `Task<T>` — computed by
    /// the type checker at the call site. Kept a distinct type from
    /// `Task<T>` rather than an alias, since this is where
    /// inference-specific metadata (model, tokens, latency, trace)
    /// attaches in later milestones. See
    /// `docs/milestones/08-first-ai-primitive/SPEC.md`.
    Inference(Box<Type>),
    /// A user-declared `enum`, compared nominally by name — the full
    /// variant list lives in the type checker's/interpreter's own
    /// registry, not here. See
    /// `docs/milestones/09-typed-structured-inference/SPEC.md`.
    Enum(String),
    /// A probability distribution over an `enum`'s variants — only
    /// ever over `Type::Enum`, enforced by the type checker, not here.
    /// See `docs/milestones/10-uncertainty/SPEC.md` for the explicit
    /// decision on what "probability" means.
    Distribution(Box<Type>),
    /// The type of an unawaited call to a `tool`-declared function.
    /// Structurally identical to `Task`/`Inference` and deliberately
    /// kept as its own type rather than reusing one of them — see
    /// `docs/milestones/11-typed-tools/SPEC.md`.
    Tool(Box<Type>),
    /// A closure's type (milestone 30): parameter types, then the
    /// return type. Only ever produced by a `fn(...) -> T { ... }`
    /// lambda expression or a bare reference to a plain, synchronous,
    /// non-`infer`/`tool` top-level `fn` — see
    /// `docs/milestones/30-closures/SPEC.md`.
    Function(Vec<Type>, Box<Type>),
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
            Type::Inference(inner) => write!(f, "Inference<{inner}>"),
            Type::Enum(name) => write!(f, "{name}"),
            Type::Distribution(inner) => write!(f, "Distribution<{inner}>"),
            Type::Tool(inner) => write!(f, "Tool<{inner}>"),
            Type::Function(params, ret) => {
                write!(f, "fn(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{param}")?;
                }
                write!(f, ") -> {ret}")
            }
        }
    }
}
