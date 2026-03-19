//! Web fetch tools
//!
//! Provides HTTP fetch capabilities for LLM agents.

use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

use crate::error::AxonError;
use crate::Result;

use super::registry::Tool;
use super::{ToolDefinition, ToolResult};

/// Configuration for web tools
#[derive(Debug, Clone)]
pub struct WebConfig {
    /// Request timeout
    pub timeout: Duration,
    /// Maximum response size (bytes)
    pub max_response_size: usize,
    /// User agent string
    pub user_agent: String,
    /// Allowed URL schemes
    pub allowed_schemes: Vec<String>,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_response_size: 1024 * 1024, // 1MB
            user_agent: "Axon/0.1.0".to_string(),
            allowed_schemes: vec!["http".to_string(), "https".to_string()],
        }
    }
}

/// Tool for fetching web content
pub struct WebFetchTool {
    client: Client,
    config: WebConfig,
}

impl WebFetchTool {
    pub fn new(config: WebConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| AxonError::tool("web_fetch", format!("Failed to create client: {}", e)))?;

        Ok(Self { client, config })
    }

    fn validate_url(&self, url: &str) -> Result<()> {
        let parsed = url::Url::parse(url)
            .map_err(|e| AxonError::tool("web_fetch", format!("Invalid URL: {}", e)))?;

        let scheme = parsed.scheme();
        if !self.config.allowed_schemes.contains(&scheme.to_string()) {
            return Err(AxonError::tool(
                "web_fetch",
                format!("URL scheme '{}' not allowed", scheme),
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_fetch".to_string(),
            description: "Fetch content from a URL".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to fetch"
                    },
                    "method": {
                        "type": "string",
                        "enum": ["GET", "POST"],
                        "default": "GET",
                        "description": "HTTP method"
                    },
                    "headers": {
                        "type": "object",
                        "description": "Additional headers to send"
                    },
                    "body": {
                        "type": "string",
                        "description": "Request body (for POST)"
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("web_fetch", "Missing 'url' argument"))?;

        self.validate_url(url)?;

        let method = args
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let mut request = match method.to_uppercase().as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            _ => return Err(AxonError::tool("web_fetch", "Unsupported HTTP method")),
        };

        // Add custom headers
        if let Some(headers) = args.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers {
                if let Some(v) = value.as_str() {
                    request = request.header(key.as_str(), v);
                }
            }
        }

        // Add body for POST
        if method.to_uppercase() == "POST" {
            if let Some(body) = args.get("body").and_then(|v| v.as_str()) {
                request = request.body(body.to_string());
            }
        }

        let response = request.send().await.map_err(|e| {
            AxonError::tool("web_fetch", format!("Request failed: {}", e))
        })?;

        let status = response.status();
        let content_length = response.content_length().unwrap_or(0) as usize;

        if content_length > self.config.max_response_size {
            return Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!(
                    "Response too large: {} bytes (max: {} bytes)",
                    content_length, self.config.max_response_size
                )),
            });
        }

        let body = response.text().await.map_err(|e| {
            AxonError::tool("web_fetch", format!("Failed to read response: {}", e))
        })?;

        if body.len() > self.config.max_response_size {
            return Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!(
                    "Response too large: {} bytes (max: {} bytes)",
                    body.len(),
                    self.config.max_response_size
                )),
            });
        }

        Ok(ToolResult {
            success: status.is_success(),
            content: body,
            error: if status.is_success() {
                None
            } else {
                Some(format!("HTTP {}", status))
            },
        })
    }

    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("web_fetch", "Missing 'url' argument"))?;

        self.validate_url(url)
    }
}

/// Register web tools with a registry
pub async fn register_web_tools(registry: &super::ToolRegistry, config: WebConfig) -> Result<()> {
    let tool = WebFetchTool::new(config)?;
    registry.register(tool).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_validation() {
        let config = WebConfig::default();
        let tool = WebFetchTool::new(config).unwrap();

        assert!(tool.validate_url("https://example.com").is_ok());
        assert!(tool.validate_url("http://example.com").is_ok());
        assert!(tool.validate_url("ftp://example.com").is_err());
        assert!(tool.validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn test_definition() {
        let config = WebConfig::default();
        let tool = WebFetchTool::new(config).unwrap();
        let def = tool.definition();

        assert_eq!(def.name, "web_fetch");
        assert!(def.parameters["properties"]["url"].is_object());
    }

    // Integration tests would require a mock server
    // Skipping actual HTTP tests here
}
