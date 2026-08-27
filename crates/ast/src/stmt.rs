use crate::{Expr, Span, Type};

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
        params: Vec<Param>,
        return_type: Type,
        body: Block,
        is_async: bool,
    },
    Return(Expr),
    Import(String),
    /// A signature-only declaration: `infer name(params) -> Type`, no
    /// body. The implementation is external (a model), not AINT source
    /// — see `docs/milestones/08-first-ai-primitive/SPEC.md` for why
    /// this doesn't reuse `Fn`'s shape.
    Infer {
        name: String,
        params: Vec<Param>,
        return_type: Type,
    },
    /// `enum Name { Variant1 Variant2 ... }`. Variant values aren't a
    /// separate AST node — see
    /// `docs/milestones/09-typed-structured-inference/SPEC.md` for why
    /// `EnumName_Variant` is plain `ExprKind::Identifier` syntax
    /// instead.
    Enum {
        name: String,
        variants: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
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
