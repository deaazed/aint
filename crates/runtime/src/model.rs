//! The `Model` trait and its only implementation so far, `MockModel`.
//!
//! See `docs/milestones/08-first-ai-primitive/SPEC.md` for why this is
//! static dispatch (`Interpreter<W, M: Model>`), not `dyn Model` — the
//! short version is that there's only one implementation today, so
//! there's nothing to dynamically choose between yet. That's milestone
//! 16's problem, once real model adapters exist alongside `MockModel`.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use aint_ast::{Span, Type};

use crate::error::RuntimeError;
use crate::tool::{ToolExchange, ToolSignature};
use crate::value::Value;

/// What the interpreter sends a [`Model`] when an `infer` call is
/// awaited: which function, its already-evaluated arguments, and the
/// declared return type. `return_type` is the "structured-output
/// request" half of milestone 09 — a real model adapter (milestone 16)
/// builds a JSON-schema request from it; `MockModel` ignores it, since
/// schema validation of *its* response happens generically in the
/// interpreter afterward, not per-`Model`. See
/// `docs/milestones/09-typed-structured-inference/SPEC.md`.
///
/// `available_tools` and `history` are milestone 12's addition: what
/// the model is allowed to call, and what's already happened in this
/// tool-calling conversation so far. `MockModel` ignores both, same as
/// it ignores `return_type` — the shape exists for a real adapter.
pub struct InferenceRequest {
    pub function: String,
    pub args: Vec<Value>,
    pub return_type: Type,
    pub available_tools: Vec<ToolSignature>,
    pub history: Vec<ToolExchange>,
    pub span: Span,
}

/// What a [`Model`] decides in response to an [`InferenceRequest`]:
/// either a final answer, or a request to run a declared `tool` and
/// come back with the result. See
/// `docs/milestones/12-ai-tool-calling/SPEC.md`.
#[derive(Debug, Clone, PartialEq)]
pub enum InferenceOutcome {
    Answer(Value),
    CallTool { tool: String, args: Vec<Value> },
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
    async fn infer(&self, request: InferenceRequest) -> Result<InferenceOutcome, RuntimeError>;
}

/// The only `Model` implementation before milestone 16. Configured
/// entirely through this Rust API, since there's no AINT-level mocking
/// syntax yet (that's milestone 15).
///
/// Internally a queue of [`InferenceOutcome`]s per function name,
/// popped one at a time on each call — a single call to `.mock(name,
/// value)` still behaves exactly as it always has (a length-one queue,
/// popped once), but `.script(name, outcomes)` can now script a
/// multi-step tool-calling conversation. An `infer` call whose queue is
/// empty (never configured, or exhausted) fails clearly rather than
/// guessing. See `docs/milestones/12-ai-tool-calling/SPEC.md`.
#[derive(Debug, Default)]
pub struct MockModel {
    scripts: HashMap<String, RefCell<VecDeque<InferenceOutcome>>>,
}

impl MockModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the value to return for calls to `function`. Sugar
    /// for `.script(function, vec![InferenceOutcome::Answer(value)])`.
    /// Consumes and returns `self` so calls chain: `MockModel::new()
    /// .mock("a", ...).mock("b", ...)`.
    pub fn mock(self, function: impl Into<String>, value: Value) -> Self {
        self.script(function, vec![InferenceOutcome::Answer(value)])
    }

    /// Registers a sequence of outcomes for calls to `function`,
    /// popped one per call — for scripting a tool-calling conversation
    /// (`CallTool`, then `CallTool` again, then `Answer`, for
    /// instance).
    pub fn script(mut self, function: impl Into<String>, outcomes: Vec<InferenceOutcome>) -> Self {
        self.scripts.insert(
            function.into(),
            RefCell::new(outcomes.into_iter().collect()),
        );
        self
    }
}

impl Model for MockModel {
    async fn infer(&self, request: InferenceRequest) -> Result<InferenceOutcome, RuntimeError> {
        match self.scripts.get(&request.function) {
            Some(queue) => match queue.borrow_mut().pop_front() {
                Some(outcome) => Ok(outcome),
                None => Err(RuntimeError::ModelError {
                    message: format!(
                        "no mock response configured for `{}` (script exhausted)",
                        request.function
                    ),
                    span: request.span,
                }),
            },
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

    fn request(function: &str) -> InferenceRequest {
        InferenceRequest {
            function: function.to_string(),
            args: vec![Value::String("great".to_string())],
            return_type: Type::Bool,
            available_tools: vec![],
            history: vec![],
            span: span(),
        }
    }

    #[tokio::test]
    async fn returns_the_configured_response() {
        let model = MockModel::new().mock("sentiment", Value::Bool(true));
        let result = model.infer(request("sentiment")).await;
        assert_eq!(result, Ok(InferenceOutcome::Answer(Value::Bool(true))));
    }

    #[tokio::test]
    async fn errors_clearly_when_nothing_is_configured() {
        let model = MockModel::new();
        let err = model.infer(request("sentiment")).await.unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }

    #[tokio::test]
    async fn script_pops_one_outcome_per_call() {
        let model = MockModel::new().script(
            "agent",
            vec![
                InferenceOutcome::CallTool {
                    tool: "lookup".to_string(),
                    args: vec![],
                },
                InferenceOutcome::Answer(Value::Bool(true)),
            ],
        );
        assert_eq!(
            model.infer(request("agent")).await,
            Ok(InferenceOutcome::CallTool {
                tool: "lookup".to_string(),
                args: vec![],
            })
        );
        assert_eq!(
            model.infer(request("agent")).await,
            Ok(InferenceOutcome::Answer(Value::Bool(true)))
        );
    }

    #[tokio::test]
    async fn errors_clearly_when_a_script_is_exhausted() {
        let model = MockModel::new().mock("sentiment", Value::Bool(true));
        model.infer(request("sentiment")).await.unwrap();
        let err = model.infer(request("sentiment")).await.unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }
}
