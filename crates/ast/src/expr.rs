use crate::{Block, Param, Span, Type};

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Identifier(String),
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    List(Vec<Expr>),
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Await(Box<Expr>),
    /// `fn(params) -> ReturnType { body }` in expression position
    /// (milestone 30) — an anonymous, always-synchronous function
    /// value. No `effects` clause: a lambda is untracked, exactly like
    /// a top-level `fn` with none. See
    /// `docs/milestones/30-closures/SPEC.md`.
    Lambda {
        params: Vec<Param>,
        return_type: Type,
        body: Block,
    },
    /// `if condition { then_value } else { else_value }` used as a
    /// value (milestone 37) — deliberately distinct from `StmtKind::If`
    /// rather than reusing it: each branch is exactly one expression,
    /// not a `Block` of statements, and `else` is required, not
    /// optional, since both branches must produce a value. `else if`
    /// is parser-level sugar with no AST footprint of its own — the
    /// parser recurses directly into another `ExprKind::If` for
    /// `else_value` instead of requiring `{ }` around it. See
    /// `docs/milestones/37-conditional-expressions/SPEC.md`.
    If {
        condition: Box<Expr>,
        then_value: Box<Expr>,
        else_value: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Less,
    Greater,
}
