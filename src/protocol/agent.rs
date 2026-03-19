//! Agent identification and configuration
//!
//! Types for identifying and configuring LLM agents.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for an LLM agent
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new agent ID
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the agent name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for AgentId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for AgentId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// LLM provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// Anthropic Claude
    Anthropic,
    /// Google Gemini
    Google,
    /// OpenAI GPT
    OpenAi,
    /// Local Ollama
    Ollama,
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anthropic => write!(f, "anthropic"),
            Self::Google => write!(f, "google"),
            Self::OpenAi => write!(f, "openai"),
            Self::Ollama => write!(f, "ollama"),
        }
    }
}

impl std::str::FromStr for Provider {
    type Err = crate::AxonError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Ok(Self::Anthropic),
            "google" | "gemini" => Ok(Self::Google),
            "openai" | "gpt" => Ok(Self::OpenAi),
            "ollama" | "local" => Ok(Self::Ollama),
            _ => Err(crate::AxonError::config(format!("Unknown provider: {}", s))),
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent identifier
    pub id: AgentId,

    /// LLM provider
    pub provider: Provider,

    /// Model name (e.g., "claude-sonnet-4-20250514", "gemini-pro")
    pub model: String,

    /// Custom API endpoint (for Ollama or custom deployments)
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Environment variable name for API key
    #[serde(default)]
    pub api_key_env: Option<String>,

    /// System prompt for this agent
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Maximum tokens for response
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Temperature setting (0.0 - 1.0)
    #[serde(default)]
    pub temperature: Option<f32>,
}

impl AgentConfig {
    /// Create a new agent configuration
    pub fn new(id: impl Into<AgentId>, provider: Provider, model: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider,
            model: model.into(),
            endpoint: None,
            api_key_env: None,
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        }
    }

    /// Set custom endpoint
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set API key environment variable
    pub fn with_api_key_env(mut self, env_var: impl Into<String>) -> Self {
        self.api_key_env = Some(env_var.into());
        self
    }

    /// Set system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_creation() {
        let id = AgentId::new("claude");
        assert_eq!(id.as_str(), "claude");
        assert_eq!(id.to_string(), "claude");
    }

    #[test]
    fn test_agent_id_from_str() {
        let id: AgentId = "gemini".into();
        assert_eq!(id.as_str(), "gemini");
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(Provider::Anthropic.to_string(), "anthropic");
        assert_eq!(Provider::Google.to_string(), "google");
        assert_eq!(Provider::OpenAi.to_string(), "openai");
        assert_eq!(Provider::Ollama.to_string(), "ollama");
    }

    #[test]
    fn test_provider_from_str() {
        assert_eq!("anthropic".parse::<Provider>().unwrap(), Provider::Anthropic);
        assert_eq!("claude".parse::<Provider>().unwrap(), Provider::Anthropic);
        assert_eq!("google".parse::<Provider>().unwrap(), Provider::Google);
        assert_eq!("gemini".parse::<Provider>().unwrap(), Provider::Google);
        assert_eq!("openai".parse::<Provider>().unwrap(), Provider::OpenAi);
        assert_eq!("ollama".parse::<Provider>().unwrap(), Provider::Ollama);
    }

    #[test]
    fn test_agent_config() {
        let config = AgentConfig::new("claude", Provider::Anthropic, "claude-sonnet-4-20250514")
            .with_api_key_env("ANTHROPIC_API_KEY")
            .with_system_prompt("You are a helpful assistant.");

        assert_eq!(config.id.as_str(), "claude");
        assert_eq!(config.provider, Provider::Anthropic);
        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert_eq!(config.api_key_env, Some("ANTHROPIC_API_KEY".to_string()));
    }

    #[test]
    fn test_agent_config_serialization() {
        let config = AgentConfig::new("test", Provider::Ollama, "llama2")
            .with_endpoint("http://localhost:11434");

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id.as_str(), "test");
        assert_eq!(deserialized.provider, Provider::Ollama);
        assert_eq!(deserialized.endpoint, Some("http://localhost:11434".to_string()));
    }
}
