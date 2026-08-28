//! `Program -> String`: a canonical pretty-printer. Two correctness
//! properties matter more than matching any particular hand-written
//! style — see `docs/milestones/24-language-tooling/SPEC.md`:
//!
//! - **Idempotent**: formatting already-formatted output changes
//!   nothing.
//! - **AST-preserving**: re-parsing the output produces the same AST
//!   (modulo the source positions themselves, which necessarily
//!   change) as the input had — this is what makes reformatting safe
//!   at all.

use aint_ast::{BinaryOp, Block, Expr, ExprKind, Param, Program, Stmt, StmtKind, UnaryOp};

const INDENT: &str = "    ";

pub fn format_program(program: &Program) -> String {
    let mut printer = Printer::default();
    printer.program(program);
    printer.out
}

/// Binary/unary operator precedence, used only for deciding when a
/// child expression needs parentheses to reproduce the same AST on
/// re-parse — not related to the type checker's or interpreter's own
/// logic. Higher binds tighter. Mirrors `aint-parser`'s own descent
/// order exactly (`parse_equality` -> `parse_comparison` ->
/// `parse_term` -> `parse_factor` -> `parse_unary`/`await` ->
/// `parse_postfix`/primary).
fn binary_precedence(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Eq | BinaryOp::NotEq => 0,
        BinaryOp::Less | BinaryOp::Greater => 1,
        BinaryOp::Add | BinaryOp::Sub => 2,
        BinaryOp::Mul | BinaryOp::Div => 3,
    }
}
const UNARY_PRECEDENCE: u8 = 4;
const PRIMARY_PRECEDENCE: u8 = 5;

fn binary_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Less => "<",
        BinaryOp::Greater => ">",
    }
}

/// A float that would print without a `.` (Rust's default `Display`
/// for `2.0_f64` is `"2"`) has to gain one back, or the formatted
/// output would re-lex as an `Integer`, not a `Float` — a real
/// AST-preservation bug, not a style nit.
fn format_float(n: f64) -> String {
    let s = format!("{n}");
    if s.contains(['.', 'e']) || s == "inf" || s == "-inf" || s == "NaN" {
        s
    } else {
        format!("{s}.0")
    }
}

/// Escapes exactly the escapes `aint-lexer`'s `lex_string` recognizes
/// (`"`, `\`, `\n`, `\t`, `\r`) — not Rust's own `Debug` escaping,
/// which also emits `\u{...}` for other control/non-ASCII characters
/// that AINT's lexer doesn't understand as an escape at all (it would
/// keep `\u` literally, changing the string's value on re-parse).
fn format_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[derive(Default)]
struct Printer {
    out: String,
    depth: usize,
}

impl Printer {
    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str(INDENT);
        }
    }

    /// Prints a statement list (top-level or a block's body),
    /// **preserving** — not inventing — blank lines: a gap of at
    /// least one blank source line between two statements is kept as
    /// exactly one blank line; statements written back to back in the
    /// source stay back to back. A formatter that invented its own
    /// spacing rule (e.g. "always blank between top-level
    /// statements") would fight every author's own grouping instead
    /// of respecting it — the same reason gofmt/rustfmt preserve
    /// existing blank lines rather than imposing a policy.
    fn statements(&mut self, statements: &[Stmt]) {
        let mut previous_end_line: Option<u32> = None;
        for stmt in statements {
            if let Some(previous_end_line) = previous_end_line {
                if stmt.span.start.line > previous_end_line + 1 {
                    self.out.push('\n');
                }
            }
            self.stmt(stmt);
            self.out.push('\n');
            previous_end_line = Some(stmt.span.end.line);
        }
    }

    fn program(&mut self, program: &Program) {
        self.statements(&program.statements);
    }

    fn block(&mut self, block: &Block) {
        self.out.push_str("{\n");
        self.depth += 1;
        self.statements(&block.statements);
        self.depth -= 1;
        self.indent();
        self.out.push('}');
    }

    fn params(&mut self, params: &[Param]) {
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.out.push_str(&param.name);
            self.out.push_str(": ");
            self.out.push_str(&param.ty.to_string());
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        self.indent();
        match &stmt.kind {
            StmtKind::Let { name, value } => {
                self.out.push_str("let ");
                self.out.push_str(name);
                self.out.push_str(" = ");
                self.expr(value, 0);
            }
            StmtKind::Expr(expr) => self.expr(expr, 0),
            StmtKind::Return(expr) => {
                self.out.push_str("return ");
                self.expr(expr, 0);
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.out.push_str("if ");
                self.expr(condition, 0);
                self.out.push(' ');
                self.block(then_branch);
                if let Some(else_branch) = else_branch {
                    self.out.push_str(" else ");
                    self.block(else_branch);
                }
            }
            StmtKind::Fn {
                name,
                params,
                return_type,
                body,
                is_async,
                effects,
            } => {
                if *is_async {
                    self.out.push_str("async ");
                }
                self.out.push_str("fn ");
                self.out.push_str(name);
                self.out.push('(');
                self.params(params);
                self.out.push_str(") -> ");
                self.out.push_str(&return_type.to_string());
                if let Some(effects) = effects {
                    self.out.push_str(" effects [");
                    let words: Vec<&str> = effects.iter().map(|e| effect_word(*e)).collect();
                    self.out.push_str(&words.join(", "));
                    self.out.push(']');
                }
                self.out.push(' ');
                self.block(body);
            }
            StmtKind::Import(module) => {
                self.out.push_str("import ");
                self.out.push_str(module);
            }
            StmtKind::Infer {
                name,
                params,
                return_type,
                permissions,
            } => {
                self.out.push_str("infer ");
                self.out.push_str(name);
                self.out.push('(');
                self.params(params);
                self.out.push_str(") -> ");
                self.out.push_str(&return_type.to_string());
                if let Some(names) = permissions {
                    self.out.push_str(" permissions [");
                    self.out.push_str(&names.join(", "));
                    self.out.push(']');
                }
            }
            StmtKind::Enum { name, variants } => {
                self.out.push_str("enum ");
                self.out.push_str(name);
                self.out.push_str(" { ");
                self.out.push_str(&variants.join(" "));
                self.out.push_str(" }");
            }
            StmtKind::Tool {
                name,
                params,
                return_type,
            } => {
                self.out.push_str("tool ");
                self.out.push_str(name);
                self.out.push('(');
                self.params(params);
                self.out.push_str(") -> ");
                self.out.push_str(&return_type.to_string());
            }
            StmtKind::Test { name, body } => {
                self.out.push_str("test ");
                self.out.push_str(&format_string_literal(name));
                self.out.push(' ');
                self.block(body);
            }
            StmtKind::Mock { function, value } => {
                self.out.push_str("mock ");
                self.out.push_str(function);
                self.out.push_str(" -> ");
                self.expr(value, 0);
            }
            StmtKind::Assert { condition } => {
                self.out.push_str("assert ");
                self.expr(condition, 0);
            }
            StmtKind::Budget {
                max_tokens,
                max_model_calls,
                max_cost,
                timeout_ms,
            } => {
                self.out.push_str("budget {\n");
                self.depth += 1;
                if let Some(v) = max_tokens {
                    self.indent();
                    self.out.push_str(&format!("max_tokens = {v}\n"));
                }
                if let Some(v) = max_model_calls {
                    self.indent();
                    self.out.push_str(&format!("max_model_calls = {v}\n"));
                }
                if let Some(v) = max_cost {
                    self.indent();
                    self.out
                        .push_str(&format!("max_cost = {}\n", format_float(*v)));
                }
                if let Some(v) = timeout_ms {
                    self.indent();
                    self.out.push_str(&format!("timeout_ms = {v}\n"));
                }
                self.depth -= 1;
                self.indent();
                self.out.push('}');
            }
        }
    }

    fn expr(&mut self, expr: &Expr, min_prec: u8) {
        match &expr.kind {
            ExprKind::Integer(n) => self.out.push_str(&n.to_string()),
            ExprKind::Float(n) => self.out.push_str(&format_float(*n)),
            ExprKind::String(s) => self.out.push_str(&format_string_literal(s)),
            ExprKind::Bool(b) => self.out.push_str(if *b { "true" } else { "false" }),
            ExprKind::Identifier(name) => self.out.push_str(name),
            ExprKind::Unary { op, operand } => {
                let needs_parens = UNARY_PRECEDENCE < min_prec;
                if needs_parens {
                    self.out.push('(');
                }
                self.out.push_str(match op {
                    UnaryOp::Neg => "-",
                });
                self.expr(operand, UNARY_PRECEDENCE);
                if needs_parens {
                    self.out.push(')');
                }
            }
            ExprKind::Binary { op, left, right } => {
                let prec = binary_precedence(*op);
                let needs_parens = prec < min_prec;
                if needs_parens {
                    self.out.push('(');
                }
                self.expr(left, prec);
                self.out.push(' ');
                self.out.push_str(binary_symbol(*op));
                self.out.push(' ');
                self.expr(right, prec + 1);
                if needs_parens {
                    self.out.push(')');
                }
            }
            ExprKind::Call { callee, args } => {
                self.expr(callee, PRIMARY_PRECEDENCE);
                self.out.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.expr(arg, 0);
                }
                self.out.push(')');
            }
            ExprKind::List(items) => {
                self.out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.expr(item, 0);
                }
                self.out.push(']');
            }
            ExprKind::Index { object, index } => {
                self.expr(object, PRIMARY_PRECEDENCE);
                self.out.push('[');
                self.expr(index, 0);
                self.out.push(']');
            }
            ExprKind::Await(inner) => {
                let needs_parens = UNARY_PRECEDENCE < min_prec;
                if needs_parens {
                    self.out.push('(');
                }
                self.out.push_str("await ");
                self.expr(inner, UNARY_PRECEDENCE);
                if needs_parens {
                    self.out.push(')');
                }
            }
        }
    }
}

fn effect_word(effect: aint_ast::Effect) -> &'static str {
    match effect {
        aint_ast::Effect::Pure => "pure",
        aint_ast::Effect::Inference => "inference",
        aint_ast::Effect::Tool => "tool",
        aint_ast::Effect::Network => "network",
        aint_ast::Effect::Filesystem => "filesystem",
    }
}
