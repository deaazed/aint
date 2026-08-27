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
        /// An optional `effects [ ... ]` clause (milestone 13). `None`
        /// means untracked, not "no effects" — see
        /// `docs/milestones/13-effects/SPEC.md` for why an unannotated
        /// function is exempt from checking rather than implicitly
        /// `pure`. Not extended to `Infer`/`Tool`: their effect is
        /// intrinsic and never anything else, so there's no legal
        /// second value the clause could hold there.
        effects: Option<Vec<Effect>>,
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
    /// A signature-only declaration: `tool name(params) -> Type`, no
    /// body — structurally identical to `Infer` but kept as its own
    /// variant, not shared, since they diverge starting the next
    /// milestone each is involved in. See
    /// `docs/milestones/11-typed-tools/SPEC.md`.
    Tool {
        name: String,
        params: Vec<Param>,
        return_type: Type,
    },
    /// `test "name" { ... }` — a top-level, isolated test block. Inert
    /// during `aint run` (skipped entirely); `aint test` gives each one
    /// its own fresh `Interpreter`. See
    /// `docs/milestones/15-deterministic-ai-testing/SPEC.md`.
    Test {
        name: String,
        body: Block,
    },
    /// `mock function -> value` — configures what a declared `infer`
    /// or `tool` returns, for the enclosing `test` block only. `value`
    /// is deliberately restricted (checked by the type checker, not
    /// the parser) to literals and `EnumName_Variant` references — see
    /// SPEC.md for why this isn't a general expression.
    Mock {
        function: String,
        value: Expr,
    },
    /// `assert condition` — a general statement, not test-only syntax;
    /// see SPEC.md for how `aint run` and `aint test` handle a failed
    /// one differently without the statement itself needing to know.
    Assert {
        condition: Expr,
    },
}

/// One of the five effect words `ROADMAP.md` names. See
/// `docs/milestones/13-effects/SPEC.md` for what's actually checked
/// (`Inference`/`Tool`) versus accepted-but-vacuous
/// (`Network`/`Filesystem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    Pure,
    Inference,
    Tool,
    Network,
    Filesystem,
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
