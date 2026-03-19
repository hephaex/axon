//! Streaming implementation for Ollama adapter
//!
//! Implements NDJSON streaming for local Ollama API.

use async_stream::stream;
use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;

use crate::error::AxonError;
use crate::protocol::{AgentId, LlmMessage};
use crate::Result;

use super::ollama::OllamaAdapter;
use super::streaming::{BoxStream, StreamChunk, StreamResult, StreamUsage, StreamingAdapter};

/// Streaming response from Ollama API
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamResponse {
    model: String,
    message: StreamMessage,
    done: bool,
    done_reason: Option<String>,
    prompt_eval_count: Option<u32>,
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamMessage {
    role: String,
    content: String,
}

impl OllamaAdapter {
    /// Create streaming request body
    fn create_streaming_request(
        &self,
        message: &LlmMessage,
        history: &[LlmMessage],
    ) -> serde_json::Value {
        let base = self.to_ollama_request(message, history);
        let mut json = serde_json::to_value(base).unwrap_or_default();

        // Set stream to true
        if let serde_json::Value::Object(ref mut map) = json {
            map.insert("stream".to_string(), serde_json::Value::Bool(true));
        }

        json
    }
}

#[async_trait]
impl StreamingAdapter for OllamaAdapter {
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
        let url = self.build_url();

        let response = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    AxonError::config(format!(
                        "Cannot connect to Ollama at {}. Is Ollama running?",
                        self.base_url
                    ))
                } else {
                    AxonError::api(format!("Stream request failed: {}", e))
                }
            })?;

        let status = response.status();

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

        let chunk_stream = stream! {
            let mut buffer = String::new();
            let mut stream = byte_stream;
            let mut prompt_tokens = 0u32;

            while let Some(result) = stream.next().await {
                match result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // Process complete lines (NDJSON format)
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim().to_string();
                            buffer = buffer[newline_pos + 1..].to_string();

                            if line.is_empty() {
                                continue;
                            }

                            if let Ok(resp) = serde_json::from_str::<StreamResponse>(&line) {
                                // Track prompt tokens from first response
                                if let Some(prompt_count) = resp.prompt_eval_count {
                                    prompt_tokens = prompt_count;
                                }

                                // Emit content delta
                                if !resp.message.content.is_empty() {
                                    yield Ok(StreamChunk::delta(resp.message.content));
                                }

                                // Check if done
                                if resp.done {
                                    let stream_usage = resp.eval_count.map(|eval_count| StreamUsage {
                                        input_tokens: prompt_tokens,
                                        output_tokens: eval_count,
                                    });
                                    yield Ok(StreamChunk::final_chunk(
                                        resp.done_reason,
                                        stream_usage,
                                    ));
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
