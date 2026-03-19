//! Configuration management
//!
//! Handles loading and managing configuration from:
//! - `~/.axon/config.toml`
//! - Environment variables
//! - Command line arguments

use serde::{Deserialize, Serialize};

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Agent configurations
    #[serde(default)]
    pub agents: std::collections::HashMap<String, AgentConfig>,

    /// Tool configurations
    #[serde(default)]
    pub tools: std::collections::HashMap<String, ToolConfig>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,

    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_port() -> u16 {
    8090
}
fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            log_level: default_log_level(),
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Provider name (anthropic, google, openai, ollama)
    pub provider: String,

    /// Model name
    pub model: String,

    /// Environment variable for API key
    pub api_key_env: Option<String>,

    /// Custom endpoint (for Ollama)
    pub endpoint: Option<String>,
}

/// Tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Tool endpoint
    pub endpoint: String,

    /// API key environment variable
    pub api_key_env: Option<String>,
}

impl Config {
    /// Load configuration from file
    pub fn load(path: Option<&str>) -> crate::Result<Self> {
        let config_path = path
            .map(std::path::PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".axon").join("config.toml")));

        if let Some(path) = config_path {
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                let config: Config = toml::from_str(&content).map_err(|e| {
                    crate::AxonError::config(format!("Failed to parse config: {}", e))
                })?;
                return Ok(config);
            }
        }

        Ok(Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.port, 8090);
        assert_eq!(config.server.host, "127.0.0.1");
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
[server]
port = 9000
host = "0.0.0.0"

[agents.claude]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"

[tools.minky]
endpoint = "http://localhost:3000/api"
"#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.server.port, 9000);
        assert!(config.agents.contains_key("claude"));
        assert!(config.tools.contains_key("minky"));
    }
}
