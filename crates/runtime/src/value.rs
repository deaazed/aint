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
/// source. Just `print` for now — see
/// `docs/milestones/04-tree-walk-interpreter/SPEC.md` for why this
/// isn't a generic registry yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFunction {
    Print,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Bool(_) => "Bool",
            Value::Unit => "Unit",
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
            Value::Function(func) => write!(f, "<fn {}>", func.name),
            Value::Native(NativeFunction::Print) => write!(f, "<native fn print>"),
        }
    }
}
