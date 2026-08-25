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
}

#[derive(Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Block,
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
            NativeFunction::CollectionsLength => "collections_length",
        }
    }
}
