//! # Axon - LLM-to-LLM Communication Framework
//!
//! Axon (축삭) enables multiple LLM agents to communicate, collaborate,
//! and share tools through a CLI-based orchestration framework.
//!
//! ## Features
//!
//! - **Multi-Agent Communication**: Claude, Gemini, GPT, and local LLMs
//! - **Tool Sharing**: MCP-compatible function calling
//! - **CLI First**: Pipeline-friendly, scriptable interface
//! - **MinKy Integration**: Knowledge search via MinKy platform
//!
//! ## Quick Start
//!
//! ```bash
//! # Start router
//! axon serve
//!
//! # Register agents
//! axon agent add claude --provider anthropic --model claude-sonnet-4-20250514
//!
//! # Send message
//! axon send --from claude --to gemini "Review this code"
//! ```

pub mod adapters;
pub mod config;
pub mod error;
pub mod persistence;
pub mod protocol;
pub mod router;
pub mod server;
pub mod tools;
pub mod utils;

// Re-exports
pub use adapters::{collect_stream, StreamChunk, StreamUsage, StreamingAdapter};
pub use error::{AxonError, Result};
pub use persistence::{ConversationStore, FileStore, MemoryStore};
pub use protocol::{AgentConfig, AgentId, LlmMessage, MessageContent, MessageType, Provider};
pub use router::{MessageRouter, RouterStats};
pub use server::{start_server, ServerConfig, ServerState};
pub use tools::{Tool, ToolDefinition, ToolRegistry, ToolResult};
pub use utils::{retry_with_backoff, RateLimiter, RateLimiterRegistry, RetryConfig};
