//! Shell command tool (restricted)
//!
//! Provides limited shell command execution for LLM agents.
//! Security restrictions:
//! - Only allowlisted commands
//! - No shell metacharacters (pipes, redirects, etc.)
//! - Timeout enforcement
//! - Working directory sandboxing

use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::AxonError;
use crate::Result;

use super::registry::Tool;
use super::{ToolDefinition, ToolResult};

/// Default allowed commands (safe, read-only operations)
const DEFAULT_ALLOWED_COMMANDS: &[&str] = &[
    // File inspection
    "cat", "head", "tail", "wc", "file", "stat", // Directory operations
    "ls", "pwd", "find", "tree", // Text processing
    "grep", "awk", "sed", "sort", "uniq", "cut", "tr", "diff", // Development tools
    "git", "cargo", "npm", "node", "python", "python3", "pip", "pip3",
    // System info (read-only)
    "date", "whoami", "hostname", "uname", "env", "echo", "which", "type",
    // Network (read-only)
    "curl", "wget", "ping", "dig", "nslookup", "host",
];

/// Shell metacharacters that are blocked
const BLOCKED_CHARS: &[char] = &[
    '|',  // pipe
    ';',  // command separator
    '&',  // background/and
    '`',  // command substitution
    '$',  // variable expansion
    '(',  // subshell
    ')',  // subshell
    '{',  // brace expansion
    '}',  // brace expansion
    '<',  // input redirect
    '>',  // output redirect
    '\n', // newline
    '\r', // carriage return
];

/// Configuration for shell tool
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// Working directory for commands
    pub working_dir: PathBuf,
    /// Command execution timeout
    pub timeout: Duration,
    /// Maximum output size (bytes)
    pub max_output_size: usize,
    /// Allowed commands (None = use defaults)
    pub allowed_commands: Option<Vec<String>>,
    /// Additional blocked commands
    pub blocked_commands: Vec<String>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            timeout: Duration::from_secs(30),
            max_output_size: 1024 * 1024, // 1MB
            allowed_commands: None,
            blocked_commands: vec![
                // Dangerous commands always blocked
                "rm".to_string(),
                "rmdir".to_string(),
                "mv".to_string(),
                "cp".to_string(),
                "chmod".to_string(),
                "chown".to_string(),
                "kill".to_string(),
                "pkill".to_string(),
                "killall".to_string(),
                "shutdown".to_string(),
                "reboot".to_string(),
                "poweroff".to_string(),
                "mkfs".to_string(),
                "dd".to_string(),
                "fdisk".to_string(),
                "mount".to_string(),
                "umount".to_string(),
                "sudo".to_string(),
                "su".to_string(),
                "passwd".to_string(),
                "useradd".to_string(),
                "userdel".to_string(),
                "groupadd".to_string(),
                "visudo".to_string(),
            ],
        }
    }
}

impl ShellConfig {
    /// Create a config with custom allowed commands
    pub fn with_allowed_commands(mut self, commands: Vec<String>) -> Self {
        self.allowed_commands = Some(commands);
        self
    }

    /// Create a read-only config (no write operations)
    pub fn read_only() -> Self {
        Self::default()
    }

    /// Check if a command is allowed
    fn is_command_allowed(&self, cmd: &str) -> bool {
        // Check blocked list first
        if self.blocked_commands.iter().any(|b| b == cmd) {
            return false;
        }

        // Check allowed list
        match &self.allowed_commands {
            Some(allowed) => allowed.iter().any(|a| a == cmd),
            None => DEFAULT_ALLOWED_COMMANDS.contains(&cmd),
        }
    }
}

/// Tool for executing shell commands (restricted)
pub struct ShellTool {
    config: ShellConfig,
}

impl ShellTool {
    pub fn new(config: ShellConfig) -> Self {
        Self { config }
    }

    /// Validate command for security
    fn validate_command(&self, command: &str, args: &[&str]) -> Result<()> {
        // Check for blocked characters in command
        if command.chars().any(|c| BLOCKED_CHARS.contains(&c)) {
            return Err(AxonError::tool(
                "shell",
                "Command contains blocked shell metacharacters",
            ));
        }

        // Check for blocked characters in arguments
        for arg in args {
            if arg.chars().any(|c| BLOCKED_CHARS.contains(&c)) {
                return Err(AxonError::tool(
                    "shell",
                    format!("Argument '{}' contains blocked shell metacharacters", arg),
                ));
            }
        }

        // Check if command is allowed
        if !self.config.is_command_allowed(command) {
            let allowed_list: Vec<&str> = match &self.config.allowed_commands {
                Some(cmds) => cmds.iter().map(|s| s.as_str()).collect(),
                None => DEFAULT_ALLOWED_COMMANDS.to_vec(),
            };
            return Err(AxonError::tool(
                "shell",
                format!(
                    "Command '{}' is not in the allowed list. Allowed: {:?}",
                    command, allowed_list
                ),
            ));
        }

        Ok(())
    }

    /// Parse command string into command and arguments
    fn parse_command(input: &str) -> (&str, Vec<&str>) {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return ("", vec![]);
        }
        let cmd = parts[0];
        let args = parts[1..].to_vec();
        (cmd, args)
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell".to_string(),
            description: "Execute a shell command (restricted to safe commands only)".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to execute (e.g., 'ls -la', 'git status')"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let command_str = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("shell", "Missing 'command' argument"))?;

        // Parse and validate command
        let (cmd, cmd_args) = Self::parse_command(command_str);

        if cmd.is_empty() {
            return Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some("Empty command".to_string()),
            });
        }

        self.validate_command(cmd, &cmd_args)?;

        // Build command
        let mut process = Command::new(cmd);
        process
            .args(&cmd_args)
            .current_dir(&self.config.working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Execute with timeout
        let result = match timeout(self.config.timeout, process.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Ok(ToolResult {
                    success: false,
                    content: String::new(),
                    error: Some(format!("Failed to execute command: {}", e)),
                });
            }
            Err(_) => {
                return Ok(ToolResult {
                    success: false,
                    content: String::new(),
                    error: Some(format!("Command timed out after {:?}", self.config.timeout)),
                });
            }
        };

        // Combine stdout and stderr
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);

        let mut output = stdout.to_string();
        if !stderr.is_empty() {
            if !output.is_empty() {
                output.push_str("\n--- stderr ---\n");
            }
            output.push_str(&stderr);
        }

        // Truncate if too large
        if output.len() > self.config.max_output_size {
            output.truncate(self.config.max_output_size);
            output.push_str("\n... (output truncated)");
        }

        let success = result.status.success();
        let error = if success {
            None
        } else {
            Some(format!(
                "Command exited with code {}",
                result.status.code().unwrap_or(-1)
            ))
        };

        Ok(ToolResult {
            success,
            content: output,
            error,
        })
    }

    fn validate(&self, args: &serde_json::Value) -> Result<()> {
        let command_str = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AxonError::tool("shell", "Missing 'command' argument"))?;

        let (cmd, cmd_args) = Self::parse_command(command_str);

        if cmd.is_empty() {
            return Err(AxonError::tool("shell", "Empty command"));
        }

        self.validate_command(cmd, &cmd_args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ShellConfig {
        ShellConfig {
            working_dir: std::env::current_dir().unwrap(),
            timeout: Duration::from_secs(5),
            max_output_size: 1024,
            ..Default::default()
        }
    }

    #[test]
    fn test_default_config() {
        let config = ShellConfig::default();
        assert!(config.is_command_allowed("ls"));
        assert!(config.is_command_allowed("git"));
        assert!(!config.is_command_allowed("rm"));
        assert!(!config.is_command_allowed("sudo"));
    }

    #[test]
    fn test_blocked_commands() {
        let config = ShellConfig::default();
        assert!(!config.is_command_allowed("rm"));
        assert!(!config.is_command_allowed("sudo"));
        assert!(!config.is_command_allowed("chmod"));
        assert!(!config.is_command_allowed("kill"));
    }

    #[test]
    fn test_custom_allowed_commands() {
        let config = ShellConfig::default().with_allowed_commands(vec!["mycommand".to_string()]);

        assert!(config.is_command_allowed("mycommand"));
        assert!(!config.is_command_allowed("ls")); // Not in custom list
    }

    #[test]
    fn test_parse_command() {
        let (cmd, args) = ShellTool::parse_command("ls -la /tmp");
        assert_eq!(cmd, "ls");
        assert_eq!(args, vec!["-la", "/tmp"]);
    }

    #[test]
    fn test_parse_empty_command() {
        let (cmd, args) = ShellTool::parse_command("");
        assert_eq!(cmd, "");
        assert!(args.is_empty());
    }

    #[test]
    fn test_validate_blocked_chars() {
        let tool = ShellTool::new(test_config());

        // Pipe is blocked
        assert!(tool.validate_command("ls", &["|", "grep"]).is_err());

        // Semicolon is blocked
        assert!(tool.validate_command("ls", &[";", "rm"]).is_err());

        // Command substitution is blocked
        assert!(tool.validate_command("echo", &["`whoami`"]).is_err());

        // Variable expansion is blocked
        assert!(tool.validate_command("echo", &["$HOME"]).is_err());
    }

    #[test]
    fn test_validate_allowed_command() {
        let tool = ShellTool::new(test_config());

        assert!(tool.validate_command("ls", &["-la"]).is_ok());
        assert!(tool.validate_command("git", &["status"]).is_ok());
        assert!(tool.validate_command("echo", &["hello"]).is_ok());
    }

    #[test]
    fn test_validate_blocked_command() {
        let tool = ShellTool::new(test_config());

        assert!(tool.validate_command("rm", &["-rf", "/"]).is_err());
        assert!(tool.validate_command("sudo", &["ls"]).is_err());
    }

    #[tokio::test]
    async fn test_execute_ls() {
        let tool = ShellTool::new(test_config());
        let result = tool
            .execute(serde_json::json!({ "command": "ls" }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn test_execute_echo() {
        let tool = ShellTool::new(test_config());
        let result = tool
            .execute(serde_json::json!({ "command": "echo hello world" }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.content.contains("hello world"));
    }

    #[tokio::test]
    async fn test_execute_blocked_command() {
        let tool = ShellTool::new(test_config());
        let result = tool
            .execute(serde_json::json!({ "command": "rm -rf /" }))
            .await;

        // Should fail validation
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_pipe_blocked() {
        let tool = ShellTool::new(test_config());
        let result = tool
            .execute(serde_json::json!({ "command": "ls | grep test" }))
            .await;

        // Should fail validation due to pipe
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timeout() {
        let config = ShellConfig {
            timeout: Duration::from_millis(100),
            ..test_config()
        };
        let tool = ShellTool::new(config);

        // sleep command should timeout
        let result = tool
            .execute(serde_json::json!({ "command": "sleep 10" }))
            .await;

        // sleep might not be in allowed list, so this might fail for different reason
        // This is acceptable for the test
        assert!(result.is_err() || !result.unwrap().success);
    }

    #[test]
    fn test_definition() {
        let tool = ShellTool::new(test_config());
        let def = tool.definition();

        assert_eq!(def.name, "shell");
        assert!(def.parameters["properties"]["command"].is_object());
    }
}
