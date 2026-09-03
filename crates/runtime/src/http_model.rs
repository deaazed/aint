//! `HttpModel` — a real `Model` implementation talking to any
//! OpenAI-compatible chat completions endpoint (vLLM's OpenAI-compatible
//! server, Ollama's OpenAI-compatible endpoint, or OpenAI itself). See
//! `docs/milestones/16-model-adapters/SPEC.md` for what's deliberately
//! not supported yet (tool calling, `Distribution<T>`) and why one
//! adapter serves all three vendors instead of three separate types.

use aint_ast::{Span, Type};
use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;
use crate::model::{InferenceOutcome, InferenceRequest, Model};
use crate::value::Value;

/// A real model reached over HTTP. `base_url` should include any
/// version prefix the backend expects (e.g. `http://localhost:11434/v1`
/// for Ollama's OpenAI-compatible endpoint) — this type appends only
/// `/chat/completions`.
pub struct HttpModel {
    base_url: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl HttpModel {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            api_key: None,
            client: reqwest::Client::new(),
        }
    }

    /// Sent as a bearer token — needed for OpenAI itself, optional for
    /// a local vLLM/Ollama deployment.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

impl Model for HttpModel {
    async fn infer(&self, request: InferenceRequest) -> Result<InferenceOutcome, RuntimeError> {
        if !request.available_tools.is_empty() {
            return Err(RuntimeError::ModelError {
                message:
                    "HttpModel does not support tool calling yet; use MockModel for agentic tests"
                        .to_string(),
                span: request.span,
            });
        }
        if matches!(request.return_type, Type::Distribution(_)) {
            return Err(RuntimeError::ModelError {
                message: "HttpModel does not support Distribution<T>-returning infer calls yet"
                    .to_string(),
                span: request.span,
            });
        }

        let body = ChatRequest {
            model: &self.model,
            messages: vec![ChatMessage {
                role: "user",
                content: build_prompt(&request),
            }],
        };

        let mut http_request = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .json(&body);
        if let Some(api_key) = &self.api_key {
            http_request = http_request.bearer_auth(api_key);
        }

        let response = http_request
            .send()
            .await
            .map_err(|err| RuntimeError::ModelError {
                message: format!("request to {} failed: {err}", self.base_url),
                span: request.span,
            })?;

        if !response.status().is_success() {
            return Err(RuntimeError::ModelError {
                message: format!("{} responded with {}", self.base_url, response.status()),
                span: request.span,
            });
        }

        let parsed: ChatResponse =
            response
                .json()
                .await
                .map_err(|err| RuntimeError::ModelError {
                    message: format!("could not parse the response as JSON: {err}"),
                    span: request.span,
                })?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| RuntimeError::ModelError {
                message: "response had no message content".to_string(),
                span: request.span,
            })?;

        let value = parse_response_text(&content, &request.return_type, request.span)?;
        Ok(InferenceOutcome::Answer(value))
    }
}

fn build_prompt(request: &InferenceRequest) -> String {
    let args_description = if request.args.is_empty() {
        "no arguments".to_string()
    } else {
        request
            .args
            .iter()
            .enumerate()
            .map(|(index, value)| format!("argument {}: {value}", index + 1))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "You are implementing the function `{}`. Given {args_description}, {}",
        request.function,
        expected_shape(
            &request.return_type,
            request.return_type_variants.as_deref()
        ),
    )
}

fn expected_shape(ty: &Type, variants: Option<&[String]>) -> String {
    match ty {
        Type::Bool => "respond with exactly `true` or `false` and nothing else.".to_string(),
        Type::Int => "respond with a single integer and nothing else.".to_string(),
        Type::Float => "respond with a single number and nothing else.".to_string(),
        Type::String => "respond with the answer as plain text and nothing else.".to_string(),
        Type::Enum(name) => match variants {
            Some(names) if !names.is_empty() => format!(
                "respond with exactly one of these {} variant names and nothing else: {}.",
                name,
                names.join(", ")
            ),
            _ => format!(
                "respond with exactly one variant name of the `{name}` enum and nothing else."
            ),
        },
        other => format!("respond with a value of type {other} and nothing else."),
    }
}

/// Type-directed parsing of a model's free-text response — deliberately
/// simple, not a real structured-output feature; see SPEC.md. An
/// `Enum` response isn't validated against real variant names here —
/// that's milestone 09's schema validation, which runs on whatever
/// this returns exactly as it does for `MockModel`.
fn parse_response_text(text: &str, ty: &Type, span: Span) -> Result<Value, RuntimeError> {
    let trimmed = text.trim();
    match ty {
        Type::Bool => match trimmed.to_ascii_lowercase().as_str() {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(RuntimeError::ModelError {
                message: format!("expected `true` or `false`, got {trimmed:?}"),
                span,
            }),
        },
        Type::Int => trimmed
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| RuntimeError::ModelError {
                message: format!("expected an integer, got {trimmed:?}"),
                span,
            }),
        Type::Float => {
            trimmed
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| RuntimeError::ModelError {
                    message: format!("expected a number, got {trimmed:?}"),
                    span,
                })
        }
        Type::String => Ok(Value::String(trimmed.to_string())),
        Type::Enum(name) => Ok(Value::Enum(name.clone(), trimmed.to_string())),
        other => Err(RuntimeError::ModelError {
            message: format!("HttpModel does not support responses of type {other}"),
            span,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use aint_ast::Position;

    use super::*;

    fn span() -> Span {
        Span::new(Position::new(1, 1), Position::new(1, 1))
    }

    fn request(return_type: Type) -> InferenceRequest {
        InferenceRequest {
            function: "classify".to_string(),
            args: vec![Value::String("great product".to_string())],
            return_type,
            return_type_variants: None,
            available_tools: vec![],
            history: vec![],
            span: span(),
        }
    }

    /// Starts a minimal HTTP/1.1 server that accepts exactly one
    /// connection, reads (and discards) whatever request arrives, and
    /// writes back `raw_response` — a full, properly-framed HTTP
    /// response. Returns the base URL to point an `HttpModel` at. See
    /// SPEC.md for why this is hand-rolled instead of a mocking crate.
    fn start_mock_server(raw_response: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind a local port");
        let addr = listener.local_addr().expect("failed to read local addr");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 65536];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(raw_response.as_bytes());
                let _ = stream.flush();
            }
        });
        format!("http://{addr}")
    }

    fn http_ok(json_body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            json_body.len(),
            json_body
        )
    }

    #[tokio::test]
    async fn parses_a_bool_answer() {
        let base_url =
            start_mock_server(http_ok(r#"{"choices":[{"message":{"content":"true"}}]}"#));
        let model = HttpModel::new(base_url, "test-model");
        let outcome = model.infer(request(Type::Bool)).await.unwrap();
        assert_eq!(outcome, InferenceOutcome::Answer(Value::Bool(true)));
    }

    #[tokio::test]
    async fn parses_an_enum_answer_without_validating_the_variant() {
        // Validation is milestone 09's schema-validation job, which
        // runs downstream in `Interpreter`, not inside `HttpModel`.
        let base_url = start_mock_server(http_ok(
            r#"{"choices":[{"message":{"content":"  Positive  "}}]}"#,
        ));
        let model = HttpModel::new(base_url, "test-model");
        let outcome = model
            .infer(request(Type::Enum("Sentiment".to_string())))
            .await
            .unwrap();
        assert_eq!(
            outcome,
            InferenceOutcome::Answer(Value::Enum("Sentiment".to_string(), "Positive".to_string()))
        );
    }

    #[test]
    fn the_prompt_lists_real_variant_names_when_theyre_known() {
        let mut req = request(Type::Enum("Sentiment".to_string()));
        req.return_type_variants = Some(vec!["Positive".to_string(), "Negative".to_string()]);
        let prompt = build_prompt(&req);
        assert!(prompt.contains("Positive"));
        assert!(prompt.contains("Negative"));
    }

    #[test]
    fn the_prompt_falls_back_to_a_generic_shape_when_variants_are_unknown() {
        let req = request(Type::Enum("Sentiment".to_string()));
        let prompt = build_prompt(&req);
        assert!(prompt.contains("variant name of the `Sentiment` enum"));
    }

    #[tokio::test]
    async fn parses_an_int_answer() {
        let base_url = start_mock_server(http_ok(r#"{"choices":[{"message":{"content":"42"}}]}"#));
        let model = HttpModel::new(base_url, "test-model");
        let outcome = model.infer(request(Type::Int)).await.unwrap();
        assert_eq!(outcome, InferenceOutcome::Answer(Value::Int(42)));
    }

    #[tokio::test]
    async fn a_response_that_does_not_match_the_expected_type_is_a_clear_model_error() {
        let base_url = start_mock_server(http_ok(
            r#"{"choices":[{"message":{"content":"not a number"}}]}"#,
        ));
        let model = HttpModel::new(base_url, "test-model");
        let err = model.infer(request(Type::Int)).await.unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }

    #[tokio::test]
    async fn a_non_success_status_is_a_clear_model_error() {
        let base_url = start_mock_server(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
        );
        let model = HttpModel::new(base_url, "test-model");
        let err = model.infer(request(Type::Bool)).await.unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }

    #[tokio::test]
    async fn malformed_json_is_a_clear_model_error() {
        let base_url = start_mock_server(http_ok("not json"));
        let model = HttpModel::new(base_url, "test-model");
        let err = model.infer(request(Type::Bool)).await.unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }

    #[tokio::test]
    async fn a_connection_failure_is_a_clear_model_error() {
        // Nothing is listening on this port - bind then immediately
        // drop the listener, freeing the port but guaranteeing nothing
        // answers it.
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        let addr = listener.local_addr().expect("failed to read local addr");
        drop(listener);

        let model = HttpModel::new(format!("http://{addr}"), "test-model");
        let err = model.infer(request(Type::Bool)).await.unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }

    #[tokio::test]
    async fn distribution_return_type_is_rejected_without_a_network_call() {
        let model = HttpModel::new("http://127.0.0.1:1", "test-model");
        let err = model
            .infer(request(Type::Distribution(Box::new(Type::Enum(
                "Sentiment".to_string(),
            )))))
            .await
            .unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }

    #[tokio::test]
    async fn available_tools_is_rejected_without_a_network_call() {
        let model = HttpModel::new("http://127.0.0.1:1", "test-model");
        let mut req = request(Type::Bool);
        req.available_tools = vec![crate::tool::ToolSignature {
            name: "database_get_email".to_string(),
            params: vec![Type::String],
            return_type: Type::String,
        }];
        let err = model.infer(req).await.unwrap_err();
        assert!(matches!(err, RuntimeError::ModelError { .. }));
    }
}
