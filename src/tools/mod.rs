//! Tool registry and implementations
//!
//! Tools that can be used by LLM agents:
//! - `MinkyTool` - MinKy knowledge search integration
//! - `FilesystemTool` - File operations
//! - `WebTool` - Web fetch

// TODO: Implement tools
// pub mod minky;
// pub mod filesystem;
// pub mod web;

use serde::{Deserialize, Serialize};

/// Tool definition for LLM function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,

    /// Tool description
    pub description: String,

    /// JSON Schema for parameters
    pub parameters: serde_json::Value,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool call succeeded
    pub success: bool,

    /// Result content
    pub content: String,

    /// Error message if failed
    pub error: Option<String>,
}
