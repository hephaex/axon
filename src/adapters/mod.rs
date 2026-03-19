//! LLM provider adapters
//!
//! Adapters for different LLM providers:
//! - `ClaudeAdapter` - Anthropic Claude API
//! - `GeminiAdapter` - Google Gemini API
//! - `OpenAiAdapter` - OpenAI GPT API
//! - `OllamaAdapter` - Local Ollama

pub mod claude;

pub use claude::ClaudeAdapter;

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
                Err(crate::AxonError::config("Gemini adapter not yet implemented"))
            }
            Provider::OpenAi => {
                Err(crate::AxonError::config("OpenAI adapter not yet implemented"))
            }
            Provider::Ollama => {
                Err(crate::AxonError::config("Ollama adapter not yet implemented"))
            }
        }
    }
}
