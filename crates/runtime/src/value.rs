use std::fmt;
use std::rc::Rc;

use aint_ast::Block;

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
            Value::Function(_) | Value::Native(_) => "Function",
            Value::Task(_) => "Task",
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
        }
    }

    /// Whether calling this defers into a [`Value::Task`] instead of
    /// running immediately. Only `time_sleep_ms` today.
    pub(crate) fn is_async(self) -> bool {
        matches!(self, NativeFunction::TimeSleepMs)
    }
}
