//! Streaming support for LLM adapters
//!
//! Provides types and traits for streaming responses from LLM APIs.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::protocol::{AgentId, LlmMessage};
use crate::Result;

/// A chunk of streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// The content delta (new text)
    pub delta: String,

    /// Whether this is the final chunk
    pub is_final: bool,

    /// Token usage (only set on final chunk)
    pub usage: Option<StreamUsage>,

    /// Stop reason (only set on final chunk)
    pub stop_reason: Option<String>,
}

impl StreamChunk {
    /// Create a text delta chunk
    pub fn delta(text: impl Into<String>) -> Self {
        Self {
            delta: text.into(),
            is_final: false,
            usage: None,
            stop_reason: None,
        }
    }

    /// Create a final chunk
    pub fn final_chunk(stop_reason: Option<String>, usage: Option<StreamUsage>) -> Self {
        Self {
            delta: String::new(),
            is_final: true,
            usage,
            stop_reason,
        }
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Type alias for boxed stream
pub type BoxStream<'a, T> = Pin<Box<dyn Stream<Item = T> + Send + 'a>>;

/// Streaming result type
pub type StreamResult = Result<StreamChunk>;

/// Extension trait for streaming LLM adapters
#[async_trait]
pub trait StreamingAdapter: Send + Sync {
    /// Get the agent ID
    fn agent_id(&self) -> &AgentId;

    /// Check if streaming is supported
    fn supports_streaming(&self) -> bool {
        true
    }

    /// Process a message and return a stream of response chunks
    async fn process_stream<'a>(
        &'a self,
        message: &'a LlmMessage,
    ) -> Result<BoxStream<'a, StreamResult>>;

    /// Process a message with history and return a stream
    async fn process_stream_with_history<'a>(
        &'a self,
        message: &'a LlmMessage,
        history: &'a [LlmMessage],
    ) -> Result<BoxStream<'a, StreamResult>>;
}

/// Collect all chunks from a stream into a complete message
pub async fn collect_stream(
    from: AgentId,
    conversation_id: uuid::Uuid,
    mut stream: BoxStream<'_, StreamResult>,
) -> Result<LlmMessage> {
    use futures::StreamExt;
    use uuid::Uuid;

    let mut full_text = String::new();
    let mut final_usage = None;
    let mut stop_reason = None;

    while let Some(result) = stream.next().await {
        let chunk = result?;
        full_text.push_str(&chunk.delta);

        if chunk.is_final {
            final_usage = chunk.usage;
            stop_reason = chunk.stop_reason;
        }
    }

    Ok(LlmMessage {
        id: Uuid::new_v4(),
        from,
        to: None,
        message_type: crate::protocol::MessageType::Chat,
        content: crate::protocol::MessageContent::Text(full_text),
        conversation_id,
        timestamp: chrono::Utc::now(),
        metadata: Some(serde_json::json!({
            "streaming": true,
            "stop_reason": stop_reason,
            "usage": final_usage,
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_stream_chunk_delta() {
        let chunk = StreamChunk::delta("Hello");
        assert_eq!(chunk.delta, "Hello");
        assert!(!chunk.is_final);
        assert!(chunk.usage.is_none());
    }

    #[test]
    fn test_stream_chunk_final() {
        let usage = StreamUsage {
            input_tokens: 10,
            output_tokens: 20,
        };
        let chunk = StreamChunk::final_chunk(Some("end_turn".to_string()), Some(usage));
        assert!(chunk.delta.is_empty());
        assert!(chunk.is_final);
        assert!(chunk.usage.is_some());
        assert_eq!(chunk.stop_reason, Some("end_turn".to_string()));
    }

    #[tokio::test]
    async fn test_collect_stream() {
        use async_stream::stream;
        use crate::protocol::AgentId;

        let conv_id = Uuid::new_v4();
        let from: AgentId = "test-agent".into();

        let test_stream: BoxStream<'_, StreamResult> = Box::pin(stream! {
            yield Ok(StreamChunk::delta("Hello"));
            yield Ok(StreamChunk::delta(" "));
            yield Ok(StreamChunk::delta("World"));
            yield Ok(StreamChunk::final_chunk(Some("end_turn".to_string()), None));
        });

        let message = collect_stream(from, conv_id, test_stream).await.unwrap();

        if let crate::protocol::MessageContent::Text(text) = &message.content {
            assert_eq!(text, "Hello World");
        } else {
            panic!("Expected text content");
        }
    }
}
