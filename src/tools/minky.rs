//! MinKy integration tools
//!
//! Provides tools for interacting with the MinKy knowledge platform.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::AxonError;
use crate::Result;

use super::registry::Tool;
use super::{ToolDefinition, ToolResult};

/// Configuration for MinKy tools
#[derive(Debug, Clone)]
pub struct MinkyConfig {
    /// MinKy API endpoint
    pub endpoint: String,
    /// API key (optional)
    pub api_key: Option<String>,
    /// Request timeout
    pub timeout: Duration,
}

impl Default for MinkyConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:3000/api".to_string(),
            api_key: None,
            timeout: Duration::from_secs(30),
        }
    }
}

impl MinkyConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            ..Default::default()
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }
}

/// MinKy search modes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// Keyword-based search
    Keyword,
    /// Vector similarity search
    Vector,
    /// Hybrid (keyword + vector)
    #[default]
    Hybrid,
    /// Deep semantic search with AI
    Deep,
}

/// Search result from MinKy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub content: String,
    pub score: f32,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// RAG response from MinKy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResponse {
    pub answer: String,
    pub sources: Vec<SearchResult>,
    pub confidence: f32,
}

/// HTTP client wrapper for MinKy API
struct MinkyClient {
    client: Client,
    config: MinkyConfig,
}

impl MinkyClient {
    fn new(config: MinkyConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| AxonError::tool("minky", format!("Failed to create client: {}", e)))?;

        Ok(Self { client, config })
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let url = format!("{}{}", self.config.endpoint, path);
        let mut request = self.client.request(method, &url);

        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| AxonError::tool("minky", format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AxonError::tool(
                "minky",
                format!("API error ({}): {}", status, error_text),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| AxonError::tool("minky", format!("Failed to parse response: {}", e)))
    }
}

/// Tool for searching MinKy knowledge base
pub struct MinkySearchTool {
    client: MinkyClient,
}

impl MinkySearchTool {
    pub fn new(config: MinkyConfig) -> Result<Self> {
        Ok(Self {
            client: MinkyClient::new(config)?,
        })
    }
}

#[async_trait]
impl Tool for MinkySearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "minky_search".to_string(),
            description: "Search the MinKy knowledge base for relevant documents".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["keyword", "vector", "hybrid", "deep"],
                        "default": "hybrid",
                        "description": "Search mode"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 5,
                        "description": "Maximum number of results"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("minky_search", "Missing 'query' argument"))?;

        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("hybrid");

        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(5) as usize;

        let body = serde_json::json!({
            "query": query,
            "mode": mode,
            "limit": limit,
        });

        let results: Vec<SearchResult> = self
            .client
            .request(reqwest::Method::POST, "/search/semantic", Some(body))
            .await?;

        let content = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "{}. [{}] {} (score: {:.2})\n   {}",
                    i + 1,
                    r.id,
                    r.title,
                    r.score,
                    r.content.chars().take(200).collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(ToolResult {
            success: true,
            content,
            error: None,
        })
    }

    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        if args.get("query").and_then(|v| v.as_str()).is_none() {
            return Err(AxonError::tool("minky_search", "Missing 'query' argument"));
        }
        Ok(())
    }
}

/// Tool for asking questions to MinKy RAG system
pub struct MinkyAskTool {
    client: MinkyClient,
}

impl MinkyAskTool {
    pub fn new(config: MinkyConfig) -> Result<Self> {
        Ok(Self {
            client: MinkyClient::new(config)?,
        })
    }
}

#[async_trait]
impl Tool for MinkyAskTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "minky_ask".to_string(),
            description:
                "Ask a question and get an AI-generated answer based on the knowledge base"
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "Question to ask"
                    }
                },
                "required": ["question"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let question = args
            .get("question")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("minky_ask", "Missing 'question' argument"))?;

        let body = serde_json::json!({
            "question": question,
        });

        let response: RagResponse = self
            .client
            .request(reqwest::Method::POST, "/search/ask", Some(body))
            .await?;

        let sources = response
            .sources
            .iter()
            .map(|s| format!("- [{}] {}", s.id, s.title))
            .collect::<Vec<_>>()
            .join("\n");

        let content = format!(
            "Answer:\n{}\n\nConfidence: {:.0}%\n\nSources:\n{}",
            response.answer,
            response.confidence * 100.0,
            sources
        );

        Ok(ToolResult {
            success: true,
            content,
            error: None,
        })
    }

    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        if args.get("question").and_then(|v| v.as_str()).is_none() {
            return Err(AxonError::tool("minky_ask", "Missing 'question' argument"));
        }
        Ok(())
    }
}

/// Tool for getting a specific document from MinKy
pub struct MinkyGetTool {
    client: MinkyClient,
}

impl MinkyGetTool {
    pub fn new(config: MinkyConfig) -> Result<Self> {
        Ok(Self {
            client: MinkyClient::new(config)?,
        })
    }
}

#[async_trait]
impl Tool for MinkyGetTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "minky_get".to_string(),
            description: "Get a specific document from the MinKy knowledge base by ID".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID"
                    }
                },
                "required": ["id"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("minky_get", "Missing 'id' argument"))?;

        #[derive(Deserialize)]
        struct Document {
            id: String,
            title: String,
            content: String,
            #[serde(default)]
            metadata: serde_json::Value,
        }

        let document: Document = self
            .client
            .request(reqwest::Method::GET, &format!("/documents/{}", id), None)
            .await?;

        let content = format!(
            "Title: {}\nID: {}\n\nContent:\n{}\n\nMetadata: {}",
            document.title,
            document.id,
            document.content,
            serde_json::to_string_pretty(&document.metadata).unwrap_or_default()
        );

        Ok(ToolResult {
            success: true,
            content,
            error: None,
        })
    }

    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        if args.get("id").and_then(|v| v.as_str()).is_none() {
            return Err(AxonError::tool("minky_get", "Missing 'id' argument"));
        }
        Ok(())
    }
}

/// Register all MinKy tools with a registry
pub async fn register_minky_tools(
    registry: &super::ToolRegistry,
    config: MinkyConfig,
) -> Result<()> {
    registry
        .register(MinkySearchTool::new(config.clone())?)
        .await;
    registry.register(MinkyAskTool::new(config.clone())?).await;
    registry.register(MinkyGetTool::new(config)?).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minky_config_default() {
        let config = MinkyConfig::default();
        assert_eq!(config.endpoint, "http://localhost:3000/api");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_minky_config_builder() {
        let config = MinkyConfig::new("https://minky.example.com/api").with_api_key("secret-key");

        assert_eq!(config.endpoint, "https://minky.example.com/api");
        assert_eq!(config.api_key, Some("secret-key".to_string()));
    }

    #[test]
    fn test_search_tool_definition() {
        let config = MinkyConfig::default();
        let tool = MinkySearchTool::new(config).unwrap();
        let def = tool.definition();

        assert_eq!(def.name, "minky_search");
        assert!(def.parameters["properties"]["query"].is_object());
    }

    #[test]
    fn test_ask_tool_definition() {
        let config = MinkyConfig::default();
        let tool = MinkyAskTool::new(config).unwrap();
        let def = tool.definition();

        assert_eq!(def.name, "minky_ask");
        assert!(def.parameters["properties"]["question"].is_object());
    }

    #[test]
    fn test_get_tool_definition() {
        let config = MinkyConfig::default();
        let tool = MinkyGetTool::new(config).unwrap();
        let def = tool.definition();

        assert_eq!(def.name, "minky_get");
        assert!(def.parameters["properties"]["id"].is_object());
    }

    // Integration tests would require a running MinKy server
}
