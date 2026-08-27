//! The `Model` trait and its only implementation so far, `MockModel`.
//!
//! See `docs/milestones/08-first-ai-primitive/SPEC.md` for why this is
//! static dispatch (`Interpreter<W, M: Model>`), not `dyn Model` — the
//! short version is that there's only one implementation today, so
//! there's nothing to dynamically choose between yet. That's milestone
//! 16's problem, once real model adapters exist alongside `MockModel`.

use std::collections::HashMap;

use aint_ast::{Span, Type};

use crate::error::RuntimeError;
use crate::value::Value;

/// What the interpreter sends a [`Model`] when an `infer` call is
/// awaited: which function, its already-evaluated arguments, and the
/// declared return type. `return_type` is the "structured-output
/// request" half of milestone 09 — a real model adapter (milestone 16)
/// builds a JSON-schema request from it; `MockModel` ignores it, since
/// schema validation of *its* response happens generically in the
/// interpreter afterward, not per-`Model`. See
/// `docs/milestones/09-typed-structured-inference/SPEC.md`.
pub struct InferenceRequest {
    pub function: String,
    pub args: Vec<Value>,
    pub return_type: Type,
    pub span: Span,
}

/// Something that can answer an inference request. Only [`MockModel`]
/// implements this today.
///
/// `async fn` in a `pub` trait normally trips rustc's `async_fn_in_trait`
/// lint, which exists because the returned future's `Send`-ness can't be
/// named or bounded — a real concern for `dyn Model`. Nothing here is
/// ever behind `dyn`; every user is generic over a concrete `M: Model`,
/// so there's no call site that needs that bound. Hence the `allow`.
#[allow(async_fn_in_trait)]
pub trait Model {
    async fn infer(&self, request: InferenceRequest) -> Result<Value, RuntimeError>;
}

/// The only `Model` implementation before milestone 16. Returns
/// pre-configured canned responses, keyed by `infer` function name —
/// configured entirely through this Rust API
/// (`MockModel::new().mock("name", value)`), since there's no
/// AINT-level mocking syntax yet (that's milestone 15). An `infer` call
/// with nothing configured for it fails clearly rather than guessing.
#[derive(Debug, Default)]
pub struct MockModel {
    responses: HashMap<String, Value>,
}

impl MockModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the value to return for calls to `function`. Consumes
    /// and returns `self` so calls chain: `MockModel::new().mock("a",
    /// ...).mock("b", ...)`.
    pub fn mock(mut self, function: impl Into<String>, value: Value) -> Self {
        self.responses.insert(function.into(), value);
        self
    }
}

impl Model for MockModel {
    async fn infer(&self, request: InferenceRequest) -> Result<Value, RuntimeError> {
        match self.responses.get(&request.function) {
            Some(value) => Ok(value.clone()),
            None => Err(RuntimeError::ModelError {
                message: format!("no mock response configured for `{}`", request.function),
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
        let model = MockModel::new().mock("sentiment", Value::Bool(true));
        let result = model
            .infer(InferenceRequest {
                function: "sentiment".to_string(),
                args: vec![Value::String("great".to_string())],
                return_type: Type::Bool,
                span: span(),
            })
            .await;
        assert_eq!(result, Ok(Value::Bool(true)));
    }

    #[tokio::test]
    async fn errors_clearly_when_nothing_is_configured() {
        let model = MockModel::new();
        let err = model
            .infer(InferenceRequest {
                function: "sentiment".to_string(),
                args: vec![],
                return_type: Type::Bool,
                span: span(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }
}
