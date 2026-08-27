use aint_ast::{
    BinaryOp, Block, Effect, Expr, ExprKind, Param, Position, Program, Span, Stmt, StmtKind, Type,
    UnaryOp,
};
use aint_lexer::{tokenize, Token, TokenKind};

use crate::error::ParseError;

/// Recursive-descent parser over an already-lexed token stream.
///
/// `tokens` is expected to end in exactly one `Eof` token, which is what
/// `aint_lexer::tokenize` always produces.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current(&self) -> &Token {
        // `tokens` always ends in `Eof`, and `advance` never steps past
        // it, so `pos` is always a valid index.
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if !self.is_at_end() {
            self.pos += 1;
        }
        token
    }

    fn check(&self, kind: &TokenKind) -> bool {
        &self.current().kind == kind
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, expected: &str) -> Result<Token, ParseError> {
        if self.check(&kind) {
            Ok(self.advance())
        } else {
            Err(ParseError::Unexpected {
                expected: expected.to_string(),
                found: self.current().clone(),
            })
        }
    }

    fn expect_identifier(&mut self) -> Result<(String, Span), ParseError> {
        match &self.current().kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                let span = self.current().span;
                self.advance();
                Ok((name, span))
            }
            _ => Err(ParseError::Unexpected {
                expected: "identifier".to_string(),
                found: self.current().clone(),
            }),
        }
    }

    // --- program / statements ---------------------------------------

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }
        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        match self.current().kind {
            TokenKind::Let => self.parse_let_statement(),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::Fn => {
                let fn_token = self.expect(TokenKind::Fn, "`fn`")?;
                self.parse_fn_body(false, fn_token.span.start)
            }
            TokenKind::Async => {
                let async_token = self.expect(TokenKind::Async, "`async`")?;
                self.expect(TokenKind::Fn, "`fn`")?;
                self.parse_fn_body(true, async_token.span.start)
            }
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::Import => self.parse_import_statement(),
            TokenKind::Infer => self.parse_infer_statement(),
            TokenKind::Enum => self.parse_enum_statement(),
            TokenKind::Tool => self.parse_tool_statement(),
            TokenKind::Test => self.parse_test_statement(),
            TokenKind::Mock => self.parse_mock_statement(),
            TokenKind::Assert => self.parse_assert_statement(),
            TokenKind::Budget => self.parse_budget_statement(),
            _ => self.parse_expr_statement(),
        }
    }

    /// Parses `test "name" { ... }`.
    fn parse_test_statement(&mut self) -> Result<Stmt, ParseError> {
        let test_token = self.expect(TokenKind::Test, "`test`")?;
        let name_token = self.current().clone();
        let name = match name_token.kind {
            TokenKind::String(s) => {
                self.advance();
                s
            }
            _ => {
                return Err(ParseError::Unexpected {
                    expected: "a string literal naming the test".to_string(),
                    found: name_token,
                });
            }
        };
        let body = self.parse_block()?;
        let span = Span::new(test_token.span.start, body.span.end);
        Ok(Stmt::new(StmtKind::Test { name, body }, span))
    }

    /// Parses `mock function -> value`. `value` stays a general `Expr`
    /// at parse time — restricting it to literals/`EnumName_Variant` is
    /// the type checker's job (see SPEC.md), the same layering
    /// `parse_type` already uses for enum names.
    fn parse_mock_statement(&mut self) -> Result<Stmt, ParseError> {
        let mock_token = self.expect(TokenKind::Mock, "`mock`")?;
        let (function, _) = self.expect_identifier()?;
        self.expect(TokenKind::Arrow, "`->`")?;
        let value = self.parse_expr()?;
        let span = Span::new(mock_token.span.start, value.span.end);
        Ok(Stmt::new(StmtKind::Mock { function, value }, span))
    }

    /// Parses `assert condition`.
    fn parse_assert_statement(&mut self) -> Result<Stmt, ParseError> {
        let assert_token = self.expect(TokenKind::Assert, "`assert`")?;
        let condition = self.parse_expr()?;
        let span = Span::new(assert_token.span.start, condition.span.end);
        Ok(Stmt::new(StmtKind::Assert { condition }, span))
    }

    /// Parses `budget { field = literal ... }` — a fixed, known set of
    /// fields (not general statements), each an integer or float
    /// literal, in any order, each optional. See
    /// `docs/milestones/17-ai-resource-management/SPEC.md`.
    fn parse_budget_statement(&mut self) -> Result<Stmt, ParseError> {
        let budget_token = self.expect(TokenKind::Budget, "`budget`")?;
        self.expect(TokenKind::LeftBrace, "`{`")?;

        let mut max_tokens = None;
        let mut max_model_calls = None;
        let mut max_cost = None;
        let mut timeout_ms = None;

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let (field, field_span) = self.expect_identifier()?;
            self.expect(TokenKind::Equal, "`=`")?;
            match field.as_str() {
                "max_tokens" => max_tokens = Some(self.expect_integer_literal()?),
                "max_model_calls" => max_model_calls = Some(self.expect_integer_literal()?),
                "max_cost" => max_cost = Some(self.expect_number_literal()?),
                "timeout_ms" => timeout_ms = Some(self.expect_integer_literal()?),
                _ => {
                    return Err(ParseError::Unexpected {
                        expected:
                            "a budget field (max_tokens, max_model_calls, max_cost, timeout_ms)"
                                .to_string(),
                        found: Token::new(TokenKind::Identifier(field), field_span),
                    });
                }
            }
        }

        let close = self.expect(TokenKind::RightBrace, "`}`")?;
        let span = Span::new(budget_token.span.start, close.span.end);
        Ok(Stmt::new(
            StmtKind::Budget {
                max_tokens,
                max_model_calls,
                max_cost,
                timeout_ms,
            },
            span,
        ))
    }

    fn expect_integer_literal(&mut self) -> Result<i64, ParseError> {
        match self.current().kind.clone() {
            TokenKind::Integer(n) => {
                self.advance();
                Ok(n)
            }
            _ => Err(ParseError::Unexpected {
                expected: "an integer".to_string(),
                found: self.current().clone(),
            }),
        }
    }

    /// An integer or float literal, either way returned as `f64` — for
    /// `max_cost`, where writing `max_cost = 0` should work without
    /// forcing `0.0`.
    fn expect_number_literal(&mut self) -> Result<f64, ParseError> {
        match self.current().kind.clone() {
            TokenKind::Integer(n) => {
                self.advance();
                Ok(n as f64)
            }
            TokenKind::Float(n) => {
                self.advance();
                Ok(n)
            }
            _ => Err(ParseError::Unexpected {
                expected: "a number".to_string(),
                found: self.current().clone(),
            }),
        }
    }

    fn parse_let_statement(&mut self) -> Result<Stmt, ParseError> {
        let let_token = self.expect(TokenKind::Let, "`let`")?;
        let (name, _) = self.expect_identifier()?;
        self.expect(TokenKind::Equal, "`=`")?;
        let value = self.parse_expr()?;
        let span = Span::new(let_token.span.start, value.span.end);
        Ok(Stmt::new(StmtKind::Let { name, value }, span))
    }

    fn parse_import_statement(&mut self) -> Result<Stmt, ParseError> {
        let import_token = self.expect(TokenKind::Import, "`import`")?;
        let (module, module_span) = self.expect_identifier()?;
        let span = Span::new(import_token.span.start, module_span.end);
        Ok(Stmt::new(StmtKind::Import(module), span))
    }

    /// Parses everything after `fn`/`async fn` — shared by both, since
    /// they differ only in whether `is_async` ends up `true` and where
    /// the statement's span starts (`fn`'s own position, or `async`'s).
    fn parse_fn_body(&mut self, is_async: bool, start: Position) -> Result<Stmt, ParseError> {
        let (name, _) = self.expect_identifier()?;
        self.expect(TokenKind::LeftParen, "`(`")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let (param_name, _) = self.expect_identifier()?;
                self.expect(TokenKind::Colon, "`:`")?;
                let (ty, _) = self.parse_type()?;
                params.push(Param {
                    name: param_name,
                    ty,
                });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "`)`")?;
        self.expect(TokenKind::Arrow, "`->`")?;
        let (return_type, _) = self.parse_type()?;
        let effects = self.parse_effects_clause()?;
        let body = self.parse_block()?;
        let span = Span::new(start, body.span.end);
        Ok(Stmt::new(
            StmtKind::Fn {
                name,
                params,
                return_type,
                body,
                is_async,
                effects,
            },
            span,
        ))
    }

    /// Parses an optional `effects [ Effect, Effect, ... ]` clause —
    /// `fn`/`async fn` only, see
    /// `docs/milestones/13-effects/SPEC.md` for why `infer`/`tool`
    /// don't get this syntax. Absent entirely (not `Effect` being the
    /// current token) means untracked, not `pure` — returns `None`.
    fn parse_effects_clause(&mut self) -> Result<Option<Vec<Effect>>, ParseError> {
        if !self.matches(&TokenKind::Effects) {
            return Ok(None);
        }
        self.expect(TokenKind::LeftBracket, "`[`")?;
        let mut effects = Vec::new();
        if !self.check(&TokenKind::RightBracket) {
            loop {
                effects.push(self.parse_effect_word()?);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightBracket, "`]`")?;
        Ok(Some(effects))
    }

    /// One word inside an `effects [ ... ]` list. `tool` is handled
    /// separately from the other four: it's already a keyword
    /// (`TokenKind::Tool`, milestone 11), so it never lexes as
    /// `TokenKind::Identifier("tool")` the way `pure`/`inference`/
    /// `network`/`filesystem` do.
    fn parse_effect_word(&mut self) -> Result<Effect, ParseError> {
        let token = self.current().clone();
        let effect = match &token.kind {
            TokenKind::Tool => Effect::Tool,
            TokenKind::Identifier(name) => match name.as_str() {
                "pure" => Effect::Pure,
                "inference" => Effect::Inference,
                "network" => Effect::Network,
                "filesystem" => Effect::Filesystem,
                _ => {
                    return Err(ParseError::Unexpected {
                        expected: "an effect (pure, inference, tool, network, filesystem)"
                            .to_string(),
                        found: token,
                    });
                }
            },
            _ => {
                return Err(ParseError::Unexpected {
                    expected: "an effect (pure, inference, tool, network, filesystem)".to_string(),
                    found: token,
                });
            }
        };
        self.advance();
        Ok(effect)
    }

    /// Parses `infer name(params) -> Type` — deliberately no body, see
    /// `docs/milestones/08-first-ai-primitive/SPEC.md`. Shares its
    /// parameter-list and return-type syntax with `parse_fn_body`, but
    /// isn't merged with it since there's no block to parse afterward.
    fn parse_infer_statement(&mut self) -> Result<Stmt, ParseError> {
        let infer_token = self.expect(TokenKind::Infer, "`infer`")?;
        let (name, _) = self.expect_identifier()?;
        self.expect(TokenKind::LeftParen, "`(`")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let (param_name, _) = self.expect_identifier()?;
                self.expect(TokenKind::Colon, "`:`")?;
                let (ty, _) = self.parse_type()?;
                params.push(Param {
                    name: param_name,
                    ty,
                });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "`)`")?;
        self.expect(TokenKind::Arrow, "`->`")?;
        let (return_type, return_span) = self.parse_type()?;
        let (permissions, permissions_end) = self.parse_permissions_clause()?;
        let span = Span::new(
            infer_token.span.start,
            permissions_end.unwrap_or(return_span.end),
        );
        Ok(Stmt::new(
            StmtKind::Infer {
                name,
                params,
                return_type,
                permissions,
            },
            span,
        ))
    }

    /// Parses an optional `permissions [ name, name, ... ]` clause —
    /// `infer` only. Absent entirely (not `Permissions` being the
    /// current token) means unrestricted, not "no tools allowed" — see
    /// `docs/milestones/20-security-model/SPEC.md`. Returns the end
    /// position of the closing `]` when present, so the caller can
    /// extend the statement's span to cover it.
    fn parse_permissions_clause(
        &mut self,
    ) -> Result<(Option<Vec<String>>, Option<Position>), ParseError> {
        if !self.matches(&TokenKind::Permissions) {
            return Ok((None, None));
        }
        self.expect(TokenKind::LeftBracket, "`[`")?;
        let mut names = Vec::new();
        if !self.check(&TokenKind::RightBracket) {
            loop {
                let (name, _) = self.expect_identifier()?;
                names.push(name);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        let close = self.expect(TokenKind::RightBracket, "`]`")?;
        Ok((Some(names), Some(close.span.end)))
    }

    /// Parses `tool name(params) -> Type` — no body, same shape as
    /// `parse_infer_statement`, kept separate rather than shared; see
    /// `docs/milestones/11-typed-tools/SPEC.md`.
    fn parse_tool_statement(&mut self) -> Result<Stmt, ParseError> {
        let tool_token = self.expect(TokenKind::Tool, "`tool`")?;
        let (name, _) = self.expect_identifier()?;
        self.expect(TokenKind::LeftParen, "`(`")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let (param_name, _) = self.expect_identifier()?;
                self.expect(TokenKind::Colon, "`:`")?;
                let (ty, _) = self.parse_type()?;
                params.push(Param {
                    name: param_name,
                    ty,
                });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "`)`")?;
        self.expect(TokenKind::Arrow, "`->`")?;
        let (return_type, return_span) = self.parse_type()?;
        let span = Span::new(tool_token.span.start, return_span.end);
        Ok(Stmt::new(
            StmtKind::Tool {
                name,
                params,
                return_type,
            },
            span,
        ))
    }

    /// Parses a type name from an identifier token — `Int`, `Float`,
    /// `Bool`, `String`, `Unit`, `List<T>`/`Option<T>` reusing the
    /// existing `<`/`>` operator tokens for the generic brackets, or any
    /// other identifier as a reference to a user-declared `enum`. The
    /// parser has no symbol table, so it can't reject an unknown enum
    /// name here the way it used to reject *any* unrecognized name —
    /// that's the type checker's job now
    /// (`docs/milestones/09-typed-structured-inference/SPEC.md`).
    fn parse_type(&mut self) -> Result<(Type, Span), ParseError> {
        let (name, span) = self.expect_identifier()?;
        let ty = match name.as_str() {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "Bool" => Type::Bool,
            "String" => Type::String,
            "Unit" => Type::Unit,
            "List" => {
                self.expect(TokenKind::Less, "`<`")?;
                let (inner, _) = self.parse_type()?;
                self.expect(TokenKind::Greater, "`>`")?;
                Type::List(Box::new(inner))
            }
            "Option" => {
                self.expect(TokenKind::Less, "`<`")?;
                let (inner, _) = self.parse_type()?;
                self.expect(TokenKind::Greater, "`>`")?;
                Type::Option(Box::new(inner))
            }
            "Distribution" => {
                self.expect(TokenKind::Less, "`<`")?;
                let (inner, _) = self.parse_type()?;
                self.expect(TokenKind::Greater, "`>`")?;
                Type::Distribution(Box::new(inner))
            }
            _ => Type::Enum(name),
        };
        Ok((ty, span))
    }

    /// Parses `enum Name { Variant1 Variant2 ... }` — bare identifiers,
    /// no separators, same as everywhere else in AINT statements don't
    /// need them.
    fn parse_enum_statement(&mut self) -> Result<Stmt, ParseError> {
        let enum_token = self.expect(TokenKind::Enum, "`enum`")?;
        let (name, _) = self.expect_identifier()?;
        self.expect(TokenKind::LeftBrace, "`{`")?;
        let mut variants = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let (variant, _) = self.expect_identifier()?;
            variants.push(variant);
        }
        let close = self.expect(TokenKind::RightBrace, "`}`")?;
        let span = Span::new(enum_token.span.start, close.span.end);
        Ok(Stmt::new(StmtKind::Enum { name, variants }, span))
    }

    fn parse_return_statement(&mut self) -> Result<Stmt, ParseError> {
        let return_token = self.expect(TokenKind::Return, "`return`")?;
        let value = self.parse_expr()?;
        let span = Span::new(return_token.span.start, value.span.end);
        Ok(Stmt::new(StmtKind::Return(value), span))
    }

    fn parse_if_statement(&mut self) -> Result<Stmt, ParseError> {
        let if_token = self.expect(TokenKind::If, "`if`")?;
        let condition = self.parse_expr()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.matches(&TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };

        let end = else_branch
            .as_ref()
            .map_or(then_branch.span.end, |b| b.span.end);
        let span = Span::new(if_token.span.start, end);
        Ok(Stmt::new(
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            },
            span,
        ))
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let open = self.expect(TokenKind::LeftBrace, "`{`")?;
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }
        let close = self.expect(TokenKind::RightBrace, "`}`")?;
        Ok(Block {
            statements,
            span: Span::new(open.span.start, close.span.end),
        })
    }

    fn parse_expr_statement(&mut self) -> Result<Stmt, ParseError> {
        let expr = self.parse_expr()?;
        let span = expr.span;
        Ok(Stmt::new(StmtKind::Expr(expr), span))
    }

    // --- expressions, lowest to highest precedence --------------------

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_equality()
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(Self::parse_comparison, |kind| match kind {
            TokenKind::EqualEqual => Some(BinaryOp::Eq),
            TokenKind::BangEqual => Some(BinaryOp::NotEq),
            _ => None,
        })
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(Self::parse_term, |kind| match kind {
            TokenKind::Less => Some(BinaryOp::Less),
            TokenKind::Greater => Some(BinaryOp::Greater),
            _ => None,
        })
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(Self::parse_factor, |kind| match kind {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        })
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(Self::parse_unary, |kind| match kind {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            _ => None,
        })
    }

    /// Shared left-associative binary-operator loop: parses one operand
    /// at `next_level`, then keeps folding in `operator(token) op operand`
    /// for as long as `operator` recognizes the current token.
    fn parse_binary_level(
        &mut self,
        next_level: fn(&mut Self) -> Result<Expr, ParseError>,
        operator: fn(&TokenKind) -> Option<BinaryOp>,
    ) -> Result<Expr, ParseError> {
        let mut left = next_level(self)?;
        while let Some(op) = operator(&self.current().kind) {
            self.advance();
            let right = next_level(self)?;
            let span = Span::new(left.span.start, right.span.end);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.current().kind, TokenKind::Minus) {
            let minus = self.advance();
            let operand = self.parse_unary()?;
            let span = Span::new(minus.span.start, operand.span.end);
            return Ok(Expr::new(
                ExprKind::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                },
                span,
            ));
        }
        if matches!(self.current().kind, TokenKind::Await) {
            let await_token = self.advance();
            let operand = self.parse_unary()?;
            let span = Span::new(await_token.span.start, operand.span.end);
            return Ok(Expr::new(ExprKind::Await(Box::new(operand)), span));
        }
        self.parse_postfix()
    }

    /// Parses a primary expression followed by zero or more postfix
    /// operations: calls (`expr(args)`) and indexing (`expr[index]`),
    /// left-associative and freely mixable (`f()[0]`, `list[0]()`).
    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.check(&TokenKind::LeftParen) {
                expr = self.finish_call(expr)?;
            } else if self.check(&TokenKind::LeftBracket) {
                expr = self.finish_index(expr)?;
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn finish_call(&mut self, callee: Expr) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LeftParen, "`(`")?;
        let mut args = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                args.push(self.parse_expr()?);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        let close = self.expect(TokenKind::RightParen, "`)`")?;
        let span = Span::new(callee.span.start, close.span.end);
        Ok(Expr::new(
            ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
            span,
        ))
    }

    fn finish_index(&mut self, object: Expr) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LeftBracket, "`[`")?;
        let index = self.parse_expr()?;
        let close = self.expect(TokenKind::RightBracket, "`]`")?;
        let span = Span::new(object.span.start, close.span.end);
        Ok(Expr::new(
            ExprKind::Index {
                object: Box::new(object),
                index: Box::new(index),
            },
            span,
        ))
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Integer(n) => {
                self.advance();
                Ok(Expr::new(ExprKind::Integer(n), token.span))
            }
            TokenKind::Float(n) => {
                self.advance();
                Ok(Expr::new(ExprKind::Float(n), token.span))
            }
            TokenKind::String(s) => {
                self.advance();
                Ok(Expr::new(ExprKind::String(s), token.span))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::new(ExprKind::Bool(true), token.span))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::new(ExprKind::Bool(false), token.span))
            }
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(Expr::new(ExprKind::Identifier(name), token.span))
            }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RightParen, "`)`")?;
                Ok(expr)
            }
            TokenKind::LeftBracket => {
                self.advance();
                let mut elements = Vec::new();
                if !self.check(&TokenKind::RightBracket) {
                    loop {
                        elements.push(self.parse_expr()?);
                        if !self.matches(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let close = self.expect(TokenKind::RightBracket, "`]`")?;
                let span = Span::new(token.span.start, close.span.end);
                Ok(Expr::new(ExprKind::List(elements), span))
            }
            _ => Err(ParseError::Unexpected {
                expected: "expression".to_string(),
                found: token,
            }),
        }
    }
}

/// Lexes then parses `source` in full, stopping at the first error.
pub fn parse_source(source: &str) -> Result<Program, ParseError> {
    let tokens = tokenize(source)?;
    Parser::new(tokens).parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Renders an expression as a parenthesized S-expression, e.g.
    /// `1 + 2 * 3` -> `(+ 1 (* 2 3))`, so precedence/associativity tests
    /// can assert against a literal string instead of hand-building an
    /// `Expr` tree (with real spans) for every case.
    fn describe_expr(expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Integer(n) => n.to_string(),
            ExprKind::Float(n) => n.to_string(),
            ExprKind::String(s) => format!("{s:?}"),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Identifier(name) => name.clone(),
            ExprKind::Unary { op, operand } => {
                format!("({} {})", unary_op_str(*op), describe_expr(operand))
            }
            ExprKind::Binary { op, left, right } => format!(
                "({} {} {})",
                binary_op_str(*op),
                describe_expr(left),
                describe_expr(right)
            ),
            ExprKind::Call { callee, args } => {
                let callee_str = describe_expr(callee);
                if args.is_empty() {
                    format!("({callee_str})")
                } else {
                    let args_str: Vec<String> = args.iter().map(describe_expr).collect();
                    format!("({callee_str} {})", args_str.join(" "))
                }
            }
            ExprKind::List(elements) => {
                let items: Vec<String> = elements.iter().map(describe_expr).collect();
                format!("[{}]", items.join(" "))
            }
            ExprKind::Index { object, index } => {
                format!("(index {} {})", describe_expr(object), describe_expr(index))
            }
            ExprKind::Await(inner) => format!("(await {})", describe_expr(inner)),
        }
    }

    fn binary_op_str(op: BinaryOp) -> &'static str {
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

    fn unary_op_str(op: UnaryOp) -> &'static str {
        match op {
            UnaryOp::Neg => "-",
        }
    }

    fn expr_str(src: &str) -> String {
        let tokens = tokenize(src).expect("should lex");
        let expr = Parser::new(tokens)
            .parse_expr()
            .expect("should parse as an expression");
        describe_expr(&expr)
    }

    fn parse_one_stmt(src: &str) -> Stmt {
        let tokens = tokenize(src).expect("should lex");
        Parser::new(tokens)
            .parse_statement()
            .expect("should parse as a statement")
    }

    // --- precedence / expressions -----------------------------------

    #[test]
    fn precedence_multiplication_before_addition() {
        assert_eq!(expr_str("1 + 2 * 3"), "(+ 1 (* 2 3))");
    }

    #[test]
    fn precedence_addition_before_comparison() {
        assert_eq!(expr_str("1 + 2 < 5"), "(< (+ 1 2) 5)");
    }

    #[test]
    fn precedence_comparison_before_equality() {
        assert_eq!(expr_str("1 < 2 == 3 > 4"), "(== (< 1 2) (> 3 4))");
    }

    #[test]
    fn unary_minus_binds_tighter_than_binary() {
        assert_eq!(expr_str("-1 + 2"), "(+ (- 1) 2)");
    }

    #[test]
    fn await_binds_like_unary_minus() {
        assert_eq!(expr_str("await foo() + 1"), "(+ (await (foo)) 1)");
    }

    #[test]
    fn parens_override_precedence() {
        assert_eq!(expr_str("(1 + 2) * 3"), "(* (+ 1 2) 3)");
    }

    #[test]
    fn left_associative_subtraction() {
        assert_eq!(expr_str("10 - 3 - 2"), "(- (- 10 3) 2)");
    }

    #[test]
    fn parses_each_literal_kind() {
        assert_eq!(expr_str("42"), "42");
        assert_eq!(expr_str("2.5"), "2.5");
        assert_eq!(expr_str("\"hi\""), "\"hi\"");
        assert_eq!(expr_str("true"), "true");
        assert_eq!(expr_str("false"), "false");
        assert_eq!(expr_str("x"), "x");
    }

    // --- function calls -----------------------------------------------

    #[test]
    fn call_with_no_args() {
        assert_eq!(expr_str("f()"), "(f)");
    }

    #[test]
    fn call_with_multiple_args() {
        assert_eq!(expr_str("f(1, 2, 3)"), "(f 1 2 3)");
    }

    #[test]
    fn chained_calls() {
        assert_eq!(expr_str("f()()"), "((f))");
    }

    // --- statements ---------------------------------------------------

    #[test]
    fn parses_let_statement() {
        let stmt = parse_one_stmt("let x = 42");
        match stmt.kind {
            StmtKind::Let { name, value } => {
                assert_eq!(name, "x");
                assert_eq!(describe_expr(&value), "42");
            }
            other => panic!("expected Let, got {other:?}"),
        }
    }

    #[test]
    fn parses_if_without_else() {
        let stmt = parse_one_stmt("if x { print(x) }");
        match stmt.kind {
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                assert_eq!(describe_expr(&condition), "x");
                assert_eq!(then_branch.statements.len(), 1);
                assert!(else_branch.is_none());
            }
            other => panic!("expected If, got {other:?}"),
        }
    }

    #[test]
    fn parses_if_with_else() {
        let stmt = parse_one_stmt("if x { a() } else { b() }");
        match stmt.kind {
            StmtKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                assert_eq!(then_branch.statements.len(), 1);
                assert_eq!(else_branch.expect("else branch").statements.len(), 1);
            }
            other => panic!("expected If, got {other:?}"),
        }
    }

    #[test]
    fn parses_expression_statement() {
        let stmt = parse_one_stmt("print(x)");
        match stmt.kind {
            StmtKind::Expr(expr) => assert_eq!(describe_expr(&expr), "(print x)"),
            other => panic!("expected Expr, got {other:?}"),
        }
    }

    #[test]
    fn program_needs_no_separators_between_statements() {
        let program = parse_source("let x = 1\nlet y = 2").expect("should parse");
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn parses_fn_statement() {
        let stmt = parse_one_stmt("fn add(a: Int, b: Int) -> Int { return a + b }");
        match stmt.kind {
            StmtKind::Fn {
                name,
                params,
                return_type,
                body,
                is_async,
                effects,
            } => {
                assert_eq!(name, "add");
                assert!(effects.is_none());
                assert_eq!(
                    params,
                    vec![
                        Param {
                            name: "a".to_string(),
                            ty: Type::Int
                        },
                        Param {
                            name: "b".to_string(),
                            ty: Type::Int
                        },
                    ]
                );
                assert_eq!(return_type, Type::Int);
                assert_eq!(body.statements.len(), 1);
                assert!(!is_async);
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_async_fn_statement() {
        let stmt = parse_one_stmt("async fn wait(n: Int) -> Int { return n }");
        match stmt.kind {
            StmtKind::Fn { name, is_async, .. } => {
                assert_eq!(name, "wait");
                assert!(is_async);
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_fn_with_no_params() {
        let stmt = parse_one_stmt("fn hello() -> Unit { print(x) }");
        match stmt.kind {
            StmtKind::Fn {
                params,
                return_type,
                ..
            } => {
                assert!(params.is_empty());
                assert_eq!(return_type, Type::Unit);
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_list_and_option_types() {
        let stmt = parse_one_stmt("fn f(x: List<Int>) -> Option<String> { return x }");
        match stmt.kind {
            StmtKind::Fn {
                params,
                return_type,
                ..
            } => {
                assert_eq!(params[0].ty, Type::List(Box::new(Type::Int)));
                assert_eq!(return_type, Type::Option(Box::new(Type::String)));
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn unknown_type_name_parses_as_a_speculative_enum_reference() {
        // No symbol table at parse time - see SPEC.md. Whether
        // `Frobnicate` is a real enum is the type checker's job now;
        // this only has to parse.
        let stmt = parse_one_stmt("fn f(x: Frobnicate) -> Int { return 1 }");
        match stmt.kind {
            StmtKind::Fn { params, .. } => {
                assert_eq!(params[0].ty, Type::Enum("Frobnicate".to_string()));
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_return_statement() {
        let stmt = parse_one_stmt("return 42");
        match stmt.kind {
            StmtKind::Return(expr) => assert_eq!(describe_expr(&expr), "42"),
            other => panic!("expected Return, got {other:?}"),
        }
    }

    #[test]
    fn parses_infer_statement() {
        let stmt = parse_one_stmt("infer is_positive(text: String) -> Bool");
        match stmt.kind {
            StmtKind::Infer {
                name,
                params,
                return_type,
                permissions,
            } => {
                assert_eq!(name, "is_positive");
                assert_eq!(
                    params,
                    vec![Param {
                        name: "text".to_string(),
                        ty: Type::String,
                    }]
                );
                assert_eq!(return_type, Type::Bool);
                assert_eq!(permissions, None);
            }
            other => panic!("expected Infer, got {other:?}"),
        }
    }

    #[test]
    fn parses_infer_statement_with_no_params() {
        let stmt = parse_one_stmt("infer greeting() -> String");
        match stmt.kind {
            StmtKind::Infer { params, .. } => assert!(params.is_empty()),
            other => panic!("expected Infer, got {other:?}"),
        }
    }

    #[test]
    fn parses_infer_statement_with_a_permissions_clause() {
        let stmt = parse_one_stmt(
            "infer summarize(id: String) -> String permissions [database_get_email]",
        );
        match stmt.kind {
            StmtKind::Infer { permissions, .. } => {
                assert_eq!(permissions, Some(vec!["database_get_email".to_string()]));
            }
            other => panic!("expected Infer, got {other:?}"),
        }
    }

    #[test]
    fn parses_infer_statement_with_a_multi_tool_permissions_clause() {
        let stmt = parse_one_stmt(
            "infer summarize(id: String) -> String permissions [database_get_email, database_get_name]",
        );
        match stmt.kind {
            StmtKind::Infer { permissions, .. } => {
                assert_eq!(
                    permissions,
                    Some(vec![
                        "database_get_email".to_string(),
                        "database_get_name".to_string(),
                    ])
                );
            }
            other => panic!("expected Infer, got {other:?}"),
        }
    }

    #[test]
    fn parses_infer_statement_with_an_empty_permissions_clause() {
        let stmt = parse_one_stmt("infer summarize(id: String) -> String permissions []");
        match stmt.kind {
            StmtKind::Infer { permissions, .. } => assert_eq!(permissions, Some(vec![])),
            other => panic!("expected Infer, got {other:?}"),
        }
    }

    #[test]
    fn parses_enum_statement() {
        let stmt = parse_one_stmt("enum Sentiment { Positive Neutral Negative }");
        match stmt.kind {
            StmtKind::Enum { name, variants } => {
                assert_eq!(name, "Sentiment");
                assert_eq!(variants, vec!["Positive", "Neutral", "Negative"]);
            }
            other => panic!("expected Enum, got {other:?}"),
        }
    }

    #[test]
    fn parses_enum_statement_with_one_variant() {
        let stmt = parse_one_stmt("enum Unit { Only }");
        match stmt.kind {
            StmtKind::Enum { variants, .. } => assert_eq!(variants, vec!["Only"]),
            other => panic!("expected Enum, got {other:?}"),
        }
    }

    #[test]
    fn parses_tool_statement() {
        let stmt = parse_one_stmt("tool database_get_email(id: String) -> String");
        match stmt.kind {
            StmtKind::Tool {
                name,
                params,
                return_type,
            } => {
                assert_eq!(name, "database_get_email");
                assert_eq!(
                    params,
                    vec![Param {
                        name: "id".to_string(),
                        ty: Type::String,
                    }]
                );
                assert_eq!(return_type, Type::String);
            }
            other => panic!("expected Tool, got {other:?}"),
        }
    }

    #[test]
    fn parses_tool_statement_with_no_params() {
        let stmt = parse_one_stmt("tool clock_now() -> Int");
        match stmt.kind {
            StmtKind::Tool { params, .. } => assert!(params.is_empty()),
            other => panic!("expected Tool, got {other:?}"),
        }
    }

    #[test]
    fn fn_with_no_effects_clause_is_untracked() {
        let stmt = parse_one_stmt("fn f() -> Int { return 1 }");
        match stmt.kind {
            StmtKind::Fn { effects, .. } => assert!(effects.is_none()),
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_pure_effects_clause() {
        let stmt = parse_one_stmt("fn f() -> Int effects [pure] { return 1 }");
        match stmt.kind {
            StmtKind::Fn { effects, .. } => {
                assert_eq!(effects, Some(vec![Effect::Pure]));
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_multi_effect_clause() {
        let stmt = parse_one_stmt("fn f() -> Int effects [inference, tool] { return 1 }");
        match stmt.kind {
            StmtKind::Fn { effects, .. } => {
                assert_eq!(effects, Some(vec![Effect::Inference, Effect::Tool]));
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_all_five_effect_words() {
        let stmt = parse_one_stmt(
            "fn f() -> Int effects [pure, inference, tool, network, filesystem] { return 1 }",
        );
        match stmt.kind {
            StmtKind::Fn { effects, .. } => {
                assert_eq!(
                    effects,
                    Some(vec![
                        Effect::Pure,
                        Effect::Inference,
                        Effect::Tool,
                        Effect::Network,
                        Effect::Filesystem,
                    ])
                );
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_unknown_effect_word() {
        let err = parse_source("fn f() -> Int effects [flying] { return 1 }").unwrap_err();
        assert!(matches!(err, ParseError::Unexpected { .. }));
    }

    #[test]
    fn async_fn_can_have_an_effects_clause_too() {
        let stmt = parse_one_stmt("async fn f() -> Int effects [pure] { return 1 }");
        match stmt.kind {
            StmtKind::Fn {
                is_async, effects, ..
            } => {
                assert!(is_async);
                assert_eq!(effects, Some(vec![Effect::Pure]));
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_test_statement() {
        let stmt = parse_one_stmt("test \"greets a customer\" { assert true }");
        match stmt.kind {
            StmtKind::Test { name, body } => {
                assert_eq!(name, "greets a customer");
                assert_eq!(body.statements.len(), 1);
            }
            other => panic!("expected Test, got {other:?}"),
        }
    }

    #[test]
    fn parses_mock_statement() {
        let stmt = parse_one_stmt("mock classify -> Sentiment_Positive");
        match stmt.kind {
            StmtKind::Mock { function, value } => {
                assert_eq!(function, "classify");
                assert_eq!(describe_expr(&value), "Sentiment_Positive");
            }
            other => panic!("expected Mock, got {other:?}"),
        }
    }

    #[test]
    fn parses_assert_statement() {
        let stmt = parse_one_stmt("assert 1 == 1");
        match stmt.kind {
            StmtKind::Assert { condition } => {
                assert_eq!(describe_expr(&condition), "(== 1 1)");
            }
            other => panic!("expected Assert, got {other:?}"),
        }
    }

    #[test]
    fn test_block_can_contain_mock_and_assert() {
        let stmt = parse_one_stmt(
            "test \"positive\" {\n\
                 mock classify -> Sentiment_Positive\n\
                 assert await classify(\"great\") == Sentiment_Positive\n\
             }",
        );
        match stmt.kind {
            StmtKind::Test { body, .. } => assert_eq!(body.statements.len(), 2),
            other => panic!("expected Test, got {other:?}"),
        }
    }

    #[test]
    fn parses_budget_statement_with_all_fields() {
        let stmt = parse_one_stmt(
            "budget {\n\
                 max_tokens = 5000\n\
                 max_model_calls = 3\n\
                 max_cost = 0.02\n\
                 timeout_ms = 10000\n\
             }",
        );
        match stmt.kind {
            StmtKind::Budget {
                max_tokens,
                max_model_calls,
                max_cost,
                timeout_ms,
            } => {
                assert_eq!(max_tokens, Some(5000));
                assert_eq!(max_model_calls, Some(3));
                assert_eq!(max_cost, Some(0.02));
                assert_eq!(timeout_ms, Some(10000));
            }
            other => panic!("expected Budget, got {other:?}"),
        }
    }

    #[test]
    fn parses_budget_statement_with_some_fields_omitted() {
        let stmt = parse_one_stmt("budget { max_model_calls = 3 }");
        match stmt.kind {
            StmtKind::Budget {
                max_tokens,
                max_model_calls,
                max_cost,
                timeout_ms,
            } => {
                assert_eq!(max_tokens, None);
                assert_eq!(max_model_calls, Some(3));
                assert_eq!(max_cost, None);
                assert_eq!(timeout_ms, None);
            }
            other => panic!("expected Budget, got {other:?}"),
        }
    }

    #[test]
    fn max_cost_accepts_a_bare_integer() {
        let stmt = parse_one_stmt("budget { max_cost = 1 }");
        match stmt.kind {
            StmtKind::Budget { max_cost, .. } => assert_eq!(max_cost, Some(1.0)),
            other => panic!("expected Budget, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_unknown_budget_field() {
        let err = parse_source("budget { max_bananas = 1 }").unwrap_err();
        assert!(matches!(err, ParseError::Unexpected { .. }));
    }

    #[test]
    fn parses_distribution_type() {
        let stmt = parse_one_stmt("infer classify(text: String) -> Distribution<Sentiment>");
        match stmt.kind {
            StmtKind::Infer { return_type, .. } => {
                assert_eq!(
                    return_type,
                    Type::Distribution(Box::new(Type::Enum("Sentiment".to_string())))
                );
            }
            other => panic!("expected Infer, got {other:?}"),
        }
    }

    #[test]
    fn enum_return_type_and_variant_reference_parse() {
        let stmt = parse_one_stmt("fn f() -> Sentiment { return Sentiment_Positive }");
        match stmt.kind {
            StmtKind::Fn { return_type, .. } => {
                assert_eq!(return_type, Type::Enum("Sentiment".to_string()));
            }
            other => panic!("expected Fn, got {other:?}"),
        }
    }

    #[test]
    fn parses_import_statement() {
        let stmt = parse_one_stmt("import math");
        match stmt.kind {
            StmtKind::Import(module) => assert_eq!(module, "math"),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn parses_list_literal() {
        assert_eq!(expr_str("[1, 2, 3]"), "[1 2 3]");
        assert_eq!(expr_str("[]"), "[]");
    }

    #[test]
    fn parses_indexing() {
        assert_eq!(expr_str("list[0]"), "(index list 0)");
    }

    #[test]
    fn parses_chained_indexing() {
        assert_eq!(expr_str("list[0][1]"), "(index (index list 0) 1)");
    }

    #[test]
    fn parses_index_with_arbitrary_expression() {
        assert_eq!(expr_str("list[i + 1]"), "(index list (+ i 1))");
    }

    #[test]
    fn indexing_and_calls_compose() {
        assert_eq!(expr_str("f()[0]"), "(index (f) 0)");
    }

    // --- errors ---------------------------------------------------------

    #[test]
    fn errors_on_missing_equals_in_let() {
        let err = parse_source("let x 42").unwrap_err();
        assert!(matches!(err, ParseError::Unexpected { .. }));
    }

    #[test]
    fn errors_on_unclosed_paren() {
        let err = parse_source("(1 + 2").unwrap_err();
        assert!(matches!(err, ParseError::Unexpected { .. }));
    }

    #[test]
    fn lex_errors_surface_through_parser() {
        let err = parse_source("\"unterminated").unwrap_err();
        assert!(matches!(err, ParseError::Lex(_)));
    }

    #[test]
    fn unexpected_token_error_reports_a_position() {
        let err = parse_source("let x 42").unwrap_err();
        // Just confirms `.span()` resolves without panicking and lands
        // on the offending token (`42`), not somewhere arbitrary.
        assert_eq!(err.span().start, Position::new(1, 7));
    }
}
