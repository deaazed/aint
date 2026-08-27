//! AIR's own type set — a parallel representation to `aint-ast`'s
//! `Stmt`/`Expr`, not a reuse of them. See
//! `docs/milestones/18-compiler-ir/SPEC.md` for why.

use aint_ast::{BinaryOp, UnaryOp};

/// A full lowered program: a flat sequence of top-level statements,
/// mirroring `aint_ast::Program`.
#[derive(Debug, Clone, PartialEq)]
pub struct AirProgram {
    pub statements: Vec<AirStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AirBlock {
    pub statements: Vec<AirStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AirStmt {
    Let {
        name: String,
        value: AirExpr,
    },
    If {
        condition: AirExpr,
        then_branch: AirBlock,
        else_branch: Option<AirBlock>,
    },
    Expr(AirExpr),
    /// Parameter *names* only — not types or an `effects` clause, both
    /// already fully spent by type-checking time. See SPEC.md.
    Fn {
        name: String,
        params: Vec<String>,
        body: AirBlock,
        is_async: bool,
    },
    Return(AirExpr),
    Import(String),
    /// The declaration itself, not a call site — a call to this name
    /// lowers to `AirExpr::Infer` wherever it's actually invoked.
    Infer {
        name: String,
        params: Vec<String>,
    },
    /// The declaration itself — see `Infer`. A call site lowers to
    /// `AirExpr::ToolCall`.
    Tool {
        name: String,
        params: Vec<String>,
    },
    Enum {
        name: String,
        variants: Vec<String>,
    },
    Test {
        name: String,
        body: AirBlock,
    },
    Mock {
        function: String,
        value: AirExpr,
    },
    Assert {
        condition: AirExpr,
    },
    Budget {
        max_tokens: Option<i64>,
        max_model_calls: Option<i64>,
        max_cost: Option<f64>,
        timeout_ms: Option<i64>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AirExpr {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Identifier(String),
    Unary {
        op: UnaryOp,
        operand: Box<AirExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<AirExpr>,
        right: Box<AirExpr>,
    },
    List(Vec<AirExpr>),
    Index {
        object: Box<AirExpr>,
        index: Box<AirExpr>,
    },
    Await(Box<AirExpr>),
    /// A plain function call — `fn`/`async fn`, `print`, any stdlib
    /// function, including `option_is_some`/`option_unwrap`. See
    /// SPEC.md for why `Option<T>`'s operations don't get their own
    /// node the way `Distribution<T>`'s do.
    Call {
        callee: String,
        args: Vec<AirExpr>,
    },
    /// A call to a declared `infer` function — indistinguishable from
    /// `Call` in the AST; explicit here, so nothing downstream ever
    /// has to re-derive "is this an inference" from a name lookup.
    Infer {
        function: String,
        args: Vec<AirExpr>,
    },
    /// A call to a declared `tool` function.
    ToolCall {
        tool: String,
        args: Vec<AirExpr>,
    },
    /// `distribution_argmax`/`entropy`/`sample`/`require_confidence` —
    /// which one, not just "a call to a function with this name".
    /// `distribution_probability` is `Probability` instead, its own
    /// node — see SPEC.md for why `ROADMAP.md` naming it separately
    /// from `DISTRIBUTION` earns it a separate `AirExpr` case too.
    Distribution {
        op: DistributionOp,
        args: Vec<AirExpr>,
    },
    /// `distribution_probability(distribution, value)` specifically.
    Probability {
        distribution: Box<AirExpr>,
        value: Box<AirExpr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionOp {
    Argmax,
    Entropy,
    Sample,
    RequireConfidence,
}
