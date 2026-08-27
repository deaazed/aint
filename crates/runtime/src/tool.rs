//! `MockTool`, the only way a `tool`-declared function produces a
//! result before milestone 16 adds real backends.
//!
//! Unlike [`crate::Model`], this isn't a trait. See
//! `docs/milestones/11-typed-tools/SPEC.md`: a program can declare
//! many independently-backed tools, and nothing on the roadmap needs
//! `Interpreter` to be generic over a single swappable tool
//! implementation the way it needs to be generic over `Model` for
//! milestone 16. `MockTool` is a plain name-keyed table, exactly like
//! `MockModel`'s internal one, just without the trait wrapper around
//! it.

use std::collections::HashMap;

use aint_ast::{Span, Type};

use crate::error::RuntimeError;
use crate::value::Value;

/// What the interpreter needs to answer a `tool` call: which tool, and
/// its already-evaluated arguments.
pub struct ToolRequest {
    pub tool: String,
    pub args: Vec<Value>,
    pub span: Span,
}

/// A declared tool's shape, as a `Model` sees it (milestone 12): what
/// it's called, and its typed signature — enough for a model (real or
/// mock) to decide whether, and how, to call it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSignature {
    pub name: String,
    pub params: Vec<Type>,
    pub return_type: Type,
}

/// One completed tool call within an inference's tool-calling
/// conversation: what was called, with what, and what came back. Fed
/// to the model on the *next* call so it has the result to work from.
/// See `docs/milestones/12-ai-tool-calling/SPEC.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolExchange {
    pub tool: String,
    pub args: Vec<Value>,
    pub result: Value,
}

/// The only way a `tool` call produces a value before milestone 16.
/// Configured entirely through this Rust API
/// (`MockTool::new().mock("name", value)`) — there's no AINT-level way
/// to configure it yet, same gap as `MockModel` since milestone 08.
#[derive(Debug, Default)]
pub struct MockTool {
    responses: HashMap<String, Value>,
}

impl MockTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the value to return for calls to `tool`. Consumes and
    /// returns `self` so calls chain, same as `MockModel::mock`.
    pub fn mock(mut self, tool: impl Into<String>, value: Value) -> Self {
        self.responses.insert(tool.into(), value);
        self
    }

    pub async fn call(&self, request: ToolRequest) -> Result<Value, RuntimeError> {
        match self.responses.get(&request.tool) {
            Some(value) => Ok(value.clone()),
            None => Err(RuntimeError::ToolError {
                message: format!("no mock response configured for `{}`", request.tool),
                span: request.span,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use aint_ast::Position;

    use super::*;

    fn span() -> Span {
        Span::new(Position::new(1, 1), Position::new(1, 1))
    }

    #[tokio::test]
    async fn returns_the_configured_response() {
        let tool = MockTool::new().mock("database_get_email", Value::String("a@b.com".into()));
        let result = tool
            .call(ToolRequest {
                tool: "database_get_email".to_string(),
                args: vec![Value::String("1".to_string())],
                span: span(),
            })
            .await;
        assert_eq!(result, Ok(Value::String("a@b.com".to_string())));
    }

    #[tokio::test]
    async fn errors_clearly_when_nothing_is_configured() {
        let tool = MockTool::new();
        let err = tool
            .call(ToolRequest {
                tool: "database_get_email".to_string(),
                args: vec![],
                span: span(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, RuntimeError::ToolError { .. }));
    }
}
