//! Execution trace records — `Inference #N` / `Tool Call #N`, captured
//! unconditionally by `Interpreter` for every `infer`/`tool` call. See
//! `docs/milestones/14-ai-execution-tracing/SPEC.md`.

use std::time::Duration;

use crate::value::Value;

/// Token usage for one inference call. Always `{0, 0}` today —
/// `MockModel` has no text to tokenize. A real placeholder, not an
/// omitted field: milestone 16's real adapters fill it in without
/// changing this shape. See SPEC.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TokenUsage {
    pub prompt: u64,
    pub completion: u64,
}

/// What an `Inference` trace's call actually resulted in — mirrors
/// `InferenceOutcome`, plus the failure case tracing specifically
/// needs to capture.
#[derive(Debug, Clone, PartialEq)]
pub enum InferenceTraceOutcome {
    Answer(Value),
    CallTool { tool: String, args: Vec<Value> },
    Error(String),
}

/// One captured `infer` or `tool` call. See `Interpreter::traces`.
#[derive(Debug, Clone, PartialEq)]
pub enum TraceRecord {
    /// One round trip to the model — an `infer` call may produce
    /// several of these before it answers, one per iteration of
    /// `eval_inference`'s tool-calling loop (milestone 12).
    Inference {
        id: u64,
        function: String,
        /// Which backend answered — always `"mock"` today; see
        /// SPEC.md for why this isn't yet a `Model` trait method.
        model: String,
        tokens: TokenUsage,
        latency: Duration,
        outcome: InferenceTraceOutcome,
    },
    ToolCall {
        id: u64,
        tool: String,
        args: Vec<Value>,
        latency: Duration,
        /// `Err` holds the error's rendered message, not a
        /// `RuntimeError` itself — a trace record is a plain data
        /// snapshot, not something that should keep error-type
        /// machinery alive.
        outcome: Result<Value, String>,
    },
}

impl TraceRecord {
    /// The `"Inference #N"` / `"Tool Call #N"` label `ROADMAP.md`
    /// itself uses.
    pub fn label(&self) -> String {
        match self {
            TraceRecord::Inference { id, .. } => format!("Inference #{id}"),
            TraceRecord::ToolCall { id, .. } => format!("Tool Call #{id}"),
        }
    }
}
