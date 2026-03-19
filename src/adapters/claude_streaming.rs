//! Streaming implementation for Claude adapter
//!
//! Implements SSE streaming for Anthropic Claude API.

use async_stream::stream;
use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;

use crate::error::AxonError;
use crate::protocol::{AgentId, LlmMessage};
use crate::Result;

use super::claude::ClaudeAdapter;
use super::streaming::{BoxStream, StreamChunk, StreamResult, StreamUsage, StreamingAdapter};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

/// SSE event from Anthropic streaming API
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageInfo },

    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },

    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: u32, delta: ContentDelta },

    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },

    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDeltaInfo, usage: Usage },

    #[serde(rename = "message_stop")]
    MessageStop,

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "error")]
    Error { error: ErrorInfo },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageInfo {
    id: String,
    model: String,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ContentBlock {
    r#type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageDeltaInfo {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Usage {
    input_tokens: Option<u32>,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ErrorInfo {
    r#type: String,
    message: String,
}

impl ClaudeAdapter {
    /// Create streaming request body
    fn create_streaming_request(
        &self,
        message: &LlmMessage,
        history: &[LlmMessage],
    ) -> serde_json::Value {
        let base = self.to_anthropic_request(message, history);
        let mut json = serde_json::to_value(base).unwrap_or_default();

        // Add stream flag
        if let serde_json::Value::Object(ref mut map) = json {
            map.insert("stream".to_string(), serde_json::Value::Bool(true));
        }

        json
    }

    /// Parse SSE line into event
    fn parse_sse_line(line: &str) -> Option<StreamEvent> {
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                return None;
            }
            serde_json::from_str(data).ok()
        } else {
            None
        }
    }
}

#[async_trait]
impl StreamingAdapter for ClaudeAdapter {
    fn agent_id(&self) -> &AgentId {
        &self.config.id
    }

    async fn process_stream<'a>(
        &'a self,
        message: &'a LlmMessage,
    ) -> Result<BoxStream<'a, StreamResult>> {
        self.process_stream_with_history(message, &[]).await
    }

    async fn process_stream_with_history<'a>(
        &'a self,
        message: &'a LlmMessage,
        history: &'a [LlmMessage],
    ) -> Result<BoxStream<'a, StreamResult>> {
        let request_body = self.create_streaming_request(message, history);

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AxonError::api(format!("Stream request failed: {}", e)))?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(60);
            return Err(AxonError::RateLimited {
                retry_after_secs: retry_after,
            });
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AxonError::api(format!(
                "Stream API error ({}): {}",
                status, error_text
            )));
        }

        let byte_stream = response.bytes_stream();
        let mut input_tokens = 0u32;

        let chunk_stream = stream! {
            let mut buffer = String::new();
            let mut stream = byte_stream;

            while let Some(result) = stream.next().await {
                match result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // Process complete lines
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim().to_string();
                            buffer = buffer[newline_pos + 1..].to_string();

                            if line.is_empty() {
                                continue;
                            }

                            if let Some(event) = Self::parse_sse_line(&line) {
                                match event {
                                    StreamEvent::MessageStart { message: info } => {
                                        input_tokens = info.usage.input_tokens.unwrap_or(0);
                                    }
                                    StreamEvent::ContentBlockDelta { delta, .. } => {
                                        match delta {
                                            ContentDelta::TextDelta { text } => {
                                                yield Ok(StreamChunk::delta(text));
                                            }
                                        }
                                    }
                                    StreamEvent::MessageDelta { delta, usage } => {
                                        let stream_usage = StreamUsage {
                                            input_tokens,
                                            output_tokens: usage.output_tokens,
                                        };
                                        yield Ok(StreamChunk::final_chunk(
                                            delta.stop_reason,
                                            Some(stream_usage),
                                        ));
                                    }
                                    StreamEvent::Error { error } => {
                                        yield Err(AxonError::api(format!(
                                            "Stream error: {}",
                                            error.message
                                        )));
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(AxonError::api(format!("Stream read error: {}", e)));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(chunk_stream))
    }
}

