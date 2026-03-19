//! Tool registry for managing and executing tools
//!
//! Provides a central registry for tools that LLM agents can use.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::AxonError;
use crate::Result;

use super::{ToolDefinition, ToolResult};

/// Trait for executable tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool definition
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with given arguments
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult>;

    /// Validate arguments before execution
    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        // Default implementation: no validation
        let _ = args;
        Ok(())
    }
}

/// Registry for managing tools
pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a tool
    pub async fn register<T: Tool + 'static>(&self, tool: T) {
        let definition = tool.definition();
        let mut tools = self.tools.write().await;
        tools.insert(definition.name.clone(), Arc::new(tool));
    }

    /// Unregister a tool by name
    pub async fn unregister(&self, name: &str) -> bool {
        let mut tools = self.tools.write().await;
        tools.remove(name).is_some()
    }

    /// Get a tool by name
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }

    /// List all registered tools
    pub async fn list(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name with given arguments
    pub async fn execute(&self, name: &str, args: serde_json::Value) -> Result<ToolResult> {
        let tool = self.get(name).await.ok_or_else(|| {
            AxonError::tool(name, "Tool not found")
        })?;

        // Validate arguments
        tool.validate(&args)?;

        // Execute
        tool.execute(args).await
    }

    /// Get tool definitions in MCP-compatible format
    pub async fn to_mcp_format(&self) -> Vec<serde_json::Value> {
        let tools = self.tools.read().await;
        tools
            .values()
            .map(|t| {
                let def = t.definition();
                serde_json::json!({
                    "name": def.name,
                    "description": def.description,
                    "inputSchema": def.parameters,
                })
            })
            .collect()
    }

    /// Check if a tool is registered
    pub async fn has(&self, name: &str) -> bool {
        let tools = self.tools.read().await;
        tools.contains_key(name)
    }

    /// Get the number of registered tools
    pub async fn len(&self) -> usize {
        let tools = self.tools.read().await;
        tools.len()
    }

    /// Check if registry is empty
    pub async fn is_empty(&self) -> bool {
        let tools = self.tools.read().await;
        tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "echo".to_string(),
                description: "Echo the input".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"]
                }),
            }
        }

        async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
            let message = args
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("no message");

            Ok(ToolResult {
                success: true,
                content: format!("Echo: {}", message),
                error: None,
            })
        }
    }

    #[tokio::test]
    async fn test_register_tool() {
        let registry = ToolRegistry::new();
        registry.register(EchoTool).await;

        assert!(registry.has("echo").await);
        assert_eq!(registry.len().await, 1);
    }

    #[tokio::test]
    async fn test_list_tools() {
        let registry = ToolRegistry::new();
        registry.register(EchoTool).await;

        let tools = registry.list().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
    }

    #[tokio::test]
    async fn test_execute_tool() {
        let registry = ToolRegistry::new();
        registry.register(EchoTool).await;

        let result = registry
            .execute("echo", serde_json::json!({ "message": "hello" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.content, "Echo: hello");
    }

    #[tokio::test]
    async fn test_execute_unknown_tool() {
        let registry = ToolRegistry::new();

        let result = registry
            .execute("unknown", serde_json::json!({}))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unregister_tool() {
        let registry = ToolRegistry::new();
        registry.register(EchoTool).await;

        assert!(registry.has("echo").await);
        assert!(registry.unregister("echo").await);
        assert!(!registry.has("echo").await);
    }

    #[tokio::test]
    async fn test_mcp_format() {
        let registry = ToolRegistry::new();
        registry.register(EchoTool).await;

        let mcp = registry.to_mcp_format().await;
        assert_eq!(mcp.len(), 1);
        assert_eq!(mcp[0]["name"], "echo");
        assert!(mcp[0]["inputSchema"].is_object());
    }
}
