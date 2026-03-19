//! Streaming implementation for Gemini adapter
//!
//! Implements SSE streaming for Google Gemini API.

use async_stream::stream;
use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;

use crate::error::AxonError;
use crate::protocol::{AgentId, LlmMessage};
use crate::Result;

use super::gemini::GeminiAdapter;
use super::streaming::{BoxStream, StreamChunk, StreamResult, StreamUsage, StreamingAdapter};

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Streaming response from Gemini API
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamResponse {
    candidates: Option<Vec<StreamCandidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct StreamCandidate {
    content: Option<StreamContent>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamContent {
    role: Option<String>,
    parts: Option<Vec<StreamPart>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamPart {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    prompt_token_count: Option<u32>,
    candidates_token_count: Option<u32>,
    total_token_count: Option<u32>,
}

impl GeminiAdapter {
    /// Build streaming API URL for the model
    fn build_stream_url(&self) -> String {
        format!(
            "{}/{}:streamGenerateContent?key={}&alt=sse",
            GEMINI_API_URL, self.config.model, self.api_key
        )
    }

    /// Create streaming request body
    fn create_streaming_request(
        &self,
        message: &LlmMessage,
        history: &[LlmMessage],
    ) -> serde_json::Value {
        let base = self.to_gemini_request(message, history);
        serde_json::to_value(base).unwrap_or_default()
    }
}

#[async_trait]
impl StreamingAdapter for GeminiAdapter {
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
        let url = self.build_stream_url();

        let response = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AxonError::api(format!("Stream request failed: {}", e)))?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AxonError::RateLimited {
                retry_after_secs: 60,
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
        let mut usage_info: Option<UsageMetadata> = None;

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

                            if let Ok(resp) = serde_json::from_str::<StreamResponse>(data) {
                                // Track usage info
                                if let Some(usage) = resp.usage_metadata {
                                    usage_info = Some(usage);
                                }

                                if let Some(candidates) = resp.candidates {
                                    for candidate in candidates {
                                        // Emit text delta
                                        if let Some(content) = &candidate.content {
                                            if let Some(parts) = &content.parts {
                                                for part in parts {
                                                    if let Some(text) = &part.text {
                                                        if !text.is_empty() {
                                                            yield Ok(StreamChunk::delta(text.clone()));
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Check for finish
                                        if let Some(finish_reason) = &candidate.finish_reason {
                                            if finish_reason != "STOP" || candidate.content.is_none() {
                                                // Only emit final chunk on actual stop
                                                continue;
                                            }
                                            let stream_usage = usage_info.as_ref().and_then(|u| {
                                                Some(StreamUsage {
                                                    input_tokens: u.prompt_token_count?,
                                                    output_tokens: u.candidates_token_count?,
                                                })
                                            });
                                            yield Ok(StreamChunk::final_chunk(
                                                Some(finish_reason.clone()),
                                                stream_usage,
                                            ));
                                        }
                                    }
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
