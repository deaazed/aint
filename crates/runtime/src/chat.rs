//! A plain, unstructured chat-completion call to any OpenAI-compatible
//! endpoint — the same wire protocol [`crate::HttpModel`] speaks for
//! `infer`, but without the type-directed request/response shaping an
//! `infer` call needs (a system prompt, a user prompt, one string back
//! — no [`crate::InferenceRequest`]/[`crate::InferenceOutcome`], no
//! declared return type to parse against). Used by `aint scaffold`
//! (milestone 32) to ask a model for free-form AINT source, not a
//! structured inference answer — a genuinely different shape of
//! request, not a special case of [`crate::Model`]. See
//! `docs/milestones/32-ai-scaffolding/SPEC.md`.
//!
//! Deliberately duplicates `http_model.rs`'s tiny `ChatRequest`/
//! `ChatMessage`/`ChatResponse` shapes rather than sharing them — the
//! same small-duplication-over-coupling call this codebase already
//! makes for the typechecker/runtime stdlib signature tables (see
//! `stdlib.rs`'s own doc comment).

use serde::{Deserialize, Serialize};

pub struct ChatClient {
    base_url: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl ChatClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            api_key: None,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sends `system_prompt` and `user_prompt` as a two-message chat
    /// completion request and returns the response's raw text content.
    pub async fn complete(&self, system_prompt: &str, user_prompt: &str) -> Result<String, String> {
        let body = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: user_prompt.to_string(),
                },
            ],
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
            .map_err(|err| format!("request to {} failed: {err}", self.base_url))?;

        if !response.status().is_success() {
            return Err(format!(
                "{} responded with {}",
                self.base_url,
                response.status()
            ));
        }

        let parsed: ChatResponse = response
            .json()
            .await
            .map_err(|err| format!("could not parse the response as JSON: {err}"))?;

        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| "response had no message content".to_string())
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

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

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
    async fn returns_the_response_text() {
        let base_url =
            start_mock_server(http_ok(r#"{"choices":[{"message":{"content":"hello"}}]}"#));
        let client = ChatClient::new(base_url, "test-model");
        let text = client.complete("system", "user").await.unwrap();
        assert_eq!(text, "hello");
    }

    #[tokio::test]
    async fn a_non_success_status_is_a_clear_error() {
        let base_url = start_mock_server(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
        );
        let client = ChatClient::new(base_url, "test-model");
        let err = client.complete("system", "user").await.unwrap_err();
        assert!(err.contains("500"));
    }
}
