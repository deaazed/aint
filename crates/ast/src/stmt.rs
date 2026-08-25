use crate::{Expr, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Let {
        name: String,
        value: Expr,
    },
    If {
        condition: Expr,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    Expr(Expr),
    Fn {
        name: String,
        params: Vec<String>,
        body: Block,
    },
    Return(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

/// A full AINT source file: a flat sequence of top-level statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}
