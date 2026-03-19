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

use std::sync::Arc;

pub use claude::ClaudeAdapter;
pub use gemini::GeminiAdapter;
pub use ollama::OllamaAdapter;
pub use openai::OpenAiAdapter;
pub use streaming::{
    collect_stream, BoxStream, StreamChunk, StreamResult, StreamUsage, StreamingAdapter,
};

use async_trait::async_trait;

use crate::protocol::{AgentConfig, AgentId, LlmMessage, Provider};
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

    /// Get streaming adapter if this adapter supports streaming
    fn as_streaming(&self) -> Option<&dyn StreamingAdapter> {
        None
    }
}

/// Builder for creating adapters with fluent API
pub struct AdapterBuilder {
    config: AgentConfig,
    system_prompt: Option<String>,
    api_key: Option<String>,
}

impl AdapterBuilder {
    /// Create a new adapter builder
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            system_prompt: None,
            api_key: None,
        }
    }

    /// Set the system prompt
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        let prompt = prompt.into();
        if !prompt.is_empty() {
            self.system_prompt = Some(prompt);
        }
        self
    }

    /// Set the API key (overrides environment variable)
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Build the adapter
    pub fn build(self) -> crate::Result<Arc<dyn LlmAdapter>> {
        let mut config = self.config;

        // Apply system prompt if set
        if let Some(prompt) = self.system_prompt {
            config = config.with_system_prompt(prompt);
        }

        // Set API key in environment if provided
        if let Some(key) = self.api_key {
            match config.provider {
                Provider::Anthropic => std::env::set_var("ANTHROPIC_API_KEY", &key),
                Provider::Google => std::env::set_var("GOOGLE_API_KEY", &key),
                Provider::OpenAi => std::env::set_var("OPENAI_API_KEY", &key),
                Provider::Ollama => {} // Ollama doesn't need API key
            }
        }

        Self::from_config(config)
    }

    /// Create an adapter from agent configuration
    pub fn from_config(config: AgentConfig) -> crate::Result<Arc<dyn LlmAdapter>> {
        match config.provider {
            Provider::Anthropic => {
                let adapter = claude::ClaudeAdapter::new(config)?;
                Ok(Arc::new(adapter))
            }
            Provider::Google => {
                let adapter = gemini::GeminiAdapter::new(config)?;
                Ok(Arc::new(adapter))
            }
            Provider::OpenAi => {
                let adapter = openai::OpenAiAdapter::new(config)?;
                Ok(Arc::new(adapter))
            }
            Provider::Ollama => {
                let adapter = ollama::OllamaAdapter::new(config)?;
                Ok(Arc::new(adapter))
            }
        }
    }
}
