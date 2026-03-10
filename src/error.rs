//! Error types for Axon
//!
//! Centralized error handling for the LLM-to-LLM communication framework.

use thiserror::Error;

/// Result type alias for Axon operations
pub type Result<T> = std::result::Result<T, AxonError>;

/// Main error type for Axon
#[derive(Error, Debug)]
pub enum AxonError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// API communication errors
    #[error("API error: {0}")]
    Api(String),

    /// Message protocol errors
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Agent errors
    #[error("Agent error: {message}")]
    Agent {
        agent_id: String,
        message: String,
    },

    /// Router errors
    #[error("Router error: {0}")]
    Router(String),

    /// Tool execution errors
    #[error("Tool error: {tool} - {message}")]
    Tool {
        tool: String,
        message: String,
    },

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// HTTP client errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Timeout errors
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Rate limit errors
    #[error("Rate limited: retry after {retry_after_secs} seconds")]
    RateLimited {
        retry_after_secs: u64,
    },

    /// Authentication errors
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Conversation errors
    #[error("Conversation error: {0}")]
    Conversation(String),

    /// Unknown/internal errors
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AxonError {
    /// Create a new configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a new API error
    pub fn api(msg: impl Into<String>) -> Self {
        Self::Api(msg.into())
    }

    /// Create a new protocol error
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    /// Create a new agent error
    pub fn agent(agent_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Agent {
            agent_id: agent_id.into(),
            message: message.into(),
        }
    }

    /// Create a new tool error
    pub fn tool(tool: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Tool {
            tool: tool.into(),
            message: message.into(),
        }
    }

    /// Check if this is a retryable error
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Http(_) | Self::Timeout(_) | Self::RateLimited { .. }
        )
    }

    /// Get retry delay in seconds if applicable
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            Self::RateLimited { retry_after_secs } => Some(*retry_after_secs),
            Self::Timeout(_) => Some(1),
            Self::Http(_) => Some(2),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AxonError::config("missing API key");
        assert_eq!(err.to_string(), "Configuration error: missing API key");
    }

    #[test]
    fn test_agent_error() {
        let err = AxonError::agent("claude-1", "connection failed");
        assert_eq!(
            err.to_string(),
            "Agent error: connection failed"
        );
    }

    #[test]
    fn test_tool_error() {
        let err = AxonError::tool("minky_search", "timeout");
        assert_eq!(err.to_string(), "Tool error: minky_search - timeout");
    }

    #[test]
    fn test_retryable() {
        let timeout = AxonError::Timeout("request timed out".into());
        assert!(timeout.is_retryable());

        let config = AxonError::config("invalid");
        assert!(!config.is_retryable());
    }

    #[test]
    fn test_retry_after() {
        let rate_limited = AxonError::RateLimited { retry_after_secs: 30 };
        assert_eq!(rate_limited.retry_after(), Some(30));

        let config = AxonError::config("invalid");
        assert_eq!(config.retry_after(), None);
    }
}
