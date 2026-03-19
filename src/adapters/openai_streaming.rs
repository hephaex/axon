//! Streaming implementation for OpenAI adapter
//!
//! Implements SSE streaming for OpenAI GPT API.

use async_stream::stream;
use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;

use crate::error::AxonError;
use crate::protocol::{AgentId, LlmMessage};
use crate::Result;

use super::openai::OpenAiAdapter;
use super::streaming::{BoxStream, StreamChunk, StreamResult, StreamUsage, StreamingAdapter};

/// SSE chunk from OpenAI streaming API
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamResponse {
    id: String,
    model: String,
    choices: Vec<StreamChoice>,
    usage: Option<StreamResponseUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamChoice {
    index: u32,
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Delta {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ToolCallDelta {
    index: u32,
    id: Option<String>,
    function: Option<FunctionDelta>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamResponseUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl OpenAiAdapter {
    /// Create streaming request body
    fn create_streaming_request(
        &self,
        message: &LlmMessage,
        history: &[LlmMessage],
    ) -> serde_json::Value {
        let base = self.to_openai_request(message, history);
        let mut json = serde_json::to_value(base).unwrap_or_default();

        // Add stream flag and stream_options
        if let serde_json::Value::Object(ref mut map) = json {
            map.insert("stream".to_string(), serde_json::Value::Bool(true));
            // Request usage data in streaming response
            map.insert(
                "stream_options".to_string(),
                serde_json::json!({"include_usage": true}),
            );
        }

        json
    }

    /// Parse SSE line into stream response
    fn parse_sse_data(data: &str) -> Option<StreamResponse> {
        if data == "[DONE]" {
            return None;
        }
        serde_json::from_str(data).ok()
    }
}

#[async_trait]
impl StreamingAdapter for OpenAiAdapter {
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
        let url = self.get_api_url();

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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
        let mut usage_info: Option<StreamResponseUsage> = None;

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

                            if line.is_empty() || !line.starts_with("data: ") {
                                continue;
                            }

                            let data = &line[6..];

                            if let Some(resp) = Self::parse_sse_data(data) {
                                // Check for usage info (sent in last message)
                                if let Some(usage) = resp.usage {
                                    usage_info = Some(usage);
                                }

                                for choice in resp.choices {
                                    // Emit text delta
                                    if let Some(content) = choice.delta.content {
                                        if !content.is_empty() {
                                            yield Ok(StreamChunk::delta(content));
                                        }
                                    }

                                    // Check for finish
                                    if let Some(finish_reason) = choice.finish_reason {
                                        let stream_usage = usage_info.as_ref().map(|u| StreamUsage {
                                            input_tokens: u.prompt_tokens,
                                            output_tokens: u.completion_tokens,
                                        });
                                        yield Ok(StreamChunk::final_chunk(
                                            Some(finish_reason),
                                            stream_usage,
                                        ));
                                    }
                                }
                            } else if data == "[DONE]" {
                                // Stream complete
                                break;
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
