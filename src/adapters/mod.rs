//! LLM provider adapters
//!
//! Adapters for different LLM providers:
//! - `ClaudeAdapter` - Anthropic Claude API
//! - `GeminiAdapter` - Google Gemini API
//! - `OpenAiAdapter` - OpenAI GPT API
//! - `OllamaAdapter` - Local Ollama

// TODO: Implement adapters
// pub mod claude;
// pub mod gemini;
// pub mod openai;
// pub mod ollama;

use async_trait::async_trait;

/// Trait for LLM adapters
#[async_trait]
pub trait LlmAdapter: Send + Sync {
    /// Get the agent ID
    fn agent_id(&self) -> &str;

    /// Process a message and return a response
    async fn process(&self, message: &str) -> crate::Result<String>;

    /// Get available tools for this adapter
    fn available_tools(&self) -> Vec<String> {
        vec![]
    }
}
