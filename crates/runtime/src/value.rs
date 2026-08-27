use std::fmt;
use std::rc::Rc;

use aint_ast::{Block, Type};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Unit,
    List(Vec<Value>),
    Function(Rc<Function>),
    Native(NativeFunction),
    /// A deferred call to an `async fn` or an async native — captured,
    /// not run. Nothing happens until this is `await`-ed; see
    /// `docs/milestones/07-async-concurrency/SPEC.md`.
    Task(Rc<Task>),
    /// An `infer`-declared function itself — callable, but has no body
    /// to run; calling it produces a [`Value::Inference`] instead. See
    /// `docs/milestones/08-first-ai-primitive/SPEC.md`.
    InferenceFn(Rc<InferenceFn>),
    /// A deferred call to an `infer`-declared function — captured, not
    /// run, exactly like [`Value::Task`]. `await` sends it to the
    /// interpreter's `Model`.
    Inference(Rc<PendingInference>),
    /// A value of a user-declared `enum`: `(enum name, variant name)`.
    /// Compared with plain `PartialEq`, exactly like every other value
    /// — see `docs/milestones/09-typed-structured-inference/SPEC.md`.
    Enum(String, String),
    /// A probability distribution over an enum's variants:
    /// `(enum name, [(variant name, probability), ...])`. Only ever
    /// produced by a validated `infer` response — see
    /// `docs/milestones/10-uncertainty/SPEC.md` for the structural
    /// guarantees the runtime enforces (and doesn't) on the
    /// probabilities.
    Distribution(String, Vec<(String, f64)>),
    /// `Option<T>`'s first real value — previously type-only. Reuses
    /// Rust's own `Option` for the Some/None shape rather than
    /// inventing parallel variants.
    Option(Option<Box<Value>>),
    /// A `tool`-declared function itself — callable, but has no body to
    /// run; calling it produces a [`Value::ToolCall`] instead.
    /// Structurally identical to [`Value::InferenceFn`], kept separate
    /// — see `docs/milestones/11-typed-tools/SPEC.md`.
    ToolFn(Rc<ToolFn>),
    /// A deferred call to a `tool`-declared function — captured, not
    /// run, exactly like [`Value::Inference`]. `await` sends it to the
    /// interpreter's `MockTool`.
    ToolCall(Rc<PendingToolCall>),
}

#[derive(Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Block,
    pub is_async: bool,
}

/// The deferred computation behind a [`Value::Task`]: either a call to
/// a user-defined `async fn`, or a call to one of the (currently one)
/// async native functions.
#[derive(Debug, PartialEq)]
pub enum Task {
    Function {
        function: Rc<Function>,
        args: Vec<Value>,
    },
    Native {
        native: NativeFunction,
        args: Vec<Value>,
    },
}

impl Task {
    fn name(&self) -> &str {
        match self {
            Task::Function { function, .. } => &function.name,
            Task::Native { native, .. } => native.name(),
        }
    }
}

/// An `infer`-declared function: name, parameter names, and its
/// declared return type. The return type wasn't needed before
/// milestone 09 (the type checker already validated the call site) —
/// it's needed now to validate the model's response against it at
/// `await` time. See
/// `docs/milestones/09-typed-structured-inference/SPEC.md`.
#[derive(Debug, PartialEq)]
pub struct InferenceFn {
    pub name: String,
    pub params: Vec<String>,
    pub return_type: Type,
}

/// The deferred computation behind a [`Value::Inference`]: which
/// `infer` function, its already-evaluated argument values, and its
/// declared return type — all captured at call time, exactly like
/// `args`, so `await` doesn't need to re-look-up the function.
#[derive(Debug, PartialEq)]
pub struct PendingInference {
    pub function: String,
    pub args: Vec<Value>,
    pub return_type: Type,
}

/// A `tool`-declared function: name, parameter names, and declared
/// return type — the runtime counterpart of [`InferenceFn`].
#[derive(Debug, PartialEq)]
pub struct ToolFn {
    pub name: String,
    pub params: Vec<String>,
    pub return_type: Type,
}

/// The deferred computation behind a [`Value::ToolCall`] — the runtime
/// counterpart of [`PendingInference`].
#[derive(Debug, PartialEq)]
pub struct PendingToolCall {
    pub tool: String,
    pub args: Vec<Value>,
    pub return_type: Type,
}

/// A function implemented in the runtime itself rather than in AINT
/// source: `print` (always available) plus the stdlib functions gated
/// behind `import` (milestone 06). See
/// `docs/milestones/06-modules-stdlib/SPEC.md` for why this stays a
/// flat enum rather than a generic registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFunction {
    Print,
    MathSqrt,
    MathPow,
    MathFloor,
    MathCeil,
    MathRound,
    MathAbs,
    MathMin,
    MathMax,
    StringLength,
    StringToUpper,
    StringToLower,
    StringTrim,
    StringContains,
    StringConcat,
    TimeNowSeconds,
    /// The one genuinely asynchronous native function (milestone 07),
    /// chosen as the simplest possible thing that actually suspends —
    /// see SPEC.md for why one real async primitive matters here.
    TimeSleepMs,
    CollectionsLength,
    DistributionProbability,
    DistributionArgmax,
    DistributionEntropy,
    /// Genuinely random (milestone 10) — see
    /// `docs/milestones/10-uncertainty/SPEC.md` for why that's fine.
    DistributionSample,
    DistributionRequireConfidence,
    OptionIsSome,
    OptionUnwrap,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Bool(_) => "Bool",
            Value::Unit => "Unit",
            Value::List(_) => "List",
            Value::Function(_) | Value::Native(_) | Value::InferenceFn(_) | Value::ToolFn(_) => {
                "Function"
            }
            Value::Task(_) => "Task",
            Value::Inference(_) => "Inference",
            Value::ToolCall(_) => "Tool",
            // The specific enum name is dynamic and this method
            // returns `&'static str`; callers that need it match on
            // `Value::Enum` directly instead (see e.g.
            // `Interpreter::validate_inference_result`).
            Value::Enum(_, _) => "Enum",
            Value::Distribution(_, _) => "Distribution",
            Value::Option(_) => "Option",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Unit => write!(f, "()"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Function(func) => write!(f, "<fn {}>", func.name),
            Value::Native(native) => write!(f, "<native fn {}>", native.name()),
            Value::Task(task) => write!(f, "<task {}>", task.name()),
            Value::InferenceFn(infer_fn) => write!(f, "<infer fn {}>", infer_fn.name),
            Value::Inference(pending) => write!(f, "<inference {}>", pending.function),
            Value::Enum(_, variant) => write!(f, "{variant}"),
            Value::Distribution(name, entries) => {
                write!(f, "Distribution<{name}>{{")?;
                for (i, (variant, probability)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{variant}: {probability}")?;
                }
                write!(f, "}}")
            }
            Value::Option(Some(inner)) => write!(f, "Some({inner})"),
            Value::Option(None) => write!(f, "None"),
            Value::ToolFn(tool_fn) => write!(f, "<tool fn {}>", tool_fn.name),
            Value::ToolCall(pending) => write!(f, "<tool call {}>", pending.tool),
        }
    }
}

impl NativeFunction {
    pub(crate) fn name(self) -> &'static str {
        match self {
            NativeFunction::Print => "print",
            NativeFunction::MathSqrt => "math_sqrt",
            NativeFunction::MathPow => "math_pow",
            NativeFunction::MathFloor => "math_floor",
            NativeFunction::MathCeil => "math_ceil",
            NativeFunction::MathRound => "math_round",
            NativeFunction::MathAbs => "math_abs",
            NativeFunction::MathMin => "math_min",
            NativeFunction::MathMax => "math_max",
            NativeFunction::StringLength => "string_length",
            NativeFunction::StringToUpper => "string_to_upper",
            NativeFunction::StringToLower => "string_to_lower",
            NativeFunction::StringTrim => "string_trim",
            NativeFunction::StringContains => "string_contains",
            NativeFunction::StringConcat => "string_concat",
            NativeFunction::TimeNowSeconds => "time_now_seconds",
            NativeFunction::TimeSleepMs => "time_sleep_ms",
            NativeFunction::CollectionsLength => "collections_length",
            NativeFunction::DistributionProbability => "distribution_probability",
            NativeFunction::DistributionArgmax => "distribution_argmax",
            NativeFunction::DistributionEntropy => "distribution_entropy",
            NativeFunction::DistributionSample => "distribution_sample",
            NativeFunction::DistributionRequireConfidence => "distribution_require_confidence",
            NativeFunction::OptionIsSome => "option_is_some",
            NativeFunction::OptionUnwrap => "option_unwrap",
        }
    }

    /// Whether calling this defers into a [`Value::Task`] instead of
    /// running immediately. Only `time_sleep_ms` today.
    pub(crate) fn is_async(self) -> bool {
        matches!(self, NativeFunction::TimeSleepMs)
    }
}
