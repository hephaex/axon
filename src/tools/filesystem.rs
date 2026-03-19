//! Filesystem tools for file operations
//!
//! Provides read, write, and list operations for LLM agents.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::error::AxonError;
use crate::Result;

use super::registry::Tool;
use super::{ToolDefinition, ToolResult};

/// Configuration for filesystem tool
#[derive(Debug, Clone)]
pub struct FilesystemConfig {
    /// Base directory for operations (sandboxing)
    pub base_dir: PathBuf,
    /// Allow write operations
    pub allow_write: bool,
    /// Maximum file size to read (bytes)
    pub max_read_size: usize,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            allow_write: false,
            max_read_size: 1024 * 1024, // 1MB
        }
    }
}

/// Tool for reading files
pub struct ReadFileTool {
    config: FilesystemConfig,
}

impl ReadFileTool {
    pub fn new(config: FilesystemConfig) -> Self {
        Self { config }
    }

    fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        let requested = Path::new(path);
        let full_path = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.config.base_dir.join(requested)
        };

        // Security: ensure path is within base_dir
        let canonical = full_path
            .canonicalize()
            .map_err(|e| AxonError::tool("read_file", format!("Invalid path: {}", e)))?;

        if !canonical.starts_with(&self.config.base_dir) {
            return Err(AxonError::tool(
                "read_file",
                "Access denied: path outside base directory",
            ));
        }

        Ok(canonical)
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("read_file", "Missing 'path' argument"))?;

        let resolved = self.resolve_path(path)?;

        // Check file size
        let metadata = fs::metadata(&resolved)
            .await
            .map_err(|e| AxonError::tool("read_file", format!("Cannot access file: {}", e)))?;

        if metadata.len() as usize > self.config.max_read_size {
            return Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!(
                    "File too large: {} bytes (max: {} bytes)",
                    metadata.len(),
                    self.config.max_read_size
                )),
            });
        }

        let content = fs::read_to_string(&resolved)
            .await
            .map_err(|e| AxonError::tool("read_file", format!("Cannot read file: {}", e)))?;

        Ok(ToolResult {
            success: true,
            content,
            error: None,
        })
    }

    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        if args.get("path").and_then(|v| v.as_str()).is_none() {
            return Err(AxonError::tool("read_file", "Missing 'path' argument"));
        }
        Ok(())
    }
}

/// Tool for writing files
pub struct WriteFileTool {
    config: FilesystemConfig,
}

impl WriteFileTool {
    pub fn new(config: FilesystemConfig) -> Self {
        Self { config }
    }

    fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        if !self.config.allow_write {
            return Err(AxonError::tool(
                "write_file",
                "Write operations not allowed",
            ));
        }

        let requested = Path::new(path);
        let full_path = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.config.base_dir.join(requested)
        };

        // For new files, check parent directory
        if let Some(parent) = full_path.parent() {
            if parent.exists() {
                let canonical_parent = parent.canonicalize().map_err(|e| {
                    AxonError::tool("write_file", format!("Invalid parent path: {}", e))
                })?;
                if !canonical_parent.starts_with(&self.config.base_dir) {
                    return Err(AxonError::tool(
                        "write_file",
                        "Access denied: path outside base directory",
                    ));
                }
            }
        }

        Ok(full_path)
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Write content to a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("write_file", "Missing 'path' argument"))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("write_file", "Missing 'content' argument"))?;

        let resolved = self.resolve_path(path)?;

        // Create parent directories if needed
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                AxonError::tool("write_file", format!("Cannot create directory: {}", e))
            })?;
        }

        fs::write(&resolved, content)
            .await
            .map_err(|e| AxonError::tool("write_file", format!("Cannot write file: {}", e)))?;

        Ok(ToolResult {
            success: true,
            content: format!("Written {} bytes to {}", content.len(), path),
            error: None,
        })
    }

    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        if !self.config.allow_write {
            return Err(AxonError::tool(
                "write_file",
                "Write operations not allowed",
            ));
        }
        if args.get("path").and_then(|v| v.as_str()).is_none() {
            return Err(AxonError::tool("write_file", "Missing 'path' argument"));
        }
        if args.get("content").and_then(|v| v.as_str()).is_none() {
            return Err(AxonError::tool("write_file", "Missing 'content' argument"));
        }
        Ok(())
    }
}

/// Tool for listing directory contents
pub struct ListDirTool {
    config: FilesystemConfig,
}

impl ListDirTool {
    pub fn new(config: FilesystemConfig) -> Self {
        Self { config }
    }

    fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        let requested = Path::new(path);
        let full_path = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.config.base_dir.join(requested)
        };

        let canonical = full_path
            .canonicalize()
            .map_err(|e| AxonError::tool("list_dir", format!("Invalid path: {}", e)))?;

        if !canonical.starts_with(&self.config.base_dir) {
            return Err(AxonError::tool(
                "list_dir",
                "Access denied: path outside base directory",
            ));
        }

        Ok(canonical)
    }
}

#[async_trait]
impl Tool for ListDirTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_dir".to_string(),
            description: "List contents of a directory".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the directory to list"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("list_dir", "Missing 'path' argument"))?;

        let resolved = self.resolve_path(path)?;

        let mut entries = fs::read_dir(&resolved)
            .await
            .map_err(|e| AxonError::tool("list_dir", format!("Cannot read directory: {}", e)))?;

        let mut items = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AxonError::tool("list_dir", format!("Error reading entry: {}", e)))?
        {
            let file_type = entry.file_type().await.ok();
            let type_str = match file_type {
                Some(ft) if ft.is_dir() => "dir",
                Some(ft) if ft.is_file() => "file",
                Some(ft) if ft.is_symlink() => "link",
                _ => "unknown",
            };
            items.push(format!(
                "{}\t{}",
                type_str,
                entry.file_name().to_string_lossy()
            ));
        }

        items.sort();

        Ok(ToolResult {
            success: true,
            content: items.join("\n"),
            error: None,
        })
    }

    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        if args.get("path").and_then(|v| v.as_str()).is_none() {
            return Err(AxonError::tool("list_dir", "Missing 'path' argument"));
        }
        Ok(())
    }
}

/// Register all filesystem tools with a registry
pub async fn register_filesystem_tools(registry: &super::ToolRegistry, config: FilesystemConfig) {
    registry.register(ReadFileTool::new(config.clone())).await;
    registry.register(ListDirTool::new(config.clone())).await;
    if config.allow_write {
        registry.register(WriteFileTool::new(config)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(temp_dir: &TempDir) -> FilesystemConfig {
        // Use canonical path to handle macOS /var -> /private/var symlink
        let base_dir = temp_dir
            .path()
            .canonicalize()
            .unwrap_or_else(|_| temp_dir.path().to_path_buf());
        FilesystemConfig {
            base_dir,
            allow_write: true,
            max_read_size: 1024,
        }
    }

    #[tokio::test]
    async fn test_read_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let tool = ReadFileTool::new(test_config(&temp_dir));
        let result = tool
            .execute(serde_json::json!({ "path": "test.txt" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.content, "Hello, World!");
    }

    #[tokio::test]
    async fn test_read_file_outside_base() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReadFileTool::new(test_config(&temp_dir));

        let result = tool
            .execute(serde_json::json!({ "path": "/etc/passwd" }))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = WriteFileTool::new(test_config(&temp_dir));

        let result = tool
            .execute(serde_json::json!({
                "path": "output.txt",
                "content": "Test content"
            }))
            .await
            .unwrap();

        assert!(result.success);

        let content = std::fs::read_to_string(temp_dir.path().join("output.txt")).unwrap();
        assert_eq!(content, "Test content");
    }

    #[tokio::test]
    async fn test_write_file_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);
        config.allow_write = false;

        let tool = WriteFileTool::new(config);
        let result = tool
            .execute(serde_json::json!({
                "path": "output.txt",
                "content": "Test"
            }))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_dir() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "").unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let tool = ListDirTool::new(test_config(&temp_dir));
        let result = tool
            .execute(serde_json::json!({ "path": "." }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.content.contains("file\tfile1.txt"));
        assert!(result.content.contains("file\tfile2.txt"));
        assert!(result.content.contains("dir\tsubdir"));
    }
}
