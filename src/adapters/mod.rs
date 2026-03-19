//! LLM provider adapters
//!
//! Adapters for different LLM providers:
//! - `ClaudeAdapter` - Anthropic Claude API
//! - `GeminiAdapter` - Google Gemini API
//! - `OpenAiAdapter` - OpenAI GPT API
//! - `OllamaAdapter` - Local Ollama
//!
//! Streaming support:
//! - `StreamingAdapter` trait for streaming responses
//! - `StreamChunk` for response chunks

pub mod claude;
mod claude_streaming;
pub mod gemini;
mod gemini_streaming;
pub mod ollama;
mod ollama_streaming;
pub mod openai;
mod openai_streaming;
pub mod streaming;

pub use claude::ClaudeAdapter;
pub use gemini::GeminiAdapter;
pub use ollama::OllamaAdapter;
pub use openai::OpenAiAdapter;
pub use streaming::{collect_stream, BoxStream, StreamChunk, StreamResult, StreamUsage, StreamingAdapter};

use async_trait::async_trait;

use crate::protocol::{AgentConfig, AgentId, LlmMessage};
use crate::tools::ToolDefinition;

/// Trait for LLM adapters
///
/// All LLM providers implement this trait to enable unified message processing.
#[async_trait]
pub trait LlmAdapter: Send + Sync {
    /// Get the agent ID
    fn agent_id(&self) -> &AgentId;

    /// Get the agent configuration
    fn config(&self) -> &AgentConfig;

    /// Process a message and return a response
    ///
    /// The adapter converts the LlmMessage to the provider's format,
    /// calls the API, and converts the response back to LlmMessage.
    async fn process(&self, message: &LlmMessage) -> crate::Result<LlmMessage>;

    /// Process a message with conversation history
    async fn process_with_history(
        &self,
        message: &LlmMessage,
        _history: &[LlmMessage],
    ) -> crate::Result<LlmMessage> {
        // Default implementation ignores history
        self.process(message).await
    }

    /// Get available tools for this adapter
    fn available_tools(&self) -> Vec<ToolDefinition> {
        vec![]
    }

    /// Register a tool with this adapter
    fn register_tool(&mut self, _tool: ToolDefinition) {
        // Default: no-op
    }

    /// Check if the adapter is healthy/connected
    async fn health_check(&self) -> crate::Result<bool> {
        Ok(true)
    }
}

/// Builder for creating adapters from configuration
pub struct AdapterBuilder;

impl AdapterBuilder {
    /// Create an adapter from agent configuration
    pub fn from_config(config: AgentConfig) -> crate::Result<Box<dyn LlmAdapter>> {
        use crate::protocol::Provider;

        match config.provider {
            Provider::Anthropic => {
                let adapter = claude::ClaudeAdapter::new(config)?;
                Ok(Box::new(adapter))
            }
            Provider::Google => {
                let adapter = gemini::GeminiAdapter::new(config)?;
                Ok(Box::new(adapter))
            }
            Provider::OpenAi => {
                let adapter = openai::OpenAiAdapter::new(config)?;
                Ok(Box::new(adapter))
            }
            Provider::Ollama => {
                let adapter = ollama::OllamaAdapter::new(config)?;
                Ok(Box::new(adapter))
            }
        }
    }
}
