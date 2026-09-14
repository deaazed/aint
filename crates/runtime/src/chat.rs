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

use std::time::Duration;

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

        let response = self.send_with_retry(&body).await?;

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

    /// Same reasoning and shape as `HttpModel::send_with_retry`
    /// (deliberately duplicated, not shared — see this file's own doc
    /// comment): a transient `429`/`5xx` is retried with backoff, a
    /// sustained one (the account's real quota) still fails once
    /// retries are exhausted. `aint migrate --ai` (milestone 45) is
    /// this client's heaviest real user — potentially one call per
    /// file — so this matters more here than it did for scaffold's
    /// single call.
    async fn send_with_retry(&self, body: &ChatRequest<'_>) -> Result<reqwest::Response, String> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0u32;
        loop {
            let mut http_request = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .json(body);
            if let Some(api_key) = &self.api_key {
                http_request = http_request.bearer_auth(api_key);
            }

            let response = http_request
                .send()
                .await
                .map_err(|err| format!("request to {} failed: {err}", self.base_url))?;

            let status = response.status();
            if status.is_success() {
                return Ok(response);
            }
            let retryable = status.as_u16() == 429 || status.is_server_error();
            if !retryable || attempt >= MAX_RETRIES {
                return Err(format!("{} responded with {status}", self.base_url));
            }

            let delay = retry_after(&response)
                .unwrap_or_else(|| Duration::from_millis(500 * 2u64.pow(attempt)).min(RETRY_CAP));
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }
}

const RETRY_CAP: Duration = Duration::from_secs(8);

fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
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
        start_mock_server_sequence(vec![raw_response; 5])
    }

    /// Serves one response per connection from `raw_responses`, in
    /// order — needed once retry-with-backoff means a single-response
    /// server would otherwise see a *second* connection it can't
    /// answer, masking the status this test actually wants to observe
    /// behind an unrelated connection-refused error.
    fn start_mock_server_sequence(raw_responses: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind a local port");
        let addr = listener.local_addr().expect("failed to read local addr");
        std::thread::spawn(move || {
            for raw_response in raw_responses {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 65536];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(raw_response.as_bytes());
                    let _ = stream.flush();
                }
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

    #[tokio::test]
    async fn a_429_is_retried_and_a_later_success_is_returned() {
        let base_url = start_mock_server_sequence(vec![
            "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
            http_ok(r#"{"choices":[{"message":{"content":"hello"}}]}"#),
        ]);
        let client = ChatClient::new(base_url, "test-model");
        let text = client.complete("system", "user").await.unwrap();
        assert_eq!(text, "hello");
    }
}
